use crate::{
    cache::Goal,
    domain::folder::{Folder, Role},
    domain::message::Message,
    mail::steering::Ask,
    ui::drafts::composing,
    ui::list::folder_show::{folders, relist, show},
    ui::popup::folder_picker::FolderPicker,
    ui::preview::link_hints::LinkHints,
    ui::run::Error,
    ui::state::toast::Toast,
    ui::state::ui::Ui,
    ui::state::undo::Undo,
    ui::world::World,
};

/// Delete on the selected Thread's Messages in their Folders, or in
/// Reading on the one at the cursor.
pub fn delete(ui: &mut Ui, world: World) -> Result<(), Error> {
    // A Search never shows Trash.
    if ui.view.search.is_none() && ui.view.folder.role() == Some(Role::Trash) {
        ui.screen.toast = Some(Toast::new("already in Trash"));
        return Ok(());
    }

    take(ui, world, &Goal::Role(Role::Trash), "deleted")
}

/// Archive, as `delete` takes Messages to Trash; on Gmail into All Mail.
pub fn archive(ui: &mut Ui, world: World) -> Result<(), Error> {
    let shown = ui.view.folder.role();
    if ui.view.search.is_none() && matches!(shown, Some(Role::Archive | Role::All)) {
        ui.screen.toast = Some(Toast::new("already archived"));
        return Ok(());
    }

    take(ui, world, &Goal::Role(Role::Archive), "archived")
}

/// Opens the Folder Picker on the Folders of the selected Thread's Account,
/// to move its Messages to the one picked.
pub fn pick(ui: &mut Ui, world: World) -> Result<(), Error> {
    let Ui { view, screen } = ui;
    if view.search.is_none() && view.folder.role() == Some(Role::All) {
        screen.toast = Some(Toast::new("All Mail keeps every Message"));
        return Ok(());
    }

    let Some(account) = view.stack().next().map(|m| m.account) else {
        return Ok(());
    };

    let picker = FolderPicker::moving(folders(world.caches)?, &view.folder, account);
    screen.over.open(picker);
    world.asks.send(account, Ask::ListFolders);
    Ok(())
}

/// Moves the Messages `delete` would take to `folder`.
pub fn move_to(ui: &mut Ui, world: World, folder: &Folder) -> Result<(), Error> {
    let done = format!("moved to {}", folder.label());
    take(ui, world, &Goal::Folder(folder.name.clone()), &done)
}

/// Takes the selected Thread's Messages, or in Reading the one at the
/// cursor, to `goal`, for an Undo to bring back; `done` says so.
fn take(ui: &mut Ui, world: World, goal: &Goal, done: &str) -> Result<(), Error> {
    let (caches, Ui { view, screen }) = (world.caches, &mut *ui);
    let cursor = screen.showing.reading().map(|reading| reading.cursor);
    let messages: Vec<_> = view
        .stack()
        .filter(|m| cursor.is_none_or(|cursor| m.body_key == cursor))
        .filter_map(Message::on_server)
        .collect();
    if messages.is_empty() {
        if cursor.is_some() {
            screen.toast = Some(Toast::new("only in another Folder"));
        }

        return Ok(());
    }

    // A Thread never spans Accounts.
    let Some(account) = view.stack().next().map(|m| m.account) else {
        return Ok(());
    };

    let thread = view.selected_key();
    let Some(batch) = caches[account].move_to(&messages, goal)? else {
        screen.toast = Some(Toast::new("nothing to move"));
        return Ok(());
    };

    screen.undo.push(Undo::Move(account, batch, thread));
    show(view, caches, None)?;
    screen.over.close::<LinkHints>();
    screen.follow(view, thread);

    relist(ui, caches)?;
    // Signed out, the move waits in the Cache.
    world.asks.send(account, Ask::Changes);
    ui.screen.toast = Some(Toast::new(format!("{done} · u to undo")));
    Ok(())
}

/// Undo of the latest move not yet taken back, selecting its Thread, or
/// of the latest Draft discarded, showing it again.
pub fn undo(ui: &mut Ui, world: World) -> Result<(), Error> {
    let (caches, Ui { view, screen }) = (world.caches, &mut *ui);
    let folders = match screen.undo.pop() {
        Some(Undo::Discard(path, text, notes)) => {
            composing::restore(screen, path, text, notes);
            return Ok(());
        }
        Some(Undo::Move(account, batch, thread)) => {
            let folders = caches[account].undo(batch)?;
            (!folders.is_empty()).then_some((account, folders, thread))
        }
        None => None,
    };

    let Some((account, folders, thread)) = folders else {
        screen.toast = Some(Toast::new("nothing to undo"));
        return Ok(());
    };

    show(view, caches, None)?;
    screen.over.close::<LinkHints>();
    if let Some(key) = thread {
        screen.follow(view, thread);
        view.select(key);
    }

    relist(ui, caches)?;
    world.asks.send(account, Ask::Changes);
    let Ui { view, screen } = ui;
    let shown = view
        .folder
        .of(account)
        .is_some_and(|shown| folders.iter().any(|f| f == shown));

    let here = view.search.is_some() || shown;
    screen.toast = Some(if here {
        Toast::new("restored")
    } else {
        let folder = caches[account].folder(&folders[0])?;
        Toast::new(format!("restored to {}", folder.label()))
    });
    Ok(())
}
