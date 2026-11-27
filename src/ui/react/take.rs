use ratatui::layout::Rect;

use crate::{
    account::config::ContactsChoice,
    render::icons::Icons,
    ui::drafts::drafting,
    ui::drafts::forwarding,
    ui::drafts::local_list::open_local,
    ui::error::Error,
    ui::exit::Exit,
    ui::input::step::Live,
    ui::input::step::Step,
    ui::outside::open_outside,
    ui::popup::settings::Settings,
    ui::popup::signature_edit,
    ui::preview::attachment_hints::AttachmentHints,
    ui::preview::link_labels::{browse, label_links},
    ui::react::steer::steer,
    ui::run::Model,
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
        Step::Attachments(purpose) => {
            let message = ui.view.at_cursor(ui.screen.showing.cursor());
            match message.map(|message| AttachmentHints::on(message, purpose)) {
                Some(Ok(hints)) => ui.screen.over.open(hints),
                Some(Err(toast)) => ui.screen.toast = Some(toast),
                None => {}
            }
        }
        Step::Attachment(wanted) => {
            ui.screen.toast = Some(model.parts.ask(wanted, &model.fetch));
        }
        Step::Open(to) => {
            let tty = &mut model.tty;
            ui.screen.toast = open_outside::open(to, tty, model.theme, world, ui)?;
        }
        Step::Original => {
            ui.screen.toast = match open_outside::shown(world, ui)? {
                Ok((message, body)) => Some(model.parts.original(message, &body, &model.fetch)),
                Err(toast) => toast,
            };
        }
        Step::Draft(drafting) => {
            drafting::draft(drafting, &mut model.tty, ui, world, &model.fetch)?;
        }
        Step::Reply(to) => drafting::reply(to, &mut model.tty, ui, world, &model.fetch)?,
        Step::Forward => forwarding::forward(&mut model.tty, ui, world, &model.fetch)?,
        Step::LocalDrafts => open_local(&mut ui.screen, world.outbox),
        Step::Settings => match Settings::open(world.config_dir) {
            Ok(settings) => ui.screen.over.open(settings),
            Err(error) => ui.screen.toast = Some(Toast::error(error.to_string())),
        },
        Step::Signature(email) => {
            signature_edit::open(&mut model.tty, &mut ui.screen, world.config_dir, &email)?;
        }
        Step::Live(live) => match live {
            Live::Icons(choice) => {
                model.theme.icons = Icons::choose(choice);
                model.previews.set_look(model.theme.mail_look());
            }
            Live::Notify(choice) => model.notifier.notify = choice,
            Live::Contacts(choice) => {
                model.world.contacts = choice;
                if choice != ContactsChoice::None {
                    ui.screen.contacts_check.start(choice);
                }
            }
        },
        Step::Exit(exit) => return Ok(Some(exit)),
    }

    Ok(None)
}
