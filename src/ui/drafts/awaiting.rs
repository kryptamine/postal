use crate::{
    compose::awaited::Awaited,
    domain::per_account::AccountId,
    mail::awaited_fetch,
    mail::fetch_requests::FetchRequests,
    mail::update::{BodyLink, Update},
    ui::drafts::composing,
    ui::drafts::forwarding,
    ui::run::Error,
    ui::state::screen::Screen,
    ui::terminal::tty::Tty,
    ui::world::World,
};

/// What was awaited, taken off the screen, and how its fetch ended.
pub struct Done(Awaited, Result<(), String>);

/// Asks for what `awaited` lacks and holds it in place of any other.
pub fn start(screen: &mut Screen, fetch: &FetchRequests, awaited: Awaited) {
    fetch.send(awaited.message().account, awaited_fetch::requests(&awaited));
    screen.awaited = Some(awaited);
}

/// Follows `update` of `account` once `fetch` has: what went down with the
/// fetch connection is asked for again when it is back up.
pub fn notice(
    screen: &mut Screen,
    fetch: &FetchRequests,
    account: AccountId,
    update: &Update,
) -> Option<Done> {
    let awaited = screen.awaited.as_mut()?;
    let ours = awaited.message().account == account;
    if ours && update.fetch_link() == Some(BodyLink::Up) {
        fetch.send(account, awaited_fetch::requests(awaited));
    }

    let result = awaited_fetch::notice(account, update, awaited)?;
    Some(Done(screen.awaited.take()?, result))
}

/// The editor on what was awaited, or a Toast saying why not.
pub fn finish(
    done: Option<Done>,
    tty: &mut Tty,
    screen: &mut Screen,
    world: World,
) -> Result<(), Error> {
    match done {
        None => Ok(()),
        Some(Done(Awaited::Reply(reply), result)) => {
            composing::fetched(&reply, result, tty, screen, world)
        }
        Some(Done(Awaited::Forward(forward), result)) => {
            forwarding::forwarded(forward, result, tty, screen, world)
        }
    }
}
