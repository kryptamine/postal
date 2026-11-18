use std::{io, sync::Arc};

use crossterm::event::{Event, EventStream};
use futures::{
    FutureExt, Stream, StreamExt,
    future::{Fuse, FusedFuture},
};
use ratatui::{Terminal, layout::Rect};
use tokio::time::Instant;

use crate::{
    alloc_count,
    backfill::Backfill,
    browser,
    cache::{self, Cache},
    help_popup::HelpPopup,
    hint_line::HintLine,
    inbox_view::InboxView,
    keymap::{self, Action},
    layout,
    link_hints::{LinkHints, Typed},
    message_list::{self, MessageList},
    mode::Mode,
    older_pages::OlderPages,
    pager, pager_text,
    preview_pane::{self, PreviewPane},
    preview_render::{self, Job},
    previews::Previews,
    rendered_previews::PreviewKey,
    search::Search,
    status::Status,
    suspend, sync,
    terminal_backend::{self, TerminalBackend},
    theme::Theme,
    toast::Toast,
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

/// The network work behind the screen, what it reports as it goes, and the
/// pages of older Messages asked of it.
pub struct Background<W, U> {
    pub work: Work<W>,
    pub updates: U,
    pub older: OlderPages,
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
pub async fn run(
    terminal: &mut Terminal<TerminalBackend>,
    theme: &Theme,
    cache: &Cache,
    previews: &mut Previews,
    background: Background<
        impl Future<Output = Result<(), sync::Error>>,
        impl Stream<Item = Update> + Unpin,
    >,
) -> Result<Exit, Error> {
    let mut events = EventStream::new();
    let mut view = InboxView::loading();
    let rows = first_rows(terminal)?;
    show(&mut view, cache, Some(rows))?;
    let (mut updates, mut older) = (background.updates, background.older);
    let mut screen = Screen::opening(cache, &background.work);
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
        want(previews, cache, &view, text.filter(|_| whole))?;
        whole.then(|| older.want(&view));
        screen.notice_write_failure(cache);
        draw(terminal, theme, &mut view, previews, &screen)?;
        alloc_count::report(frame_start);
        if !whole {
            whole = true;
            frame_start = alloc_count::count();
            show(&mut view, cache, None)?;
            continue;
        }

        let change = screen.changes_at(Instant::now());
        tokio::select! {
            () = tokio::time::sleep_until(change.unwrap_or_else(Instant::now)), if change.is_some() => {
                frame_start = alloc_count::count();
                screen.expire(Instant::now());
            }
            result = &mut work, if !work.is_terminated() => {
                frame_start = alloc_count::count();
                finished(result)?;
            }
            update = updates.next(), if listening => {
                frame_start = alloc_count::count();
                match update {
                    Some(update) => apply(older.notice(update), &mut view, &mut screen, cache, previews)?,
                    None => listening = false,
                }
            }
            event = events.next() => {
                frame_start = alloc_count::count();
                let half_page = text.map_or(1, |t| usize::from(t.height / 2).max(1));
                match handle(event, &mut view, &mut screen, half_page)? {
                    Step::Stay => {}
                    Step::Links => label_links(&mut screen, previews, &view, text),
                    Step::Browse(url) => screen.toast = browse(&url),
                    Step::Page => screen.toast = open(terminal, &mut events, theme, cache, &view)?,
                    Step::Suspend => outside(terminal, &mut events, suspend::suspend)?,
                    Step::SignIn => return Ok(Exit::SignIn),
                    Step::Quit => return Ok(Exit::Quit),
                }
            }
        }
    }
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

/// Readies the Previews around the selection, once `text` is laid out.
fn want(
    previews: &mut Previews,
    cache: &Cache,
    view: &InboxView,
    text: Option<Rect>,
) -> Result<(), Error> {
    text.map_or(Ok(()), |text| {
        previews.want(cache, view, text.width).map_err(Error::Cache)
    })
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
    theme: &Theme,
    view: &mut InboxView,
    previews: &Previews,
    screen: &Screen,
) -> Result<(), Error> {
    terminal
        .draw(|frame| {
            let panes = layout::split(frame.area(), screen.mode);
            if let Some(list) = panes.list {
                frame.render_stateful_widget(MessageList { theme }, list, view);
            }

            if let Some(pane) = panes.preview {
                let preview = previews.shown(view, preview_pane::text_area(pane).width);
                let hints = screen.hints.as_ref();
                let widget = PreviewPane {
                    theme,
                    preview,
                    hints,
                };

                frame.render_stateful_widget(widget, pane, view);
            }

            let hint_line = HintLine {
                theme,
                toast: screen.toast.as_ref(),
                status: screen.status(),
                mode: screen.mode,
                query: view.search.as_ref().map(Search::query),
                now: Instant::now(),
            };

            if let Some(cursor) = hint_line.cursor(panes.hint) {
                frame.set_cursor_position(cursor);
            }

            frame.render_widget(hint_line, panes.hint);
            if screen.help {
                let signed_out = screen.status == Status::SignedOut;
                let mode = screen.mode;
                let popup = HelpPopup {
                    theme,
                    mode,
                    signed_out,
                };

                frame.render_widget(popup, panes.body);
            }
        })
        .map_err(Error::Terminal)?;
    Ok(())
}

fn apply(
    update: Update,
    view: &mut InboxView,
    screen: &mut Screen,
    cache: &Cache,
    previews: &mut Previews,
) -> Result<(), Error> {
    match update {
        Update::Inbox => {
            let selected = view.selected_uid();
            show(view, cache, None)?;
            // The labels, and the full-screen Message, belong to the Message
            // they were opened on; a Query being typed stays.
            if view.selected_uid() != selected {
                screen.hints = None;
                if screen.mode == Mode::Reading {
                    screen.mode = Mode::List;
                }
            }
        }
        Update::Bodies(fetched) => previews.fetched(fetched),
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
    }

    Ok(())
}

/// What the main screen shows over and beside the panes.
#[derive(Default)]
struct Screen {
    help: bool,
    toast: Option<Toast>,
    /// The sync connection's.
    status: Status,
    /// A page of older Messages under way on the fetch connection.
    backfill: Option<Status>,
    hints: Option<LinkHints>,
    mode: Mode,
}

impl Screen {
    fn opening<W>(cache: &Cache, work: &Work<W>) -> Self {
        let (status, toast) = match work {
            Work::Running(_) => (Status::Live, None),
            Work::SignedOut { failed } => (
                Status::SignedOut,
                failed.then(|| Toast::error("sign-in failed")),
            ),
        };

        Self {
            status,
            toast: toast.or_else(|| cache.rebuilt().then(|| Toast::new("cache rebuilt"))),
            ..Self::default()
        }
    }

    /// The sync connection's Status, or the backfill's while that one is live.
    const fn status(&self) -> Status {
        match (self.status, self.backfill) {
            (Status::Live, Some(backfill)) => backfill,
            (status, _) => status,
        }
    }

    /// A failure is toasted once: the Cache stays in memory after it.
    fn notice_write_failure(&mut self, cache: &Cache) {
        if cache.take_write_failure() {
            self.toast = Some(Toast::error("cache write failed"));
        }
    }

    /// When the Toast goes or the Status's countdown ticks, whichever is
    /// first; `None` while neither will.
    fn changes_at(&self, now: Instant) -> Option<Instant> {
        let toast = self.toast.as_ref().map(|toast| toast.until);
        [toast, self.status().ticks_at(now)]
            .into_iter()
            .flatten()
            .min()
    }

    fn expire(&mut self, now: Instant) {
        if self.toast.as_ref().is_some_and(|toast| toast.until <= now) {
            self.toast = None;
        }
    }
}

/// What the event loop does after one input event.
enum Step {
    Stay,
    Links,
    Browse(Arc<str>),
    Page,
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
        Some(Action::Read) => {
            if view.selected_uid().is_some() {
                screen.mode = Mode::Reading;
            }

            Step::Stay
        }
        Some(Action::Back | Action::KeepQuery) => {
            screen.mode = Mode::List;
            Step::Stay
        }
        Some(Action::Page) => Step::Page,
        Some(Action::Suspend) => Step::Suspend,
        Some(Action::SignIn) if screen.status == Status::SignedOut => Step::SignIn,
        Some(Action::Links) => Step::Links,
        Some(Action::SwitchView) => {
            view.html = view.html.other();
            view.preview_scroll = 0;
            // The labels sit where the other View laid the links out.
            screen.hints = None;
            Step::Stay
        }
        Some(Action::Help) => {
            screen.help = true;
            Step::Stay
        }
        Some(
            action @ (Action::Search | Action::Type(_) | Action::Erase | Action::EndSearch { .. }),
        ) => {
            search(action, view, screen);
            Step::Stay
        }
        Some(Action::Move(movement)) => {
            view.apply(movement);
            Step::Stay
        }
        Some(Action::Scroll(scroll)) => {
            view.scroll(scroll, half_page);
            Step::Stay
        }
        Some(Action::SignIn) | None => Step::Stay,
    }
}

fn search(action: Action, view: &mut InboxView, screen: &mut Screen) {
    match action {
        Action::Search if view.start_search() => screen.mode = Mode::Query,
        Action::Type(c) => view.edit_query(|search| search.push(c)),
        Action::Erase => view.edit_query(Search::pop),
        Action::EndSearch { restore } => {
            view.end_search(restore);
            screen.mode = Mode::List;
        }
        _ => {}
    }
}

/// Enters hint mode on the links the Preview window shows, or says there
/// are none. `text` is where the Preview's text goes, if it is shown.
fn label_links(screen: &mut Screen, previews: &Previews, view: &InboxView, text: Option<Rect>) {
    screen.hints = text.and_then(|text| {
        let preview = previews.get(view, text.width)?;
        let lines = preview_pane::body_lines(view.preview_scroll, text);
        LinkHints::find(&preview.body, lines)
    });

    if screen.hints.is_none() {
        screen.toast = Some(Toast::new("no links"));
    }
}

/// Opens `url`; returns the Toast to show when that fails.
fn browse(url: &str) -> Option<Toast> {
    let error = browser::open(url).err()?;
    tracing::error!("opening a link failed: {error}");
    Some(Toast::new("could not open the link"))
}

/// Runs `program` in the foreground with the terminal handed back, then
/// takes the terminal again. A failure of `program` alone is only logged.
fn outside(
    terminal: &mut Terminal<TerminalBackend>,
    events: &mut EventStream,
    program: impl FnOnce() -> io::Result<()>,
) -> Result<(), Error> {
    // A live EventStream keeps reading the terminal, and would take the
    // pager's keys; dropping it stops its reader thread.
    *events = EventStream::new();
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
    events: &mut EventStream,
    theme: &Theme,
    cache: &Cache,
    view: &InboxView,
) -> Result<Option<Toast>, Error> {
    let Some(message) = view.selected_message() else {
        return Ok(None);
    };

    let Some(body) = cache.body(message.uid).map_err(Error::Cache)? else {
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
    if let Some(inbox) = cache.inbox(limit).map_err(Error::Cache)? {
        view.show(inbox);
    }

    Ok(())
}
