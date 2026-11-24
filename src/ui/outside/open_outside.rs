use crate::{
    compose::editor,
    domain::message::Message,
    render::preview_render::{self, Job},
    render::rendered_previews::PreviewKey,
    render::view::View,
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

/// Opens the selected Message, in Reading the one at the cursor, in `to`.
/// Returns the Toast to show instead; `None` drops any shown.
pub fn open(
    to: Outside,
    tty: &mut Tty,
    theme: &Theme,
    world: World<'_>,
    ui: &Ui,
) -> Result<Option<Toast>, Error> {
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
            let text = paged(message, body, width, view.html, theme);
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

/// The bytes piped to the pager: the Preview laid out in `width` columns,
/// in the View `html` says, its Quote shown.
fn paged(message: &Message, body: Vec<u8>, width: u16, html: View, theme: &Theme) -> Vec<u8> {
    let preview = preview_render::paged(&Job {
        key: PreviewKey {
            account: message.account,
            body: message.body_key,
            width,
            view: html,
            // The pager shows the Message whole.
            quoted: true,
        },
        body,
        look: theme.mail_look(),
    });

    pager_text::ansi(message, &preview, theme)
}
