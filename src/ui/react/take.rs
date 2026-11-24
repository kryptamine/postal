use ratatui::layout::Rect;

use crate::{
    render::icons::Icons,
    ui::drafts::drafting,
    ui::drafts::forwarding,
    ui::drafts::local_list::open_local,
    ui::exit::Exit,
    ui::input::step::Step,
    ui::outside::open_outside,
    ui::popup::settings::Settings,
    ui::popup::signature_edit,
    ui::preview::attachment_hints::label_attachments,
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
        Step::Attachments(purpose) => label_attachments(ui, purpose),
        Step::Attachment(wanted) => ui.screen.toast = Some(model.attachments.ask(wanted)),
        Step::Open(to) => {
            let tty = &mut model.tty;
            ui.screen.toast = open_outside::open(to, tty, model.theme, world, ui)?;
        }
        Step::Draft(drafting) => drafting::draft(drafting, &mut model.tty, ui, world)?,
        Step::Reply(to) => drafting::reply(to, &mut model.tty, ui, world, &model.replies)?,
        Step::Forward => forwarding::forward(&mut model.tty, ui, world, &model.forwards)?,
        Step::LocalDrafts => open_local(&mut ui.screen, world.outbox),
        Step::Settings => match Settings::open(world.config_dir) {
            Ok(settings) => ui.screen.over.open(settings),
            Err(error) => ui.screen.toast = Some(Toast::error(error.to_string())),
        },
        Step::Signature(email) => {
            signature_edit::open(&mut model.tty, &mut ui.screen, world.config_dir, &email)?;
        }
        Step::Icons(choice) => {
            model.theme.icons = Icons::choose(choice);
            model.previews.set_look(model.theme.mail_look());
        }
        Step::Notify(choice) => model.notifier.notify = choice,
        Step::Exit(exit) => return Ok(Some(exit)),
    }

    Ok(None)
}
