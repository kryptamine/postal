use std::path::PathBuf;

use futures::channel::mpsc::UnboundedSender;
use ratatui::Terminal;

use crate::{
    answering::Answering,
    awaited_reply::AwaitedReply,
    cache::Cache,
    draft_file,
    draft_preview::DraftPreview,
    editor,
    input::Input,
    message::Message,
    notes::{self, Notes},
    outbox::Outbox,
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
    if let Some(path) = answered_by(message) {
        return edit(terminal, events, screen, (path, None));
    }

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

/// The Draft already answering `message`, which a Reply takes up again.
fn answered_by(message: &Message) -> Option<PathBuf> {
    let folder = message.folder.as_deref()?;
    let found =
        draft_file::dir().and_then(|dir| draft_file::answering(&dir, (folder, message.uid)));
    found.unwrap_or_else(|error| {
        tracing::error!("looking for a Draft answering the Message failed: {error}");
        None
    })
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
    let Some(path) = create(screen, Some(&text)) else {
        return Ok(());
    };

    // Without its notes, the Draft no longer answers once its preview closes.
    if let Err(error) = draft_file::note(&path, &answering) {
        tracing::error!("noting what a Draft answers failed: {error}");
    }

    edit(terminal, events, screen, (path, Some(answering)))
}

/// A new Draft's file holding `text`, else the template; `None` once the
/// failure is toasted.
fn create(screen: &mut Screen, text: Option<&str>) -> Option<PathBuf> {
    let created = draft_file::dir().and_then(|dir| draft_file::create(&dir, text));
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

/// Send: the Draft shown moves to the Outbox, unless it does not parse.
pub fn send(
    screen: &mut Screen,
    (outgoing, outbox): (Outgoing, &Outbox),
    asks: &UnboundedSender<Ask>,
) {
    let Some(mut preview) = screen.draft.take() else {
        return;
    };

    let Ok(draft) = &preview.draft else {
        screen.toast = Some(Toast::error("the Draft does not parse"));
        screen.draft = Some(preview);
        return;
    };

    let built = outgoing.build(draft, preview.answering.as_ref());

    let notes = Notes {
        draft: Some(preview.text.clone()),
        answering: preview.answering.clone(),
        error: None,
    };

    let queued = built.and_then(|message| {
        outbox
            .queue(&preview.path, &message, &notes)
            .map_err(|error| {
                tracing::error!("moving the Draft to the Outbox failed: {error}");
                "could not move the Draft to the Outbox".to_owned()
            })
    });

    if let Err(error) = queued {
        preview.failure = Some(error);
        screen.draft = Some(preview);
        return;
    }

    screen.outbox = outbox.count();
    screen.toast = Some(Toast::new("Sending"));
    // The send fails only while signed out: the Outbox waits for a sync.
    asks.unbounded_send(Ask::Send).ok();
}

/// Closes the shown Draft's preview, its file kept for Local Drafts.
pub fn keep(screen: &mut Screen) {
    if screen.draft.take().is_some() {
        screen.toast = Some(Toast::new("Saved to Drafts"));
    }
}

/// Discard of the Draft at `path`, as from its preview.
pub fn discard_file(screen: &mut Screen, path: PathBuf) {
    match DraftPreview::read(path, None) {
        Ok(preview) => {
            screen.draft = Some(preview);
            discard(screen);
        }
        Err(error) => {
            tracing::error!("reading the Draft failed: {error}");
            screen.toast = Some(Toast::error("could not read the Draft"));
        }
    }
}

/// Deletes the shown Draft's file, for an Undo to write back.
pub fn discard(screen: &mut Screen) {
    let Some(preview) = screen.draft.take() else {
        return;
    };

    if let Err(error) = notes::remove_message(&preview.path) {
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
    if let Err(error) = draft_file::restore(&path, &text, answering.as_ref()) {
        tracing::error!("restoring the Draft failed: {error}");
        screen.toast = Some(Toast::error("could not restore the Draft"));
        screen.undo.push(Undo::Discard(path, text, answering));
        return;
    }

    screen.draft = Some(DraftPreview::new(path, text, answering));
}
