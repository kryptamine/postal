use std::{io, path::PathBuf, sync::Arc};

use crate::{
    account::config::ComposeFormat,
    compose::draft::Draft,
    compose::draft_notes::DraftNotes,
    compose::markdown,
    compose::notes::Notes,
    domain::answering::Answering,
    render::html_job::{HtmlJob, LaidOut},
    render::mail_look::MailLook,
    render::styled_line::StyledLine,
    render::view::View,
};

/// A Draft shown after the editor exits, for `y` to send, `e` to edit again,
/// `d` to keep or `q` to discard.
pub struct DraftPreview {
    pub path: PathBuf,
    /// The file as written, kept for an Undo of its discard.
    pub text: String,
    /// The error says why `y` is refused.
    pub draft: Result<Draft, String>,
    /// The last Send's error, such as the server's answer.
    pub failure: Option<String>,
    /// The address of the Account it goes out from; the first Account's
    /// when `None`.
    pub account: Option<String>,
    /// For a Reply.
    pub answering: Option<Answering>,
    /// An Outbox Message, shown read-only.
    pub queued: bool,
    /// How a Markdown Draft's body shows: its HTML as sent, or its text.
    pub view: View,
    /// The HTML of a Markdown Draft, made once it is first shown.
    html: Option<Arc<str>>,
    laid_out: Option<LaidOut>,
    /// The width the HTML was last asked to be laid out at.
    asked: Option<u16>,
}

impl DraftPreview {
    /// The Draft's file, with the error and the answered Message its notes
    /// hold; `answering` wins over the notes'.
    pub fn read(path: PathBuf, answering: Option<Answering>) -> io::Result<Self> {
        let text = std::fs::read_to_string(&path)?;
        let notes = Notes::read(&path)?.unwrap_or_default();
        let mut preview = Self::new(
            path,
            text,
            DraftNotes {
                account: notes.account,
                answering: answering.or(notes.answering),
            },
        );
        preview.failure = notes.error;
        Ok(preview)
    }

    /// The Outbox Message at `path`, as its Draft was written, with the
    /// last Send's error.
    pub fn queued(path: PathBuf) -> io::Result<Self> {
        let notes = Notes::read(&path)?.unwrap_or_default();
        let text = notes.draft.unwrap_or_default();
        let mut preview = Self::new(
            path,
            text,
            DraftNotes {
                account: notes.account,
                answering: notes.answering,
            },
        );
        (preview.failure, preview.queued) = (notes.error, true);
        Ok(preview)
    }

    pub fn new(path: PathBuf, text: String, notes: DraftNotes) -> Self {
        Self {
            account: notes.account,
            draft: Draft::parse(&text).and_then(|draft| draft.attachable().map(|()| draft)),
            path,
            text,
            failure: None,
            answering: notes.answering,
            queued: false,
            view: View::default(),
            html: None,
            laid_out: None,
            asked: None,
        }
    }

    /// Whether the body shows as its HTML: a Markdown Draft's, unless `v`
    /// asked for its text; an Outbox Message was built already, perhaps in
    /// another format, and shows as written.
    fn shows_html(&self, format: ComposeFormat) -> bool {
        format == ComposeFormat::Markdown && !self.queued && self.view == View::Rendered
    }

    /// The job that lays the HTML out `width` cells wide, when it shows
    /// and is neither laid out nor asked for at that width.
    pub fn wanted(&mut self, format: ComposeFormat, width: u16, look: MailLook) -> Option<HtmlJob> {
        let draft = self.draft.as_ref().ok()?;
        let done = self
            .laid_out
            .as_ref()
            .is_some_and(|laid| laid.width == width);
        if !self.shows_html(format) || done || self.asked == Some(width) {
            return None;
        }

        self.asked = Some(width);
        let html = self
            .html
            .get_or_insert_with(|| markdown::html(&draft.body).into());
        Some(HtmlJob {
            html: Arc::clone(html),
            width,
            look,
        })
    }

    /// Keeps `laid` when it is of this preview's HTML.
    pub fn laid(&mut self, laid: LaidOut) {
        if self.html.as_ref() == Some(&laid.html) {
            self.laid_out = Some(laid);
        }
    }

    /// The HTML laid out `width` cells wide, when it shows and is ready; a
    /// layout the engine failed on shows the text.
    pub fn rendered(&self, format: ComposeFormat, width: u16) -> Option<&[StyledLine]> {
        let laid = self.laid_out.as_ref().filter(|laid| laid.width == width)?;
        self.shows_html(format)
            .then_some(laid.rows.as_deref())
            .flatten()
    }
}
