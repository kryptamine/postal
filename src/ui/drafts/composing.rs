use std::path::{Path, PathBuf};

use crate::{
    compose::accounts::Accounts,
    compose::awaited_reply::AwaitedReply,
    compose::draft_file,
    compose::draft_notes::DraftNotes,
    compose::editor,
    compose::notes::{self, Notes},
    compose::reply_draft,
    domain::message::Message,
    mail::reply_fetch::ReplyFetch,
    mail::status::Status,
    mail::steering::Ask,
    ui::drafts::draft_preview::DraftPreview,
    ui::run::Error,
    ui::state::main_screen::Ui,
    ui::state::screen::Screen,
    ui::state::toast::Toast,
    ui::state::undo::Undo,
    ui::terminal::tty::Tty,
    ui::world::World,
};

/// Compose from the Account `from`: a new Draft's file in the editor,
/// then its preview, unless it was left as the template.
pub fn compose(tty: &mut Tty, screen: &mut Screen, from: Option<&str>) -> Result<(), Error> {
    // A Reply still waiting on its body would open over this Draft.
    screen.replying = None;
    let Some(path) = create(screen, None) else {
        return Ok(());
    };

    let notes = DraftNotes {
        account: from.map(str::to_owned),
        answering: None,
    };

    if let Err(error) = draft_file::note(&path, &notes) {
        tracing::error!("noting the Account of a Draft failed: {error}");
    }

    tty.outside(|| editor::edit(&path))?;
    if draft_file::untouched(&path) {
        drop_untouched(screen, &path);
        return Ok(());
    }

    preview(screen, path, notes);
    Ok(())
}

/// Deletes a Compose left as the template: nothing is lost, so no Undo.
fn drop_untouched(screen: &mut Screen, path: &Path) {
    if let Err(error) = notes::remove_message(path) {
        tracing::error!("dropping an empty Draft failed: {error}");
        screen.toast = Some(Toast::error("could not drop the empty Draft"));
        return;
    }

    screen.toast = Some(Toast::new("Empty Draft dropped"));
}

/// Reply, or Reply All, once every body of the selected Thread is in
/// the Cache, which the fetch connection may first bring; while it is
/// not live, at once on the answered body alone.
pub fn reply(
    tty: &mut Tty,
    ui: &mut Ui,
    world: World,
    fetch: &ReplyFetch,
    mut awaited: AwaitedReply,
) -> Result<(), Error> {
    let Ui { view, screen } = ui;
    screen.replying = None;
    let message = &awaited.message;
    if let Some(path) = answered_by(message, world.accounts) {
        return edit(tty, screen, path, DraftNotes::default());
    }

    let cache = &world.caches[message.account];
    let raw = cache.body(message.body_key)?;
    // The answered Message may be the List's selection, outside the stack.
    let answered = (message.on_server(), message.body_key);
    let each = view.stack().map(|m| (m.on_server(), m.body_key));
    for (on_server, key) in each.chain([answered]) {
        if let Some(on_server) = on_server
            && !cache.has_body(key)?
            && !awaited.missing.contains(&on_server)
        {
            awaited.missing.push(on_server);
        }
    }

    let status = screen.statuses[message.account];
    let signed_out = status == Status::SignedOut;
    match raw {
        Some(raw) if awaited.missing.is_empty() || status != Status::Live => {
            answer(tty, screen, &raw, &awaited, world.accounts)
        }
        None if message.folder.is_none() => {
            screen.toast = Some(Toast::new("not downloaded"));
            Ok(())
        }
        None if signed_out => {
            screen.toast = Some(Toast::new("not downloaded · signed out"));
            Ok(())
        }
        _ => {
            fetch.ask(&awaited);
            screen.replying = Some(awaited);
            Ok(())
        }
    }
}

/// The Draft already answering `message`, which a Reply takes up again.
fn answered_by(message: &Message, accounts: Accounts) -> Option<PathBuf> {
    let folder = message.folder.as_ref()?;
    let email = accounts.email(message.account);
    let found =
        draft_file::dir().and_then(|dir| draft_file::answering(&dir, email, folder, message.uid));
    found.unwrap_or_else(|error| {
        tracing::error!("looking for a Draft answering the Message failed: {error}");
        None
    })
}

/// The end of the fetch a Reply waits on: its editor, or why not.
pub fn fetched(
    result: Result<(), String>,
    tty: &mut Tty,
    screen: &mut Screen,
    world: World,
) -> Result<(), Error> {
    let Some(awaited) = screen.replying.take() else {
        return Ok(());
    };

    // The answered body may be cached though the rest of its Thread failed.
    if let Err(reason) = result {
        tracing::warn!("fetching the bodies to reply to failed: {reason}");
    }

    let message = &awaited.message;
    let body = world.caches[message.account].body(message.body_key)?;

    let Some(raw) = body else {
        screen.toast = Some(Toast::error("could not fetch the Message"));
        return Ok(());
    };

    answer(tty, screen, &raw, &awaited, world.accounts)
}

/// A Reply's Draft from the answered Message's body `raw`, in the editor,
/// going out from the Account the Message reached.
fn answer(
    tty: &mut Tty,
    screen: &mut Screen,
    raw: &[u8],
    awaited: &AwaitedReply,
    accounts: Accounts,
) -> Result<(), Error> {
    let account = awaited.message.account;
    let email = accounts.email(account);
    let (text, answering) = reply_draft::reply(raw, &awaited.message, email, awaited.to);
    let Some(path) = create(screen, Some(&text)) else {
        return Ok(());
    };

    // Without its notes, the Draft no longer answers once its preview closes.
    let notes = DraftNotes {
        account: accounts.noted(account).map(str::to_owned),
        answering: Some(answering),
    };

    if let Err(error) = draft_file::note(&path, &notes) {
        tracing::error!("noting what a Draft answers failed: {error}");
    }

    edit(tty, screen, path, notes)
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

/// The editor on the Draft at `path`, then its preview as the file reads
/// now; `notes` win over those beside it.
pub fn edit(
    tty: &mut Tty,
    screen: &mut Screen,
    path: PathBuf,
    notes: DraftNotes,
) -> Result<(), Error> {
    tty.outside(|| editor::edit(&path))?;
    preview(screen, path, notes);
    Ok(())
}

/// The preview of the Draft at `path` as the file reads now; `notes` win
/// over those beside it.
fn preview(screen: &mut Screen, path: PathBuf, notes: DraftNotes) {
    match DraftPreview::read(path, notes.answering) {
        Ok(mut preview) => {
            preview.account = notes.account.or(preview.account);
            screen.over.open(preview);
        }
        Err(error) => {
            screen.over.close::<DraftPreview>();
            tracing::error!("reading the Draft failed: {error}");
            screen.toast = Some(Toast::error("could not read the Draft"));
        }
    }
}

/// Send: the Draft shown moves to the Outbox, unless it does not parse,
/// and its Account is asked to send it.
pub fn send(screen: &mut Screen, world: World) {
    let (accounts, outbox) = (world.accounts, world.outbox);
    let Some(mut preview) = screen.over.take::<DraftPreview>() else {
        return;
    };

    let Ok(draft) = &preview.draft else {
        screen.toast = Some(Toast::error("the Draft does not parse"));
        screen.over.open(preview);
        return;
    };

    let Some(account) = accounts.place(preview.account.as_deref()) else {
        preview.failure = Some("no Account to send from".to_owned());
        screen.over.open(preview);
        return;
    };

    let built = accounts
        .outgoing(account)
        .build(draft, preview.answering.as_ref());

    let notes = Notes {
        draft: Some(preview.text.clone()),
        // The Account it is built for, which alone sends it: one named and
        // gone from the config would leave it in the Outbox.
        account: accounts.noted(account).map(str::to_owned),
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
        screen.over.open(preview);
        return;
    }

    screen.outbox = outbox.count();
    screen.toast = Some(Toast::new("Sending"));
    // Signed out, the Outbox waits for a sync.
    world.asks.send(account, Ask::Send);
}

/// Closes the shown Draft's preview, its file kept for Local Drafts.
pub fn keep(screen: &mut Screen) {
    if screen.over.take::<DraftPreview>().is_some() {
        screen.toast = Some(Toast::new("Saved to Drafts"));
    }
}

/// Discard of the Draft at `path`, as from its preview.
pub fn discard_file(screen: &mut Screen, path: PathBuf) {
    match DraftPreview::read(path, None) {
        Ok(preview) => {
            screen.over.open(preview);
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
    let Some(preview) = screen.over.take::<DraftPreview>() else {
        return;
    };

    if let Err(error) = notes::remove_message(&preview.path) {
        tracing::error!("discarding the Draft failed: {error}");
        screen.toast = Some(Toast::error("could not discard the Draft"));
        screen.over.open(preview);
        return;
    }

    let notes = DraftNotes {
        account: preview.account,
        answering: preview.answering,
    };

    screen
        .undo
        .push(Undo::Discard(preview.path, preview.text, notes));
    screen.toast = Some(Toast::new("Discarded · u undo"));
}

/// Undo of a discard: the file as it was, and its preview again.
pub fn restore(screen: &mut Screen, path: PathBuf, text: String, notes: DraftNotes) {
    if let Err(error) = draft_file::restore(&path, &text, &notes) {
        tracing::error!("restoring the Draft failed: {error}");
        screen.toast = Some(Toast::error("could not restore the Draft"));
        screen.undo.push(Undo::Discard(path, text, notes));
        return;
    }

    screen.over.open(DraftPreview::new(path, text, notes));
}
