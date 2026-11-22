use crate::{
    cache::Cache, domain::per_account::PerAccount, mail::status::Status, mail::update::BodyLink,
    ui::list::folder_show::show, ui::list::inbox_view::InboxView, ui::list::message_list,
    ui::main_screen::Ui, ui::mode::Mode, ui::preview::previews::Previews, ui::run::Error,
    ui::screen::Screen, ui::terminal::tty::Tty, ui::world::World,
};

/// The screen as the TUI opens: as many rows of the list from the Cache as
/// the first frame shows, each Account's Status and what failed before it
/// opened.
pub fn opening(
    world: World,
    tty: &Tty,
    statuses: PerAccount<Status>,
    failure: Option<&'static str>,
    previews: &mut Previews,
) -> Result<Ui, Error> {
    let list = tty.panes(Mode::List)?.list;
    let rows = message_list::capacity(list.map_or(0, |list| list.height));
    let emails = world.accounts.0.map(|config| config.email.as_str().into());
    let mut view = InboxView::loading(emails);
    show(&mut view, world.caches, Some(rows))?;
    let rebuilt = world.caches.values().any(Cache::rebuilt);
    let mut screen = Screen::opening(rebuilt, statuses, failure);
    screen.outbox = world.outbox.count();
    for (account, &status) in screen.statuses.iter() {
        (status == Status::SignedOut).then(|| previews.link(account, BodyLink::Down));
    }

    Ok(Ui { view, screen })
}
