use std::{io, path::PathBuf};

use crate::{
    compose::awaited_forward::AwaitedForward, compose::draft_file,
    compose::draft_notes::DraftNotes, compose::editor, compose::forward_draft, compose::forwarded,
    domain::message::Message, mail::forward_fetch::ForwardFetch, mail::status::Status,
    ui::drafts::composing, ui::run::Error, ui::state::screen::Screen, ui::state::toast::Toast,
    ui::state::ui::Ui, ui::terminal::tty::Tty, ui::world::World,
};

/// Forward of the selected Message, in Reading of the one at the cursor,
/// once its body and every attachment are in, which the fetch connection
/// may first bring.
pub fn forward(
    tty: &mut Tty,
    ui: &mut Ui,
    world: World,
    fetch: &ForwardFetch,
) -> Result<(), Error> {
    let Ui { view, screen } = ui;
    let message = screen.showing.reading().map_or_else(
        || view.selected_message(),
        |reading| view.stack().find(|m| m.body_key == reading.cursor),
    );

    let Some(message) = message.cloned() else {
        return Ok(());
    };

    // Either would open its own Draft over this one.
    screen.replying = None;
    screen.forwarding = None;
    let body_missing = !world.caches[message.account].has_body(message.body_key)?;
    let attachments = message.attachments.iter().count();
    let Some(on_server) = message.on_server() else {
        if body_missing || attachments > 0 {
            screen.toast = Some(Toast::new("open the Message from its Folder"));
            return Ok(());
        }

        return open(tty, screen, &message, &[], world);
    };

    let awaited = AwaitedForward {
        message,
        on_server,
        body_missing,
        attachments: vec![None; attachments],
    };

    if awaited.complete() {
        return open(tty, screen, &awaited.message, &[], world);
    }

    if screen.statuses[awaited.message.account] == Status::SignedOut {
        screen.toast = Some(Toast::new("not downloaded · signed out"));
        return Ok(());
    }

    fetch.ask(&awaited);
    screen.forwarding = Some(awaited);
    Ok(())
}

/// The end of the fetch a Forward waits on: its editor, or why not.
pub fn forwarded(
    result: Result<(), String>,
    tty: &mut Tty,
    screen: &mut Screen,
    world: World,
) -> Result<(), Error> {
    let Some(awaited) = screen.forwarding.take() else {
        return Ok(());
    };

    if let Err(reason) = result {
        tracing::warn!("fetching the Message to forward failed: {reason}");
        screen.toast = Some(Toast::error(format!("couldn't forward: {reason}")));
        return Ok(());
    }

    let attachments: Vec<Vec<u8>> = awaited.attachments.into_iter().flatten().collect();
    open(tty, screen, &awaited.message, &attachments, world)
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

    tty.outside(|| editor::edit(&path))?;
    composing::toast_unread(screen, &signature);
    composing::preview(screen, path, notes);
    Ok(())
}
