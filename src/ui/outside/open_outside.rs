use std::sync::Arc;

use crate::{
    compose::editor,
    domain::message::Message,
    render::preview_render::{self, Job},
    render::rendered_previews::PreviewKey,
    ui::frame::theme::Theme,
    ui::outside::message_file::MessageFile,
    ui::outside::pager,
    ui::outside::pager_text,
    ui::run::Error,
    ui::state::toast::Toast,
    ui::state::ui::Ui,
    ui::terminal::tty::Tty,
    ui::world::World,
};

/// Where a Message opens outside the TUI.
#[derive(Clone, Copy)]
pub enum Outside {
    /// `$PAGER`, styled as the Preview shows it.
    Pager,
    /// `$EDITOR`, read-only, as text.
    Editor,
}

/// The shown Message with its cached body, else the Toast to show instead:
/// `None` drops any shown.
pub type Shown<'u> = Result<(&'u Message, Vec<u8>), Option<Toast>>;

/// The selected Message, in Reading the one at the cursor.
pub fn shown<'u>(world: World<'_>, ui: &'u Ui) -> Result<Shown<'u>, Error> {
    let (view, reading) = (&ui.view, ui.screen.showing.reading());
    let message = reading.map_or_else(
        || view.selected_message(),
        |reading| view.stack().find(|m| m.body_key == reading.cursor),
    );

    let Some(message) = message else {
        return Ok(Err(None));
    };

    let found = world.caches[message.account].body(message.body_key);
    Ok(found?
        .map(|body| (message, body))
        .ok_or_else(|| Some(Toast::new("not downloaded"))))
}

/// Opens the shown Message in `to`; returns the Toast to show instead.
pub fn open(
    to: Outside,
    tty: &mut Tty,
    theme: &Theme,
    world: World<'_>,
    ui: &Ui,
) -> Result<Option<Toast>, Error> {
    let (message, body) = match shown(world, ui)? {
        Ok(found) => found,
        Err(toast) => return Ok(toast),
    };

    match to {
        Outside::Pager => {
            let width = tty.terminal.size().map_err(Error::Terminal)?.width;
            let job = Job {
                key: PreviewKey {
                    account: message.account,
                    body: message.body_key,
                    width,
                    view: ui.view.html,
                    // The pager shows the Message whole.
                    quoted: true,
                },
                body,
                look: theme.mail_look(),
                own: Arc::from(world.accounts.email(message.account)),
            };

            let text = pager_text::ansi(message, &preview_render::paged(&job), theme);
            tty.outside(|| pager::page(&text))?;
        }
        Outside::Editor => {
            let file =
                match MessageFile::write(world.accounts.email(message.account), message, &body) {
                    Ok(file) => file,
                    Err(error) => return Ok(Some(Toast::error(error.to_string()))),
                };

            tty.outside(|| editor::edit(file.path()))?;
        }
    }

    Ok(None)
}
