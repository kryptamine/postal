use ratatui::Terminal;

use crate::{
    cache::Cache,
    editor,
    inbox_view::InboxView,
    input::Input,
    message::Message,
    message_file::MessageFile,
    pager, pager_text,
    preview_render::{self, Job},
    reading::Reading,
    rendered_previews::PreviewKey,
    run::{self, Error},
    terminal_backend::TerminalBackend,
    theme::Theme,
    toast::Toast,
    view::View,
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
    (terminal, events): (&mut Terminal<TerminalBackend>, &mut Input),
    theme: &Theme,
    caches: &[Cache],
    (view, reading): (&InboxView, Option<&Reading>),
) -> Result<Option<Toast>, Error> {
    let message = reading.map_or_else(
        || view.selected_message(),
        |reading| view.stack().find(|m| m.body_key == reading.cursor),
    );

    let Some(message) = message else {
        return Ok(None);
    };

    let found = caches[message.account].body(message.body_key);
    let Some(body) = found.map_err(Error::Cache)? else {
        return Ok(Some(Toast::new("not downloaded")));
    };

    match to {
        Outside::Pager => {
            let width = terminal.size().map_err(Error::Terminal)?.width;
            let text = paged(message, body, (width, view.html), theme);
            run::outside(terminal, events, || pager::page(&text))?;
        }
        Outside::Editor => {
            let file = match MessageFile::write(message, &body) {
                Ok(file) => file,
                Err(error) => return Ok(Some(Toast::error(error.to_string()))),
            };

            run::outside(terminal, events, || editor::edit(file.path()))?;
        }
    }

    Ok(None)
}

/// The bytes piped to the pager: the Preview laid out in `width` columns,
/// in the View `html` says, its Quote shown.
fn paged(message: &Message, body: Vec<u8>, (width, html): (u16, View), theme: &Theme) -> Vec<u8> {
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
