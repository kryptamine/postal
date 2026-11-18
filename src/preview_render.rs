use chrono::{DateTime, FixedOffset};
use mail_parser::{MessageParser, PartType};

use crate::{
    html_preview::{self, Links},
    mail_colours::MailColours,
    preview::Preview,
    rendered_previews::PreviewKey,
    rendered_view,
    styled_line::StyledLine,
    view::View,
    wrap::wrap,
};

/// How a Preview header shows the date.
pub const DATE: &str = "%a, %-d %b %Y %H:%M";

/// What rendering one Preview needs, sent to the thread that renders it.
pub struct Job {
    pub key: PreviewKey,
    /// `BODY[]`: the header and every part, still MIME-encoded.
    pub body: Vec<u8>,
    pub sender: String,
    pub subject: String,
    pub received: DateTime<FixedOffset>,
    pub colours: MailColours,
}

/// The Preview of `job` at the width in its key.
pub fn render(job: Job) -> Preview {
    rendered(job, Links::Inline)
}

/// `job` as the `$PAGER` shows it: the Preview, with link footnotes in the
/// text View.
pub fn paged(job: Job) -> Preview {
    rendered(job, Links::Footnotes)
}

fn rendered(job: Job, links: Links) -> Preview {
    let (body, laid_out) = body(&job, links);
    Preview {
        body,
        laid_out,
        ..header(job.sender, job.subject, job.received)
    }
}

/// A Preview of the header alone, for a Message whose body is not here.
pub fn header(sender: String, subject: String, received: DateTime<FixedOffset>) -> Preview {
    Preview {
        from: sender,
        subject,
        date: received.format(DATE).to_string(),
        body: Vec::new(),
        laid_out: false,
    }
}

/// The HTML part when there is one, laid out in the rendered View; else
/// the `text/plain` part wrapped, else a note. `true` when laid out.
fn body(job: &Job, links: Links) -> (Vec<StyledLine>, bool) {
    let width = usize::from(job.key.width);
    let message = MessageParser::default().parse(&job.body);
    let html = message
        .as_ref()
        .and_then(|m| m.html_part(0))
        .map(|p| &p.body);
    if let Some(PartType::Html(html)) = html {
        let laid_out = (job.key.view == View::Rendered)
            .then(|| rendered_view::render(html, job.key.width, job.colours))
            .flatten();
        if let Some(rendered) = laid_out {
            return (rendered, true);
        }

        let converted = html_preview::render(html, width, links)
            .unwrap_or_else(|_| note("(this Message could not be shown)", width));
        return (converted, false);
    }

    let text = message
        .as_ref()
        .and_then(|m| m.text_part(0))
        .map(|p| &p.body);
    let wrapped = match text {
        Some(PartType::Text(text)) => wrap(text, width)
            .into_iter()
            .map(StyledLine::plain)
            .collect(),
        _ => note("(this Message has no text)", width),
    };

    (wrapped, false)
}

fn note(text: &str, width: usize) -> Vec<StyledLine> {
    wrap(text, width)
        .into_iter()
        .map(StyledLine::plain)
        .collect()
}
