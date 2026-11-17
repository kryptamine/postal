use std::io;

use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use ratatui::DefaultTerminal;

use crate::{
    imap,
    inbox::Inbox,
    inbox_view::InboxView,
    keymap::{self, Action},
    message_list::MessageList,
    theme::Theme,
};

#[derive(Debug)]
pub enum Error {
    Terminal(io::Error),
    Imap(imap::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Terminal(source) => write!(f, "terminal: {source}"),
            Self::Imap(source) => source.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

/// The one event loop: terminal input and the Inbox fetch, redrawn after each.
pub async fn run(
    terminal: &mut DefaultTerminal,
    theme: &Theme,
    inbox: impl Future<Output = Result<Inbox, imap::Error>>,
) -> Result<(), Error> {
    let mut events = EventStream::new();
    let mut view = InboxView::loading();
    let mut inbox = std::pin::pin!(inbox);
    let mut fetching = true;
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
        tokio::select! {
            result = &mut inbox, if fetching => {
                fetching = false;
                view.show(result.map_err(Error::Imap)?);
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
