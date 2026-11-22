use crate::{
    mail::steering::Ask,
    ui::input::step::Steer,
    ui::list::deleting,
    ui::list::folder_show::{edit_query, folders, start_search, switch},
    ui::list::marking::{mark_read, mark_unread},
    ui::list::search::Search,
    ui::popup::folder_picker::FolderPicker,
    ui::run::Error,
    ui::state::main_screen::Ui,
    ui::world::World,
};

/// Opens the Folder Picker as the Cache last saw the Folders, starts or
/// edits a Search, shows the Folder picked, or marks, deletes or restores Messages,
/// asking the sync connection to follow.
pub fn steer(steer: Steer, ui: &mut Ui, world: World) -> Result<(), Error> {
    let (caches, asks) = (world.caches, world.asks);
    // Signed out, nothing syncs and the ask is dropped.
    match steer {
        Steer::Folders => {
            let folders = folders(caches)?;
            let view = &ui.view;
            let picker = FolderPicker::new(folders, &view.folder, &view.accounts);
            ui.screen.over.open(picker);
            for account in caches.ids() {
                asks.send(account, Ask::ListFolders);
            }
        }
        Steer::Show(folder) => {
            if switch(ui, caches, folder)? {
                for account in caches.ids() {
                    if let Some(shown) = ui.view.folder.of(account) {
                        asks.send(account, Ask::Show(shown.clone()));
                    }
                }
            }
        }
        Steer::Read(thread) => {
            mark_read(ui, world)?;
            if let Some((account, id)) = thread {
                asks.send(account, Ask::Thread(id));
            }
        }
        Steer::MarkUnread => mark_unread(ui, world)?,
        Steer::Delete => deleting::delete(ui, world)?,
        Steer::Undo => deleting::undo(ui, world)?,
        Steer::Search => start_search(ui, caches)?,
        Steer::Type(c) => edit_query(ui, caches, |search| search.push(c))?,
        Steer::Erase => edit_query(ui, caches, Search::pop)?,
    }

    Ok(())
}
