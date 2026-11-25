use std::{
    cell::{Ref, RefCell},
    io,
    path::PathBuf,
};

use crate::{
    compose::draft::Draft, compose::draft_notes::DraftNotes, compose::markdown,
    compose::notes::Notes, domain::answering::Answering, render::mail_look::MailLook,
    render::rendered_view, render::styled_line::StyledLine, render::view::View,
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
    /// The HTML laid out at the width it was last drawn; `None` where the
    /// engine failed on it.
    laid_out: RefCell<Option<(u16, Option<Vec<StyledLine>>)>>,
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
            laid_out: RefCell::new(None),
        }
    }

    /// The body's HTML as a Markdown Draft sends it, laid out `width` cells
    /// wide; `None` when the Draft does not parse or the engine fails.
    pub fn laid_out(&self, width: u16, look: MailLook) -> Option<Ref<'_, [StyledLine]>> {
        let draft = self.draft.as_ref().ok()?;
        let stale = self
            .laid_out
            .borrow()
            .as_ref()
            .is_none_or(|(at, _)| *at != width);
        if stale {
            let rows = rendered_view::render(&markdown::html(&draft.body), width, look);
            *self.laid_out.borrow_mut() = Some((width, rows));
        }

        Ref::filter_map(self.laid_out.borrow(), |laid| laid.as_ref()?.1.as_deref()).ok()
    }
}
