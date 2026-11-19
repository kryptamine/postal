use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Clear, Paragraph, Widget, Wrap},
};

use crate::{border, draft_preview::DraftPreview, layout::PADDING, theme::Theme};

/// A Draft over the whole body: what went wrong first, then the headers
/// Postal parsed and the body; a Draft that does not parse shows as written.
pub struct DraftPane<'a> {
    pub theme: &'a Theme,
    pub preview: &'a DraftPreview,
    /// The `From` Postal adds, as it will read.
    pub from: &'a str,
}

impl Widget for DraftPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        Block::bordered()
            .border_style(self.theme.border)
            .render(area, buf);
        border::label(
            buf,
            area,
            (area.x + 1, area.y),
            &[" Draft "],
            self.theme.title,
        );
        let text = Rect {
            x: area.x.saturating_add(1 + PADDING),
            y: area.y.saturating_add(1),
            width: area.width.saturating_sub(2 * (1 + PADDING)),
            height: area.height.saturating_sub(2),
        };

        Paragraph::new(self.lines())
            .wrap(Wrap { trim: false })
            .render(text, buf);
    }
}

impl DraftPane<'_> {
    fn lines(&self) -> Vec<Line<'_>> {
        let preview = self.preview;
        let mut lines = Vec::new();
        if preview.sending {
            lines.push(Line::styled("sending…", self.theme.muted));
        }

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
        lines.push(Line::default());
        lines.extend(draft.body.lines().map(Line::raw));
        lines
    }
}
