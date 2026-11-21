use ratatui::{Frame, Terminal, layout::Rect};
use tokio::time::Instant;

use crate::{
    accounts::Accounts,
    draft_pane::DraftPane,
    help_popup::HelpPopup,
    hint_line::HintLine,
    layout,
    local_drafts_pane::LocalDraftsPane,
    message_list::MessageList,
    mode::Mode,
    over::Layer,
    picker_popup::PickerPopup,
    preview_pane::{self, PreviewPane},
    previews::Previews,
    run::Error,
    screen::Screen,
    search::Search,
    settings_popup::SettingsPopup,
    terminal_backend::TerminalBackend,
    theme::Theme,
    thread_pane::ThreadPane,
    ui::Ui,
};

pub fn draw(
    terminal: &mut Terminal<TerminalBackend>,
    theme: &Theme,
    accounts: Accounts,
    ui: &mut Ui,
    previews: &Previews,
) -> Result<(), Error> {
    let Ui { view, screen } = ui;
    terminal
        .draw(|frame| {
            let panes = layout::split(frame.area(), screen.showing.mode());
            if let Some(list) = panes.list {
                frame.render_stateful_widget(MessageList { theme }, list, view);
            }

            if let (Some(pane), Some(reading)) = (panes.preview, screen.showing.reading_mut()) {
                let hints = screen.over.hints.as_ref();
                let widget = ThreadPane {
                    theme,
                    view,
                    previews,
                    hints,
                };

                frame.render_stateful_widget(widget, pane, reading);
            } else if let Some(pane) = panes.preview {
                let body = previews.selected(view, preview_pane::text_area(pane).width);
                let hints = screen.over.hints.as_ref();
                let widget = PreviewPane { theme, body, hints };

                frame.render_stateful_widget(widget, pane, view);
            }

            let keys = screen.keys();
            let (status, account) = screen.status();
            let hint_line =
                HintLine {
                    theme,
                    toast: screen.toast.as_ref(),
                    status,
                    account: account.map(|account| accounts.email(account)),
                    outbox: screen.outbox,
                    mode: keys,
                    query: view.search.as_ref().map(Search::query).filter(|_| {
                        !matches!(keys, Mode::Draft | Mode::LocalDrafts | Mode::Queued)
                    }),
                    now: Instant::now(),
                };

            if let Some(cursor) = hint_line.cursor(panes.hint) {
                frame.set_cursor_position(cursor);
            }

            frame.render_widget(hint_line, panes.hint);
            popups(frame, theme, accounts, panes.body, screen);
        })
        .map_err(Error::Terminal)?;
    Ok(())
}

/// What shows over the panes in `body`, the layer taking the keys on top,
/// then the `?` popup over any.
fn popups(frame: &mut Frame, theme: &Theme, accounts: Accounts, body: Rect, screen: &Screen) {
    for layer in screen.over.layers().rev() {
        match layer {
            Layer::Picker(picker) => {
                let popup = PickerPopup { theme, picker };
                frame.set_cursor_position(popup.cursor(body));
                frame.render_widget(popup, body);
            }
            Layer::LocalDrafts(local) => {
                let pane = LocalDraftsPane {
                    theme,
                    local,
                    accounts,
                };

                frame.render_widget(pane, body);
            }
            Layer::Draft(preview) => {
                // The `From` a Send adds.
                let outgoing = accounts
                    .place(preview.account.as_deref())
                    .map(|account| accounts.outgoing(account));
                let from = outgoing.map(|outgoing| {
                    let email = || outgoing.config.email.clone();
                    outgoing
                        .from()
                        .map_or_else(|_bad| email(), |from| from.to_string())
                });

                let pane = DraftPane {
                    theme,
                    preview,
                    from: from.as_deref().unwrap_or_default(),
                };

                frame.render_widget(pane, body);
            }
            Layer::Settings(settings) => {
                let popup = SettingsPopup { theme, settings };
                if let Some(cursor) = popup.cursor(body) {
                    frame.set_cursor_position(cursor);
                }

                frame.render_widget(popup, body);
            }
            // The panes draw their own labels.
            Layer::Hints => {}
        }
    }

    if screen.help {
        let popup = HelpPopup {
            theme,
            mode: screen.keys(),
            signed_out: screen.signed_out().is_some(),
        };

        frame.render_widget(popup, body);
    }
}
