use std::{io, sync::Arc};

use crossterm::event::{Event, EventStream};
use futures::{Stream, StreamExt};
use ratatui::{Terminal, layout::Rect};
use tokio::time::Instant;

use crate::{
    alloc_count, browser,
    cache::{self, Cache},
    help_popup::HelpPopup,
    hint_line::HintLine,
    inbox_view::InboxView,
    keymap::{self, Action},
    layout,
    link_hints::{LinkHints, Typed},
    message_list::{self, MessageList},
    pager, pager_text,
    preview_pane::{self, PreviewPane},
    preview_render::{self, Job},
    previews::Previews,
    rendered_previews::PreviewKey,
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

/// The network work behind the screen, and what it reports as it goes.
pub struct Background<W, U> {
    pub work: W,
    pub updates: U,
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
) -> Result<(), Error> {
    let mut events = EventStream::new();
    let mut view = InboxView::loading();
    let rows = message_list::capacity(panes(terminal)?.list.height);
    show(&mut view, cache, Some(rows))?;
    let Background { work, mut updates } = background;
    let mut work = std::pin::pin!(work);
    let mut whole = false;
    let mut working = true;
    let mut listening = true;
    let mut screen = Screen::opening(cache);
    // A frame's allocations count from what triggered it, the input or the
    // Cache read, to the end of its draw; waiting on input is left out.
    let mut frame_start = alloc_count::count();
    loop {
        let text = panes(terminal)?.preview.map(preview_pane::text_area);
        want(previews, cache, &view, text.filter(|_| whole))?;
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
            result = &mut work, if working => {
                frame_start = alloc_count::count();
                working = false;
                result.map_err(Error::Sync)?;
            }
            update = updates.next(), if listening => {
                frame_start = alloc_count::count();
                match update {
                    Some(update) => apply(update, &mut view, &mut screen, cache, previews)?,
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
                    Step::Quit => return Ok(()),
                }
            }
        }
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

fn panes(terminal: &Terminal<TerminalBackend>) -> Result<layout::Panes, Error> {
    let size = terminal.size().map_err(Error::Terminal)?;
    Ok(layout::split(Rect::new(0, 0, size.width, size.height)))
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
            let panes = layout::split(frame.area());
            frame.render_stateful_widget(MessageList { theme }, panes.list, view);
            if let Some(pane) = panes.preview {
                let preview = previews.get(view, preview_pane::text_area(pane).width);
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
                status: screen.status,
                now: Instant::now(),
            };

            frame.render_widget(hint_line, panes.hint);
            if screen.help {
                frame.render_widget(HelpPopup { theme }, panes.body);
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
            // The labels point into the Preview they were found in.
            if view.selected_uid() != selected {
                screen.hints = None;
            }
        }
        Update::Bodies(uids) => previews.stored(&uids),
        Update::Rendered(key, preview) => previews.rendered(key, preview),
        Update::Status(status) => screen.status = status,
    }

    Ok(())
}

/// What the main screen shows over and beside the panes.
#[derive(Default)]
struct Screen {
    help: bool,
    toast: Option<Toast>,
    status: Status,
    hints: Option<LinkHints>,
}

impl Screen {
    fn opening(cache: &Cache) -> Self {
        Self {
            toast: cache.rebuilt().then(|| Toast::new("cache rebuilt")),
            ..Self::default()
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
        [toast, self.status.ticks_at(now)]
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
        Some(Ok(Event::Key(key))) => Ok(match keymap::action(key) {
            Some(Action::Quit) => Step::Quit,
            Some(Action::Open) => Step::Page,
            Some(Action::Suspend) => Step::Suspend,
            Some(Action::Links) => Step::Links,
            Some(Action::Help) => {
                screen.help = true;
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
            None => Step::Stay,
        }),
        // The labels sit where the old width laid the links out.
        Some(Ok(Event::Resize(..))) => {
            screen.hints = None;
            Ok(Step::Stay)
        }
        Some(Ok(_)) => Ok(Step::Stay),
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
    let Some(message) = view.messages.as_ref().and_then(|m| m.get(view.selected)) else {
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
        },
        body,
        sender: message.sender.clone(),
        subject: message.subject.clone(),
        received: message.received,
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
