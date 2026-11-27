use std::{io, path::Path};

use crossterm::event::Event;
use futures::{FutureExt, Stream, StreamExt, future::FusedFuture};
use ratatui::layout::Rect;
use tokio::time::Instant;

use crate::{
    account::config::{ContactsChoice, NotifyChoice},
    cache::Cache,
    compose::accounts::Accounts,
    compose::mailto::Mailto,
    compose::outbox::Outbox,
    domain::per_account::{AccountId, PerAccount},
    mail::fetch_requests::FetchRequests,
    mail::routes::Routes,
    mail::status::Status,
    mail::steering::Ask,
    mail::sync,
    mail::update::Update,
    ui::alloc_count,
    ui::drafts::awaiting,
    ui::drafts::draft_layer,
    ui::drafts::draft_preview::DraftPreview,
    ui::drafts::drafting,
    ui::drafts::local_list::relist_local,
    ui::error::Error,
    ui::exit::Exit,
    ui::frame::draw,
    ui::frame::theme::Theme,
    ui::input::handle,
    ui::list::folder_show::show,
    ui::list::message_list,
    ui::list::older_pages::OlderPages,
    ui::outside::notifier::Notifier,
    ui::outside::wanted_parts::WantedParts,
    ui::preview::preview_pane,
    ui::preview::previews::{Previews, Sources},
    ui::react::apply,
    ui::react::opening::opening,
    ui::react::take::take,
    ui::react::timed::{expire, next_change},
    ui::state::screen::Screen,
    ui::state::toast::Toast,
    ui::state::ui::Ui,
    ui::takeover::leaving::Leaving,
    ui::terminal::tty::Tty,
    ui::world::World,
};

/// The network work behind the screen and what it reports; what is asked of
/// the fetch and sync connections; and where a Send leaves from.
pub struct Background<'a, W, U> {
    pub work: W,
    /// Each tells the Account it is of.
    pub updates: U,
    pub fetch: FetchRequests,
    pub asks: Routes<Ask>,
    pub accounts: Accounts<'a>,
    pub outbox: &'a Outbox,
    /// Where `config.toml` is, which Settings writes.
    pub config_dir: &'a Path,
    /// Each Account's at the start: signed out until `L` signs it in again.
    pub statuses: PerAccount<Status>,
    /// What failed before the TUI opened again, toasted.
    pub failure: Option<&'static str>,
    /// A `mailto:` link to compose from once the first frame is drawn.
    pub link: Option<Mailto>,
    pub notify: NotifyChoice,
    pub contacts: ContactsChoice,
}

/// What the event loop woke to.
enum Woken {
    /// The screen changes on its own: a Toast goes, a Dwell ends.
    Due,
    Leaving,
    Finished(Result<(), sync::Error>),
    Update(Option<(AccountId, Update)>),
    Input(Option<io::Result<Event>>),
}

/// The screen, with the terminal, the look, the Previews and the network
/// work it acts on.
pub struct Model<'a> {
    pub tty: Tty<'a>,
    pub theme: &'a mut Theme,
    pub previews: &'a mut Previews,
    pub ui: Ui,
    pub world: World<'a>,
    pub fetch: FetchRequests,
    older: OlderPages,
    pub parts: WantedParts,
    pub notifier: Notifier,
}

/// The one event loop: input, and each `Update` from the background work,
/// redrawn after each. The first frame reads only the rows it shows from the
/// Cache, before any network; the rest follows right after it.
pub async fn run(
    tty: Tty<'_>,
    theme: &mut Theme,
    caches: &PerAccount<Cache>,
    previews: &mut Previews,
    background: Background<
        '_,
        impl Future<Output = Result<(), sync::Error>>,
        impl Stream<Item = (AccountId, Update)> + Unpin,
    >,
) -> Result<Exit, Error> {
    let (mut updates, asks) = (background.updates, background.asks);
    let world = World {
        caches,
        asks: &asks,
        outbox: background.outbox,
        accounts: background.accounts,
        config_dir: background.config_dir,
        contacts: background.contacts,
    };

    let (statuses, failure) = (background.statuses, background.failure);
    let ui = opening(world, &tty, statuses, failure)?;
    let mut model = Model {
        tty,
        theme,
        previews,
        ui,
        world,
        fetch: background.fetch,
        older: OlderPages::default(),
        parts: WantedParts::default(),
        notifier: Notifier::new(background.notify),
    };

    let mut work = std::pin::pin!(background.work.fuse());
    let (mut listening, mut leaving) = (true, Leaving::start());
    let mut frame_start = model.first_frame(background.link)?;
    loop {
        if leaving.due() {
            return Ok(Exit::Moved);
        }

        let text = model.frame()?;
        alloc_count::report(frame_start);
        let change = next_change(&mut model.ui, model.previews, text);
        let woken = tokio::select! {
            () = tokio::time::sleep_until(change.unwrap_or_else(Instant::now)), if change.is_some() => Woken::Due,
            () = leaving.woken() => Woken::Leaving,
            result = &mut work, if !work.is_terminated() => Woken::Finished(result),
            update = updates.next(), if listening => Woken::Update(update),
            event = model.tty.events.next() => Woken::Input(event),
        };

        frame_start = alloc_count::count();
        let step = match woken {
            Woken::Due => expire(&mut model.ui, world).map(|()| None),
            Woken::Leaving => Ok(None),
            Woken::Finished(result) => finished(result).map(|()| None),
            Woken::Update(Some((account, update))) => model.arrive(account, update).map(|()| None),
            Woken::Update(None) => {
                listening = false;
                Ok(None)
            }
            Woken::Input(event) => model.input(event, text),
        };

        if let Some(exit) = survive(step, &mut model.ui.screen)? {
            return Ok(exit);
        }
    }
}

impl Model<'_> {
    /// Draws the first frame from the rows it read, reads the rest of the
    /// list, then composes `link`'s Draft. A frame's allocations count from
    /// what triggered it to the end of its draw; the next counts from here.
    fn first_frame(&mut self, link: Option<Mailto>) -> Result<u64, Error> {
        let start = alloc_count::count();
        self.draw()?;
        alloc_count::report(start);
        let start = alloc_count::count();
        show(&mut self.ui.view, self.world.caches, None)?;
        if let Some(link) = link {
            drafting::compose_link(&link, &mut self.tty, &mut self.ui, self.world)?;
        }

        Ok(start)
    }

    /// Reads the cards around the selection, readies the Previews and older
    /// pages the screen wants, then draws it; returns where the Preview's
    /// text goes.
    fn frame(&mut self) -> Result<Option<Rect>, Error> {
        let panes = self.tty.panes(self.ui.screen.showing.mode())?;
        if let Some(list) = panes.list {
            let screen = message_list::capacity(list.height);
            self.ui.view.fill(self.world.caches, screen)?;
        }

        let text = panes.preview.map(preview_pane::text_area);
        if let Some(text) = text {
            let sources = Sources {
                caches: self.world.caches,
                fetch: &self.fetch,
            };
            let reading = self.ui.screen.showing.reading();
            self.previews
                .want(sources, &self.ui.view, reading, text.width)?;
        }

        if let Some(preview) = self.ui.screen.over.get_mut::<DraftPreview>() {
            let (accounts, look) = (self.world.accounts, self.theme.mail_look());
            draft_layer::lay_out(preview, accounts, panes.body, self.previews, look);
        }

        self.older.want(&self.ui.view, &self.fetch);
        self.draw()?;
        Ok(text)
    }

    fn draw(&mut self) -> Result<(), Error> {
        self.ui.screen.notice_write_failure(self.world.caches);
        let accounts = self.world.accounts;
        draw::draw(
            self.tty.terminal,
            self.theme,
            accounts,
            &mut self.ui,
            self.previews,
        )?;
        Ok(())
    }

    fn arrive(&mut self, account: AccountId, update: Update) -> Result<(), Error> {
        self.fetch.notice(account, &update);
        let screen = &mut self.ui.screen;
        let awaited = screen.awaited.notice(account, &update);
        if let Some(toast) = self.parts.notice(account, &update) {
            screen.toast = Some(toast);
        }

        self.older.notice(&update);
        if let Update::Arrived(arrivals) = &update {
            self.notifier.arrived(arrivals);
        }

        apply::apply(account, update, &mut self.ui, self.world, self.previews)?;
        awaiting::finish(awaited, &mut self.tty, &mut self.ui.screen, self.world)
    }

    /// Takes one input event; the `Exit` when it ends the TUI.
    fn input(
        &mut self,
        event: Option<io::Result<Event>>,
        text: Option<Rect>,
    ) -> Result<Option<Exit>, Error> {
        // A Draft opened, closed, discarded or brought back changes the list.
        let drafts = |screen: &Screen| (screen.over.is_open::<DraftPreview>(), screen.undo.count());
        let before = drafts(&self.ui.screen);
        match event {
            Some(Ok(Event::FocusGained)) => self.notifier.focus(true),
            Some(Ok(Event::FocusLost)) => self.notifier.focus(false),
            _ => {}
        }

        let step = handle::handle(event, &mut self.ui, half_page(text))?;
        let exit = take(self, step, text)?;
        if exit.is_none() && before != drafts(&self.ui.screen) {
            relist_local(&mut self.ui.screen, self.world.outbox);
        }

        Ok(exit)
    }
}

/// Rows a half-page scroll moves the Preview `text` by.
fn half_page(text: Option<Rect>) -> usize {
    text.map_or(1, |t| usize::from(t.height / 2).max(1))
}

/// A Cache error in answer to one event is toasted and the session goes
/// on. One in the frame ends it, as it would recur on every frame.
fn survive(step: Result<Option<Exit>, Error>, screen: &mut Screen) -> Result<Option<Exit>, Error> {
    match step {
        Err(Error::Cache(error)) => {
            tracing::error!("{error}");
            screen.toast = Some(Toast::error("cache error"));
            Ok(None)
        }
        step => step,
    }
}

/// The background work's end; signed out, the screen stays usable.
fn finished(result: Result<(), sync::Error>) -> Result<(), Error> {
    match result {
        Err(error) if error.signed_out() => {
            tracing::warn!("{error}");
            Ok(())
        }
        result => Ok(result?),
    }
}
