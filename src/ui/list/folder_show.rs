use crate::{
    cache::Cache, domain::folder::Folder, domain::per_account::PerAccount, domain::shown::Shown,
    ui::list::all_inboxes, ui::list::inbox_view::InboxView, ui::list::search::Search,
    ui::popup::folder_picker::FolderPicker, ui::preview::link_hints::LinkHints, ui::run::Error,
    ui::state::showing::Showing, ui::state::ui::Ui,
};

/// Reads the shown Folder from the Cache into `view`, at most `limit`
/// Threads, and the Search under way anew.
pub fn show(
    view: &mut InboxView,
    caches: &PerAccount<Cache>,
    limit: Option<usize>,
) -> Result<(), Error> {
    let mut rows = view.take_rows();
    let rows = match &view.folder {
        Shown::AllInboxes { .. } => {
            let read = all_inboxes::read(caches, view, limit, rows)?;
            read.map(|inboxes| {
                view.edge = inboxes.edge.map(|(_, account)| account);
                (inboxes.rows, inboxes.edge.map(|(at, _)| at))
            })
        }
        Shown::Folder(folder) => {
            let (account, cache) = (folder.account, &caches[folder.account]);
            let read = cache.thread_rows(&folder.name, limit, &mut rows)?;
            match read {
                Some(held) => {
                    let bottomed = view.bottomed(cache, account, (&folder.name, held))?;
                    view.edge = (!bottomed).then_some(account);
                    Some((rows, None))
                }
                None => None,
            }
        }
        // Local Drafts opens over the Message List and never shows in it.
        Shown::LocalDrafts => None,
    };

    if let Some((rows, since)) = rows {
        view.show(rows, since, caches)?;
    }

    view.research(caches)?;
    Ok(())
}

/// Shows `folder` in the Message List, from the Cache, unless it is shown
/// already; true when it was not, for the sync connection to follow.
pub fn switch(ui: &mut Ui, caches: &PerAccount<Cache>, folder: Shown) -> Result<bool, Error> {
    if folder.same(&ui.view.folder) {
        return Ok(false);
    }

    ui.screen.list();
    ui.screen.over.close::<LinkHints>();
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
    if let Some(picker) = ui.screen.over.get_mut::<FolderPicker>() {
        picker.reload(folders(caches)?, &ui.view.folder);
    }

    Ok(())
}

/// Types the Query of a new Search over the whole Cache, or of the one under way.
pub fn start_search(ui: &mut Ui, caches: &PerAccount<Cache>) -> Result<(), Error> {
    if ui.view.start_search(caches)? {
        ui.screen.showing = Showing::Query;
    }

    Ok(())
}

/// Edits the Query of the Search under way and asks the Cache for it anew.
pub fn edit_query(
    ui: &mut Ui,
    caches: &PerAccount<Cache>,
    edit: impl FnOnce(&mut Search) -> bool,
) -> Result<(), Error> {
    ui.view.edit_query(caches, edit)?;
    Ok(())
}
