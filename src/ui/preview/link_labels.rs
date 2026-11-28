use ratatui::layout::Rect;

use crate::{
    account::browser,
    render::body::Body,
    ui::list::inbox_view::InboxView,
    ui::preview::link_hints::LinkHints,
    ui::preview::message_header,
    ui::preview::preview_pane,
    ui::preview::previews::Previews,
    ui::preview::reading::Reading,
    ui::preview::stack::{self, Shape},
    ui::state::toast::Toast,
};

/// The links the Preview window shows, `None` when there are none. `text`
/// is where the Preview's text goes, if it is shown.
pub fn shown_links(
    view: &InboxView,
    reading: Option<&Reading>,
    previews: &Previews,
    text: Option<Rect>,
) -> Option<LinkHints> {
    let text = text?;
    if let Some(reading) = reading {
        return stack_links(view, previews, reading, text);
    }

    let preview = previews.get(view, text.width)?;
    let header = message_header::rows(view.selected_message());
    let lines = preview_pane::body_lines(view.preview_scroll(), text, header);
    LinkHints::find([(0, &preview.body[..], lines)])
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
        let Shape::Unfolded(Body::Rendered(preview)) = placed.shape else {
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
