use ratatui::{Frame, Terminal, layout::Rect};
use tokio::time::Instant;
use unicode_width::UnicodeWidthStr;

use crate::{
    compose::accounts::Accounts,
    ui::frame::layout,
    ui::frame::theme::Theme,
    ui::list::completion_menu::CompletionMenu,
    ui::list::message_list::MessageList,
    ui::list::search::Search,
    ui::popup::help_popup::HelpPopup,
    ui::preview::attachment_hints::AttachmentHints,
    ui::preview::link_hints::LinkHints,
    ui::preview::preview_pane::{self, PreviewPane},
    ui::preview::previews::Previews,
    ui::preview::thread_pane::ThreadPane,
    ui::run::Error,
    ui::state::mode::Mode,
    ui::state::screen::Screen,
    ui::state::ui::Ui,
    ui::terminal::terminal_backend::TerminalBackend,
    ui::widgets::hint_line::HintLine,
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
                let widget = ThreadPane {
                    theme,
                    view,
                    previews,
                    hints: screen.over.get::<LinkHints>(),
                    attachment_hints: screen.over.get::<AttachmentHints>(),
                };

                frame.render_stateful_widget(widget, pane, reading);
            } else if let Some(pane) = panes.preview {
                let body = previews.selected(view, preview_pane::text_area(pane).width);
                let widget = PreviewPane {
                    theme,
                    body,
                    hints: screen.over.get::<LinkHints>(),
                    attachment_hints: screen.over.get::<AttachmentHints>(),
                };

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
                    parsed: view.search.as_ref().map(Search::parsed),
                    capped: view.search.as_ref().is_some_and(Search::capped),
                    indexing: screen.index_progress(),
                    picks: if keys == Mode::List {
                        view.count_picked()
                    } else {
                        0
                    },
                    now: Instant::now(),
                };

            if let Some(cursor) = hint_line.cursor(panes.hint) {
                frame.set_cursor_position(cursor);
            }

            frame.render_widget(hint_line, panes.hint);
            if let Some(search) = view.search.as_ref().filter(|_| keys.typing()) {
                frame.render_widget(menu(theme, search, panes.hint), frame.area());
            }

            popups(frame, theme, accounts, panes.body, screen);
        })
        .map_err(Error::Terminal)?;
    Ok(())
}

/// The Search's suggestions over the hint line `hint`, from the word they
/// complete.
fn menu<'a>(theme: &'a Theme, search: &'a Search, hint: Rect) -> CompletionMenu<'a> {
    let completion = search.completion();
    let typed = search.query().get(..completion.at()).unwrap_or_default();
    let typed = u16::try_from(typed.width()).unwrap_or(u16::MAX);
    CompletionMenu {
        theme,
        completion,
        // The hint line's margin and the `/` before the Query.
        x: hint.x.saturating_add(2).saturating_add(typed),
        above: hint.y,
    }
}

/// What shows over the panes in `body`, the layer taking the keys on top,
/// then the `?` popup over any.
fn popups(frame: &mut Frame, theme: &Theme, accounts: Accounts, body: Rect, screen: &mut Screen) {
    for layer in screen.over.layers() {
        layer.draw(frame, theme, accounts, body);
    }

    let (mode, signed_out) = (screen.keys(), screen.signed_out().is_some());
    if let Some(help) = &mut screen.help {
        let popup = HelpPopup {
            theme,
            mode,
            signed_out,
        };

        frame.render_stateful_widget(popup, body, help);
        if let Some(cursor) = help.cursor {
            frame.set_cursor_position(cursor);
        }
    }
}
