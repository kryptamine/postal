use crate::{
    domain::inline_image::InlineImage,
    domain::message::Message,
    domain::message_ref::MessageRef,
    domain::per_account::{AccountId, PerAccount},
    mail::body_fetch::Request,
    mail::inline_fetch::{InlineFetched, InlineRequest},
    mail::routes::Routes,
    mail::status::Status,
    mail::update::{BodyLink, Update},
    render::original_page::Original,
    ui::outside::attachment_file,
    ui::state::toast::Toast,
};

/// A Message waiting on its inline images to open in the browser.
struct Wanted {
    account: AccountId,
    message: MessageRef,
    name: String,
    original: Original,
}

/// Opens Messages in the browser, once the fetch connection brings the
/// images their HTML shows by `cid:`.
pub struct WantedOriginals {
    requests: Routes<Request>,
    /// Whether each Account's fetch connection is up.
    live: PerAccount<bool>,
    wanted: Vec<Wanted>,
}

impl WantedOriginals {
    pub const fn new(requests: Routes<Request>, live: PerAccount<bool>) -> Self {
        Self {
            requests,
            live,
            wanted: Vec::new(),
        }
    }

    /// Opens `message`, whose cached body is `raw`, or asks for its inline
    /// images first; offline, it opens without them. Returns the Toast.
    pub fn ask(&mut self, message: &Message, raw: &[u8]) -> Toast {
        let original = Original::parse(raw);
        let name = format!("{}.html", message.body_key);
        let ids = original.inline_ids();
        let account = message.account;
        match message.on_server() {
            Some(on_server) if !ids.is_empty() && self.live[account] => {
                let request = InlineRequest {
                    message: on_server.clone(),
                    ids,
                };

                self.requests.send(account, Request::Inline(request));
                self.wanted.push(Wanted {
                    account,
                    message: on_server,
                    name,
                    original,
                });
                Toast::lasting("downloading its images…")
            }
            _ => opened(&name, &original, &[], None),
        }
    }

    /// Follows the fetch connection of `account`; the Toast to show when
    /// `update` brings the images of a Message asked for, or loses them.
    pub fn notice(&mut self, account: AccountId, update: &Update) -> Option<Toast> {
        let up = match update {
            Update::BodyLink(link) => *link == BodyLink::Up,
            Update::Status(Status::SignedOut) => false,
            Update::Inline(fetched) => return self.arrived(account, fetched),
            _ => return None,
        };

        self.live[account] = up;
        let (lost, kept) = std::mem::take(&mut self.wanted)
            .into_iter()
            .partition::<Vec<_>, _>(|w| !up && w.account == account);
        self.wanted = kept;
        let mut toast = None;
        for w in &lost {
            toast = Some(opened(&w.name, &w.original, &[], Some("offline")));
        }

        toast
    }

    fn arrived(&mut self, account: AccountId, fetched: &InlineFetched) -> Option<Toast> {
        let at = self
            .wanted
            .iter()
            .position(|w| w.account == account && w.message == fetched.message)?;
        let wanted = self.wanted.remove(at);
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
