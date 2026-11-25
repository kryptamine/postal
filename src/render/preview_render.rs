use std::sync::Arc;

use mail_parser::{HeaderValue, Message, MessageParser, PartType};

use crate::{
    render::html_preview::{self, Links},
    render::invitation::Invitation,
    render::invitation_card,
    render::mail_look::MailLook,
    render::marks::Marks,
    render::preview::Preview,
    render::quoted_html,
    render::quoted_text,
    render::rendered_previews::PreviewKey,
    render::rendered_view,
    render::signature,
    render::styled_line::StyledLine,
    render::view::View,
    render::wrap::wrap,
};

/// The line that stands for a Quote left out; it names the key of
/// `Action::SwitchQuoted`.
const QUOTE_CUT: &str = "… quoted text hidden (z)";

/// What rendering one Preview needs, sent to the thread that renders it.
pub struct Job {
    pub key: PreviewKey,
    /// The raw body: the header, the text parts and any calendar part, still
    /// MIME-encoded.
    pub body: Vec<u8>,
    pub look: MailLook,
    /// The Account's address, which an invitation's attendees are matched to.
    pub own: Arc<str>,
}

/// The Preview of `job` at the width in its key.
pub fn render(job: &Job) -> Preview {
    rendered(job, Links::Inline)
}

/// What the Preview of `key` shows when rendering it panicked.
pub fn failed(key: PreviewKey) -> Preview {
    let body = note(
        "(this Message could not be rendered)",
        usize::from(key.width),
    );
    Preview {
        body,
        laid_out: false,
    }
}

/// `job` as the `$PAGER` shows it: the Preview, with link footnotes in the
/// text View.
pub fn paged(job: &Job) -> Preview {
    rendered(job, Links::Footnotes)
}

/// The body under the card of the invitation it carries, if any.
fn rendered(job: &Job, links: Links) -> Preview {
    let message = MessageParser::default().parse(&job.body);
    let (body, laid_out) = body(job, message.as_ref(), links);
    let card = message
        .as_ref()
        .and_then(|m| Invitation::find(m, &job.own))
        .map(|invitation| invitation_card::rows(&invitation, usize::from(job.key.width)));
    let Some(mut rows) = card else {
        return Preview { body, laid_out };
    };

    rows.extend(body);
    Preview {
        body: rows,
        laid_out,
    }
}

/// The HTML part when there is one, laid out in the rendered View; else
/// the `text/plain` part wrapped, else a note. `true` when laid out. Its
/// Quote is left out unless the key keeps it (ADR 0015).
fn body(job: &Job, message: Option<&Message<'_>>, links: Links) -> (Vec<StyledLine>, bool) {
    // Outlook names what it answers in `References` alone.
    let reply = message.is_some_and(|m| {
        [m.in_reply_to(), m.references()]
            .iter()
            .any(|header| !matches!(header, HeaderValue::Empty))
    });

    let cutting = reply && !job.key.quoted;
    let html = message.and_then(|m| m.html_part(0)).map(|p| &p.body);
    if let Some(PartType::Html(html)) = html {
        let own = cutting.then(|| quoted_html::cut(html)).flatten();
        let (rows, laid_out) = html_rows(own.unwrap_or(html), job, links);
        return (cut(rows, own.is_some()), laid_out);
    }

    let width = usize::from(job.key.width);
    let text = message.and_then(|m| m.text_part(0)).map(|p| &p.body);
    let Some(PartType::Text(text)) = text else {
        return (note("(this Message has no text)", width), false);
    };

    // The signature ends at the Quote, shown or not.
    let own = reply.then(|| quoted_text::cut(text)).flatten();
    let left_out = own.filter(|_| cutting);
    let shown = left_out.unwrap_or(text);
    let rows = signature::rows(shown, own.unwrap_or(text), width);
    (cut(rows, left_out.is_some()), false)
}

/// `html` laid out in the rendered View when the key asks for it and it
/// fits, which is `true`; else as text.
fn html_rows(html: &str, job: &Job, links: Links) -> (Vec<StyledLine>, bool) {
    let laid_out = (job.key.view == View::Rendered)
        .then(|| rendered_view::render(html, job.key.width, job.look))
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
