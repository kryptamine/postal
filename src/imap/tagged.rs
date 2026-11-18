use async_imap::{
    Session,
    imap_proto::{Response, Status},
};

use super::{Error, connect::Transport};

/// Sends `command` and hands `each` every response until the one completing
/// it. Read by hand, as async-imap's own fetch ends quietly on a `NO` and on
/// a closed connection alike, and keeps back what it has no accessor for.
pub async fn run(
    session: &mut Session<Box<dyn Transport>>,
    command: String,
    mut each: impl FnMut(&Response<'_>),
) -> Result<(), Error> {
    let tag = session.run_command(command).await?;
    loop {
        let response = session
            .read_response()
            .await
            .map_err(|e| Error::Imap(e.into()))?
            .ok_or(Error::Closed)?;
        match response.parsed() {
            Response::Done {
                tag: done,
                status,
                outcome,
            } if *done == tag => {
                // Worded as async-imap words its own, which `Error` reads.
                let text = format!("code: {:?}, info: {:?}", outcome.code, outcome.information);
                return match status {
                    Status::Ok => Ok(()),
                    Status::No => Err(Error::Imap(async_imap::error::Error::No(text))),
                    _ => Err(Error::Imap(async_imap::error::Error::Bad(text))),
                };
            }
            parsed => each(parsed),
        }
    }
}
