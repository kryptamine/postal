use crate::{
    compose::local_drafts::LocalDrafts, compose::outbox::Outbox, ui::screen::Screen,
    ui::toast::Toast,
};

pub fn open_local(screen: &mut Screen, outbox: &Outbox) {
    match LocalDrafts::open(outbox) {
        Ok(local) => screen.over.local = Some(local),
        Err(error) => {
            tracing::error!("listing Local Drafts failed: {error}");
            screen.toast = Some(Toast::error("could not list Local Drafts"));
        }
    }
}

pub fn relist_local(screen: &mut Screen, outbox: &Outbox) {
    if let Some(Err(error)) = screen.over.local.as_mut().map(|local| local.reload(outbox)) {
        tracing::error!("listing Local Drafts failed: {error}");
    }
}
