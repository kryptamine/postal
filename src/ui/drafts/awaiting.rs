use crate::{
    domain::message::Message,
    domain::per_account::AccountId,
    mail::awaited_parts::AwaitedParts,
    mail::fetch_requests::FetchRequests,
    mail::update::Update,
    ui::drafts::composing::{self, Reply},
    ui::drafts::forwarding,
    ui::error::Error,
    ui::state::screen::Screen,
    ui::terminal::tty::Tty,
    ui::world::World,
};

enum Kind {
    Reply(Reply),
    Forward(Message),
}

/// A Reply or Forward waiting on the fetch connection for the parts it lacks.
pub struct Waiting {
    parts: AwaitedParts,
    kind: Kind,
}

impl Waiting {
    pub const fn reply(reply: Reply, parts: AwaitedParts) -> Self {
        Self {
            parts,
            kind: Kind::Reply(reply),
        }
    }

    pub const fn forward(message: Message, parts: AwaitedParts) -> Self {
        Self {
            parts,
            kind: Kind::Forward(message),
        }
    }
}

/// What was awaited, taken off the screen, and how its fetch ended.
pub struct Done(Waiting, Result<(), String>);

/// The Reply or Forward whose Draft waits on the fetch connection: one at a
/// time, as each would open its own Draft over the other.
#[derive(Default)]
pub struct Awaited(Option<Waiting>);

impl Awaited {
    /// Asks for what `waiting` lacks and holds it, in place of any prior wait.
    pub fn start(&mut self, fetch: &FetchRequests, waiting: Waiting) {
        self.cancel(fetch);
        fetch.send(waiting.parts.account(), waiting.parts.request());
        self.0 = Some(waiting);
    }

    /// Drops any wait under way, and the standing request it made.
    pub fn cancel(&mut self, fetch: &FetchRequests) {
        if let Some(previous) = self.0.take() {
            let account = previous.parts.account();
            fetch.send(account, AwaitedParts::new(account).request());
        }
    }

    /// Follows `update` of `account` for the awaited Reply or Forward.
    pub fn notice(&mut self, account: AccountId, update: &Update) -> Option<Done> {
        let waiting = self.0.as_mut()?;
        let result = waiting.parts.notice(account, update)?;
        Some(Done(self.0.take()?, result))
    }

    pub const fn fetching(&self) -> bool {
        self.0.is_some()
    }
}

/// The editor on what was awaited, or a Toast saying why not.
pub fn finish(
    done: Option<Done>,
    tty: &mut Tty,
    screen: &mut Screen,
    world: World,
) -> Result<(), Error> {
    let Some(Done(Waiting { parts, kind }, result)) = done else {
        return Ok(());
    };

    match kind {
        Kind::Reply(reply) => composing::fetched(&reply, result, tty, screen, world),
        Kind::Forward(message) => {
            let attachments = result.map(|()| parts.into_attachments());
            forwarding::forwarded(&message, attachments, tty, screen, world)
        }
    }
}
