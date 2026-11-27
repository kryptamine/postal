use crate::{
    compose::local_drafts::LocalDrafts, compose::outbox::Outbox, ui::state::screen::Screen,
    ui::state::toast::Toast,
};

pub fn open_local(screen: &mut Screen, outbox: &Outbox) {
    match LocalDrafts::open(outbox) {
        Ok(local) => screen.over.open(local),
        Err(error) => {
            tracing::error!("listing Local Drafts failed: {error}");
            screen.set_toast(Toast::error("could not list Local Drafts"));
        }
    }
}

pub fn relist_local(screen: &mut Screen, outbox: &Outbox) {
    if let Some(Err(error)) = screen
        .over
        .get_mut::<LocalDrafts>()
        .map(|local| local.reload(outbox))
    {
        tracing::error!("listing Local Drafts failed: {error}");
    }
}
