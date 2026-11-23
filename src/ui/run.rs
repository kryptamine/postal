use std::{io, path::Path};

use crossterm::event::Event;
use futures::{FutureExt, Stream, StreamExt, future::FusedFuture};
use ratatui::layout::Rect;
use tokio::time::Instant;

use crate::{
    account::config::NotifyChoice,
    cache::{self, Cache},
    compose::accounts::Accounts,
    compose::outbox::Outbox,
    domain::per_account::{AccountId, PerAccount},
    mail::forward_fetch::ForwardFetch,
    mail::reply_fetch::ReplyFetch,
    mail::routes::Routes,
    mail::status::Status,
    mail::steering::Ask,
    mail::sync,
    mail::update::Update,
    ui::alloc_count,
    ui::drafts::composing,
    ui::drafts::draft_preview::DraftPreview,
    ui::drafts::forwarding,
    ui::drafts::local_list::relist_local,
    ui::exit::Exit,
    ui::frame::draw,
    ui::frame::theme::Theme,
    ui::input::handle,
    ui::list::folder_show::show,
    ui::list::message_list,
    ui::list::older_pages::OlderPages,
    ui::outside::notifier::Notifier,
    ui::outside::wanted_attachments::WantedAttachments,
    ui::preview::preview_pane,
    ui::preview::previews::Previews,
    ui::react::apply,
    ui::react::opening::opening,
    ui::react::take::take,
    ui::react::timed::{expire, next_change},
    ui::state::screen::Screen,
    ui::state::ui::Ui,
    ui::terminal::tty::Tty,
    ui::world::World,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("terminal: {0}")]
    Terminal(io::Error),
    #[error(transparent)]
    Cache(#[from] cache::Error),
    #[error(transparent)]
    Sync(#[from] sync::Error),
}

/// The network work behind the screen and what it reports; what is asked of
/// it (older pages, what a Reply or Forward waits on, attachments) and of
/// the sync connection; and where a Send leaves from.
pub struct Background<'a, W, U> {
    pub work: W,
    /// Each tells the Account it is of.
    pub updates: U,
    pub older: OlderPages,
    pub replies: ReplyFetch,
    pub forwards: ForwardFetch,
    pub attachments: WantedAttachments,
    pub asks: Routes<Ask>,
    pub accounts: Accounts<'a>,
    pub outbox: &'a Outbox,
    /// Where `config.toml` is, which Settings writes.
    pub config_dir: &'a Path,
    /// Each Account's at the start: signed out until `L` signs it in again.
    pub statuses: PerAccount<Status>,
    /// What failed before the TUI opened again, toasted.
    pub failure: Option<&'static str>,
    pub notify: NotifyChoice,
}

/// The screen, with the terminal, the look, the Previews and the network
/// work it acts on.
pub struct Model<'a> {
    pub tty: Tty<'a>,
    pub theme: &'a mut Theme,
    pub previews: &'a mut Previews,
    pub ui: Ui,
    pub world: World<'a>,
    older: OlderPages,
    pub replies: ReplyFetch,
    pub forwards: ForwardFetch,
    pub attachments: WantedAttachments,
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
    };

    let (statuses, failure) = (background.statuses, background.failure);
    let ui = opening(world, &tty, statuses, failure, previews)?;
    let mut model = Model {
        tty,
        theme,
        previews,
        ui,
        world,
        older: background.older,
        replies: background.replies,
        forwards: background.forwards,
        attachments: background.attachments,
        notifier: Notifier::new(background.notify),
    };

    let mut work = std::pin::pin!(background.work.fuse());
    let mut listening = true;
    // A frame's allocations count from what triggered it, the input or the
    // Cache read, to the end of its draw; waiting on input is left out.
    let mut frame_start = alloc_count::count();
    frame_start = model.first_frame(frame_start)?;
    loop {
        let text = model.frame()?;
        alloc_count::report(frame_start);
        let change = next_change(&mut model.ui, model.previews, text);
        tokio::select! {
            () = tokio::time::sleep_until(change.unwrap_or_else(Instant::now)), if change.is_some() => {
                frame_start = alloc_count::count();
                expire(&mut model.ui, world)?;
            }
            result = &mut work, if !work.is_terminated() => {
                frame_start = alloc_count::count();
                finished(result)?;
            }
            update = updates.next(), if listening => {
                frame_start = alloc_count::count();
                match update {
                    Some((account, update)) => model.arrive(account, update)?,
                    None => listening = false,
                }
            }
            event = model.tty.events.next() => {
                frame_start = alloc_count::count();
                if let Some(exit) = model.input(event, text)? {
                    return Ok(exit);
                }
            }
        }
    }
}

impl Model<'_> {
    /// Draws the first frame from the rows it read, then reads the rest of
    /// the list; the next frame's allocations count from here.
    fn first_frame(&mut self, start: u64) -> Result<u64, Error> {
        self.draw()?;
        alloc_count::report(start);
        let start = alloc_count::count();
        show(&mut self.ui.view, self.world.caches, None)?;
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
            let (view, reading) = (&self.ui.view, self.ui.screen.showing.reading());
            let caches = self.world.caches;
            self.previews.want(caches, view, reading, text.width)?;
        }

        self.older.want(&self.ui.view);
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
        let replying = self.ui.screen.replying.as_mut();
        let arrived = self.replies.notice(account, &update, replying);
        let forwarding = self.ui.screen.forwarding.as_mut();
        let forwarded = self.forwards.notice(account, &update, forwarding);
        if let Some(toast) = self.attachments.notice(account, &update) {
            self.ui.screen.toast = Some(toast);
        }

        if let Update::Arrived(arrivals) = &update {
            self.notifier.arrived(arrivals);
        }

        let (account, update) = self.older.notice((account, update));
        apply::apply(account, update, &mut self.ui, self.world, self.previews)?;
        if let Some(result) = arrived {
            composing::fetched(result, &mut self.tty, &mut self.ui.screen, self.world)?;
        }

        if let Some(result) = forwarded {
            forwarding::forwarded(result, &mut self.tty, &mut self.ui.screen, self.world)?;
        }

        Ok(())
    }

    /// Takes one input event; the `Exit` when it ends the TUI.
    fn input(
        &mut self,
        event: Option<io::Result<Event>>,
        text: Option<Rect>,
    ) -> Result<Option<Exit>, Error> {
        // A Draft opened, closed, discarded or brought back changes the list.
        let drafts = |screen: &Screen| (screen.over.is_open::<DraftPreview>(), screen.undo.len());
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
