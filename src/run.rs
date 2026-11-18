use std::io;

use crossterm::event::{Event, EventStream};
use futures::{Stream, StreamExt};
use ratatui::{Terminal, layout::Rect};

use crate::{
    alloc_count,
    cache::{self, Cache},
    inbox_view::InboxView,
    keymap::{self, Action},
    layout,
    message_list::{self, MessageList},
    pager, pager_text,
    preview_pane::{self, PreviewPane},
    preview_render::{self, Job},
    previews::Previews,
    rendered_previews::PreviewKey,
    suspend, sync,
    terminal_backend::{self, TerminalBackend},
    theme::Theme,
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
    let height = terminal.size().map_err(Error::Terminal)?.height;
    show(&mut view, cache, Some(message_list::capacity(height)))?;
    let Background { work, mut updates } = background;
    let mut work = std::pin::pin!(work);
    let mut whole = false;
    let mut working = true;
    let mut listening = true;
    // A frame's allocations count from what triggered it, the input or the
    // Cache read, to the end of its draw; waiting on input is left out.
    let mut frame_start = alloc_count::count();
    loop {
        let size = terminal.size().map_err(Error::Terminal)?;
        let text = layout::split(Rect::new(0, 0, size.width, size.height))
            .preview
            .map(preview_pane::text_area);
        if whole && let Some(text) = text {
            previews
                .want(cache, &view, text.width)
                .map_err(Error::Cache)?;
        }

        draw(terminal, theme, &mut view, previews)?;
        alloc_count::report(frame_start);
        if !whole {
            whole = true;
            frame_start = alloc_count::count();
            show(&mut view, cache, None)?;
            continue;
        }

        tokio::select! {
            result = &mut work, if working => {
                frame_start = alloc_count::count();
                working = false;
                result.map_err(Error::Sync)?;
            }
            update = updates.next(), if listening => {
                frame_start = alloc_count::count();
                match update {
                    Some(update) => apply(update, &mut view, cache, previews)?,
                    None => listening = false,
                }
            }
            event = events.next() => {
                frame_start = alloc_count::count();
                let half_page = text.map_or(1, |t| usize::from(t.height / 2).max(1));
                match handle(event, &mut view, half_page)? {
                    Step::Stay => {}
                    Step::Page => {
                        if let Some(text) = paged(terminal, theme, cache, &view)? {
                            outside(terminal, &mut events, || pager::page(&text))?;
                        }
                    }
                    Step::Suspend => outside(terminal, &mut events, suspend::suspend)?,
                    Step::Quit => return Ok(()),
                }
            }
        }
    }
}

fn draw(
    terminal: &mut Terminal<TerminalBackend>,
    theme: &Theme,
    view: &mut InboxView,
    previews: &Previews,
) -> Result<(), Error> {
    terminal
        .draw(|frame| {
            let panes = layout::split(frame.area());
            frame.render_stateful_widget(MessageList { theme }, panes.list, view);
            if let Some(pane) = panes.preview {
                let preview = previews.get(view, preview_pane::text_area(pane).width);
                frame.render_stateful_widget(PreviewPane { theme, preview }, pane, view);
            }
        })
        .map_err(Error::Terminal)?;
    Ok(())
}

fn apply(
    update: Update,
    view: &mut InboxView,
    cache: &Cache,
    previews: &mut Previews,
) -> Result<(), Error> {
    match update {
        Update::Inbox => show(view, cache, None)?,
        Update::Bodies(uids) => previews.stored(&uids),
        Update::Rendered(key, preview) => previews.rendered(key, preview),
    }

    Ok(())
}

/// What the event loop does after one input event.
enum Step {
    Stay,
    Page,
    Suspend,
    Quit,
}

/// Acts on one input event, where the view alone can take it.
fn handle(
    event: Option<io::Result<Event>>,
    view: &mut InboxView,
    half_page: usize,
) -> Result<Step, Error> {
    match event {
        None => Ok(Step::Quit),
        Some(Err(source)) => Err(Error::Terminal(source)),
        Some(Ok(Event::Key(key))) => Ok(match keymap::action(key) {
            Some(Action::Quit) => Step::Quit,
            Some(Action::Open) => Step::Page,
            Some(Action::Suspend) => Step::Suspend,
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
        Some(Ok(_)) => Ok(Step::Stay),
    }
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

/// The selected Message as the pager shows it, at the terminal's width;
/// `None` while its body is not in the Cache.
fn paged(
    terminal: &Terminal<TerminalBackend>,
    theme: &Theme,
    cache: &Cache,
    view: &InboxView,
) -> Result<Option<Vec<u8>>, Error> {
    let Some(message) = view.messages.as_ref().and_then(|m| m.get(view.selected)) else {
        return Ok(None);
    };

    let Some(body) = cache.body(message.uid).map_err(Error::Cache)? else {
        return Ok(None);
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

    Ok(Some(pager_text::ansi(&preview, theme)))
}

fn show(view: &mut InboxView, cache: &Cache, limit: Option<usize>) -> Result<(), Error> {
    if let Some(inbox) = cache.inbox(limit).map_err(Error::Cache)? {
        view.show(inbox);
    }

    Ok(())
}
