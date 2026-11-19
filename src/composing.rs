use std::{io, path::PathBuf};

use futures::{channel::mpsc::UnboundedSender, future::LocalBoxFuture};
use ratatui::Terminal;

use crate::{
    answering::Answering,
    awaited_reply::AwaitedReply,
    cache::Cache,
    draft_file,
    draft_preview::DraftPreview,
    editor,
    input::Input,
    outgoing::Outgoing,
    reply_draft,
    reply_fetch::ReplyFetch,
    run::{self, Error},
    screen::Screen,
    status::Status,
    steering::Ask,
    terminal_backend::TerminalBackend,
    toast::Toast,
    undo::Undo,
    xdg,
};

/// Compose: a new Draft's file in the editor, then its preview.
pub fn compose(
    terminal: &mut Terminal<TerminalBackend>,
    events: &mut Input,
    screen: &mut Screen,
) -> Result<(), Error> {
    // A Reply still waiting on its body would open over this Draft.
    screen.replying = None;
    create(screen, None).map_or(Ok(()), |path| edit(terminal, events, screen, (path, None)))
}

/// Reply, or Reply All, at once on a body in the Cache; else once the
/// fetch connection brings it.
pub fn reply(
    terminal: &mut Terminal<TerminalBackend>,
    events: &mut Input,
    screen: &mut Screen,
    (cache, fetch): (&Cache, &ReplyFetch),
    (awaited, account): (AwaitedReply, &str),
) -> Result<(), Error> {
    screen.replying = None;
    let message = &awaited.message;
    if let Some(raw) = cache.body(message.body_key).map_err(Error::Cache)? {
        return answer(terminal, events, screen, (&raw, &awaited, account));
    }

    if message.folder.is_none() {
        screen.toast = Some(Toast::new("not downloaded"));
    } else if screen.status == Status::SignedOut {
        screen.toast = Some(Toast::new("not downloaded · signed out"));
    } else {
        fetch.ask(&awaited);
        screen.replying = Some(awaited);
    }

    Ok(())
}

/// The end of the fetch a Reply waits on: its editor, or why not.
pub fn fetched(
    result: Result<(), String>,
    terminal: &mut Terminal<TerminalBackend>,
    events: &mut Input,
    screen: &mut Screen,
    (cache, account): (&Cache, &str),
) -> Result<(), Error> {
    let Some(awaited) = screen.replying.take() else {
        return Ok(());
    };

    let body = match result {
        Ok(()) => cache.body(awaited.message.body_key).map_err(Error::Cache)?,
        Err(reason) => {
            tracing::warn!("fetching the body to reply to failed: {reason}");
            None
        }
    };

    let Some(raw) = body else {
        screen.toast = Some(Toast::error("could not fetch the Message"));
        return Ok(());
    };

    answer(terminal, events, screen, (&raw, &awaited, account))
}

/// A Reply's Draft from the answered Message's body `raw`, in the editor.
fn answer(
    terminal: &mut Terminal<TerminalBackend>,
    events: &mut Input,
    screen: &mut Screen,
    (raw, awaited, account): (&[u8], &AwaitedReply, &str),
) -> Result<(), Error> {
    let (text, answering) = reply_draft::reply(raw, &awaited.message, account, awaited.all);
    create(screen, Some(&text)).map_or(Ok(()), |path| {
        edit(terminal, events, screen, (path, Some(answering)))
    })
}

/// A new Draft's file holding `text`, else the template; `None` once the
/// failure is toasted.
fn create(screen: &mut Screen, text: Option<&str>) -> Option<PathBuf> {
    let created = xdg::data_dir()
        .map_err(io::Error::other)
        .and_then(|dir| draft_file::create(&dir.join("drafts"), text));
    match created {
        Ok(path) => Some(path),
        Err(error) => {
            tracing::error!("creating a Draft failed: {error}");
            screen.toast = Some(Toast::error("could not create the Draft"));
            None
        }
    }
}

/// The editor on the Draft at `path`, then its preview as the file reads now.
pub fn edit(
    terminal: &mut Terminal<TerminalBackend>,
    events: &mut Input,
    screen: &mut Screen,
    (path, answering): (PathBuf, Option<Answering>),
) -> Result<(), Error> {
    run::outside(terminal, events, || editor::edit(&path))?;
    screen.draft = match DraftPreview::read(path, answering) {
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

    let sending = outgoing.send(draft, preview.answering.as_ref());
    preview.sending = true;
    preview.failure = None;
    Some(sending)
}

/// A Send's end: the Draft goes, the Message a Reply answers is flagged
/// `\Answered` and a sync starts; or its preview shows why not.
pub fn sent(
    result: Result<(), String>,
    screen: &mut Screen,
    cache: &Cache,
    asks: &UnboundedSender<Ask>,
) -> Result<(), Error> {
    let Err(error) = result else {
        let Some(preview) = screen.draft.take() else {
            return Ok(());
        };

        if let Err(error) = std::fs::remove_file(&preview.path) {
            tracing::error!("removing the sent Draft failed: {error}");
        }

        let original = preview.answering.and_then(|answering| answering.original);
        if let Some((folder, uid)) = original {
            cache.answer(&folder, uid).map_err(Error::Cache)?;
            asks.unbounded_send(Ask::Changes).ok();
        }

        screen.toast = Some(Toast::new("Sent"));
        // The send fails only once the sync has stopped.
        asks.unbounded_send(Ask::Resync).ok();
        return Ok(());
    };

    tracing::warn!("sending the Draft failed: {error}");
    if let Some(preview) = &mut screen.draft {
        preview.sending = false;
        preview.failure = Some(error);
    }

    Ok(())
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

    screen
        .undo
        .push(Undo::Discard(preview.path, preview.text, preview.answering));
    screen.toast = Some(Toast::new("Discarded · u undo"));
}

/// Undo of a discard: the file as it was, and its preview again.
pub fn restore(screen: &mut Screen, (path, text, answering): (PathBuf, String, Option<Answering>)) {
    if let Err(error) = draft_file::restore(&path, &text) {
        tracing::error!("restoring the Draft failed: {error}");
        screen.toast = Some(Toast::error("could not restore the Draft"));
        screen.undo.push(Undo::Discard(path, text, answering));
        return;
    }

    screen.draft = Some(DraftPreview::new(path, text, answering));
}
