use crate::{
    domain::message::Message,
    domain::message_ref::MessageRef,
    domain::per_account::AccountId,
    mail::steering::Ask,
    ui::error::Error,
    ui::list::folder_show::{relist, show},
    ui::list::per_account::per_account,
    ui::state::toast::Toast,
    ui::state::ui::Ui,
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
/// the one at the cursor. On the Picks, Read when any is Unread, else
/// Unread on each one's newest Message.
pub fn mark_unread(ui: &mut Ui, world: World) -> Result<(), Error> {
    if let Some(listing) = ui.view.picked_listing(world.caches)? {
        let threads = listing.threads();
        let unread = !threads.iter().any(|t| t.unread);
        let marked = if unread {
            per_account(&listing, |thread| listing.newest(thread).into_iter())
        } else {
            let each = |thread| listing.stack(Some(thread)).filter(|m| m.unread);
            per_account(&listing, each)
        };

        let mut changed = false;
        for (account, messages) in &marked {
            changed |= mark(ui, world, *account, messages, unread)?;
        }

        ui.view.unpick();
        if let Some(key) = ui.view.selected_key().filter(|_| unread) {
            ui.screen.dwell.spare(key);
        }

        let done = if unread { "Unread" } else { "Read" };
        if changed {
            ui.screen.toast = Some(Toast::new(format!("{} marked {done}", threads.len())));
        }

        return Ok(());
    }

    let Ui { view, screen } = &mut *ui;
    let message = view.at_cursor(screen.showing.cursor());

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
