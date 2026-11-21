use crate::{
    compose::awaited_reply::AwaitedReply, compose::draft_notes::DraftNotes,
    domain::message::Message, domain::per_account::AccountId, domain::shown::Shown,
    mail::reply_fetch::ReplyFetch, ui::composing, ui::inbox_view::InboxView, ui::main_screen::Ui,
    ui::run::Error, ui::step::Step, ui::tty::Tty, ui::world::World,
};

/// Compose, the editor again on the shown Draft, or its Send.
pub fn draft(step: &Step, tty: &mut Tty, ui: &mut Ui, world: World) -> Result<(), Error> {
    let screen = &mut ui.screen;
    match step {
        Step::Compose => {
            let from = composing_from(&ui.view).and_then(|account| world.accounts.noted(account));
            composing::compose(tty, screen, from)?;
        }
        Step::Edit => {
            if let Some(preview) = screen.over.draft.take() {
                let notes = DraftNotes {
                    account: preview.account,
                    answering: preview.answering,
                };

                composing::edit(tty, screen, preview.path, notes)?;
            }
        }
        Step::Send => composing::send(screen, world),
        _ => {}
    }

    Ok(())
}

/// Reply, or Reply All when `all`, to what the screen answers.
pub fn reply(
    all: bool,
    tty: &mut Tty,
    ui: &mut Ui,
    world: World,
    replies: &ReplyFetch,
) -> Result<(), Error> {
    let Some(message) = answered(ui) else {
        return Ok(());
    };

    let awaited = AwaitedReply {
        message,
        all,
        missing: Vec::new(),
    };

    composing::reply(tty, ui, world, replies, awaited)
}

/// What a Reply answers: the newest Message of the selected Thread not from
/// the Account, else its newest; in Reading, the one at the cursor.
fn answered(ui: &Ui) -> Option<Message> {
    let view = &ui.view;
    let message = ui.screen.showing.reading().map_or_else(
        || {
            view.stack()
                .filter(|m| !m.from_me)
                .max_by_key(|m| m.received)
                .or_else(|| view.selected_message())
        },
        |reading| view.stack().find(|m| m.body_key == reading.cursor),
    );

    message.cloned()
}

/// The Account a Compose goes out from: the shown Folder's; in All
/// Inboxes, the selected Thread's, else the first.
fn composing_from(view: &InboxView) -> Option<AccountId> {
    match &view.folder {
        Shown::Folder(folder) => Some(folder.account),
        _ => view
            .selected_message()
            .map(|m| m.account)
            .or_else(|| view.accounts.first()),
    }
}
