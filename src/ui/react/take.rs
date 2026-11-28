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
    ui::preview::link_labels::{browse, shown_links},
    ui::react::steer::steer,
    ui::run::Model,
    ui::state::toast::Toast,
};

/// Does what one input event's `step` asks; the `Exit` when it ends the TUI.
pub fn take(model: &mut Model, step: Step, text: Option<Rect>) -> Result<Option<Exit>, Error> {
    let (ui, world) = (&mut model.ui, model.world);
    match step {
        Step::Stay => {}
        Step::Links => {
            let reading = ui.screen.showing.reading();
            let hints = shown_links(&ui.view, reading, model.previews, text);
            ui.screen
                .open_or_toast(hints.ok_or_else(|| Toast::new("no links")));
        }
        Step::Steer(to) => steer(to, ui, world)?,
        Step::Browse(url) => ui.screen.toast = browse(&url),
        Step::Attachments(purpose) => {
            if let Some(message) = ui.view.at_cursor(ui.screen.showing.cursor()) {
                ui.screen
                    .open_or_toast(AttachmentHints::on(message, purpose));
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
        Step::Settings => {
            let settings = Settings::open(world.config_dir);
            let settings = settings.map_err(|error| Toast::error(error.to_string()));
            ui.screen.open_or_toast(settings);
        }
        Step::Signature(email) => {
            signature_edit::open(&mut model.tty, &mut ui.screen, world.config_dir, &email)?;
        }
        Step::Live(live) => apply(model, live),
        Step::Exit(exit) => return Ok(Some(exit)),
    }

    Ok(None)
}

/// A setting changed in Settings, on the running screen at once.
fn apply(model: &mut Model, live: Live) {
    match live {
        Live::Icons(choice) => {
            model.theme.icons = Icons::choose(choice);
            model.previews.set_look(model.theme.mail_look());
        }
        Live::Avatars(choice) => model.theme.avatars = choice,
        Live::Notify(choice) => model.notifier.notify = choice,
        Live::Contacts(choice) => {
            model.world.contacts = choice;
            if choice != ContactsChoice::None {
                model.ui.screen.contacts_check.start(choice);
            }
        }
    }
}
