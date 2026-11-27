use crate::{
    compose::draft_notes::DraftNotes,
    compose::mailto::Mailto,
    compose::recipients::Recipients,
    domain::message::Message,
    domain::per_account::AccountId,
    domain::shown::Shown,
    mail::fetch_requests::FetchRequests,
    ui::drafts::composing::{self, Reply},
    ui::drafts::draft_preview::DraftPreview,
    ui::error::Error,
    ui::input::step::Drafting,
    ui::list::inbox_view::InboxView,
    ui::state::ui::Ui,
    ui::terminal::tty::Tty,
    ui::world::World,
};

/// Compose, or the shown Draft edited again, sent, kept or discarded.
pub fn draft(
    drafting: Drafting,
    tty: &mut Tty,
    ui: &mut Ui,
    world: World,
    fetch: &FetchRequests,
) -> Result<(), Error> {
    let screen = &mut ui.screen;
    match drafting {
        Drafting::Compose => {
            // A Reply or Forward still waiting would open over this Draft.
            screen.awaited.cancel(fetch);
            let from = composing_from(&ui.view);
            composing::compose(tty, screen, world, from, None)?;
        }
        Drafting::Edit => {
            if let Some(preview) = screen.over.take::<DraftPreview>() {
                let notes = DraftNotes {
                    account: preview.account,
                    answering: preview.answering,
                };

                composing::edit(tty, screen, world, preview.path, notes)?;
            }
        }
        Drafting::Send => composing::send(screen, world),
        Drafting::Keep => composing::keep(screen),
        Drafting::Discard => composing::discard(screen),
        Drafting::DiscardFile(path) => composing::discard_file(screen, path),
    }

    Ok(())
}

/// Compose of the Draft `link` fills, from the Account `c` composes from.
pub fn compose_link(link: &Mailto, tty: &mut Tty, ui: &mut Ui, world: World) -> Result<(), Error> {
    let from = composing_from(&ui.view);
    composing::compose(tty, &mut ui.screen, world, from, Some(link))
}

/// A Reply to what the screen answers.
pub fn reply(
    to: Recipients,
    tty: &mut Tty,
    ui: &mut Ui,
    world: World,
    fetch: &FetchRequests,
) -> Result<(), Error> {
    let Some(message) = answered(ui) else {
        return Ok(());
    };

    composing::reply(tty, ui, world, fetch, Reply { message, to })
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
