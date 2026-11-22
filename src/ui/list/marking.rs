use crate::{
    domain::message::Message,
    domain::message_ref::MessageRef,
    domain::per_account::AccountId,
    mail::steering::Ask,
    ui::list::folder_show::{relist, show},
    ui::main_screen::Ui,
    ui::run::Error,
    ui::toast::Toast,
    ui::world::World,
};

/// Mark Read on the selected Thread's Messages in their Folders.
pub fn mark_read(ui: &mut Ui, world: World) -> Result<(), Error> {
    let unread: Vec<_> = ui
        .view
        .stack()
        .filter(|m| m.unread)
        .filter_map(Message::on_server)
        .collect();
    let Some(account) = ui.view.stack().next().map(|m| m.account) else {
        return Ok(());
    };

    mark(ui, world, account, &unread, false).map(drop)
}

/// Mark Unread on the selected Thread's newest Message, or in Reading on
/// the one at the cursor.
pub fn mark_unread(ui: &mut Ui, world: World) -> Result<(), Error> {
    let Ui { view, screen } = &mut *ui;
    let message = screen.showing.reading().map_or_else(
        || view.selected_message(),
        |reading| view.stack().find(|m| m.body_key == reading.cursor),
    );

    let Some(message) = message else {
        return Ok(());
    };

    let Some(marked) = message.on_server() else {
        screen.toast = Some(Toast::new("only in another Folder"));
        return Ok(());
    };

    let (account, marked) = (message.account, [marked]);
    if let Some(key) = view.selected_key() {
        screen.dwell.spare(key);
    }

    if mark(ui, world, account, &marked, true)? {
        ui.screen.toast = Some(Toast::new("marked Unread"));
    }

    Ok(())
}

/// Sets Unread on `messages` of `account` in the Cache and the screen, and
/// asks its sync connection to tell the server; false when none changed.
fn mark(
    ui: &mut Ui,
    world: World,
    account: AccountId,
    messages: &[MessageRef],
    unread: bool,
) -> Result<bool, Error> {
    if !world.caches[account].mark(messages, unread)? {
        return Ok(false);
    }

    show(&mut ui.view, world.caches, None)?;
    relist(ui, world.caches)?;
    // Signed out, the change waits in the Cache.
    world.asks.send(account, Ask::Changes);
    Ok(true)
}
