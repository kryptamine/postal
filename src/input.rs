use std::{io, time::Duration};

use crossterm::event::{Event, EventStream};
use futures::StreamExt;

/// The terminal's events, read on crossterm's thread while a `next` waits.
pub struct Input {
    /// `None` only inside `restart`.
    stream: Option<EventStream>,
}

impl Input {
    pub fn new() -> Self {
        Self {
            stream: Some(fresh()),
        }
    }

    /// `None` once the terminal has no more.
    pub async fn next(&mut self) -> Option<io::Result<Event>> {
        match &mut self.stream {
            Some(stream) => stream.next().await,
            None => None,
        }
    }

    /// Stops reading, so a program in the foreground gets the keys, then
    /// reads anew. The old stream goes first: while it waits on the
    /// terminal, it holds the lock a new one takes.
    pub fn restart(&mut self) {
        self.stream = None;
        self.stream = Some(fresh());
    }
}

/// A stream dropped idle leaves a wake behind; met with a key later, it
/// hides that key from every read. Taken now, before any key, it is harmless.
fn fresh() -> EventStream {
    let stream = EventStream::new();
    crossterm::event::poll(Duration::ZERO).ok();
    stream
}
