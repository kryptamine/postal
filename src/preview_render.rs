use chrono::{DateTime, FixedOffset};
use mail_parser::{HeaderValue, MessageParser, PartType};

use crate::{
    html_preview::{self, Links},
    mail_colours::MailColours,
    marks::Marks,
    preview::Preview,
    quoted_html, quoted_text,
    rendered_previews::PreviewKey,
    rendered_view,
    styled_line::StyledLine,
    view::View,
    wrap::wrap,
};

/// How a Preview header shows the date.
pub const DATE: &str = "%a, %-d %b %Y %H:%M";

/// The line that stands for a Quote left out; it names the key of
/// `Action::SwitchQuoted`.
const QUOTE_CUT: &str = "… quoted text hidden (z)";

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
/// the `text/plain` part wrapped, else a note. `true` when laid out. Its
/// Quote is left out unless the key keeps it (ADR 0015).
fn body(job: &Job, links: Links) -> (Vec<StyledLine>, bool) {
    let message = MessageParser::default().parse(&job.body);
    // Outlook names what it answers in `References` alone.
    let reply = message.as_ref().is_some_and(|m| {
        [m.in_reply_to(), m.references()]
            .iter()
            .any(|header| !matches!(header, HeaderValue::Empty))
    });

    let cutting = reply && !job.key.quoted;
    let html = message
        .as_ref()
        .and_then(|m| m.html_part(0))
        .map(|p| &p.body);
    if let Some(PartType::Html(html)) = html {
        let own = cutting.then(|| quoted_html::cut(html)).flatten();
        let (rows, laid_out) = html_rows(own.unwrap_or(html), job, links);
        return (cut(rows, own.is_some()), laid_out);
    }

    let width = usize::from(job.key.width);
    let text = message
        .as_ref()
        .and_then(|m| m.text_part(0))
        .map(|p| &p.body);
    let Some(PartType::Text(text)) = text else {
        return (note("(this Message has no text)", width), false);
    };

    let own = cutting.then(|| quoted_text::cut(text)).flatten();
    let rows = wrap(own.unwrap_or(text), width)
        .into_iter()
        .map(StyledLine::plain)
        .collect();
    (cut(rows, own.is_some()), false)
}

/// `html` laid out in the rendered View when the key asks for it and it
/// fits, which is `true`; else as text.
fn html_rows(html: &str, job: &Job, links: Links) -> (Vec<StyledLine>, bool) {
    let laid_out = (job.key.view == View::Rendered)
        .then(|| rendered_view::render(html, job.key.width, job.colours))
        .flatten();
    if let Some(rendered) = laid_out {
        return (rendered, true);
    }

    let width = usize::from(job.key.width);
    let converted = html_preview::render(html, width, links)
        .unwrap_or_else(|_| note("(this Message could not be shown)", width));
    (converted, false)
}

/// `rows` with the line that stands for a Quote, when one was left out.
fn cut(mut rows: Vec<StyledLine>, quote: bool) -> Vec<StyledLine> {
    if quote {
        let mut line = StyledLine::plain(QUOTE_CUT.to_owned());
        line.restyle(Marks::MUTED);
        rows.extend([StyledLine::new(), line]);
    }

    rows
}

fn note(text: &str, width: usize) -> Vec<StyledLine> {
    wrap(text, width)
        .into_iter()
        .map(StyledLine::plain)
        .collect()
}
