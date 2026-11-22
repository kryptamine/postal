use std::{error::Error, path::Path};

use futures::TryFutureExt;

use crate::{
    account::config::{Config, IconChoice},
    account::sign_in::SignIn,
    account::xdg,
    cache::Cache,
    compose::accounts::Accounts,
    compose::outbox::Outbox,
    compose::outgoing::Outgoing,
    domain::per_account::PerAccount,
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
};

/// The TUI until it exits, each Account of `configs` signed out without
/// its sign-in; `failure` toasts what failed before it.
pub async fn session(
    slot: &mut TerminalSlot,
    dir: &Path,
    configs: &PerAccount<Config>,
    sign_ins: &PerAccount<Option<SignIn>>,
    failure: Option<&'static str>,
) -> Result<Exit, Box<dyn Error>> {
    let emails = configs.map(|config| config.email.as_str());
    let caches = Cache::open(&xdg::cache_dir()?, &emails)?;
    // Without one, Compose says so when it is used.
    let outbox = Outbox::new(xdg::data_dir().ok());
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
    let attachments = WantedAttachments::new(requests.clone(), configs.map(|_| false));
    let (terminal, background) = slot.take()?;
    let icons = configs
        .values()
        .next()
        .map_or(IconChoice::Auto, |config| config.icons);
    let mut theme = Theme::from_env(background, Icons::choose(icons));
    let mut previews = Previews::start(requests, configs, sender, theme.mail_look());
    let background = Background {
        work: futures::future::try_join_all(works).map_ok(drop),
        updates,
        older,
        replies,
        attachments,
        asks: Routes::new(asks.into_iter().collect()),
        accounts: Accounts(configs),
        outbox: &outbox,
        statuses: sign_ins.map(|sign_in| starting(sign_in.as_ref())),
        failure,
        config_dir: dir,
    };

    Ok(run::run(terminal, &mut theme, &caches, &mut previews, background).await?)
}

/// The Status an Account starts in.
const fn starting(sign_in: Option<&SignIn>) -> Status {
    match sign_in {
        Some(_) => Status::Live,
        None => Status::SignedOut,
    }
}
