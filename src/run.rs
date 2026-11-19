use std::{io, sync::Arc};

use crossterm::event::Event;
use futures::{
    FutureExt, Stream, StreamExt,
    channel::mpsc::UnboundedSender,
    future::{Fuse, FusedFuture},
};
use ratatui::{Frame, Terminal, layout::Rect};
use tokio::time::Instant;

use crate::{
    alloc_count,
    awaited_reply::AwaitedReply,
    backfill::Backfill,
    body::Body,
    browser,
    cache::{self, Cache},
    composing,
    draft_pane::DraftPane,
    folder::{Folder, Role},
    folder_picker::FolderPicker,
    help_popup::HelpPopup,
    hint_line::HintLine,
    inbox_view::InboxView,
    input::Input,
    keymap::{self, Action, DraftKey, Pick},
    layout,
    link_hints::{LinkHints, Typed},
    message::Message,
    message_list::{self, MessageList},
    mode::Mode,
    older_pages::OlderPages,
    outbox::Outbox,
    outgoing::Outgoing,
    pager, pager_text,
    picker_popup::PickerPopup,
    preview_pane::{self, PreviewPane},
    preview_render::{self, Job},
    previews::Previews,
    reading::Reading,
    rendered_previews::PreviewKey,
    reply_fetch::ReplyFetch,
    screen::Screen,
    search::Search,
    stack,
    status::Status,
    steering::Ask,
    suspend, sync,
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
    /// `L` asked for the login flow, run with the terminal handed back.
    SignIn,
}

/// The network work behind the screen, what it reports as it goes, the
/// pages of older Messages and the bodies Replies wait on asked of it, what
/// the sync connection is asked, and where a Send leaves from.
pub struct Background<'a, W, U> {
    pub work: Work<W>,
    pub updates: U,
    pub older: OlderPages,
    pub replies: ReplyFetch,
    pub asks: UnboundedSender<Ask>,
    pub outgoing: Outgoing<'a>,
    pub outbox: &'a Outbox,
}

pub enum Work<W> {
    Running(W),
    /// Nothing runs until `L` signs in again; `failed` when the last try did.
    SignedOut {
        failed: bool,
    },
}

impl<W: Future> Work<W> {
    /// Finished already while signed out.
    fn fused(self) -> Fuse<W> {
        match self {
            Self::Running(work) => work.fuse(),
            Self::SignedOut { .. } => Fuse::terminated(),
        }
    }
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
    theme: &Theme,
    cache: &Cache,
    previews: &mut Previews,
    background: Background<
        '_,
        impl Future<Output = Result<(), sync::Error>>,
        impl Stream<Item = Update> + Unpin,
    >,
) -> Result<Exit, Error> {
    let mut events = Input::new();
    let mut view = InboxView::loading();
    let rows = first_rows(terminal)?;
    show(&mut view, cache, Some(rows))?;
    let (mut updates, mut older, asks) = (background.updates, background.older, background.asks);
    let (outgoing, outbox) = (background.outgoing, background.outbox);
    let mut replies = background.replies;
    let account = outgoing.config.email.as_str();
    let from = outgoing.from().map_or_else(
        |_bad| outgoing.config.email.clone(),
        |from| from.to_string(),
    );
    let mut screen = Screen::opening(cache, &background.work);
    screen.outbox = outbox.count();
    (screen.status == Status::SignedOut).then(|| previews.link(false));
    let mut work = std::pin::pin!(background.work.fused());
    let mut whole = false;
    let mut listening = true;
    // A frame's allocations count from what triggered it, the input or the
    // Cache read, to the end of its draw; waiting on input is left out.
    let mut frame_start = alloc_count::count();
    loop {
        let text = panes(terminal, screen.mode)?
            .preview
            .map(preview_pane::text_area);
        want(previews, cache, (&view, &screen), text.filter(|_| whole))?;
        whole.then(|| older.want(&view));
        screen.notice_write_failure(cache);
        draw(terminal, (theme, &from), &mut view, previews, &mut screen)?;
        alloc_count::report(frame_start);
        if !whole {
            whole = true;
            frame_start = alloc_count::count();
            show(&mut view, cache, None)?;
            continue;
        }

        let change = next_change(&view, &mut screen, previews, text);
        tokio::select! {
            () = tokio::time::sleep_until(change.unwrap_or_else(Instant::now)), if change.is_some() => {
                frame_start = alloc_count::count();
                expire(&mut view, &mut screen, cache, &asks)?;
            }
            result = &mut work, if !work.is_terminated() => {
                frame_start = alloc_count::count();
                finished(result)?;
            }
            update = updates.next(), if listening => {
                frame_start = alloc_count::count();
                let Some(update) = update else {
                    listening = false;
                    continue;
                };

                let arrived = replies.notice(&update, screen.replying.as_ref());
                apply(older.notice(update), &mut view, &mut screen, (cache, outbox), previews)?;
                if let Some(result) = arrived {
                    composing::fetched(result, terminal, &mut events, &mut screen, (cache, account))?;
                }
            }
            event = events.next() => {
                frame_start = alloc_count::count();
                match handle(event, &mut view, &mut screen, half_page(text))? {
                    Step::Stay => {}
                    Step::Links => label_links(&mut screen, previews, &view, text),
                    step @ (Step::Folders | Step::Show(_) | Step::Search | Step::Read(_) | Step::MarkUnread | Step::Delete | Step::Undo) => steer(step, &mut view, &mut screen, cache, &asks)?,
                    Step::Browse(url) => screen.toast = browse(&url),
                    Step::Page => screen.toast = open(terminal, &mut events, theme, cache, (&view, screen.reading.as_ref()))?,
                    step @ (Step::Compose | Step::Edit | Step::Send) => draft(&step, terminal, &mut events, &mut screen, (outgoing, outbox, &asks))?,
                    Step::Reply(all) => {
                        if let Some(message) = answered(&view, &screen) {
                            let awaited = AwaitedReply { message, all };
                            composing::reply(terminal, &mut events, &mut screen, (cache, &replies), (awaited, account))?;
                        }
                    }
                    Step::Suspend => outside(terminal, &mut events, suspend::suspend)?,
                    Step::SignIn => return Ok(Exit::SignIn),
                    Step::Quit => return Ok(Exit::Quit),
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
    cache: &Cache,
    asks: &UnboundedSender<Ask>,
) -> Result<(), Error> {
    let now = Instant::now();
    screen.expire(now);
    if screen.dwell.ended(now) {
        mark_read(view, screen, cache, asks)?;
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
    cache: &Cache,
    (view, screen): (&InboxView, &Screen),
    text: Option<Rect>,
) -> Result<(), Error> {
    let Some(text) = text else {
        return Ok(());
    };

    match &screen.reading {
        Some(reading) => {
            let stack = (|key| reading.unfolded(key), reading.cursor);
            previews.want_stack(cache, view, stack, text.width)
        }
        None => previews.want(cache, view, text.width),
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

/// `from` is the `From` a Send adds, for a Draft's preview.
fn draw(
    terminal: &mut Terminal<TerminalBackend>,
    (theme, from): (&Theme, &str),
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
                let preview = previews.shown(view, preview_pane::text_area(pane).width);
                let hints = screen.hints.as_ref();
                let widget = PreviewPane {
                    theme,
                    preview,
                    hints,
                };

                frame.render_stateful_widget(widget, pane, view);
            }

            let keys = screen.keys();
            let hint_line = HintLine {
                theme,
                toast: screen.toast.as_ref(),
                status: screen.status(),
                outbox: screen.outbox,
                mode: keys,
                query: view
                    .search
                    .as_ref()
                    .map(Search::query)
                    .filter(|_| keys != Mode::Draft),
                now: Instant::now(),
            };

            if let Some(cursor) = hint_line.cursor(panes.hint) {
                frame.set_cursor_position(cursor);
            }

            frame.render_widget(hint_line, panes.hint);
            popups(frame, (theme, from), panes.body, screen);
        })
        .map_err(Error::Terminal)?;
    Ok(())
}

/// What shows over the panes in `body`: the Folder Picker, a Draft's
/// preview, then the `?` popup over either.
fn popups(frame: &mut Frame, (theme, from): (&Theme, &str), body: Rect, screen: &Screen) {
    if let Some(picker) = &screen.picker {
        let popup = PickerPopup { theme, picker };
        frame.set_cursor_position(popup.cursor(body));
        frame.render_widget(popup, body);
    }

    if let Some(preview) = &screen.draft {
        let pane = DraftPane {
            theme,
            preview,
            from,
        };

        frame.render_widget(pane, body);
    }

    if screen.help {
        let popup = HelpPopup {
            theme,
            mode: screen.keys(),
            signed_out: screen.status == Status::SignedOut,
        };

        frame.render_widget(popup, body);
    }
}

fn apply(
    update: Update,
    view: &mut InboxView,
    screen: &mut Screen,
    (cache, outbox): (&Cache, &Outbox),
    previews: &mut Previews,
) -> Result<(), Error> {
    match update {
        Update::Inbox => {
            let (thread, selected) = (view.selected_key(), view.selected_body());
            let stack: Vec<i64> = match screen.reading {
                Some(_) => view.stack().map(|m| m.body_key).collect(),
                None => Vec::new(),
            };

            show(view, cache, None)?;
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

            relist(screen, view, cache)?;
        }
        Update::Folders => relist(screen, view, cache)?,
        Update::FolderGone(folder) => {
            if folder == view.folder.name {
                let gone = format!("Folder {} no longer exists", view.folder.label());
                // The sync connection is back on the Inbox already.
                switch(view, screen, cache, Folder::inbox())?;
                screen.toast = Some(Toast::new(gone));
            }

            relist(screen, view, cache)?;
        }
        Update::Bodies(folder, fetched) => previews.fetched(&folder, fetched),
        Update::BodyLink(live) => {
            previews.link(live);
            screen.backfill = None;
        }
        Update::Rendered(key, preview) => previews.rendered(key, preview),
        Update::Status(status) => {
            if status == Status::SignedOut {
                previews.link(false);
            }

            screen.status = status;
        }
        Update::Backfill(Backfill::Fetching { done, total }) => {
            screen.backfill = Some(Status::Syncing { done, total });
        }
        Update::Backfill(Backfill::Paged { .. }) => screen.backfill = None,
        Update::NoTrash => screen.toast = Some(Toast::error("no Trash Folder")),
        Update::Unrestored => screen.toast = Some(Toast::error("nothing to undo")),
        Update::Sent => {
            screen.outbox = outbox.count();
            screen.toast = Some(Toast::new("Sent"));
        }
        Update::Refused => {
            screen.outbox = outbox.count();
            screen.toast = Some(Toast::error("refused · back in Drafts"));
        }
    }

    Ok(())
}

/// What the event loop does after one input event.
enum Step {
    Stay,
    Links,
    Folders,
    Show(Folder),
    Search,
    /// Reading opened, on the Gmail Thread with this ID if it has one: its
    /// Messages in other Folders are asked for.
    Read(Option<u64>),
    MarkUnread,
    Delete,
    Undo,
    Browse(Arc<str>),
    Page,
    Compose,
    /// Reply, or Reply All when true.
    Reply(bool),
    /// The shown Draft in the editor again.
    Edit,
    Send,
    Suspend,
    SignIn,
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
        Some(Ok(Event::Key(key))) if screen.draft.is_some() => Ok(on_draft(key, screen)),
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
        Some(Action::Page) => Step::Page,
        Some(Action::Suspend) => Step::Suspend,
        Some(Action::SignIn) if screen.status == Status::SignedOut => Step::SignIn,
        Some(Action::Links) => Step::Links,
        Some(Action::Folders) => Step::Folders,
        Some(Action::SwitchView) => {
            view.html = view.html.other();
            view.preview_scroll = 0;
            if let Some(reading) = &mut screen.reading {
                reading.reveal = true;
            }

            // The labels sit where the other View laid the links out.
            screen.hints = None;
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
        Some(Action::SignIn) | None => Step::Stay,
    }
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
                Some(Key::Gmail(id)) if !apart => Some(id),
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
                .rfind(|m| !m.from_me)
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
        && screen.draft.is_none();
    let text = text.filter(|_| listed && !screen.help && view.selected_unread())?;
    previews.get(view, text.width)?;
    Some((view.selected_key()?, view.selected_body()?))
}

/// Mark Read on the selected Thread's Messages in their Folders.
fn mark_read(
    view: &mut InboxView,
    screen: &mut Screen,
    cache: &Cache,
    asks: &UnboundedSender<Ask>,
) -> Result<(), Error> {
    let unread: Vec<_> = view
        .stack()
        .filter(|m| m.unread)
        .filter_map(|m| Some((Arc::clone(m.folder.as_ref()?), m.uid)))
        .collect();
    mark((view, screen), cache, asks, (&unread, false)).map(drop)
}

/// Mark Unread on the selected Thread's newest Message, or in Reading on
/// the one at the cursor.
fn mark_unread(
    view: &mut InboxView,
    screen: &mut Screen,
    cache: &Cache,
    asks: &UnboundedSender<Ask>,
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

    let marked = [(Arc::clone(folder), message.uid)];
    if let Some(key) = view.selected_key() {
        screen.dwell.spare(key);
    }

    if mark((view, screen), cache, asks, (&marked, true))? {
        screen.toast = Some(Toast::new("marked Unread"));
    }

    Ok(())
}

/// Sets Unread on `messages` in the Cache and the screen, and asks the
/// sync connection to tell the server; false when none changed.
fn mark(
    (view, screen): (&mut InboxView, &mut Screen),
    cache: &Cache,
    asks: &UnboundedSender<Ask>,
    (messages, unread): (&[(Arc<str>, u32)], bool),
) -> Result<bool, Error> {
    if !cache.mark(messages, unread).map_err(Error::Cache)? {
        return Ok(false);
    }

    show(view, cache, None)?;
    relist(screen, view, cache)?;
    // The send fails only while signed out; the change waits in the Cache.
    asks.unbounded_send(Ask::Changes).ok();
    Ok(true)
}

/// Delete on the selected Thread's Messages in their Folders, or in
/// Reading on the one at the cursor.
fn delete(
    view: &mut InboxView,
    screen: &mut Screen,
    cache: &Cache,
    asks: &UnboundedSender<Ask>,
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

    let thread = view.selected_key();
    let Some(batch) = cache.delete(&messages).map_err(Error::Cache)? else {
        return Ok(());
    };

    screen.undo.push(Undo::Delete(batch, thread));
    show(view, cache, None)?;
    screen.hints = None;
    screen.follow(view, thread);

    relist(screen, view, cache)?;
    // The send fails only while signed out; the move waits in the Cache.
    asks.unbounded_send(Ask::Changes).ok();
    screen.toast = Some(Toast::new("deleted · u to undo"));
    Ok(())
}

/// Undo of the latest Delete not yet taken back, selecting its Thread, or
/// of the latest Draft discarded, showing it again.
fn undo(
    view: &mut InboxView,
    screen: &mut Screen,
    cache: &Cache,
    asks: &UnboundedSender<Ask>,
) -> Result<(), Error> {
    let folders = match screen.undo.pop() {
        Some(Undo::Discard(path, text, answering)) => {
            composing::restore(screen, (path, text, answering));
            return Ok(());
        }
        Some(Undo::Delete(batch, thread)) => {
            let folders = cache.undo(batch).map_err(Error::Cache)?;
            (!folders.is_empty()).then_some((folders, thread))
        }
        None => None,
    };

    let Some((folders, thread)) = folders else {
        screen.toast = Some(Toast::new("nothing to undo"));
        return Ok(());
    };

    show(view, cache, None)?;
    screen.hints = None;
    if let Some(key) = thread {
        screen.follow(view, thread);
        view.select(key);
    }

    relist(screen, view, cache)?;
    asks.unbounded_send(Ask::Changes).ok();
    let here = view.search.is_some() || folders.contains(&view.folder.name);
    screen.toast = Some(if here {
        Toast::new("restored")
    } else {
        let folder = cache.folder(&folders[0]).map_err(Error::Cache)?;
        Toast::new(format!("restored to {}", folder.label()))
    });
    Ok(())
}

/// Acts on one key in a Draft's preview.
fn on_draft(key: crossterm::event::KeyEvent, screen: &mut Screen) -> Step {
    match keymap::draft(key) {
        Some(DraftKey::Quit) => Step::Quit,
        Some(DraftKey::Help) => {
            screen.help = true;
            Step::Stay
        }
        Some(DraftKey::Send) => Step::Send,
        Some(DraftKey::Edit) => Step::Edit,
        Some(DraftKey::Discard) => {
            composing::discard(screen);
            Step::Stay
        }
        None => Step::Stay,
    }
}

/// Compose, the editor again on the shown Draft, or its Send.
fn draft(
    step: &Step,
    terminal: &mut Terminal<TerminalBackend>,
    events: &mut Input,
    screen: &mut Screen,
    (outgoing, outbox, asks): (Outgoing, &Outbox, &UnboundedSender<Ask>),
) -> Result<(), Error> {
    match step {
        Step::Compose => composing::compose(terminal, events, screen)?,
        Step::Edit => {
            if let Some(preview) = screen.draft.take() {
                composing::edit(terminal, events, screen, (preview.path, preview.answering))?;
            }
        }
        Step::Send => composing::send(screen, (outgoing, outbox), asks),
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
    cache: &Cache,
    folder: Folder,
) -> Result<bool, Error> {
    if folder.name == view.folder.name {
        return Ok(false);
    }

    screen.list();
    screen.hints = None;
    view.switch(folder);
    show(view, cache, None)?;
    Ok(true)
}

/// Opens the Folder Picker as the Cache last saw the Folders, starts a
/// Search, shows the Folder picked, or marks, deletes or restores Messages,
/// asking the sync connection to follow.
fn steer(
    step: Step,
    view: &mut InboxView,
    screen: &mut Screen,
    cache: &Cache,
    asks: &UnboundedSender<Ask>,
) -> Result<(), Error> {
    let ask = match step {
        Step::Folders => {
            let folders = cache.folders().map_err(Error::Cache)?;
            screen.picker = Some(FolderPicker::new(folders, &view.folder));
            Ask::ListFolders
        }
        Step::Show(folder) => {
            if !switch(view, screen, cache, folder)? {
                return Ok(());
            }

            Ask::Show(Arc::clone(&view.folder.name))
        }
        Step::Read(thread) => {
            mark_read(view, screen, cache, asks)?;
            let Some(id) = thread else {
                return Ok(());
            };

            Ask::Thread(id)
        }
        Step::MarkUnread => return mark_unread(view, screen, cache, asks),
        Step::Delete => return delete(view, screen, cache, asks),
        Step::Undo => return undo(view, screen, cache, asks),
        Step::Search => return start_search(view, screen, cache),
        _ => return Ok(()),
    };

    // The send fails only while signed out, when nothing syncs.
    asks.unbounded_send(ask).ok();
    Ok(())
}

/// Lists the Folders anew in an open Folder Picker.
fn relist(screen: &mut Screen, view: &InboxView, cache: &Cache) -> Result<(), Error> {
    if let Some(picker) = &mut screen.picker {
        picker.reload(cache.folders().map_err(Error::Cache)?, &view.folder);
    }

    Ok(())
}

/// Types the Query of a new Search over the whole Cache, or of the one under way.
fn start_search(view: &mut InboxView, screen: &mut Screen, cache: &Cache) -> Result<(), Error> {
    let shown = Arc::clone(&view.folder.name);
    if view
        .start_search(|| cache.everywhere(&shown))
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
        let lines = preview_pane::body_lines(view.preview_scroll, text);
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

        Some((placed.index, &preview.body[..], placed.body_lines(&window)))
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

/// Shows the selected Message in the pager. Returns the Toast to show
/// instead while its body is not in the Cache; `None` drops any shown.
fn open(
    terminal: &mut Terminal<TerminalBackend>,
    events: &mut Input,
    theme: &Theme,
    cache: &Cache,
    (view, reading): (&InboxView, Option<&Reading>),
) -> Result<Option<Toast>, Error> {
    let message = reading.map_or_else(
        || view.selected_message(),
        |reading| view.stack().find(|m| m.body_key == reading.cursor),
    );

    let Some(message) = message else {
        return Ok(None);
    };

    let Some(body) = cache.body(message.body_key).map_err(Error::Cache)? else {
        return Ok(Some(Toast::new("not downloaded")));
    };

    let width = terminal.size().map_err(Error::Terminal)?.width;
    let preview = preview_render::paged(Job {
        key: PreviewKey {
            body: message.body_key,
            width,
            view: view.html,
        },
        body,
        sender: message.sender.clone(),
        subject: message.subject.clone(),
        received: message.received,
        colours: theme.mail,
    });

    let text = pager_text::ansi(&preview, theme);
    outside(terminal, events, || pager::page(&text))?;
    Ok(None)
}

fn show(view: &mut InboxView, cache: &Cache, limit: Option<usize>) -> Result<(), Error> {
    if let Some(inbox) = cache
        .inbox(&view.folder.name, limit)
        .map_err(Error::Cache)?
    {
        view.show(inbox);
    }

    if view.search.is_some() {
        view.research(cache.everywhere(&view.folder.name).map_err(Error::Cache)?);
    }

    Ok(())
}
