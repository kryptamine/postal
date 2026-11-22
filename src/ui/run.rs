use std::{io, path::Path};

use crossterm::event::Event;
use futures::{FutureExt, Stream, StreamExt, future::FusedFuture};
use ratatui::{Terminal, layout::Rect};
use tokio::time::Instant;

use crate::{
    cache::{self, Cache},
    compose::accounts::Accounts,
    compose::outbox::Outbox,
    domain::per_account::{AccountId, PerAccount},
    mail::reply_fetch::ReplyFetch,
    mail::routes::Routes,
    mail::status::Status,
    mail::steering::Ask,
    mail::sync,
    mail::update::Update,
    render::icons::Icons,
    ui::alloc_count,
    ui::apply,
    ui::drafts::composing,
    ui::drafts::draft_preview::DraftPreview,
    ui::drafts::drafting,
    ui::drafts::local_list::{open_local, relist_local},
    ui::draw,
    ui::exit::Exit,
    ui::handle,
    ui::list::folder_show::show,
    ui::list::message_list,
    ui::list::older_pages::OlderPages,
    ui::main_screen::Ui,
    ui::opening::opening,
    ui::outside::open_outside,
    ui::popup::settings::Settings,
    ui::preview::link_labels::{browse, label_links},
    ui::preview::preview_pane,
    ui::preview::previews::Previews,
    ui::screen::Screen,
    ui::steering::steer,
    ui::step::Step,
    ui::terminal::terminal_backend::TerminalBackend,
    ui::terminal::tty::Tty,
    ui::theme::Theme,
    ui::timed::{expire, next_change},
    ui::toast::Toast,
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

/// The network work behind the screen, what it reports as it goes, the
/// pages of older Messages and the bodies Replies wait on asked of it, what
/// the sync connection is asked, and where a Send leaves from.
pub struct Background<'a, W, U> {
    pub work: W,
    /// Each tells the Account it is of.
    pub updates: U,
    pub older: OlderPages,
    pub replies: ReplyFetch,
    pub asks: Routes<Ask>,
    pub accounts: Accounts<'a>,
    pub outbox: &'a Outbox,
    /// Where `config.toml` is, which Settings writes.
    pub config_dir: &'a Path,
    /// Each Account's at the start: signed out until `L` signs it in again.
    pub statuses: PerAccount<Status>,
    /// What failed before the TUI opened again, toasted.
    pub failure: Option<&'static str>,
}

/// The screen, with the terminal, the look, the Previews and the network
/// work it acts on.
struct Model<'a> {
    tty: Tty<'a>,
    theme: &'a mut Theme,
    previews: &'a mut Previews,
    ui: Ui,
    world: World<'a>,
    older: OlderPages,
    replies: ReplyFetch,
}

/// The one event loop: input, and each `Update` from the background work,
/// redrawn after each. The first frame reads only the rows it shows from the
/// Cache, before any network; the rest follows right after it.
pub async fn run(
    terminal: &mut Terminal<TerminalBackend>,
    theme: &mut Theme,
    caches: &PerAccount<Cache>,
    previews: &mut Previews,
    background: Background<
        '_,
        impl Future<Output = Result<(), sync::Error>>,
        impl Stream<Item = (AccountId, Update)> + Unpin,
    >,
) -> Result<Exit, Error> {
    let tty = Tty::new(terminal);
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
        let (account, update) = self.older.notice((account, update));
        apply::apply(account, update, &mut self.ui, self.world, self.previews)?;
        if let Some(result) = arrived {
            composing::fetched(result, &mut self.tty, &mut self.ui.screen, self.world)?;
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
        let step = handle::handle(event, &mut self.ui, half_page(text))?;
        let exit = self.take(step, text)?;
        if exit.is_none() && before != drafts(&self.ui.screen) {
            relist_local(&mut self.ui.screen, self.world.outbox);
        }

        Ok(exit)
    }

    fn take(&mut self, step: Step, text: Option<Rect>) -> Result<Option<Exit>, Error> {
        let (ui, world) = (&mut self.ui, self.world);
        match step {
            Step::Stay => {}
            Step::Links => label_links(ui, self.previews, text),
            Step::Steer(to) => steer(to, ui, world)?,
            Step::Browse(url) => ui.screen.toast = browse(&url),
            Step::Open(to) => {
                let tty = &mut self.tty;
                ui.screen.toast = open_outside::open(to, tty, self.theme, world.caches, ui)?;
            }
            Step::Draft(drafting) => drafting::draft(drafting, &mut self.tty, ui, world)?,
            Step::Reply(to) => drafting::reply(to, &mut self.tty, ui, world, &self.replies)?,
            Step::LocalDrafts => open_local(&mut ui.screen, world.outbox),
            Step::Settings => match Settings::open(world.config_dir) {
                Ok(settings) => ui.screen.over.open(settings),
                Err(error) => ui.screen.toast = Some(Toast::error(error.to_string())),
            },
            Step::Icons(choice) => {
                self.theme.icons = Icons::choose(choice);
                self.previews.set_look(self.theme.mail_look());
            }
            Step::Exit(exit) => return Ok(Some(exit)),
        }

        Ok(None)
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
