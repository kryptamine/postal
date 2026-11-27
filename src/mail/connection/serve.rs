use futures::channel::mpsc::UnboundedReceiver;

use crate::{
    account::config::Config,
    account::sign_in::SignIn,
    cache::Cache,
    domain::folder_name::FolderName,
    mail::connection::reconnect,
    mail::fetch::body_fetch::Request,
    mail::fetch::oneshot_attempt::OneshotAttempt,
    mail::fetch::pending::Pending,
    mail::imap::{ImapSession, Mode},
    mail::outbox::outbox_flush::Sender,
    mail::passes::spare_work::SpareWork,
    mail::status::Status,
    mail::steering::{Ask, Steering},
    mail::sync::{self, Error},
    mail::update::{FetchLink, Update},
    mail::watch::arrivals::Arrivals,
    mail::watch::idle,
    mail::watch::tell::Tell,
};

/// The connections of the Account `sender` sends from until the screen
/// stops asking; signed out, they end and leave the others running.
pub async fn serve(
    cache: &Cache,
    asked: UnboundedReceiver<Ask>,
    requested: UnboundedReceiver<Request>,
    sender: Sender<'_>,
    send: impl Fn(Update),
) -> Result<(), Error> {
    let config = sender.outgoing.config;
    let report = |status| send(Update::Status(status));
    let synced = sync(cache, Steering::new(asked), sender, &send, report);
    let mut pending = Pending::new(requested);
    let fetched = bodies(config, sender.sign_in, cache, &mut pending, &send);
    match futures::future::try_join(synced, fetched).await {
        Err(error) if error.signed_out() => {
            pending.fail_oneshots(&send);
            send(Update::FetchLink(FetchLink::Down));
            tracing::warn!("{}: {error}", config.email);
            Ok(())
        }
        result => result.map(drop),
    }
}

/// The second connection, kept open for the bodies the Preview asks for:
/// opening one per fetch would add TLS and login to the first Preview.
async fn bodies(
    config: &Config,
    sign_in: &SignIn,
    cache: &Cache,
    pending: &mut Pending,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    let mut spare = SpareWork::new();
    let attempt = async |report: &dyn Fn(Status)| {
        let mut pending = OneshotAttempt {
            pending: &mut *pending,
            send,
        };

        let mut session = ImapSession::open(config, sign_in).await?;
        // Every run starts in the Inbox: its first Preview waits on no EXAMINE.
        sync::open(&mut session, FolderName::inbox(), Mode::Examine).await?;
        // Sent while the screen saw the last connection live, which then dropped.
        pending.fail_oneshots(send);
        report(Status::Live);
        pending.serve(&mut session, cache, &mut spare, send).await
    };

    // The status slot follows the sync connection alone, but for a refused Account.
    reconnect::retry(attempt, |status| match status {
        Status::Live => send(Update::FetchLink(FetchLink::Live)),
        Status::Retrying { .. } => send(Update::FetchLink(FetchLink::Down)),
        Status::SignedOut => send(Update::Status(status)),
        Status::Syncing { .. } | Status::Connecting | Status::Fetching => {}
    })
    .await
}

/// Resyncs, IDLEs and sends the Outbox on one connection, as IDLE's sequence
/// numbers count in its `EXAMINE`, on the Folder `steering` follows. `send`
/// follows every Cache write, `report` the connection.
async fn sync(
    cache: &Cache,
    mut steering: Steering,
    sender: Sender<'_>,
    send: &impl Fn(Update),
    report: impl Fn(Status),
) -> Result<(), Error> {
    let arrivals = Arrivals::new();
    let attempt = async |report: &dyn Fn(Status)| {
        let session = ImapSession::open(sender.outgoing.config, sender.sign_in).await?;
        let arrivals = &arrivals;
        let tell = Tell {
            send,
            report,
            arrivals,
        };

        idle::listen(session, cache, &mut steering, sender, tell).await
    };

    reconnect::retry(attempt, report).await
}
