use crate::{
    domain::message_ref::MessageRef,
    domain::per_account::{AccountId, PerAccount},
    mail::attachment_fetch::{AttachmentFetched, AttachmentRequest},
    mail::body_fetch::Request,
    mail::routes::Routes,
    mail::status::Status,
    mail::update::{BodyLink, Update},
    render::file_size,
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

/// Asks the fetch connection for the attachments picked, and opens or
/// saves each as it comes.
pub struct WantedAttachments {
    requests: Routes<Request>,
    /// Whether each Account's fetch connection is up.
    live: PerAccount<bool>,
    wanted: Vec<Wanted>,
}

impl WantedAttachments {
    pub const fn new(requests: Routes<Request>, live: PerAccount<bool>) -> Self {
        Self {
            requests,
            live,
            wanted: Vec::new(),
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
        self.wanted.push(wanted);
        toast
    }

    /// Follows the fetch connection of `account`; the Toast to show when
    /// `update` brings an attachment asked for, or loses those in flight.
    pub fn notice(&mut self, account: AccountId, update: &Update) -> Option<Toast> {
        let up = match update {
            Update::BodyLink(link) => *link == BodyLink::Up,
            Update::Status(Status::SignedOut) => false,
            Update::Attachment(fetched) => return self.arrived(account, fetched),
            _ => return None,
        };

        self.live[account] = up;
        let before = self.wanted.len();
        if !up {
            self.wanted.retain(|w| w.account != account);
        }

        (self.wanted.len() < before).then(|| Toast::error(OFFLINE))
    }

    fn arrived(&mut self, account: AccountId, fetched: &AttachmentFetched) -> Option<Toast> {
        let at = self.wanted.iter().position(|w| {
            w.account == account && w.message == fetched.message && w.index == fetched.index
        })?;

        let wanted = self.wanted.remove(at);
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
}
