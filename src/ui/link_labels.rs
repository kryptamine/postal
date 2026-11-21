use ratatui::layout::Rect;

use crate::{
    account::browser, render::body::Body, ui::inbox_view::InboxView, ui::link_hints::LinkHints,
    ui::main_screen::Ui, ui::message_header, ui::preview_pane, ui::previews::Previews,
    ui::reading::Reading, ui::stack, ui::toast::Toast,
};

/// Enters hint mode on the links the Preview window shows, or says there
/// are none. `text` is where the Preview's text goes, if it is shown.
pub fn label_links(ui: &mut Ui, previews: &Previews, text: Option<Rect>) {
    let Ui { view, screen } = ui;
    screen.over.hints = text.and_then(|text| {
        if let Some(reading) = screen.showing.reading() {
            return stack_links(view, previews, reading, text);
        }

        let preview = previews.get(view, text.width)?;
        let lines = preview_pane::body_lines(view.preview_scroll, text, message_header::ROWS);
        LinkHints::find([(0, &preview.body[..], lines)])
    });

    if screen.over.hints.is_none() {
        screen.toast = Some(Toast::new("no links"));
    }
}

/// The links of every unfolded body of the stack that `text` shows.
fn stack_links(
    view: &InboxView,
    previews: &Previews,
    reading: &Reading,
    text: Rect,
) -> Option<LinkHints> {
    let window = reading.scroll..reading.scroll + usize::from(text.height);
    let bodies = stack::placed(view, previews, reading, text.width).filter_map(|placed| {
        let Some(Body::Rendered(preview)) = placed.body else {
            return None;
        };

        // Links under a pinned header cannot be seen.
        let shown = window.start + placed.pinned(&window)..window.end;
        Some((placed.index, &preview.body[..], placed.body_lines(&shown)))
    });

    LinkHints::find(bodies)
}

/// Opens `url`; returns the Toast to show when that fails.
pub fn browse(url: &str) -> Option<Toast> {
    let error = browser::open(url).err()?;
    tracing::error!("opening a link failed: {error}");
    Some(Toast::new("could not open the link"))
}
