use std::{io, path::Path};

use crossterm::event::Event;
use futures::{FutureExt, Stream, StreamExt, future::FusedFuture};
use ratatui::{Terminal, layout::Rect};
use tokio::time::Instant;

use crate::{
    cache::{self, Cache},
    compose::accounts::Accounts,
    compose::outbox::Outbox,
    domain::body_key::BodyKey,
    domain::per_account::{AccountId, PerAccount},
    domain::thread::Key,
    mail::reply_fetch::ReplyFetch,
    mail::routes::Routes,
    mail::status::Status,
    mail::steering::Ask,
    mail::sync,
    mail::update::{BodyLink, Update},
    render::icons::Icons,
    ui::alloc_count,
    ui::apply,
    ui::composing,
    ui::deleting,
    ui::drafting,
    ui::draw,
    ui::exit::Exit,
    ui::folder_picker::FolderPicker,
    ui::folder_show::{folders, show, start_search, switch},
    ui::handle,
    ui::inbox_view::InboxView,
    ui::layout,
    ui::link_labels::{browse, label_links},
    ui::local_list::{open_local, relist_local},
    ui::main_screen::Ui,
    ui::marking::{mark_read, mark_unread},
    ui::message_list,
    ui::mode::Mode,
    ui::older_pages::OlderPages,
    ui::open_outside,
    ui::preview_pane,
    ui::previews::Previews,
    ui::screen::Screen,
    ui::settings::Settings,
    ui::step::{Steer, Step},
    ui::terminal_backend::TerminalBackend,
    ui::theme::Theme,
    ui::toast::Toast,
    ui::tty::Tty,
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
    let ui = opening(
        world,
        first_rows(tty.terminal)?,
        statuses,
        failure,
        previews,
    )?;
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

    /// Readies the Previews and older pages the screen wants, then draws it;
    /// returns where the Preview's text goes.
    fn frame(&mut self) -> Result<Option<Rect>, Error> {
        let text = panes(self.tty.terminal, self.ui.screen.showing.mode())?
            .preview
            .map(preview_pane::text_area);
        want(self.previews, self.world.caches, &self.ui, text)?;
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
        let drafts = |screen: &Screen| (screen.over.draft.is_some(), screen.undo.len());
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
            Step::Steer(steer) => steering(steer, ui, world)?,
            Step::Browse(url) => ui.screen.toast = browse(&url),
            Step::Open(to) => {
                let tty = &mut self.tty;
                ui.screen.toast = open_outside::open(to, tty, self.theme, world.caches, ui)?;
            }
            Step::Draft(drafting) => drafting::draft(drafting, &mut self.tty, ui, world)?,
            Step::Reply(to) => drafting::reply(to, &mut self.tty, ui, world, &self.replies)?,
            Step::LocalDrafts => open_local(&mut ui.screen, world.outbox),
            Step::Settings => match Settings::open(world.config_dir) {
                Ok(settings) => ui.screen.over.settings = Some(settings),
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

/// The screen as the TUI opens: the first `rows` of the list from the
/// Cache, each Account's Status and what failed before it opened.
fn opening(
    world: World,
    rows: usize,
    statuses: PerAccount<Status>,
    failure: Option<&'static str>,
    previews: &mut Previews,
) -> Result<Ui, Error> {
    let emails = world.accounts.0.map(|config| config.email.as_str().into());
    let mut view = InboxView::loading(emails);
    show(&mut view, world.caches, Some(rows))?;
    let rebuilt = world.caches.values().any(Cache::rebuilt);
    let mut screen = Screen::opening(rebuilt, statuses, failure);
    screen.outbox = world.outbox.count();
    for (account, &status) in screen.statuses.iter() {
        (status == Status::SignedOut).then(|| previews.link(account, BodyLink::Down));
    }

    Ok(Ui { view, screen })
}

/// Follows the Dwell, then says when the screen next changes on its own.
fn next_change(ui: &mut Ui, previews: &Previews, text: Option<Rect>) -> Option<Instant> {
    let target = dwelling(ui, previews, text);
    let Ui { view, screen } = ui;
    screen
        .dwell
        .follow(view.selected_key(), target, Instant::now());
    screen.changes_at(Instant::now())
}

/// The selected Thread and its newest body key, while a Dwell may run on
/// it: listed, Unread, and its body shown in the Preview `text`.
fn dwelling(ui: &Ui, previews: &Previews, text: Option<Rect>) -> Option<(Key, BodyKey)> {
    let Ui { view, screen } = ui;
    let listed = matches!(screen.showing.mode(), Mode::List | Mode::Query)
        && screen.over.picker.is_none()
        && screen.over.local.is_none()
        && screen.over.draft.is_none();
    let text = text.filter(|_| listed && !screen.help && view.selected_unread())?;
    previews.get(view, text.width)?;
    Some((view.selected_key()?, view.selected_body()?))
}

/// Drops the Toast and ends the Dwell when their time is up.
fn expire(ui: &mut Ui, world: World) -> Result<(), Error> {
    let now = Instant::now();
    ui.screen.expire(now);
    if ui.screen.dwell.ended(now) {
        mark_read(ui, world)?;
    }

    Ok(())
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

/// Readies the Previews around the selection, or those unfolded while
/// reading, once `text` is laid out.
fn want(
    previews: &mut Previews,
    caches: &PerAccount<Cache>,
    ui: &Ui,
    text: Option<Rect>,
) -> Result<(), Error> {
    let Some(text) = text else {
        return Ok(());
    };

    let view = &ui.view;
    match ui.screen.showing.reading() {
        Some(reading) => previews.want_stack(caches, view, reading, text.width),
        None => previews.want(caches, view, text.width),
    }?;

    Ok(())
}

/// How many cards the list shows on the first frame.
fn first_rows(terminal: &Terminal<TerminalBackend>) -> Result<usize, Error> {
    let list = panes(terminal, Mode::List)?.list;
    Ok(message_list::capacity(list.map_or(0, |list| list.height)))
}

fn panes(terminal: &Terminal<TerminalBackend>, mode: Mode) -> Result<layout::Panes, Error> {
    let size = terminal.size().map_err(Error::Terminal)?;
    Ok(layout::split(
        Rect::new(0, 0, size.width, size.height),
        mode,
    ))
}

/// Opens the Folder Picker as the Cache last saw the Folders, starts a
/// Search, shows the Folder picked, or marks, deletes or restores Messages,
/// asking the sync connection to follow.
fn steering(steer: Steer, ui: &mut Ui, world: World) -> Result<(), Error> {
    let (caches, asks) = (world.caches, world.asks);
    // Signed out, nothing syncs and the ask is dropped.
    match steer {
        Steer::Folders => {
            let folders = folders(caches)?;
            let view = &ui.view;
            ui.screen.over.picker = Some(FolderPicker::new(folders, &view.folder, &view.accounts));
            for account in caches.ids() {
                asks.send(account, Ask::ListFolders);
            }
        }
        Steer::Show(folder) => {
            if switch(ui, caches, folder)? {
                for account in caches.ids() {
                    if let Some(shown) = ui.view.folder.of(account) {
                        asks.send(account, Ask::Show(shown.clone()));
                    }
                }
            }
        }
        Steer::Read(thread) => {
            mark_read(ui, world)?;
            if let Some((account, id)) = thread {
                asks.send(account, Ask::Thread(id));
            }
        }
        Steer::MarkUnread => mark_unread(ui, world)?,
        Steer::Delete => deleting::delete(ui, world)?,
        Steer::Undo => deleting::undo(ui, world)?,
        Steer::Search => start_search(ui, caches)?,
    }

    Ok(())
}
