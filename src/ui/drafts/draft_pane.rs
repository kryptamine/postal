use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Widget, Wrap},
};

use crate::{
    render::styled_line::StyledLine, ui::drafts::draft_preview::DraftPreview,
    ui::frame::layout::PADDING, ui::frame::theme::Theme, ui::widgets::border,
};

/// A Draft over the whole body: what went wrong first, then the headers
/// Postal parsed and the body; a Draft that does not parse shows as written.
pub struct DraftPane<'a> {
    pub theme: &'a Theme,
    pub preview: &'a DraftPreview,
    /// The `From` Postal adds, as it will read.
    pub from: &'a str,
    /// The body as its HTML is laid out, in place of its text.
    pub rendered: Option<&'a [StyledLine]>,
}

impl Widget for DraftPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        Block::bordered()
            .border_style(self.theme.border)
            .render(area, buf);
        let title = if self.preview.queued {
            " Outbox "
        } else {
            " Draft "
        };

        border::label(buf, area, (area.x + 1, area.y), [title], self.theme.title);
        let text = Rect {
            x: area.x.saturating_add(1 + PADDING),
            y: area.y.saturating_add(1),
            width: text_width(area),
            height: area.height.saturating_sub(2),
        };

        Paragraph::new(self.lines())
            .wrap(Wrap { trim: false })
            .render(text, buf);
    }
}

/// The columns the pane over `area` writes in, inside its border.
pub const fn text_width(area: Rect) -> u16 {
    area.width.saturating_sub(2 * (1 + PADDING))
}

impl DraftPane<'_> {
    fn lines(&self) -> Vec<Line<'_>> {
        let preview = self.preview;
        let mut lines = Vec::new();
        let error = preview.draft.as_ref().err().or(preview.failure.as_ref());
        if let Some(error) = error {
            lines.push(Line::styled(error.as_str(), self.theme.error));
        }

        if !lines.is_empty() {
            lines.push(Line::default());
        }

        let Ok(draft) = &preview.draft else {
            lines.extend(preview.text.lines().map(Line::raw));
            return lines;
        };

        let field = |label, value: String| {
            Line::from(vec![
                Span::styled(label, self.theme.field),
                Span::styled(value, Style::new()),
            ])
        };

        lines.push(field("From    ", self.from.to_owned()));
        lines.push(field("To      ", draft.to.to_string()));
        if draft.cc.iter().next().is_some() {
            lines.push(field("Cc      ", draft.cc.to_string()));
        }

        lines.push(field("Subject ", draft.subject.clone()));
        for attach in &draft.attached {
            lines.push(field("Attach  ", attach.written.clone()));
        }

        lines.push(Line::default());
        let Some(rendered) = self.rendered else {
            lines.extend(draft.body.lines().map(Line::raw));
            return lines;
        };

        for row in rendered {
            let spans = row.segments().map(|segment| {
                let style = self.theme.painted(segment.marks, segment.paint);
                Span::styled(segment.text, style)
            });

            lines.push(Line::from(spans.collect::<Vec<_>>()));
        }

        lines
    }
}
