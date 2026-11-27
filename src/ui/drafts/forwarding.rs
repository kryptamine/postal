use std::{io, path::PathBuf};

use crate::{
    compose::draft_file, compose::draft_notes::DraftNotes, compose::forward_draft,
    compose::forwarded, domain::message::Message, mail::awaited_parts::AwaitedParts,
    mail::fetch_requests::FetchRequests, mail::status::Status, ui::drafts::awaiting::Waiting,
    ui::drafts::composing, ui::error::Error, ui::state::screen::Screen, ui::state::toast::Toast,
    ui::state::ui::Ui, ui::terminal::tty::Tty, ui::world::World,
};

/// Forward of the selected Message, in Reading of the one at the cursor,
/// once its body and every attachment are in, which the fetch connection
/// may first bring.
pub fn forward(
    tty: &mut Tty,
    ui: &mut Ui,
    world: World,
    fetch: &FetchRequests,
) -> Result<(), Error> {
    let Ui { view, screen } = ui;
    let message = view.at_cursor(screen.showing.cursor());

    let Some(message) = message.cloned() else {
        return Ok(());
    };

    // A Reply or Forward still waiting would open its own Draft over this one.
    screen.awaited.cancel(fetch);
    let body_missing = !world.caches[message.account].has_body(message.body_key)?;
    let attachments = message.attachments.iter().count();
    let Some(on_server) = message.on_server() else {
        if body_missing || attachments > 0 {
            screen.toast = Some(Toast::new("open the Message from its Folder"));
            return Ok(());
        }

        return open(tty, screen, &message, &[], world);
    };

    let mut parts = AwaitedParts::new(message.account);
    if body_missing {
        parts.add_body(on_server.clone(), true);
    }

    for index in 0..attachments {
        parts.add_attachment(on_server.clone(), index);
    }

    if parts.complete() {
        return open(tty, screen, &message, &[], world);
    }

    if screen.statuses[message.account] == Status::SignedOut {
        screen.toast = Some(Toast::new("not downloaded · signed out"));
        return Ok(());
    }

    screen
        .awaited
        .start(fetch, Waiting::forward(message, parts));
    Ok(())
}

/// The end of the fetch a Forward waits on: its editor, or why not.
pub fn forwarded(
    message: &Message,
    result: Result<Vec<Vec<u8>>, String>,
    tty: &mut Tty,
    screen: &mut Screen,
    world: World,
) -> Result<(), Error> {
    let attachments = match result {
        Ok(attachments) => attachments,
        Err(reason) => {
            tracing::warn!("fetching the Message to forward failed: {reason}");
            screen.toast = Some(Toast::error(format!("couldn't forward: {reason}")));
            return Ok(());
        }
    };

    open(tty, screen, message, &attachments, world)
}

/// A Forward's Draft of `message` in the editor, the `attachments` fetched
/// written beside it, going out from the Account the Message reached.
fn open(
    tty: &mut Tty,
    screen: &mut Screen,
    message: &Message,
    attachments: &[Vec<u8>],
    world: World,
) -> Result<(), Error> {
    let account = message.account;
    let Some(raw) = world.caches[account].body(message.body_key)? else {
        screen.toast = Some(Toast::error("could not fetch the Message"));
        return Ok(());
    };

    let accounts = world.accounts;
    let signature = accounts.outgoing(account).signature();
    let written = draft_file::dir().and_then(|dir| {
        let path = draft_file::named(&dir);
        let names = message.attachments.iter().map(|a| a.name);
        let attached = (0..)
            .zip(names.zip(attachments))
            .map(|(index, (name, bytes))| forwarded::write(&path, index, name, bytes))
            .collect::<io::Result<Vec<PathBuf>>>()?;
        let text = forward_draft::forward(&raw, message, composing::block(&signature), &attached);
        draft_file::create_at(&path, &text)?;
        Ok(path)
    });

    let path = match written {
        Ok(path) => path,
        Err(error) => {
            tracing::error!("creating a Forward's Draft failed: {error}");
            screen.toast = Some(Toast::error("could not create the Draft"));
            return Ok(());
        }
    };

    let notes = DraftNotes {
        account: accounts.noted(account).map(str::to_owned),
        answering: None,
    };

    if let Err(error) = draft_file::note(&path, &notes) {
        tracing::error!("noting the Account of a Draft failed: {error}");
    }

    let exit = composing::in_editor(tty, accounts, &path, notes.account.as_deref())?;
    composing::written(screen, world, path, notes, exit);
    composing::toast_unread(screen, &signature);
    Ok(())
}
