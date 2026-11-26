use crate::{
    domain::inline_image::InlineImage,
    domain::message::Message,
    domain::message_ref::MessageRef,
    domain::per_account::{AccountId, PerAccount},
    mail::attachment_fetch::{AttachmentFetched, AttachmentRequest},
    mail::body_fetch::Request,
    mail::inline_fetch::{InlineFetched, InlineRequest},
    mail::routes::Routes,
    mail::status::Status,
    mail::update::{BodyLink, Update},
    render::file_size,
    render::original_page::Original,
    ui::outside::attachment_file,
    ui::state::toast::Toast,
};

const OFFLINE: &str = "offline · not downloaded";

/// What a picked attachment is for.
#[derive(Clone, Copy)]
pub enum Purpose {
    /// In the program the desktop picks for it.
    Open,
    /// Into the download directory.
    Save,
}

/// One attachment picked, and what for.
pub struct Wanted {
    pub account: AccountId,
    pub message: MessageRef,
    /// Its place among its Message's attachments.
    pub index: usize,
    pub name: String,
    pub size: u64,
    pub purpose: Purpose,
}

/// A Message to open in the browser once its inline images come.
struct WantedOriginal {
    account: AccountId,
    message: MessageRef,
    name: String,
    original: Original,
}

/// What waits on the fetch connection.
enum Waiting {
    Attachment(Wanted),
    Original(WantedOriginal),
}

impl Waiting {
    const fn account(&self) -> AccountId {
        match self {
            Self::Attachment(wanted) => wanted.account,
            Self::Original(wanted) => wanted.account,
        }
    }
}

/// Asks the fetch connection for the parts the cached body leaves out (the
/// attachments picked, the inline images of a Message opened in the
/// browser) and opens or saves what each was asked for as it comes.
pub struct WantedParts {
    requests: Routes<Request>,
    /// Whether each Account's fetch connection is up.
    live: PerAccount<bool>,
    waiting: Vec<Waiting>,
}

impl WantedParts {
    pub const fn new(requests: Routes<Request>, live: PerAccount<bool>) -> Self {
        Self {
            requests,
            live,
            waiting: Vec::new(),
        }
    }

    /// Asks for `wanted`; returns the Toast saying so, or that it cannot.
    pub fn ask(&mut self, wanted: Wanted) -> Toast {
        if !self.live[wanted.account] {
            return Toast::error(OFFLINE);
        }

        let request = AttachmentRequest {
            message: wanted.message.clone(),
            index: wanted.index,
        };

        self.requests
            .send(wanted.account, Request::Attachment(request));
        let size = file_size::shown(wanted.size);
        let toast = Toast::lasting(format!("downloading {} · {}…", wanted.name, size.as_str()));
        self.waiting.push(Waiting::Attachment(wanted));
        toast
    }

    /// Opens `message`, whose cached body is `raw`, in the browser, or asks
    /// for its inline images first; offline, it opens without them.
    pub fn original(&mut self, message: &Message, raw: &[u8]) -> Toast {
        let original = Original::parse(raw);
        let name = format!("{}.html", message.body_key);
        let ids = original.inline_ids();
        let account = message.account;
        let on_server = message
            .on_server()
            .filter(|_| !ids.is_empty() && self.live[account]);
        let Some(on_server) = on_server else {
            return opened(&name, &original, &[], None);
        };

        let request = InlineRequest {
            message: on_server.clone(),
            ids,
        };

        self.requests.send(account, Request::Inline(request));
        self.waiting.push(Waiting::Original(WantedOriginal {
            account,
            message: on_server,
            name,
            original,
        }));
        Toast::lasting("downloading its images…")
    }

    /// Follows the fetch connection of `account`; the Toast to show when
    /// `update` brings a part asked for, or loses those in flight.
    pub fn notice(&mut self, account: AccountId, update: &Update) -> Option<Toast> {
        let up = match update {
            Update::BodyLink(link) => *link == BodyLink::Up,
            Update::Status(Status::SignedOut) => false,
            Update::Attachment(fetched) => return self.attachment(account, fetched),
            Update::Inline(fetched) => return self.images(account, fetched),
            _ => return None,
        };

        self.live[account] = up;
        if up {
            return None;
        }

        // What was in flight went down with the connection.
        let mut toast = None;
        for lost in self.waiting.extract_if(.., |w| w.account() == account) {
            toast = Some(match lost {
                Waiting::Attachment(_) => Toast::error(OFFLINE),
                Waiting::Original(w) => opened(&w.name, &w.original, &[], Some("offline")),
            });
        }

        toast
    }

    fn attachment(&mut self, account: AccountId, fetched: &AttachmentFetched) -> Option<Toast> {
        let at = self.waiting.iter().position(|w| {
            matches!(w, Waiting::Attachment(w) if w.account == account
                && w.message == fetched.message
                && w.index == fetched.index)
        })?;
        let Waiting::Attachment(wanted) = self.waiting.remove(at) else {
            return None;
        };

        let bytes = match &fetched.bytes {
            Ok(bytes) => bytes,
            Err(reason) => return Some(Toast::error(format!("couldn't download: {reason}"))),
        };

        let name = &wanted.name;
        let done = match wanted.purpose {
            Purpose::Save => attachment_file::save(name, bytes)
                .map(|path| Toast::new(format!("saved to {}", path.display()))),
            Purpose::Open => {
                attachment_file::open(name, bytes).map(|()| Toast::new(format!("opened {name}")))
            }
        };

        Some(done.unwrap_or_else(|error| {
            tracing::error!("writing the attachment {name} failed: {error}");
            Toast::error(format!("couldn't write {name}: {error}"))
        }))
    }

    fn images(&mut self, account: AccountId, fetched: &InlineFetched) -> Option<Toast> {
        let at = self.waiting.iter().position(|w| {
            matches!(w, Waiting::Original(w) if w.account == account && w.message == fetched.message)
        })?;
        let Waiting::Original(wanted) = self.waiting.remove(at) else {
            return None;
        };

        let (images, missing) = match &fetched.images {
            Ok(images) => (images.as_slice(), None),
            Err(reason) => (&[][..], Some(reason.as_str())),
        };

        Some(opened(&wanted.name, &wanted.original, images, missing))
    }
}

/// Writes the page and hands it to the browser; `missing` says why its
/// inline images are not in it.
fn opened(name: &str, original: &Original, images: &[InlineImage], missing: Option<&str>) -> Toast {
    match attachment_file::open(name, original.page(images).as_bytes()) {
        Ok(()) => missing.map_or_else(
            || Toast::new("opened in the browser"),
            |reason| Toast::error(format!("opened without its images: {reason}")),
        ),
        Err(error) => {
            tracing::error!("writing {name} failed: {error}");
            Toast::error(format!("couldn't write {name}: {error}"))
        }
    }
}
