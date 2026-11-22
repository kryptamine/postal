use std::{fmt::Write, ops::Range};

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, StatefulWidget, Widget},
};

use crate::{
    render::body::Body, render::stack_str::StackStr, ui::layout::PADDING,
    ui::list::inbox_view::InboxView, ui::preview::link_hints::LinkHints,
    ui::preview::message_header, ui::preview::preview_lines::PreviewLines, ui::theme::Theme,
    ui::widgets::border, ui::widgets::centred::centred,
};

/// Where a Preview's text goes in `pane`: inside the border, padded each
/// side and not on top.
pub const fn text_area(pane: Rect) -> Rect {
    Rect {
        x: pane.x.saturating_add(1 + PADDING),
        y: pane.y.saturating_add(1),
        width: pane.width.saturating_sub(2 * (1 + PADDING)),
        height: pane.height.saturating_sub(2),
    }
}

/// Where the body goes in a Preview's `text`: under the `header` rows,
/// which stay put while the body scrolls.
pub fn body_area(text: Rect, header: usize) -> Rect {
    let header = u16::try_from(header).unwrap_or(u16::MAX).min(text.height);
    Rect {
        y: text.y + header,
        height: text.height - header,
        ..text
    }
}

/// The body lines that `text` shows from scroll position `scroll`, under
/// `header` rows.
pub fn body_lines(scroll: usize, text: Rect, header: usize) -> Range<usize> {
    scroll..scroll.saturating_add(usize::from(body_area(text, header).height))
}

/// The selected Message's Preview inside a border titled with the Folder
/// and `i/N`: the header and a rule, then the body from the scroll position
/// on, or a placeholder in its place. In hint mode, `hints` labels its links.
pub struct PreviewPane<'a> {
    pub theme: &'a Theme,
    pub body: Body<'a>,
    pub hints: Option<&'a LinkHints>,
}

impl StatefulWidget for PreviewPane<'_> {
    type State = InboxView;

    fn render(self, area: Rect, buf: &mut Buffer, view: &mut InboxView) {
        Block::bordered()
            .border_style(self.theme.border)
            .render(area, buf);
        let rows = view.rows();
        let mut position = StackStr::<32>::new();
        (if rows > 0 {
            write!(position, " · {}/{rows} ", view.selected + 1)
        } else {
            position.write_str(" ")
        })
        .ok();

        let title = [" ", view.folder.label(), position.as_str()];
        border::label(buf, area, (area.x + 1, area.y), title, self.theme.title);

        // No rows, as when a Search matches none, leave the pane empty.
        if rows == 0 {
            return;
        }

        let text = text_area(area);
        // From the row, so the header shows while the body loads.
        if let Some(message) = view.selected_message() {
            let rows = (text.y..text.bottom()).zip(0..message_header::ROWS - 1);
            for (y, field) in rows {
                let row = Rect::new(text.x, y, text.width, 1);
                message_header::draw(self.theme, buf, row, message, field);
            }
        }

        let body = body_area(text, message_header::ROWS);
        if usize::from(text.height) >= message_header::ROWS {
            border::rule(buf, area, body.y - 1, self.theme.border);
        }

        let preview = match self.body {
            Body::Rendered(preview) => preview,
            Body::Loading => return centred("Loading…", body, buf),
            Body::Unavailable(why) => return centred(why, body, buf),
        };

        let last_top = preview.body.len().saturating_sub(usize::from(body.height));
        view.preview_scroll = view.preview_scroll.min(last_top);
        let lines = PreviewLines {
            theme: self.theme,
            preview,
            hints: self.hints,
            part: 0,
        };

        let shown = view.preview_scroll..preview.body.len();
        for (y, index) in (body.y..body.bottom()).zip(shown) {
            lines.draw(buf, Rect::new(text.x, y, text.width, 1), index);
        }
    }
}
