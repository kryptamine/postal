use std::{io, path::Path, sync::Arc};

use crossterm::event::Event;
use futures::{FutureExt, Stream, StreamExt, future::FusedFuture};
use ratatui::{Frame, Terminal, layout::Rect};
use tokio::time::Instant;

use crate::{
    accounts::Accounts,
    all_inboxes, alloc_count,
    awaited_reply::AwaitedReply,
    backfill::Backfill,
    body::Body,
    browser,
    cache::{self, Cache},
    composing,
    config::IconChoice,
    draft_pane::DraftPane,
    draft_preview::DraftPreview,
    folder::{Folder, Role},
    folder_picker::FolderPicker,
    help_popup::HelpPopup,
    hint_line::HintLine,
    icons::Icons,
    inbox::Inbox,
    inbox_view::InboxView,
    input::Input,
    keymap::{self, Action, DraftKey, LocalKey, Pick, QueuedKey},
    layout,
    link_hints::{LinkHints, Typed},
    local_drafts::LocalDrafts,
    local_drafts_pane::LocalDraftsPane,
    message::Message,
    message_header,
    message_list::{self, MessageList},
    mode::Mode,
    older_pages::OlderPages,
    open_outside::{self, Outside},
    outbox::Outbox,
    picker_popup::PickerPopup,
    preview_pane::{self, PreviewPane},
    previews::Previews,
    reading::Reading,
    reply_fetch::ReplyFetch,
    routes::Routes,
    screen::Screen,
    search::Search,
    settings::{self, Settings},
    settings_popup::SettingsPopup,
    stack,
    status::Status,
    steering::Ask,
    sync,
    terminal_backend::{self, TerminalBackend},
    theme::Theme,
    thread::Key,
    thread_pane::ThreadPane,
    toast::Toast,
    undo::Undo,
    update::Update,
};

#[derive(Debug)]
pub enum Error {
    Terminal(io::Error),
    Cache(cache::Error),
    Sync(sync::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Terminal(source) => write!(f, "terminal: {source}"),
            Self::Cache(source) => source.fmt(f),
            Self::Sync(source) => source.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

/// How the TUI ended.
pub enum Exit {
    Quit,
    /// `L` asked for the login flow of this Account, run with the terminal
    /// handed back.
    SignIn(usize),
    /// Settings asked for the Onboarding from the Provider list.
    AddAccount,
    /// Settings asked to unlink the Account of this address.
    Unlink(String),
    /// Settings changed what the Accounts read at start.
    Reload,
}

/// The network work behind the screen, what it reports as it goes, the
/// pages of older Messages and the bodies Replies wait on asked of it, what
/// the sync connection is asked, and where a Send leaves from.
pub struct Background<'a, W, U> {
    pub work: W,
    /// Each tells the Account it is of, by its place in this run.
    pub updates: U,
    pub older: OlderPages,
    pub replies: ReplyFetch,
    pub asks: Routes<Ask>,
    pub accounts: Accounts<'a>,
    pub outbox: &'a Outbox,
    /// Where `config.toml` is, which Settings writes.
    pub config_dir: &'a Path,
    /// Each Account's at the start: signed out until `L` signs it in again.
    pub statuses: Vec<Status>,
    /// What failed before the TUI opened again, toasted.
    pub failure: Option<&'static str>,
}

/// The one event loop: input, and each `Update` from the background work,
/// redrawn after each. The first frame reads only the rows it shows from the
/// Cache, before any network; the rest follows right after it.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
#[expect(
    clippy::too_many_lines,
    reason = "one select over every source of change, whose state its arms share"
)]
pub async fn run(
    terminal: &mut Terminal<TerminalBackend>,
    theme: &mut Theme,
    caches: &[Cache],
    previews: &mut Previews,
    background: Background<
        '_,
        impl Future<Output = Result<(), sync::Error>>,
        impl Stream<Item = (usize, Update)> + Unpin,
    >,
) -> Result<Exit, Error> {
    let mut events = Input::new();
    let accounts = background.accounts;
    let emails = accounts.0.iter().map(|config| config.email.as_str().into());
    let mut view = InboxView::loading(emails.collect());
    let rows = first_rows(terminal)?;
    show(&mut view, caches, Some(rows))?;
    let (mut updates, mut older, asks) = (background.updates, background.older, background.asks);
    let (outbox, config_dir) = (background.outbox, background.config_dir);
    let mut replies = background.replies;
    let rebuilt = caches.iter().any(Cache::rebuilt);
    let mut screen = Screen::opening(rebuilt, background.statuses, background.failure);
    screen.outbox = outbox.count();
    for (account, &status) in screen.statuses.iter().enumerate() {
        (status == Status::SignedOut).then(|| previews.link(account, false));
    }

    let mut work = std::pin::pin!(background.work.fuse());
    let mut whole = false;
    let mut listening = true;
    // A frame's allocations count from what triggered it, the input or the
    // Cache read, to the end of its draw; waiting on input is left out.
    let mut frame_start = alloc_count::count();
    loop {
        let text = panes(terminal, screen.mode)?
            .preview
            .map(preview_pane::text_area);
        want(previews, caches, (&view, &screen), text.filter(|_| whole))?;
        whole.then(|| older.want(&view));
        screen.notice_write_failure(caches);
        draw(
            terminal,
            (theme, accounts),
            &mut view,
            previews,
            &mut screen,
        )?;
        alloc_count::report(frame_start);
        if !whole {
            whole = true;
            frame_start = alloc_count::count();
            show(&mut view, caches, None)?;
            continue;
        }

        let change = next_change(&view, &mut screen, previews, text);
        tokio::select! {
            () = tokio::time::sleep_until(change.unwrap_or_else(Instant::now)), if change.is_some() => {
                frame_start = alloc_count::count();
                expire(&mut view, &mut screen, caches, &asks)?;
            }
            result = &mut work, if !work.is_terminated() => {
                frame_start = alloc_count::count();
                finished(result)?;
            }
            update = updates.next(), if listening => {
                frame_start = alloc_count::count();
                let Some((account, update)) = update else {
                    listening = false;
                    continue;
                };

                let arrived = replies.notice((account, &update), screen.replying.as_mut());
                apply(older.notice((account, update)), &mut view, &mut screen, (caches, outbox), previews)?;
                if let Some(result) = arrived {
                    composing::fetched(result, terminal, &mut events, &mut screen, (caches, accounts))?;
                }
            }
            event = events.next() => {
                frame_start = alloc_count::count();
                // A Draft opened, closed, discarded or brought back changes the list.
                let drafts_before = (screen.draft.is_some(), screen.undo.len());
                match handle(event, &mut view, &mut screen, half_page(text))? {
                    Step::Stay => {}
                    Step::Links => label_links(&mut screen, previews, &view, text),
                    step @ (Step::Folders | Step::Show(_) | Step::Search | Step::Read(_) | Step::MarkUnread | Step::Delete | Step::Undo) => steer(step, &mut view, &mut screen, caches, &asks)?,
                    Step::Browse(url) => screen.toast = browse(&url),
                    Step::Open(to) => screen.toast = open_outside::open(to, (terminal, &mut events), theme, caches, (&view, screen.reading.as_ref()))?,
                    step @ (Step::Compose | Step::Edit | Step::Send) => {
                        let from = accounts.noted(composing_from(&view));
                        draft(&step, terminal, &mut events, &mut screen, (accounts, from, outbox, &asks))?;
                    }
                    Step::Reply(all) => {
                        if let Some(message) = answered(&view, &screen) {
                            let awaited = AwaitedReply { message, all, missing: Vec::new() };
                            composing::reply(terminal, &mut events, &mut screen, (caches, &replies), (awaited, view.stack(), accounts))?;
                        }
                    }
                    Step::LocalDrafts => open_local(&mut screen, outbox),
                    Step::SignIn(account) => return Ok(Exit::SignIn(account)),
                    Step::Settings => match Settings::open(config_dir) {
                        Ok(settings) => screen.settings = Some(settings),
                        Err(error) => screen.toast = Some(Toast::error(error.to_string())),
                    },
                    Step::Icons(choice) => {
                        theme.icons = Icons::choose(choice);
                        previews.set_look(theme.mail_look());
                    }
                    Step::AddAccount => return Ok(Exit::AddAccount),
                    Step::Unlink(email) => return Ok(Exit::Unlink(email)),
                    Step::Reload => return Ok(Exit::Reload),
                    Step::Quit => return Ok(Exit::Quit),
                }

                if drafts_before != (screen.draft.is_some(), screen.undo.len()) {
                    relist_local(&mut screen, outbox);
                }
            }
        }
    }
}

/// Follows the Dwell, then says when the screen next changes on its own.
fn next_change(
    view: &InboxView,
    screen: &mut Screen,
    previews: &Previews,
    text: Option<Rect>,
) -> Option<Instant> {
    let target = dwelling(view, screen, previews, text);
    screen
        .dwell
        .follow(view.selected_key(), target, Instant::now());
    screen.changes_at(Instant::now())
}

/// Drops the Toast and ends the Dwell when their time is up.
fn expire(
    view: &mut InboxView,
    screen: &mut Screen,
    caches: &[Cache],
    asks: &Routes<Ask>,
) -> Result<(), Error> {
    let now = Instant::now();
    screen.expire(now);
    if screen.dwell.ended(now) {
        mark_read(view, screen, caches, asks)?;
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
        result => result.map_err(Error::Sync),
    }
}

/// Readies the Previews around the selection, or those unfolded while
/// reading, once `text` is laid out.
fn want(
    previews: &mut Previews,
    caches: &[Cache],
    (view, screen): (&InboxView, &Screen),
    text: Option<Rect>,
) -> Result<(), Error> {
    let Some(text) = text else {
        return Ok(());
    };

    match &screen.reading {
        Some(reading) => {
            let stack = (|key| reading.unfolded(key), reading.cursor);
            previews.want_stack(caches, view, stack, text.width)
        }
        None => previews.want(caches, view, text.width),
    }
    .map_err(Error::Cache)
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

fn draw(
    terminal: &mut Terminal<TerminalBackend>,
    (theme, accounts): (&Theme, Accounts),
    view: &mut InboxView,
    previews: &Previews,
    screen: &mut Screen,
) -> Result<(), Error> {
    terminal
        .draw(|frame| {
            let panes = layout::split(frame.area(), screen.mode);
            if let Some(list) = panes.list {
                frame.render_stateful_widget(MessageList { theme }, list, view);
            }

            if let (Some(pane), Some(reading)) = (panes.preview, screen.reading.as_mut()) {
                let hints = screen.hints.as_ref();
                let widget = ThreadPane {
                    theme,
                    view,
                    previews,
                    hints,
                };

                frame.render_stateful_widget(widget, pane, reading);
            } else if let Some(pane) = panes.preview {
                let body = previews.selected(view, preview_pane::text_area(pane).width);
                let hints = screen.hints.as_ref();
                let widget = PreviewPane { theme, body, hints };

                frame.render_stateful_widget(widget, pane, view);
            }

            let keys = screen.keys();
            let (status, account) = screen.status();
            let hint_line =
                HintLine {
                    theme,
                    toast: screen.toast.as_ref(),
                    status,
                    account: account.map(|account| accounts.email(account)),
                    outbox: screen.outbox,
                    mode: keys,
                    query: view.search.as_ref().map(Search::query).filter(|_| {
                        !matches!(keys, Mode::Draft | Mode::LocalDrafts | Mode::Queued)
                    }),
                    now: Instant::now(),
                };

            if let Some(cursor) = hint_line.cursor(panes.hint) {
                frame.set_cursor_position(cursor);
            }

            frame.render_widget(hint_line, panes.hint);
            popups(frame, (theme, accounts), panes.body, screen);
        })
        .map_err(Error::Terminal)?;
    Ok(())
}

/// What shows over the panes in `body`: the Folder Picker, Local Drafts, a
/// Draft's preview, Settings, then the `?` popup over any.
fn popups(frame: &mut Frame, (theme, accounts): (&Theme, Accounts), body: Rect, screen: &Screen) {
    if let Some(picker) = &screen.picker {
        let popup = PickerPopup { theme, picker };
        frame.set_cursor_position(popup.cursor(body));
        frame.render_widget(popup, body);
    }

    if let Some(local) = &screen.local {
        let pane = LocalDraftsPane {
            theme,
            local,
            accounts,
        };

        frame.render_widget(pane, body);
    }

    if let Some(preview) = &screen.draft {
        // The `From` a Send adds.
        let outgoing = accounts.outgoing(accounts.place(preview.account.as_deref()));
        let from = outgoing.map(|outgoing| {
            let email = || outgoing.config.email.clone();
            outgoing
                .from()
                .map_or_else(|_bad| email(), |from| from.to_string())
        });

        let pane = DraftPane {
            theme,
            preview,
            from: from.as_deref().unwrap_or_default(),
        };

        frame.render_widget(pane, body);
    }

    if let Some(settings) = &screen.settings {
        let popup = SettingsPopup { theme, settings };
        if let Some(cursor) = popup.cursor(body) {
            frame.set_cursor_position(cursor);
        }

        frame.render_widget(popup, body);
    }

    if screen.help {
        let popup = HelpPopup {
            theme,
            mode: screen.keys(),
            signed_out: screen.signed_out().is_some(),
        };

        frame.render_widget(popup, body);
    }
}

fn apply(
    (account, update): (usize, Update),
    view: &mut InboxView,
    screen: &mut Screen,
    (caches, outbox): (&[Cache], &Outbox),
    previews: &mut Previews,
) -> Result<(), Error> {
    match update {
        Update::Inbox => {
            let (thread, selected) = (view.selected_key(), view.selected_body());
            let stack: Vec<i64> = match screen.reading {
                Some(_) => view.stack().map(|m| m.body_key).collect(),
                None => Vec::new(),
            };

            show(view, caches, None)?;
            // The labels belong to the Messages they were laid out on, and
            // Reading to the Thread it was opened on; a Query being typed stays.
            if screen.reading.is_none() {
                if view.selected_body() != selected {
                    screen.hints = None;
                }
            } else if !screen.follow(view, thread) && !view.stack().map(|m| m.body_key).eq(stack) {
                screen.hints = None;
                if let Some(reading) = &mut screen.reading {
                    reading.hold = true;
                }
            }

            relist(screen, view, caches)?;
        }
        Update::Folders => relist(screen, view, caches)?,
        Update::FolderGone(folder) => {
            if account == view.folder.account && folder == view.folder.name {
                let gone = format!("Folder {} no longer exists", view.folder.label());
                // The sync connection is back on the Inbox already.
                switch(view, screen, caches, Folder::inbox(account))?;
                screen.toast = Some(Toast::new(gone));
            }

            relist(screen, view, caches)?;
        }
        Update::Bodies(folder, fetched) => previews.fetched((account, &folder), fetched),
        Update::BodyLink(live) => {
            previews.link(account, live);
            screen.backfill = None;
        }
        Update::Rendered(key, preview) => previews.rendered(key, preview),
        Update::Status(status) => report(screen, previews, (account, status)),
        Update::Backfill(Backfill::Fetching { done, total }) => {
            screen.backfill = Some(Status::Syncing { done, total });
        }
        Update::Backfill(Backfill::Paged {
            folder,
            nothing_below,
        }) => {
            screen.backfill = None;
            paged(view, caches, (account, folder, nothing_below))?;
        }
        Update::NoTrash => screen.toast = Some(Toast::error("no Trash Folder")),
        Update::Unrestored => screen.toast = Some(Toast::error("nothing to undo")),
        Update::Sent => {
            relist_local(screen, outbox);
            screen.outbox = outbox.count();
            screen.toast = Some(Toast::new("Sent"));
        }
        Update::Refused => {
            relist_local(screen, outbox);
            screen.outbox = outbox.count();
            screen.toast = Some(Toast::error("refused · back in Drafts"));
        }
    }

    Ok(())
}

/// A page of older Messages of `folder` of `account` is in; the server had
/// nothing below `nothing_below` when it says so.
fn paged(
    view: &mut InboxView,
    caches: &[Cache],
    (account, folder, nothing_below): (usize, Arc<str>, Option<u32>),
) -> Result<(), Error> {
    if let Some(known) = view.nothing_below.get_mut(account) {
        *known = nothing_below.map(|uid| (folder, uid));
    }

    // In All Inboxes, an Account with nothing older sets no edge (ADR 0014).
    show(view, caches, None)
}

/// The sync connection of `account` is in `status` now.
fn report(screen: &mut Screen, previews: &mut Previews, (account, status): (usize, Status)) {
    if status == Status::SignedOut {
        previews.link(account, false);
    }

    if let Some(was) = screen.statuses.get_mut(account) {
        *was = status;
    }
}

/// What the event loop does after one input event.
enum Step {
    Stay,
    Links,
    Folders,
    Show(Folder),
    Search,
    /// Reading opened, on the Gmail Thread with this ID if it has one, of
    /// this Account: its Messages in other Folders are asked for.
    Read(Option<(usize, u64)>),
    MarkUnread,
    Delete,
    Undo,
    Browse(Arc<str>),
    Open(Outside),
    LocalDrafts,
    Compose,
    /// Reply, or Reply All when true.
    Reply(bool),
    /// The shown Draft in the editor again.
    Edit,
    Send,
    /// The sign-in of this Account, signed out.
    SignIn(usize),
    Settings,
    Icons(IconChoice),
    AddAccount,
    Unlink(String),
    Reload,
    Quit,
}

/// Acts on one input event, where the view alone can take it.
fn handle(
    event: Option<io::Result<Event>>,
    view: &mut InboxView,
    screen: &mut Screen,
    half_page: usize,
) -> Result<Step, Error> {
    match event {
        None => Ok(Step::Quit),
        Some(Err(source)) => Err(Error::Terminal(source)),
        Some(Ok(Event::Key(key))) if screen.help => {
            screen.help = !keymap::closes_help(key);
            Ok(Step::Stay)
        }
        Some(Ok(Event::Key(key))) if !keymap::pressed(key) => Ok(Step::Stay),
        Some(Ok(Event::Key(key))) if screen.settings.is_some() => Ok(on_settings(key, screen)),
        Some(Ok(Event::Key(key))) if screen.draft.is_some() => Ok(on_draft(key, screen)),
        Some(Ok(Event::Key(key))) if screen.local.is_some() => Ok(on_local(key, screen)),
        Some(Ok(Event::Key(key))) if screen.picker.is_some() => Ok(pick(key, screen)),
        Some(Ok(Event::Key(key))) if screen.hints.is_some() => {
            let typed = screen.hints.as_mut().map_or(Typed::Cancel, |h| h.key(key));
            if !matches!(typed, Typed::Partial) {
                screen.hints = None;
            }

            Ok(match typed {
                Typed::Open(url) => Step::Browse(url),
                Typed::Partial | Typed::Cancel => Step::Stay,
            })
        }
        Some(Ok(Event::Key(key))) => Ok(act(
            keymap::action(key, screen.mode),
            view,
            screen,
            half_page,
        )),
        // The labels sit where the old width laid the links out.
        Some(Ok(Event::Resize(..))) => {
            screen.hints = None;
            Ok(Step::Stay)
        }
        Some(Ok(_)) => Ok(Step::Stay),
    }
}

fn act(
    action: Option<Action>,
    view: &mut InboxView,
    screen: &mut Screen,
    half_page: usize,
) -> Step {
    match action {
        Some(Action::Quit) => Step::Quit,
        Some(action @ (Action::Read | Action::Step(_) | Action::Fold)) => {
            read(action, view, screen)
        }
        Some(Action::MarkUnread) => Step::MarkUnread,
        Some(Action::Delete) => Step::Delete,
        Some(Action::Undo) => Step::Undo,
        Some(Action::Compose) => Step::Compose,
        Some(Action::Reply { all }) => Step::Reply(all),
        Some(Action::Back | Action::KeepQuery) => {
            screen.list();
            Step::Stay
        }
        Some(Action::Open(to)) => Step::Open(to),
        Some(Action::SignIn) => screen.signed_out().map_or(Step::Stay, Step::SignIn),
        Some(Action::Settings) => Step::Settings,
        Some(Action::Links) => Step::Links,
        Some(Action::Folders) => Step::Folders,
        Some(action @ (Action::SwitchView | Action::SwitchQuoted)) => {
            reshow(action, view, screen);
            Step::Stay
        }
        Some(Action::Help) => {
            screen.help = true;
            Step::Stay
        }
        Some(Action::Search) => Step::Search,
        Some(Action::Type(c)) => {
            view.edit_query(|search| search.push(c));
            Step::Stay
        }
        Some(Action::Erase) => {
            view.edit_query(Search::pop);
            Step::Stay
        }
        Some(Action::EndSearch { restore }) => {
            view.end_search(restore);
            screen.list();
            Step::Stay
        }
        Some(Action::Move(movement)) => {
            view.apply(movement);
            Step::Stay
        }
        Some(Action::Scroll(scroll)) => {
            match &mut screen.reading {
                Some(reading) => reading.scroll(scroll, half_page),
                None => view.scroll(scroll, half_page),
            }

            Step::Stay
        }
        None => Step::Stay,
    }
}

/// Shows HTML mail in the other View; on any other `action`, the Quote of
/// the selected Message, or in Reading of the one at the cursor.
fn reshow(action: Action, view: &mut InboxView, screen: &mut Screen) {
    if matches!(action, Action::SwitchView) {
        view.html = view.html.other();
        view.preview_scroll = 0;
        if let Some(reading) = &mut screen.reading {
            reading.reveal = true;
        }
    } else {
        let cursor = screen.reading.as_ref().map(|reading| reading.cursor);
        let message = cursor.map_or_else(
            || view.selected_message(),
            |cursor| view.stack().find(|m| m.body_key == cursor),
        );

        if let Some((account, body)) = message.map(|m| (m.account, m.body_key)) {
            view.switch_quoted(account, body);
        }
    }

    // The labels sit where the links were before the switch.
    screen.hints = None;
}

/// Opens the selected Thread in Reading, or moves through it there.
fn read(action: Action, view: &InboxView, screen: &mut Screen) -> Step {
    match (action, &mut screen.reading) {
        (Action::Read, _) => {
            let Some(reading) = Reading::open(view.stack()) else {
                return Step::Stay;
            };

            screen.mode = Mode::Reading;
            screen.reading = Some(reading);
            // Gmail keeps Trash and Spam out of All Mail, and their Threads
            // apart; a Search never shows theirs.
            let apart =
                view.search.is_none() && matches!(view.folder.role, Some(Role::Trash | Role::Spam));
            let gmail = match view.selected_key() {
                Some(Key::Gmail(account, id)) if !apart => Some((account, id)),
                _ => None,
            };

            return Step::Read(gmail);
        }
        (Action::Step(direction), Some(reading)) => reading.step(direction, view.stack()),
        (Action::Fold, Some(reading)) => reading.fold(),
        _ => {}
    }

    Step::Stay
}

/// What a Reply answers: the newest Message of the selected Thread not from
/// the Account, else its newest; in Reading, the one at the cursor.
fn answered(view: &InboxView, screen: &Screen) -> Option<Message> {
    let message = screen.reading.as_ref().map_or_else(
        || {
            view.stack()
                .filter(|m| !m.from_me)
                .max_by_key(|m| m.received)
                .or_else(|| view.selected_message())
        },
        |reading| view.stack().find(|m| m.body_key == reading.cursor),
    );

    message.cloned()
}

/// The selected Thread and its newest body key, while a Dwell may run on
/// it: listed, Unread, and its body shown in the Preview `text`.
fn dwelling(
    view: &InboxView,
    screen: &Screen,
    previews: &Previews,
    text: Option<Rect>,
) -> Option<(Key, i64)> {
    let listed = matches!(screen.mode, Mode::List | Mode::Query)
        && screen.picker.is_none()
        && screen.local.is_none()
        && screen.draft.is_none();
    let text = text.filter(|_| listed && !screen.help && view.selected_unread())?;
    previews.get(view, text.width)?;
    Some((view.selected_key()?, view.selected_body()?))
}

/// Mark Read on the selected Thread's Messages in their Folders.
fn mark_read(
    view: &mut InboxView,
    screen: &mut Screen,
    caches: &[Cache],
    asks: &Routes<Ask>,
) -> Result<(), Error> {
    let unread: Vec<_> = view
        .stack()
        .filter(|m| m.unread)
        .filter_map(|m| Some((Arc::clone(m.folder.as_ref()?), m.uid)))
        .collect();
    let Some(account) = view.stack().next().map(|m| m.account) else {
        return Ok(());
    };

    mark((view, screen), caches, asks, (account, &unread, false)).map(drop)
}

/// Mark Unread on the selected Thread's newest Message, or in Reading on
/// the one at the cursor.
fn mark_unread(
    view: &mut InboxView,
    screen: &mut Screen,
    caches: &[Cache],
    asks: &Routes<Ask>,
) -> Result<(), Error> {
    let message = screen.reading.as_ref().map_or_else(
        || view.selected_message(),
        |reading| view.stack().find(|m| m.body_key == reading.cursor),
    );

    let Some(message) = message else {
        return Ok(());
    };

    let Some(folder) = message.folder.as_ref() else {
        screen.toast = Some(Toast::new("only in another Folder"));
        return Ok(());
    };

    let (account, marked) = (message.account, [(Arc::clone(folder), message.uid)]);
    if let Some(key) = view.selected_key() {
        screen.dwell.spare(key);
    }

    if mark((view, screen), caches, asks, (account, &marked, true))? {
        screen.toast = Some(Toast::new("marked Unread"));
    }

    Ok(())
}

/// Sets Unread on `messages` of `account` in the Cache and the screen, and
/// asks its sync connection to tell the server; false when none changed.
fn mark(
    (view, screen): (&mut InboxView, &mut Screen),
    caches: &[Cache],
    asks: &Routes<Ask>,
    (account, messages, unread): (usize, &[(Arc<str>, u32)], bool),
) -> Result<bool, Error> {
    if !caches[account]
        .mark(messages, unread)
        .map_err(Error::Cache)?
    {
        return Ok(false);
    }

    show(view, caches, None)?;
    relist(screen, view, caches)?;
    // Signed out, the change waits in the Cache.
    asks.send(account, Ask::Changes);
    Ok(true)
}

/// Delete on the selected Thread's Messages in their Folders, or in
/// Reading on the one at the cursor.
fn delete(
    view: &mut InboxView,
    screen: &mut Screen,
    caches: &[Cache],
    asks: &Routes<Ask>,
) -> Result<(), Error> {
    // A Search never shows Trash.
    if view.search.is_none() && view.folder.role == Some(Role::Trash) {
        screen.toast = Some(Toast::new("already in Trash"));
        return Ok(());
    }

    let cursor = screen.reading.as_ref().map(|reading| reading.cursor);
    let messages: Vec<_> = view
        .stack()
        .filter(|m| cursor.is_none_or(|cursor| m.body_key == cursor))
        .filter_map(|m| Some((Arc::clone(m.folder.as_ref()?), m.uid)))
        .collect();
    if messages.is_empty() {
        if cursor.is_some() {
            screen.toast = Some(Toast::new("only in another Folder"));
        }

        return Ok(());
    }

    // A Thread never spans Accounts.
    let Some(account) = view.stack().next().map(|m| m.account) else {
        return Ok(());
    };

    let thread = view.selected_key();
    let Some(batch) = caches[account].delete(&messages).map_err(Error::Cache)? else {
        return Ok(());
    };

    screen.undo.push(Undo::Delete(account, batch, thread));
    show(view, caches, None)?;
    screen.hints = None;
    screen.follow(view, thread);

    relist(screen, view, caches)?;
    // Signed out, the move waits in the Cache.
    asks.send(account, Ask::Changes);
    screen.toast = Some(Toast::new("deleted · u to undo"));
    Ok(())
}

/// Undo of the latest Delete not yet taken back, selecting its Thread, or
/// of the latest Draft discarded, showing it again.
fn undo(
    view: &mut InboxView,
    screen: &mut Screen,
    caches: &[Cache],
    asks: &Routes<Ask>,
) -> Result<(), Error> {
    let folders = match screen.undo.pop() {
        Some(Undo::Discard(path, text, notes)) => {
            composing::restore(screen, (path, text, notes));
            return Ok(());
        }
        Some(Undo::Delete(account, batch, thread)) => {
            let folders = caches[account].undo(batch).map_err(Error::Cache)?;
            (!folders.is_empty()).then_some((account, folders, thread))
        }
        None => None,
    };

    let Some((account, folders, thread)) = folders else {
        screen.toast = Some(Toast::new("nothing to undo"));
        return Ok(());
    };

    show(view, caches, None)?;
    screen.hints = None;
    if let Some(key) = thread {
        screen.follow(view, thread);
        view.select(key);
    }

    relist(screen, view, caches)?;
    asks.send(account, Ask::Changes);
    let shown = view
        .folder
        .of(account)
        .is_some_and(|shown| folders.iter().any(|f| **f == *shown));

    let here = view.search.is_some() || shown;
    screen.toast = Some(if here {
        Toast::new("restored")
    } else {
        let folder = caches[account].folder(&folders[0]).map_err(Error::Cache)?;
        Toast::new(format!("restored to {}", folder.label()))
    });
    Ok(())
}

/// Acts on one key in a Draft's preview, or an Outbox Message's.
fn on_settings(key: crossterm::event::KeyEvent, screen: &mut Screen) -> Step {
    let Some(settings) = &mut screen.settings else {
        return Step::Stay;
    };

    match settings.key(key) {
        settings::Step::Stay => Step::Stay,
        settings::Step::Close { reload } => {
            screen.settings = None;
            if reload { Step::Reload } else { Step::Stay }
        }
        settings::Step::Help => {
            screen.help = true;
            Step::Stay
        }
        settings::Step::Icons(choice) => Step::Icons(choice),
        settings::Step::AddAccount => Step::AddAccount,
        settings::Step::Unlink(email) => Step::Unlink(email),
        settings::Step::Quit => Step::Quit,
    }
}

fn on_draft(key: crossterm::event::KeyEvent, screen: &mut Screen) -> Step {
    if screen.draft.as_ref().is_some_and(|preview| preview.queued) {
        match keymap::queued(key) {
            Some(QueuedKey::Close) => screen.draft = None,
            Some(QueuedKey::Help) => screen.help = true,
            Some(QueuedKey::Quit) => return Step::Quit,
            None => {}
        }

        return Step::Stay;
    }

    match keymap::draft(key) {
        Some(DraftKey::Quit) => Step::Quit,
        Some(DraftKey::Help) => {
            screen.help = true;
            Step::Stay
        }
        Some(DraftKey::Send) => Step::Send,
        Some(DraftKey::Edit) => Step::Edit,
        Some(DraftKey::Keep) => {
            composing::keep(screen);
            Step::Stay
        }
        Some(DraftKey::Discard) => {
            composing::discard(screen);
            Step::Stay
        }
        None => Step::Stay,
    }
}

fn on_local(key: crossterm::event::KeyEvent, screen: &mut Screen) -> Step {
    let Some(local) = screen.local.as_mut() else {
        return Step::Stay;
    };

    match keymap::local(key) {
        Some(LocalKey::Down) => local.down(),
        Some(LocalKey::Up) => local.up(),
        Some(LocalKey::Open) => {
            let Some(row) = local.chosen() else {
                return Step::Stay;
            };

            let path = row.path.clone();
            let read = if row.queued {
                DraftPreview::queued(path)
            } else {
                DraftPreview::read(path, None)
            };

            match read {
                Ok(preview) => screen.draft = Some(preview),
                Err(error) => {
                    tracing::error!("reading the Draft failed: {error}");
                    screen.toast = Some(Toast::error("could not read the Draft"));
                }
            }
        }
        Some(LocalKey::Discard) => {
            let draft = local.chosen().filter(|row| !row.queued);
            if let Some(path) = draft.map(|row| row.path.clone()) {
                composing::discard_file(screen, path);
            }
        }
        Some(LocalKey::Undo) => return Step::Undo,
        Some(LocalKey::Close) => screen.local = None,
        Some(LocalKey::Help) => screen.help = true,
        Some(LocalKey::Quit) => return Step::Quit,
        None => {}
    }

    Step::Stay
}

fn open_local(screen: &mut Screen, outbox: &Outbox) {
    match LocalDrafts::open(outbox) {
        Ok(local) => screen.local = Some(local),
        Err(error) => {
            tracing::error!("listing Local Drafts failed: {error}");
            screen.toast = Some(Toast::error("could not list Local Drafts"));
        }
    }
}

fn relist_local(screen: &mut Screen, outbox: &Outbox) {
    if let Some(Err(error)) = screen.local.as_mut().map(|local| local.reload(outbox)) {
        tracing::error!("listing Local Drafts failed: {error}");
    }
}

/// Compose, the editor again on the shown Draft, or its Send.
fn draft(
    step: &Step,
    terminal: &mut Terminal<TerminalBackend>,
    events: &mut Input,
    screen: &mut Screen,
    (accounts, from, outbox, asks): (Accounts, Option<&str>, &Outbox, &Routes<Ask>),
) -> Result<(), Error> {
    match step {
        Step::Compose => composing::compose(terminal, events, screen, from)?,
        Step::Edit => {
            if let Some(preview) = screen.draft.take() {
                let notes = (preview.account, preview.answering);
                composing::edit(terminal, events, screen, (preview.path, notes))?;
            }
        }
        Step::Send => composing::send(screen, (accounts, outbox), asks),
        _ => {}
    }

    Ok(())
}

/// Acts on one key in the Folder Picker, which takes them all.
fn pick(key: crossterm::event::KeyEvent, screen: &mut Screen) -> Step {
    let Some(picker) = screen.picker.as_mut() else {
        return Step::Stay;
    };

    match keymap::pick(key) {
        Some(Pick::Type(c)) => picker.push(c),
        Some(Pick::Erase) => picker.pop(),
        Some(Pick::Down) => picker.down(),
        Some(Pick::Up) => picker.up(),
        Some(Pick::Choose) => {
            let chosen = picker.chosen().cloned();
            if let Some(folder) = chosen {
                screen.picker = None;
                if folder.role == Some(Role::LocalDrafts) {
                    return Step::LocalDrafts;
                }

                return Step::Show(folder);
            }
        }
        Some(Pick::Close) => screen.picker = None,
        Some(Pick::Quit) => return Step::Quit,
        None => {}
    }

    Step::Stay
}

/// Shows `folder` in the Message List, from the Cache, unless it is shown
/// already; true when it was not, for the sync connection to follow.
fn switch(
    view: &mut InboxView,
    screen: &mut Screen,
    caches: &[Cache],
    folder: Folder,
) -> Result<bool, Error> {
    if folder.same(&view.folder) {
        return Ok(false);
    }

    screen.list();
    screen.hints = None;
    view.switch(folder);
    show(view, caches, None)?;
    Ok(true)
}

/// Opens the Folder Picker as the Cache last saw the Folders, starts a
/// Search, shows the Folder picked, or marks, deletes or restores Messages,
/// asking the sync connection to follow.
fn steer(
    step: Step,
    view: &mut InboxView,
    screen: &mut Screen,
    caches: &[Cache],
    asks: &Routes<Ask>,
) -> Result<(), Error> {
    // Signed out, nothing syncs and the ask is dropped.
    match step {
        Step::Folders => {
            let folders = folders(caches)?;
            screen.picker = Some(FolderPicker::new(folders, &view.folder, &view.accounts));
            for account in 0..caches.len() {
                asks.send(account, Ask::ListFolders);
            }
        }
        Step::Show(folder) => {
            if switch(view, screen, caches, folder)? {
                for account in 0..caches.len() {
                    if let Some(shown) = view.folder.of(account) {
                        asks.send(account, Ask::Show(shown.into()));
                    }
                }
            }
        }
        Step::Read(thread) => {
            mark_read(view, screen, caches, asks)?;
            if let Some((account, id)) = thread {
                asks.send(account, Ask::Thread(id));
            }
        }
        Step::MarkUnread => mark_unread(view, screen, caches, asks)?,
        Step::Delete => delete(view, screen, caches, asks)?,
        Step::Undo => undo(view, screen, caches, asks)?,
        Step::Search => start_search(view, screen, caches)?,
        _ => {}
    }

    Ok(())
}

/// Every Account's Folders, each Account's in the Folder Picker's order.
fn folders(caches: &[Cache]) -> Result<Vec<Folder>, Error> {
    let mut folders = Vec::new();
    for cache in caches {
        folders.extend(cache.folders().map_err(Error::Cache)?);
    }

    Ok(folders)
}

/// Lists the Folders anew in an open Folder Picker.
fn relist(screen: &mut Screen, view: &InboxView, caches: &[Cache]) -> Result<(), Error> {
    if let Some(picker) = &mut screen.picker {
        picker.reload(folders(caches)?, &view.folder);
    }

    Ok(())
}

/// Every Account's Messages a Search matches, newest first, the shown
/// Folder's copy winning in its Account, the Inbox's in All Inboxes (ADR 0010).
fn everywhere(caches: &[Cache], shown: &Folder) -> Result<Inbox, cache::Error> {
    let (mut messages, mut elsewhere) = (Vec::new(), Vec::new());
    for (account, cache) in caches.iter().enumerate() {
        let found = cache.everywhere(shown.of(account))?;
        messages.extend(found.messages);
        elsewhere.extend(found.elsewhere);
    }

    messages.sort_by(|a, b| (b.received, b.uid).cmp(&(a.received, a.uid)));
    elsewhere.sort_by_key(|m| m.received);
    Ok(Inbox {
        messages,
        elsewhere,
    })
}

/// Types the Query of a new Search over the whole Cache, or of the one under way.
fn start_search(view: &mut InboxView, screen: &mut Screen, caches: &[Cache]) -> Result<(), Error> {
    let shown = view.folder.clone();
    if view
        .start_search(|| everywhere(caches, &shown))
        .map_err(Error::Cache)?
    {
        screen.mode = Mode::Query;
    }

    Ok(())
}

/// Enters hint mode on the links the Preview window shows, or says there
/// are none. `text` is where the Preview's text goes, if it is shown.
fn label_links(screen: &mut Screen, previews: &Previews, view: &InboxView, text: Option<Rect>) {
    screen.hints = text.and_then(|text| {
        if let Some(reading) = &screen.reading {
            return stack_links(view, previews, reading, text);
        }

        let preview = previews.get(view, text.width)?;
        let lines = preview_pane::body_lines(view.preview_scroll, text, message_header::ROWS);
        LinkHints::find([(0, &preview.body[..], lines)])
    });

    if screen.hints.is_none() {
        screen.toast = Some(Toast::new("no links"));
    }
}

/// The links of every unfolded body of the stack that `text` shows.
fn stack_links(
    view: &InboxView,
    previews: &Previews,
    reading: &Reading,
    text: Rect,
) -> Option<LinkHints> {
    let window = reading.scroll..reading.scroll + usize::from(text.height);
    let bodies = stack::placed(view, previews, reading, text.width).filter_map(|placed| {
        let Some(Body::Rendered(preview)) = placed.body else {
            return None;
        };

        // Links under a pinned header cannot be seen.
        let shown = window.start + placed.pinned(&window)..window.end;
        Some((placed.index, &preview.body[..], placed.body_lines(&shown)))
    });

    LinkHints::find(bodies)
}

/// Opens `url`; returns the Toast to show when that fails.
fn browse(url: &str) -> Option<Toast> {
    let error = browser::open(url).err()?;
    tracing::error!("opening a link failed: {error}");
    Some(Toast::new("could not open the link"))
}

/// Runs `program` in the foreground with the terminal handed back, then
/// takes the terminal again. A failure of `program` alone is only logged.
pub fn outside(
    terminal: &mut Terminal<TerminalBackend>,
    events: &mut Input,
    program: impl FnOnce() -> io::Result<()>,
) -> Result<(), Error> {
    // A live EventStream keeps reading the terminal, and would take the
    // pager's keys; dropping it stops its reader thread.
    events.restart();
    terminal_backend::leave(terminal).map_err(Error::Terminal)?;
    let result = program();
    terminal_backend::resume(terminal).map_err(Error::Terminal)?;
    if let Err(error) = result {
        tracing::error!("leaving the screen failed: {error}");
    }

    Ok(())
}

/// The Account a Compose goes out from: the shown Folder's; in All
/// Inboxes, the selected Thread's, else the first.
fn composing_from(view: &InboxView) -> usize {
    if view.folder.is_all_inboxes() {
        return view.selected_message().map_or(0, |m| m.account);
    }

    view.folder.account
}

fn show(view: &mut InboxView, caches: &[Cache], limit: Option<usize>) -> Result<(), Error> {
    let inbox = if view.folder.is_all_inboxes() {
        let read = all_inboxes::read(caches, view, limit).map_err(Error::Cache)?;
        read.map(|(inbox, edge)| {
            view.edge = edge;
            inbox
        })
    } else {
        let (account, cache) = (view.folder.account, &caches[view.folder.account]);
        let bottomed = view.bottomed(cache, account, &view.folder.name);
        view.edge = (!bottomed.map_err(Error::Cache)?).then_some(account);
        let read = cache.inbox(&view.folder.name, limit);
        read.map_err(Error::Cache)?
    };

    if let Some(inbox) = inbox {
        view.show(inbox);
    }

    if view.search.is_some() {
        view.research(everywhere(caches, &view.folder).map_err(Error::Cache)?);
    }

    Ok(())
}
