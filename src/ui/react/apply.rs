use crate::{
    cache::Cache,
    domain::body_key::BodyKey,
    domain::folder::Folder,
    domain::folder_name::FolderName,
    domain::per_account::{AccountId, PerAccount},
    domain::shown::Shown,
    domain::uid::Uid,
    mail::backfill::Backfill,
    mail::status::Status,
    mail::update::{BodyLink, Update},
    ui::drafts::local_list::relist_local,
    ui::list::folder_show::{relist, show, switch},
    ui::list::inbox_view::InboxView,
    ui::preview::attachment_hints::AttachmentHints,
    ui::preview::link_hints::LinkHints,
    ui::preview::previews::Previews,
    ui::run::Error,
    ui::state::screen::Screen,
    ui::state::showing::Showing,
    ui::state::toast::Toast,
    ui::state::ui::Ui,
    ui::world::World,
};

/// What one `Update` of `account` from the background work does to the screen.
pub fn apply(
    account: AccountId,
    update: Update,
    ui: &mut Ui,
    world: World,
    previews: &mut Previews,
) -> Result<(), Error> {
    let (caches, outbox) = (world.caches, world.outbox);
    match update {
        Update::Inbox => reshow_inbox(ui, caches)?,
        Update::Folders => relist(ui, caches)?,
        Update::FolderGone(folder) => {
            if ui.view.folder.is(account, &folder) {
                let gone = format!("Folder {} no longer exists", ui.view.folder.label());
                // The sync connection is back on the Inbox already.
                switch(ui, caches, Shown::Folder(Folder::inbox(account)))?;
                ui.screen.toast = Some(Toast::new(gone));
            }

            relist(ui, caches)?;
        }
        Update::Bodies(folder, fetched) => previews.fetched(account, &folder, fetched),
        // Taken before the screen: an Attachment by the attachments asked
        // for, arrivals by the Notifier.
        Update::Attachment(_) | Update::Arrived(_) => {}
        Update::BodyLink(link) => {
            previews.link(account, link);
            ui.screen.backfill = None;
        }
        Update::Rendered(key, preview) => previews.rendered(key, preview),
        Update::Status(status) => report(&mut ui.screen, previews, account, status),
        Update::Backfill(Backfill::Fetching { done, total }) => {
            ui.screen.backfill = Some(Status::Syncing { done, total });
        }
        Update::Backfill(Backfill::Paged {
            folder,
            nothing_below,
        }) => {
            ui.screen.backfill = None;
            paged(&mut ui.view, caches, account, folder, nothing_below)?;
        }
        Update::NoFolder(role) => {
            ui.screen.toast = Some(Toast::error(format!("no {} Folder", role.label())));
        }
        Update::Unrestored => ui.screen.toast = Some(Toast::error("nothing to undo")),
        Update::Sent => {
            relist_local(&mut ui.screen, outbox);
            ui.screen.outbox = outbox.count();
            ui.screen.toast = Some(Toast::new("Sent"));
        }
        Update::Refused => {
            relist_local(&mut ui.screen, outbox);
            ui.screen.outbox = outbox.count();
            ui.screen.toast = Some(Toast::error("refused · back in Drafts"));
        }
    }

    Ok(())
}

/// The shown Folder again as the Cache reads it now.
fn reshow_inbox(ui: &mut Ui, caches: &PerAccount<Cache>) -> Result<(), Error> {
    let view = &ui.view;
    let (thread, selected) = (view.selected_key(), view.selected_body());
    let stack: Vec<BodyKey> = match ui.screen.showing {
        Showing::Reading(_) => view.stack().map(|m| m.body_key).collect(),
        Showing::List | Showing::Query => Vec::new(),
    };

    show(&mut ui.view, caches, None)?;
    let Ui { view, screen } = &mut *ui;
    // The labels belong to the Messages they were laid out on, and
    // Reading to the Thread it was opened on; a Query being typed stays.
    if screen.showing.reading().is_none() {
        if view.selected_body() != selected {
            screen.over.close::<LinkHints>();
            screen.over.close::<AttachmentHints>();
        }
    } else if !screen.follow(view, thread) && !view.stack().map(|m| m.body_key).eq(stack) {
        screen.over.close::<LinkHints>();
        screen.over.close::<AttachmentHints>();
        if let Some(reading) = screen.showing.reading_mut() {
            reading.hold = true;
        }
    }

    relist(ui, caches)
}

/// A page of older Messages of `folder` of `account` is in; the server had
/// nothing below `nothing_below` when it says so.
fn paged(
    view: &mut InboxView,
    caches: &PerAccount<Cache>,
    account: AccountId,
    folder: FolderName,
    nothing_below: Option<Uid>,
) -> Result<(), Error> {
    view.nothing_below[account] = nothing_below.map(|uid| (folder, uid));

    // In All Inboxes, an Account with nothing older sets no edge (ADR 0014).
    show(view, caches, None)
}

/// The sync connection of `account` is in `status` now.
fn report(screen: &mut Screen, previews: &mut Previews, account: AccountId, status: Status) {
    if status == Status::SignedOut {
        previews.link(account, BodyLink::Down);
    }

    screen.statuses[account] = status;
}
