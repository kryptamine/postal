use chrono::{DateTime, FixedOffset};

use crate::{
    mime_part, preview::Preview, raw_body::RawBody, rendered_previews::PreviewKey, wrap::wrap,
};

/// What rendering one Preview needs, sent to the thread that renders it.
pub struct Job {
    pub key: PreviewKey,
    pub body: RawBody,
    pub sender: String,
    pub subject: String,
    pub received: DateTime<FixedOffset>,
}

/// The Preview of `job` at the width in its key: the `text/plain` part
/// wrapped, or a note where there is none to show.
pub fn render(job: Job) -> Preview {
    let width = usize::from(job.key.width);
    let body = match &job.body.part {
        Some(part) if !part.html => mime_part::section(&job.body.raw, &part.section).map_or_else(
            || vec!["(the text part is missing from the Message)".to_owned()],
            |raw| wrap(&part.decode(raw), width),
        ),
        Some(_) => wrap("(this Message has only an HTML part, not shown yet)", width),
        None => wrap("(this Message has no text)", width),
    };

    Preview {
        from: job.sender,
        subject: job.subject,
        date: job.received.format("%a, %-d %b %Y %H:%M").to_string(),
        body,
    }
}
