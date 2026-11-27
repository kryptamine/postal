use crate::{
    cache::Goal,
    domain::body_key::BodyKey,
    domain::folder::{Folder, Role},
    domain::message::Message,
    domain::message_ref::MessageRef,
    domain::per_account::AccountId,
    domain::thread::Key,
    mail::steering::Ask,
    ui::error::Error,
    ui::list::folder_show::{folders, relist, show},
    ui::list::per_account::per_account,
    ui::popup::folder_picker::FolderPicker,
    ui::preview::link_hints::LinkHints,
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
        ui.screen.set_toast(Toast::new("already in Trash"));
        return Ok(());
    }

    take(ui, world, &Goal::Role(Role::Trash), "deleted")
}

/// Archive, as `delete` takes Messages to Trash; on Gmail into All Mail.
pub fn archive(ui: &mut Ui, world: World) -> Result<(), Error> {
    let shown = ui.view.folder.role();
    if ui.view.search.is_none() && matches!(shown, Some(Role::Archive | Role::All)) {
        ui.screen.set_toast(Toast::new("already archived"));
        return Ok(());
    }

    take(ui, world, &Goal::Role(Role::Archive), "archived")
}

/// Opens the Folder Picker on the Folders of the selected Thread's Account,
/// or of the Picks', to move their Messages to the Folder picked.
pub fn pick(ui: &mut Ui, world: World) -> Result<(), Error> {
    let Ui { view, screen } = ui;
    if view.search.is_none() && view.folder.role() == Some(Role::All) {
        screen.set_toast(Toast::new("All Mail keeps every Message"));
        return Ok(());
    }

    let accounts = view.picked_accounts();
    if accounts.len() > 1 {
        screen.set_toast(Toast::new("pick one Account's Threads to move"));
        return Ok(());
    }

    let first = || view.stack().next().map(|m| m.account);
    let Some(account) = accounts.into_iter().next().or_else(first) else {
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
/// cursor, or those of every Pick, to `goal`, for one Undo to bring back;
/// `done` says so.
fn take(ui: &mut Ui, world: World, goal: &Goal, done: &str) -> Result<(), Error> {
    let caches = world.caches;
    let picked = ui.view.picked_listing(caches)?;
    let Ui { view, screen } = &mut *ui;
    let (taken, threads) = if let Some(listing) = &picked {
        let each = |thread| listing.stack(Some(thread));
        (per_account(listing, each), Some(listing.threads().len()))
    } else {
        let cursor = screen.showing().cursor();
        let Some(taken) = shown(view.stack(), cursor) else {
            if cursor.is_some() {
                screen.set_toast(Toast::new("only in another Folder"));
            }

            return Ok(());
        };

        (vec![taken], None)
    };

    let mut batches = Vec::new();
    for (account, messages) in &taken {
        if let Some(batch) = caches[*account].move_to(messages, goal)? {
            batches.push((*account, batch));
        }
    }

    if batches.is_empty() {
        screen.set_toast(Toast::new("nothing to move"));
        return Ok(());
    }

    let thread = view.selected_key();
    view.picks.clear();
    let accounts: Vec<_> = batches.iter().map(|&(account, _)| account).collect();
    screen.push_undo(Undo::Move(batches, thread));
    show(view, caches, None)?;
    screen.over.close::<LinkHints>();
    screen.follow(view, thread);

    relist(ui, caches)?;
    for account in accounts {
        // Signed out, the move waits in the Cache.
        world.asks.send(account, Ask::Changes);
    }

    let toast = threads.map_or_else(
        || format!("{done} · u to undo"),
        |threads| format!("{threads} {done} · u to undo"),
    );
    ui.screen.set_toast(Toast::new(toast));
    Ok(())
}

/// The Messages of `stack` on the server, or only the one at `cursor`,
/// with their Account; `None` when there are none.
fn shown<'a>(
    stack: impl Iterator<Item = &'a Message> + Clone,
    cursor: Option<BodyKey>,
) -> Option<(AccountId, Vec<MessageRef>)> {
    // A Thread never spans Accounts.
    let account = stack.clone().next()?.account;
    let messages: Vec<_> = stack
        .filter(|m| cursor.is_none_or(|cursor| m.body_key == cursor))
        .filter_map(Message::on_server)
        .collect();
    (!messages.is_empty()).then_some((account, messages))
}

/// Undo of a move: each Account's `batches` back where they were, the
/// Thread `thread` selected.
pub fn undo(
    ui: &mut Ui,
    world: World,
    batches: Vec<(AccountId, i64)>,
    thread: Option<Key>,
) -> Result<(), Error> {
    let (caches, Ui { view, screen }) = (world.caches, &mut *ui);
    let mut restored = Vec::new();
    for (account, batch) in batches {
        let folders = caches[account].undo(batch)?;
        if !folders.is_empty() {
            restored.push((account, folders));
        }
    }

    if restored.is_empty() {
        screen.set_toast(Toast::new("nothing to undo"));
        return Ok(());
    }

    show(view, caches, None)?;
    screen.over.close::<LinkHints>();
    if let Some(key) = thread {
        screen.follow(view, thread);
        view.select(key);
    }

    relist(ui, caches)?;
    for (account, _) in &restored {
        world.asks.send(*account, Ask::Changes);
    }

    let Ui { view, screen } = ui;
    let shown = restored.iter().any(|(account, folders)| {
        let shown = view.folder.of(*account);
        shown.is_some_and(|shown| folders.iter().any(|f| f == shown))
    });

    let here = view.search.is_some() || shown;
    screen.set_toast(if here {
        Toast::new("restored")
    } else {
        let (account, folders) = &restored[0];
        let folder = caches[*account].folder(&folders[0])?;
        Toast::new(format!("restored to {}", folder.label()))
    });
    Ok(())
}
