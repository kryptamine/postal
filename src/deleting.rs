use crate::{
    composing,
    folder::Role,
    folder_show::{relist, show},
    message::Message,
    run::Error,
    steering::Ask,
    toast::Toast,
    ui::Ui,
    undo::Undo,
    world::World,
};

/// Delete on the selected Thread's Messages in their Folders, or in
/// Reading on the one at the cursor.
pub fn delete(ui: &mut Ui, world: World) -> Result<(), Error> {
    let (caches, Ui { view, screen }) = (world.caches, &mut *ui);
    // A Search never shows Trash.
    if view.search.is_none() && view.folder.role() == Some(Role::Trash) {
        screen.toast = Some(Toast::new("already in Trash"));
        return Ok(());
    }

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
    let Some(batch) = caches[account].delete(&messages)? else {
        return Ok(());
    };

    screen.undo.push(Undo::Delete(account, batch, thread));
    show(view, caches, None)?;
    screen.over.hints = None;
    screen.follow(view, thread);

    relist(ui, caches)?;
    // Signed out, the move waits in the Cache.
    world.asks.send(account, Ask::Changes);
    ui.screen.toast = Some(Toast::new("deleted · u to undo"));
    Ok(())
}

/// Undo of the latest Delete not yet taken back, selecting its Thread, or
/// of the latest Draft discarded, showing it again.
pub fn undo(ui: &mut Ui, world: World) -> Result<(), Error> {
    let (caches, Ui { view, screen }) = (world.caches, &mut *ui);
    let folders = match screen.undo.pop() {
        Some(Undo::Discard(path, text, notes)) => {
            composing::restore(screen, path, text, notes);
            return Ok(());
        }
        Some(Undo::Delete(account, batch, thread)) => {
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
    screen.over.hints = None;
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
        .is_some_and(|shown| folders.iter().any(|f| **f == *shown));

    let here = view.search.is_some() || shown;
    screen.toast = Some(if here {
        Toast::new("restored")
    } else {
        let folder = caches[account].folder(&folders[0])?;
        Toast::new(format!("restored to {}", folder.label()))
    });
    Ok(())
}
