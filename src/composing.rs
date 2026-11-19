use std::{io, path::PathBuf};

use crossterm::event::EventStream;
use futures::{channel::mpsc::UnboundedSender, future::LocalBoxFuture};
use ratatui::Terminal;

use crate::{
    draft_file,
    draft_preview::DraftPreview,
    editor,
    outgoing::Outgoing,
    run::{self, Error},
    screen::Screen,
    steering::Ask,
    terminal_backend::TerminalBackend,
    toast::Toast,
    undo::Undo,
    xdg,
};

/// Compose: a new Draft's file in the editor, then its preview.
pub fn compose(
    terminal: &mut Terminal<TerminalBackend>,
    events: &mut EventStream,
    screen: &mut Screen,
) -> Result<(), Error> {
    let created = xdg::data_dir()
        .map_err(io::Error::other)
        .and_then(|dir| draft_file::create(&dir.join("drafts")));
    match created {
        Ok(path) => edit(terminal, events, screen, path),
        Err(error) => {
            tracing::error!("creating a Draft failed: {error}");
            screen.toast = Some(Toast::error("could not create the Draft"));
            Ok(())
        }
    }
}

/// The editor on the Draft at `path`, then its preview as the file reads now.
pub fn edit(
    terminal: &mut Terminal<TerminalBackend>,
    events: &mut EventStream,
    screen: &mut Screen,
    path: PathBuf,
) -> Result<(), Error> {
    run::outside(terminal, events, || editor::edit(&path))?;
    screen.draft = match DraftPreview::read(path) {
        Ok(preview) => Some(preview),
        Err(error) => {
            tracing::error!("reading the Draft failed: {error}");
            screen.toast = Some(Toast::error("could not read the Draft"));
            None
        }
    };

    Ok(())
}

/// Starts the Send of the Draft shown, unless it does not parse.
pub fn send<'a>(
    screen: &mut Screen,
    outgoing: Outgoing<'a>,
) -> Option<LocalBoxFuture<'a, Result<(), String>>> {
    let preview = screen.draft.as_mut()?;
    let Ok(draft) = &preview.draft else {
        screen.toast = Some(Toast::error("the Draft does not parse"));
        return None;
    };

    let sending = outgoing.send(draft);
    preview.sending = true;
    preview.failure = None;
    Some(sending)
}

/// A Send's end: the Draft goes and a sync starts, or its preview shows why.
pub fn sent(result: Result<(), String>, screen: &mut Screen, asks: &UnboundedSender<Ask>) {
    match result {
        Ok(()) => {
            if let Some(preview) = screen.draft.take()
                && let Err(error) = std::fs::remove_file(&preview.path)
            {
                tracing::error!("removing the sent Draft failed: {error}");
            }

            screen.toast = Some(Toast::new("Sent"));
            // The send fails only once the sync has stopped.
            asks.unbounded_send(Ask::Resync).ok();
        }
        Err(error) => {
            tracing::warn!("sending the Draft failed: {error}");
            if let Some(preview) = &mut screen.draft {
                preview.sending = false;
                preview.failure = Some(error);
            }
        }
    }
}

/// Deletes the shown Draft's file, for an Undo to write back.
pub fn discard(screen: &mut Screen) {
    let Some(preview) = screen.draft.take() else {
        return;
    };

    if let Err(error) = std::fs::remove_file(&preview.path) {
        tracing::error!("discarding the Draft failed: {error}");
        screen.toast = Some(Toast::error("could not discard the Draft"));
        screen.draft = Some(preview);
        return;
    }

    screen.undo.push(Undo::Discard(preview.path, preview.text));
    screen.toast = Some(Toast::new("Discarded · u undo"));
}

/// Undo of a discard: the file as it was, and its preview again.
pub fn restore(screen: &mut Screen, path: PathBuf, text: String) {
    if let Err(error) = draft_file::restore(&path, &text) {
        tracing::error!("restoring the Draft failed: {error}");
        screen.toast = Some(Toast::error("could not restore the Draft"));
        screen.undo.push(Undo::Discard(path, text));
        return;
    }

    screen.draft = Some(DraftPreview::new(path, text));
}
