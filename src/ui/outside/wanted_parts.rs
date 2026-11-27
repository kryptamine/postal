use crate::{
    domain::inline_image::InlineImage,
    domain::message::Message,
    domain::message_ref::MessageRef,
    domain::per_account::AccountId,
    mail::attachment_fetch::{AttachmentFetched, AttachmentRequest},
    mail::body_fetch::OneShot,
    mail::fetch_requests::FetchRequests,
    mail::inline_fetch::{InlineFetched, InlineRequest},
    mail::unfetched::Unfetched,
    mail::update::Update,
    render::file_size,
    render::original_page::Original,
    ui::input::wanted::{Purpose, Wanted},
    ui::outside::attachment_file,
    ui::state::toast::Toast,
};

const OFFLINE: &str = "offline · not downloaded";

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

/// Asks the fetch connection for the parts the cached body leaves out (the
/// attachments picked, the inline images of a Message opened in the
/// browser) and opens or saves what each was asked for as it comes.
#[derive(Default)]
pub struct WantedParts {
    waiting: Vec<Waiting>,
}

impl WantedParts {
    /// Asks for `wanted`; returns the Toast saying so, or that it cannot.
    pub fn ask(&mut self, wanted: Wanted, fetch: &FetchRequests) -> Toast {
        let request = AttachmentRequest {
            message: wanted.message.clone(),
            index: wanted.index,
        };

        if !fetch.send_if_live(wanted.account, OneShot::Attachment(request)) {
            return Toast::error(OFFLINE);
        }

        let size = file_size::shown(wanted.size);
        let toast = Toast::lasting(format!("downloading {} · {}…", wanted.name, size.as_str()));
        self.waiting.push(Waiting::Attachment(wanted));
        toast
    }

    /// Opens `message`, whose cached body is `raw`, in the browser, or asks
    /// for its inline images first; offline, it opens without them.
    pub fn original(&mut self, message: &Message, raw: &[u8], fetch: &FetchRequests) -> Toast {
        let original = Original::parse(raw);
        let name = format!("{}.html", message.body_key);
        let ids = original.inline_ids();
        let account = message.account;
        let Some(on_server) = message.on_server().filter(|_| !ids.is_empty()) else {
            return opened(&name, &original, &[], None);
        };

        let request = InlineRequest {
            message: on_server.clone(),
            ids,
        };

        if !fetch.send_if_live(account, OneShot::Inline(request)) {
            return opened(&name, &original, &[], None);
        }

        self.waiting.push(Waiting::Original(WantedOriginal {
            account,
            message: on_server,
            name,
            original,
        }));
        Toast::lasting("downloading its images…")
    }

    /// The Toast to show when `update` of `account` brings a part asked for.
    pub fn notice(&mut self, account: AccountId, update: &Update) -> Option<Toast> {
        match update {
            Update::Attachment(fetched) => self.attachment(account, fetched),
            Update::Inline(fetched) => self.images(account, fetched),
            _ => None,
        }
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
            Err(Unfetched::Offline) => return Some(Toast::error(OFFLINE)),
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
            Err(reason) => (&[][..], Some(reason.to_string())),
        };

        Some(opened(
            &wanted.name,
            &wanted.original,
            images,
            missing.as_deref(),
        ))
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
