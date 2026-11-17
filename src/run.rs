use std::io;

use crossterm::event::{Event, EventStream};
use futures::{Stream, StreamExt};
use ratatui::Terminal;

use crate::{
    alloc_count,
    cache::{self, Cache},
    inbox_view::InboxView,
    keymap::{self, Action},
    message_list::{self, MessageList},
    sync,
    terminal_backend::TerminalBackend,
    theme::Theme,
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

/// The one event loop: input, and `updates` once per Cache write the sync
/// makes, redrawn after each. The first frame reads only the rows it shows
/// from the Cache, before any network; the rest follows right after it.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn run(
    terminal: &mut Terminal<TerminalBackend>,
    theme: &Theme,
    cache: &Cache,
    sync: impl Future<Output = Result<(), sync::Error>>,
    mut updates: impl Stream<Item = ()> + Unpin,
) -> Result<(), Error> {
    let mut events = EventStream::new();
    let mut view = InboxView::loading();
    let height = terminal.size().map_err(Error::Terminal)?.height;
    show(&mut view, cache, Some(message_list::capacity(height)))?;
    let mut sync = std::pin::pin!(sync);
    let mut whole = false;
    let mut syncing = true;
    let mut listening = true;
    // A frame's allocations count from what triggered it, the input or the
    // Cache read, to the end of its draw; waiting on input is left out.
    let mut frame_start = alloc_count::count();
    loop {
        terminal
            .draw(|frame| {
                frame.render_stateful_widget(MessageList { theme }, frame.area(), &mut view);
            })
            .map_err(Error::Terminal)?;
        alloc_count::report(frame_start);
        if !whole {
            whole = true;
            frame_start = alloc_count::count();
            show(&mut view, cache, None)?;
            continue;
        }

        tokio::select! {
            result = &mut sync, if syncing => {
                frame_start = alloc_count::count();
                syncing = false;
                result.map_err(Error::Sync)?;
            }
            update = updates.next(), if listening => {
                frame_start = alloc_count::count();
                match update {
                    Some(()) => show(&mut view, cache, None)?,
                    None => listening = false,
                }
            }
            event = events.next() => {
                frame_start = alloc_count::count();
                match event {
                    None => return Ok(()),
                    Some(Err(source)) => return Err(Error::Terminal(source)),
                    Some(Ok(Event::Key(key))) => match keymap::action(key) {
                        Some(Action::Quit) => return Ok(()),
                        Some(Action::Move(movement)) => view.apply(movement),
                        None => {}
                    },
                    Some(Ok(_)) => {}
                }
            }
        }
    }
}

fn show(view: &mut InboxView, cache: &Cache, limit: Option<usize>) -> Result<(), Error> {
    if let Some(inbox) = cache.inbox(limit).map_err(Error::Cache)? {
        view.show(inbox);
    }

    Ok(())
}
