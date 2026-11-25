use async_imap::{
    Session,
    imap_proto::{Response, Status},
};

use super::{Error, connect::Transport, pushed::Pushed};

/// Sends `command` and reads every response up to the one completing it,
/// that one too when `OK`: `take` keeps what it reads, the rest goes to
/// `pushed`, after what came before. By hand, as async-imap reads a `NO` as empty.
pub async fn run(
    session: &mut Session<Box<dyn Transport>>,
    pushed: &mut Vec<Pushed>,
    command: String,
    mut take: impl FnMut(&Response<'_>) -> bool,
) -> Result<(), Error> {
    Pushed::queued(session, pushed);
    let mut each = |response: &Response<'_>| {
        if !take(response) {
            pushed.extend(Pushed::from_response(response));
        }
    };

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
                    Status::Ok => {
                        each(response.parsed());
                        Ok(())
                    }
                    Status::No => Err(Error::Imap(async_imap::error::Error::No(text))),
                    _ => Err(Error::Imap(async_imap::error::Error::Bad(text))),
                };
            }
            parsed => each(parsed),
        }
    }
}
