use crate::{
    cache::{self, Cache},
    domain::folder::Folder,
    domain::inbox::Inbox,
    domain::per_account::PerAccount,
    domain::shown::Shown,
    ui::all_inboxes,
    ui::inbox_view::InboxView,
    ui::main_screen::Ui,
    ui::run::Error,
    ui::showing::Showing,
};

/// Reads the shown Folder from the Cache into `view`, at most `limit`
/// Messages, and the Search under way anew.
pub fn show(
    view: &mut InboxView,
    caches: &PerAccount<Cache>,
    limit: Option<usize>,
) -> Result<(), Error> {
    let inbox = match &view.folder {
        Shown::AllInboxes { .. } => {
            let read = all_inboxes::read(caches, view, limit)?;
            read.map(|(inbox, edge)| {
                view.edge = edge;
                inbox
            })
        }
        Shown::Folder(folder) => {
            let (account, cache) = (folder.account, &caches[folder.account]);
            let bottomed = view.bottomed(cache, account, &folder.name);
            view.edge = (!bottomed?).then_some(account);
            cache.inbox(&folder.name, limit)?
        }
        // Local Drafts opens over the Message List and never shows in it.
        Shown::LocalDrafts => None,
    };

    if let Some(inbox) = inbox {
        view.show(inbox);
    }

    if view.search.is_some() {
        view.research(everywhere(caches, &view.folder)?);
    }

    Ok(())
}

/// Shows `folder` in the Message List, from the Cache, unless it is shown
/// already; true when it was not, for the sync connection to follow.
pub fn switch(ui: &mut Ui, caches: &PerAccount<Cache>, folder: Shown) -> Result<bool, Error> {
    if folder.same(&ui.view.folder) {
        return Ok(false);
    }

    ui.screen.list();
    ui.screen.over.hints = None;
    ui.view.switch(folder);
    show(&mut ui.view, caches, None)?;
    Ok(true)
}

/// Every Account's Folders, each Account's in the Folder Picker's order.
pub fn folders(caches: &PerAccount<Cache>) -> Result<Vec<Folder>, Error> {
    let mut folders = Vec::new();
    for cache in caches.values() {
        folders.extend(cache.folders()?);
    }

    Ok(folders)
}

/// Lists the Folders anew in an open Folder Picker.
pub fn relist(ui: &mut Ui, caches: &PerAccount<Cache>) -> Result<(), Error> {
    if let Some(picker) = &mut ui.screen.over.picker {
        picker.reload(folders(caches)?, &ui.view.folder);
    }

    Ok(())
}

/// Every Account's Messages a Search matches, newest first, the shown
/// Folder's copy winning in its Account, the Inbox's in All Inboxes (ADR 0010).
fn everywhere(caches: &PerAccount<Cache>, shown: &Shown) -> Result<Inbox, cache::Error> {
    let (mut messages, mut elsewhere) = (Vec::new(), Vec::new());
    for (account, cache) in caches.iter() {
        let found = cache.everywhere(shown.of(account))?;
        messages.extend(found.messages);
        elsewhere.extend(found.elsewhere);
    }

    messages.sort_by(|a, b| (b.received, b.uid).cmp(&(a.received, a.uid)));
    elsewhere.sort_by_key(|m| m.received);
    Ok(Inbox {
        messages,
        elsewhere,
    })
}

/// Types the Query of a new Search over the whole Cache, or of the one under way.
pub fn start_search(ui: &mut Ui, caches: &PerAccount<Cache>) -> Result<(), Error> {
    let shown = ui.view.folder.clone();
    if ui.view.start_search(|| everywhere(caches, &shown))? {
        ui.screen.showing = Showing::Query;
    }

    Ok(())
}
