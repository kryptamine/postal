use crate::{
    ui::drafts::composing, ui::error::Error, ui::list::moving, ui::state::toast::Toast,
    ui::state::ui::Ui, ui::state::undo::Undo, ui::world::World,
};

/// Undo of the latest action not yet taken back: a move, a Draft
/// discarded or a Send still held.
pub fn undo(ui: &mut Ui, world: World) -> Result<(), Error> {
    let screen = &mut ui.screen;
    match screen.undo.pop() {
        Some(Undo::Move(batches, thread)) => return moving::undo(ui, world, batches, thread),
        Some(Undo::Discard(path, text, notes)) => composing::restore(screen, path, text, notes),
        Some(Undo::Send(path)) => composing::take_back(screen, world.outbox, &path),
        None => screen.toast = Some(Toast::new("nothing to undo")),
    }

    Ok(())
}
