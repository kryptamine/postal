use std::{error::Error, path::Path, time::Duration};

use futures::TryFutureExt;

use crate::{
    account::config::Config,
    account::partial_config::PartialConfig,
    account::sign_in::SignIn,
    account::xdg,
    cache::Cache,
    compose::accounts::Accounts,
    compose::mailto::Mailto,
    compose::outbox::Outbox,
    compose::outgoing::Outgoing,
    domain::per_account::PerAccount,
    mail::forward_fetch::ForwardFetch,
    mail::outbox_flush::Sender,
    mail::reply_fetch::ReplyFetch,
    mail::routes::Routes,
    mail::serve::serve,
    mail::status::Status,
    render::icons::Icons,
    ui::exit::Exit,
    ui::older_pages::OlderPages,
    ui::previews::Previews,
    ui::run::{self, Background},
    ui::terminal_slot::TerminalSlot,
    ui::theme::Theme,
    ui::wanted_attachments::WantedAttachments,
    ui::wanted_original::WantedOriginals,
};

/// What a session opens on, besides its Accounts.
pub struct Opening {
    /// What failed before it, toasted.
    pub failure: Option<&'static str>,
    /// A `mailto:` link whose Draft it composes once it starts.
    pub link: Option<Mailto>,
}

/// The TUI until it exits, each Account of `configs` signed out without
/// its sign-in.
pub async fn session(
    slot: &mut TerminalSlot,
    dir: &Path,
    configs: &PerAccount<Config>,
    sign_ins: &PerAccount<Option<SignIn>>,
    opening: Opening,
) -> Result<Exit, Box<dyn Error>> {
    let emails = configs.map(|config| config.email.as_str());
    let caches = Cache::open(&xdg::cache_dir()?, &emails)?;
    // Without one, Compose says so when it is used.
    let shared = PartialConfig::load(&dir.join("config.toml"))?;
    let outbox = Outbox::new(xdg::data_dir().ok(), send_delay(&shared));
    let (sender, updates) = futures::channel::mpsc::unbounded();
    let (mut asks, mut requests, mut works) = (Vec::new(), Vec::new(), Vec::new());
    for (account, config) in configs.iter() {
        let (ask, asked) = futures::channel::mpsc::unbounded();
        let (request, requested) = futures::channel::mpsc::unbounded();
        asks.push(ask);
        requests.push(request);
        // Its channels close with their receivers: what is asked of it is dropped.
        let Some(sign_in) = &sign_ins[account] else {
            continue;
        };

        let sender = sender.clone();
        let send = move |update| {
            // The send fails only once the event loop has stopped listening.
            sender.unbounded_send((account, update)).ok();
        };

        let sender = Sender {
            outbox: &outbox,
            outgoing: Outgoing { config },
            sign_in,
            first: configs.first() == Some(account),
        };

        let cache = &caches[account];
        works.push(Box::pin(serve(cache, asked, requested, sender, send)));
    }

    let requests = Routes::new(requests.into_iter().collect());
    let older = OlderPages::new(requests.clone(), configs.map(|_| false));
    let replies = ReplyFetch::new(requests.clone(), configs.map(|_| false));
    let forwards = ForwardFetch::new(requests.clone(), configs.map(|_| false));
    let attachments = WantedAttachments::new(requests.clone(), configs.map(|_| false));
    let originals = WantedOriginals::new(requests.clone(), configs.map(|_| false));
    let (tty, background) = slot.take()?;
    let mut theme = Theme::from_env(background, Icons::choose(shared.icons));
    let mut previews = Previews::start(requests, configs, sender, theme.mail_look());
    let background = Background {
        work: futures::future::try_join_all(works).map_ok(drop),
        updates,
        older,
        replies,
        forwards,
        attachments,
        originals,
        asks: Routes::new(asks.into_iter().collect()),
        accounts: Accounts(configs),
        outbox: &outbox,
        statuses: sign_ins.map(|sign_in| starting(sign_in.as_ref())),
        failure: opening.failure,
        link: opening.link,
        config_dir: dir,
        notify: shared.notify,
        contacts: shared.contacts,
    };

    Ok(run::run(tty, &mut theme, &caches, &mut previews, background).await?)
}

/// `send-delay` from `config`. Tests set `POSTAL_SEND_DELAY` in seconds, a
/// fraction too: the config takes whole ones.
fn send_delay(config: &PartialConfig) -> Duration {
    let set = std::env::var("POSTAL_SEND_DELAY").ok();
    let test = set.and_then(|secs| Duration::try_from_secs_f64(secs.parse().ok()?).ok());
    test.unwrap_or(config.send_delay)
}

/// The Status an Account starts in.
const fn starting(sign_in: Option<&SignIn>) -> Status {
    match sign_in {
        Some(_) => Status::Live,
        None => Status::SignedOut,
    }
}
