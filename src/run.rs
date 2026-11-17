use std::io;

use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use ratatui::Terminal;

use crate::{
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

/// The one event loop: terminal input and the resync, redrawn after each.
/// The first frame draws only the rows it shows, read from the Cache before
/// any network; the rest of the Cache follows right after it.
#[expect(
    clippy::future_not_send,
    reason = "Postal runs on a current-thread runtime"
)]
pub async fn run(
    terminal: &mut Terminal<TerminalBackend>,
    theme: &Theme,
    cache: &Cache,
    resync: impl Future<Output = Result<(), sync::Error>>,
) -> Result<(), Error> {
    let mut events = EventStream::new();
    let mut view = InboxView::loading();
    let height = terminal.size().map_err(Error::Terminal)?.height;
    show(&mut view, cache, Some(message_list::capacity(height)))?;
    let mut resync = std::pin::pin!(resync);
    let mut whole = false;
    let mut syncing = true;
    loop {
        terminal
            .draw(|frame| {
                let list = MessageList {
                    theme,
                    now: chrono::Local::now(),
                };

                frame.render_stateful_widget(list, frame.area(), &mut view);
            })
            .map_err(Error::Terminal)?;
        if !whole {
            whole = true;
            show(&mut view, cache, None)?;
            continue;
        }

        tokio::select! {
            result = &mut resync, if syncing => {
                syncing = false;
                result.map_err(Error::Sync)?;
                show(&mut view, cache, None)?;
            }
            event = events.next() => match event {
                None => return Ok(()),
                Some(Err(source)) => return Err(Error::Terminal(source)),
                Some(Ok(Event::Key(key))) => match keymap::action(key) {
                    Some(Action::Quit) => return Ok(()),
                    Some(Action::Move(movement)) => view.apply(movement),
                    None => {}
                },
                Some(Ok(_)) => {}
            },
        }
    }
}

fn show(view: &mut InboxView, cache: &Cache, limit: Option<usize>) -> Result<(), Error> {
    if let Some(inbox) = cache.inbox(limit).map_err(Error::Cache)? {
        view.show(inbox);
    }

    Ok(())
}
