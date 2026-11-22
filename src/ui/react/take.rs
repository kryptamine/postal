use ratatui::layout::Rect;

use crate::{
    render::icons::Icons,
    ui::drafts::drafting,
    ui::drafts::local_list::open_local,
    ui::exit::Exit,
    ui::input::step::Step,
    ui::outside::open_outside,
    ui::popup::settings::Settings,
    ui::preview::link_labels::{browse, label_links},
    ui::react::steer::steer,
    ui::run::{Error, Model},
    ui::state::toast::Toast,
};

/// Does what one input event's `step` asks; the `Exit` when it ends the TUI.
pub fn take(model: &mut Model, step: Step, text: Option<Rect>) -> Result<Option<Exit>, Error> {
    let (ui, world) = (&mut model.ui, model.world);
    match step {
        Step::Stay => {}
        Step::Links => label_links(ui, model.previews, text),
        Step::Steer(to) => steer(to, ui, world)?,
        Step::Browse(url) => ui.screen.toast = browse(&url),
        Step::Open(to) => {
            let tty = &mut model.tty;
            ui.screen.toast = open_outside::open(to, tty, model.theme, world.caches, ui)?;
        }
        Step::Draft(drafting) => drafting::draft(drafting, &mut model.tty, ui, world)?,
        Step::Reply(to) => drafting::reply(to, &mut model.tty, ui, world, &model.replies)?,
        Step::LocalDrafts => open_local(&mut ui.screen, world.outbox),
        Step::Settings => match Settings::open(world.config_dir) {
            Ok(settings) => ui.screen.over.open(settings),
            Err(error) => ui.screen.toast = Some(Toast::error(error.to_string())),
        },
        Step::Icons(choice) => {
            model.theme.icons = Icons::choose(choice);
            model.previews.set_look(model.theme.mail_look());
        }
        Step::Exit(exit) => return Ok(Some(exit)),
    }

    Ok(None)
}
