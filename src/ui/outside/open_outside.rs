use std::sync::Arc;

use crate::{
    compose::editor,
    render::preview_render::{self, Job},
    render::rendered_previews::PreviewKey,
    ui::outside::message_file::MessageFile,
    ui::outside::pager,
    ui::outside::pager_text,
    ui::run::{Error, Model},
    ui::state::toast::Toast,
};

/// Where a Message opens outside the TUI.
#[derive(Clone, Copy)]
pub enum Outside {
    /// `$PAGER`, styled as the Preview shows it.
    Pager,
    /// `$EDITOR`, read-only, as text.
    Editor,
    /// The browser, as its sender made it.
    Browser,
}

/// Opens the selected Message, in Reading the one at the cursor, in `to`.
/// Returns the Toast to show instead; `None` drops any shown.
pub fn open(to: Outside, model: &mut Model<'_>) -> Result<Option<Toast>, Error> {
    let (tty, theme, world, ui) = (&mut model.tty, &*model.theme, model.world, &model.ui);
    let (view, reading) = (&ui.view, ui.screen.showing.reading());
    let message = reading.map_or_else(
        || view.selected_message(),
        |reading| view.stack().find(|m| m.body_key == reading.cursor),
    );

    let Some(message) = message else {
        return Ok(None);
    };

    let found = world.caches[message.account].body(message.body_key);
    let Some(body) = found? else {
        return Ok(Some(Toast::new("not downloaded")));
    };

    match to {
        Outside::Pager => {
            let width = tty.terminal.size().map_err(Error::Terminal)?.width;
            let job = Job {
                key: PreviewKey {
                    account: message.account,
                    body: message.body_key,
                    width,
                    view: view.html,
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
        Outside::Browser => return Ok(Some(model.originals.ask(message, &body))),
    }

    Ok(None)
}
