use ratatui::layout::Rect;
use tokio::time::Instant;

use crate::{
    compose::local_drafts::LocalDrafts, domain::body_key::BodyKey, domain::thread::Key,
    ui::drafts::draft_preview::DraftPreview, ui::list::marking::mark_read,
    ui::popup::folder_picker::FolderPicker, ui::preview::previews::Previews, ui::run::Error,
    ui::state::mode::Mode, ui::state::ui::Ui, ui::world::World,
};

/// Follows the Dwell, then says when the screen next changes on its own.
pub fn next_change(ui: &mut Ui, previews: &Previews, text: Option<Rect>) -> Option<Instant> {
    let target = dwelling(ui, previews, text);
    let Ui { view, screen } = ui;
    screen
        .dwell
        .follow(view.selected_key(), target, Instant::now());
    screen.changes_at(Instant::now())
}

/// The selected Thread and its newest body key, while a Dwell may run on
/// it: listed, Unread, and its body shown in the Preview `text`.
fn dwelling(ui: &Ui, previews: &Previews, text: Option<Rect>) -> Option<(Key, BodyKey)> {
    let Ui { view, screen } = ui;
    let listed = matches!(screen.showing.mode(), Mode::List | Mode::Query)
        && !screen.over.is_open::<FolderPicker>()
        && !screen.over.is_open::<LocalDrafts>()
        && !screen.over.is_open::<DraftPreview>();
    let text = text.filter(|_| listed && !screen.help && view.selected_unread())?;
    previews.get(view, text.width)?;
    Some((view.selected_key()?, view.selected_body()?))
}

/// Drops the Toast and ends the Dwell when their time is up.
pub fn expire(ui: &mut Ui, world: World) -> Result<(), Error> {
    let now = Instant::now();
    ui.screen.expire(now);
    if ui.screen.dwell.ended(now) {
        mark_read(ui, world)?;
    }

    Ok(())
}
