use crate::{
    domain::message_ref::MessageRef,
    domain::per_account::AccountId,
    mail::fetch::attachment_fetch::AttachmentRequest,
    mail::fetch::body_fetch::{AwaitedRequest, Fetched, Standing},
    mail::status::Status,
    mail::update::Update,
};

struct AwaitedBody {
    message: MessageRef,
    /// A refusal of this body ends the wait instead of skipping it.
    required: bool,
}

struct AwaitedAttachment {
    request: AttachmentRequest,
    bytes: Option<Vec<u8>>,
}

/// The bodies and attachments a Reply or Forward waits on from one Account,
/// with its refusal policy and completion state.
pub struct AwaitedParts {
    account: AccountId,
    bodies: Vec<AwaitedBody>,
    attachments: Vec<AwaitedAttachment>,
}

impl AwaitedParts {
    pub const fn new(account: AccountId) -> Self {
        Self {
            account,
            bodies: Vec::new(),
            attachments: Vec::new(),
        }
    }

    pub const fn account(&self) -> AccountId {
        self.account
    }

    pub fn add_body(&mut self, message: MessageRef, required: bool) {
        if let Some(existing) = self.bodies.iter_mut().find(|b| b.message == message) {
            existing.required |= required;
            return;
        }

        self.bodies.push(AwaitedBody { message, required });
    }

    pub fn add_attachment(&mut self, message: MessageRef, index: usize) {
        self.attachments.push(AwaitedAttachment {
            request: AttachmentRequest { message, index },
            bytes: None,
        });
    }

    pub fn complete(&self) -> bool {
        self.bodies.is_empty() && self.attachments.iter().all(|a| a.bytes.is_some())
    }

    /// What the fetch connection is asked for while these parts wait.
    pub fn request(&self) -> Standing {
        let bodies = self.bodies.iter().map(|b| b.message.clone()).collect();
        let attachments = self
            .attachments
            .iter()
            .filter(|a| a.bytes.is_none())
            .map(|a| a.request.clone())
            .collect();
        Standing::Awaited(AwaitedRequest {
            bodies,
            attachments,
        })
    }

    /// Says when `update` of `account` brings the last awaited part, or why
    /// a required one will not come.
    pub fn notice(&mut self, account: AccountId, update: &Update) -> Option<Result<(), String>> {
        if self.account != account {
            return None;
        }

        match update {
            Update::Status(Status::SignedOut) => return Some(Err("signed out".to_owned())),
            Update::Bodies(folder, fetched) => {
                let (uids, refused) = match fetched {
                    Fetched::Stored(uids) => (uids, None),
                    Fetched::Refused { uids, reason } => (uids, Some(reason)),
                };

                if let Some(reason) = refused
                    && self.bodies.iter().any(|b| {
                        b.required && b.message.folder == *folder && uids.contains(&b.message.uid)
                    })
                {
                    return Some(Err(reason.clone()));
                }

                let before = self.bodies.len();
                self.bodies
                    .retain(|b| b.message.folder != *folder || !uids.contains(&b.message.uid));
                if self.bodies.len() == before {
                    return None;
                }
            }
            Update::Attachment(fetched) => {
                let slot = self.attachments.iter_mut().find(|a| {
                    a.request.message == fetched.message && a.request.index == fetched.index
                })?;
                match &fetched.bytes {
                    Ok(bytes) => slot.bytes = Some(bytes.clone()),
                    Err(reason) => return Some(Err(reason.to_string())),
                }
            }
            _ => return None,
        }

        self.complete().then_some(Ok(()))
    }

    pub fn into_attachments(self) -> Vec<Vec<u8>> {
        self.attachments
            .into_iter()
            .filter_map(|a| a.bytes)
            .collect()
    }
}
