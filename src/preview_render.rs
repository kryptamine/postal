use chrono::{DateTime, FixedOffset};
use mail_parser::{MessageParser, PartType};

use crate::{
    html_preview::{self, Links},
    preview::Preview,
    rendered_previews::PreviewKey,
    styled_line::StyledLine,
    wrap::wrap,
};

/// What rendering one Preview needs, sent to the thread that renders it.
pub struct Job {
    pub key: PreviewKey,
    /// `BODY[]`: the header and every part, still MIME-encoded.
    pub body: Vec<u8>,
    pub sender: String,
    pub subject: String,
    pub received: DateTime<FixedOffset>,
}

/// The Preview of `job` at the width in its key.
pub fn render(job: Job) -> Preview {
    rendered(job, Links::Inline)
}

/// `job` as the `$PAGER` shows it: the Preview with link footnotes.
pub fn paged(job: Job) -> Preview {
    rendered(job, Links::Footnotes)
}

fn rendered(job: Job, links: Links) -> Preview {
    Preview {
        body: body(&job.body, usize::from(job.key.width), links),
        ..header(job.sender, job.subject, job.received)
    }
}

/// A Preview of the header alone, for a Message whose body is not here.
pub fn header(sender: String, subject: String, received: DateTime<FixedOffset>) -> Preview {
    Preview {
        from: sender,
        subject,
        date: received.format("%a, %-d %b %Y %H:%M").to_string(),
        body: Vec::new(),
    }
}

/// The HTML part when there is one, else the `text/plain` part wrapped,
/// else a note.
fn body(raw: &[u8], width: usize, links: Links) -> Vec<StyledLine> {
    let message = MessageParser::default().parse(raw);
    let html = message
        .as_ref()
        .and_then(|m| m.html_part(0))
        .map(|p| &p.body);
    if let Some(PartType::Html(html)) = html {
        return html_preview::render(html, width, links)
            .unwrap_or_else(|_| note("(this Message could not be shown)", width));
    }

    let text = message
        .as_ref()
        .and_then(|m| m.text_part(0))
        .map(|p| &p.body);
    match text {
        Some(PartType::Text(text)) => wrap(text, width)
            .into_iter()
            .map(StyledLine::plain)
            .collect(),
        _ => note("(this Message has no text)", width),
    }
}

fn note(text: &str, width: usize) -> Vec<StyledLine> {
    wrap(text, width)
        .into_iter()
        .map(StyledLine::plain)
        .collect()
}
