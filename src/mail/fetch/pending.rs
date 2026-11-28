use std::collections::VecDeque;

use futures::{StreamExt, channel::mpsc::UnboundedReceiver};

use crate::{
    cache::Cache,
    domain::folder_name::FolderName,
    domain::message_ref::MessageRef,
    domain::uid::Uid,
    mail::fetch::attachment_fetch::{self, AttachmentFetched, AttachmentRequest},
    mail::fetch::backfill::{self, Backfill},
    mail::fetch::body_fetch::{
        self, AwaitedRequest, BodyRequest, Fetched, OneShot, Request, Standing,
    },
    mail::fetch::examine,
    mail::fetch::inline_fetch::{self, InlineFetched},
    mail::fetch::part_fetch,
    mail::fetch::unfetched::Unfetched,
    mail::imap::ImapSession,
    mail::passes::spare_work::SpareWork,
    mail::sync::Error,
    mail::update::Update,
};

/// The fetch requests received and not yet served, each kept until its
/// fetch ends. It outlives each fetch connection, so standing work resumes
/// after a reconnect.
pub struct Pending {
    requests: UnboundedReceiver<Request>,
    awaited: VecDeque<AwaitedPart>,
    oneshots: VecDeque<OneShot>,
    /// The selected body alone, then its neighbours, all in one Folder.
    bodies: VecDeque<(FolderName, Vec<Uid>)>,
    older: Option<FolderName>,
}

enum AwaitedPart {
    Bodies(FolderName, Vec<Uid>),
    Attachment(AttachmentRequest),
}

impl Pending {
    pub const fn new(requests: UnboundedReceiver<Request>) -> Self {
        Self {
            requests,
            awaited: VecDeque::new(),
            oneshots: VecDeque::new(),
            bodies: VecDeque::new(),
            older: None,
        }
    }

    /// Fails the one-shots sent to a connection that has since dropped.
    pub fn fail_oneshots(&mut self, send: &impl Fn(Update)) {
        self.receive();
        for oneshot in self.oneshots.drain(..) {
            let update = match oneshot {
                OneShot::Attachment(request) => Update::Attachment(AttachmentFetched {
                    message: request.message,
                    index: request.index,
                    bytes: Err(Unfetched::Offline),
                }),
                OneShot::Inline(request) => Update::Inline(InlineFetched {
                    message: request.message,
                    images: Err(Unfetched::Offline),
                }),
            };

            send(update);
        }
    }

    /// Serves until the channel closes, telling `send` how each went, then
    /// `SpareWork` while none waits. Only a failure but a refusal ends it.
    pub async fn serve(
        &mut self,
        server: &mut ImapSession,
        cache: &Cache,
        spare: &mut SpareWork,
        send: &impl Fn(Update),
    ) -> Result<(), Error> {
        loop {
            self.receive();
            if self.step(server, cache, spare, send).await? {
                continue;
            }

            match tokio::time::timeout(spare.patience(), self.requests.next()).await {
                Ok(Some(request)) => self.take(request),
                Ok(None) => return Ok(()),
                Err(_) => spare.step(server, cache, send).await?,
            }
        }
    }

    /// Serves the first of what waits, in this order, and removes it once
    /// served; false when none waits.
    async fn step(
        &mut self,
        server: &mut ImapSession,
        cache: &Cache,
        spare: &mut SpareWork,
        send: &impl Fn(Update),
    ) -> Result<bool, Error> {
        if let Some(part) = self.awaited.front() {
            fetch_awaited(server, cache, part, send).await?;
            self.awaited.pop_front();
        } else if let Some(oneshot) = self.oneshots.front() {
            match oneshot {
                OneShot::Attachment(request) => {
                    attachment_fetch::fetch(server, request, send).await?;
                }
                OneShot::Inline(request) => inline_fetch::fetch(server, request, send).await?,
            }

            self.oneshots.pop_front();
        } else if let Some((folder, uids)) = self.bodies.front() {
            if examine::open(server, folder).await? {
                body_fetch::fetch_each(server, cache, folder, uids.clone(), send).await?;
                self.bodies.pop_front();
            } else {
                self.bodies.clear();
            }
        } else if let Some(folder) = &self.older {
            fetch_backfill(server, cache, folder, spare, send).await?;
            self.older = None;
        } else {
            return Ok(false);
        }

        Ok(true)
    }

    // Replacing the bodies leaves behind the neighbours of a selection the
    // screen has moved on from.
    fn take(&mut self, request: Request) {
        match request {
            Request::Standing(Standing::Bodies(bodies)) => self.bodies = batches(bodies),
            Request::Standing(Standing::Backfill(folder)) => self.older = Some(folder),
            Request::Standing(Standing::Awaited(awaited)) => self.awaited = parts(awaited),
            Request::OneShot(oneshot) => self.oneshots.push_back(oneshot),
        }
    }

    fn receive(&mut self) {
        while let Ok(more) = self.requests.try_recv() {
            self.take(more);
        }
    }
}

fn batches(request: BodyRequest) -> VecDeque<(FolderName, Vec<Uid>)> {
    let BodyRequest {
        folder,
        selected,
        neighbours,
    } = request;
    let neighbours = (!neighbours.is_empty()).then_some(neighbours);
    selected
        .map(|uid| vec![uid])
        .into_iter()
        .chain(neighbours)
        .map(|uids| (folder.clone(), uids))
        .collect()
}

/// The awaited bodies a Folder at a time, then each attachment.
fn parts(request: AwaitedRequest) -> VecDeque<AwaitedPart> {
    let mut folders: Vec<(FolderName, Vec<Uid>)> = Vec::new();
    for MessageRef { folder, uid } in request.bodies {
        match folders.iter_mut().find(|(f, _)| *f == folder) {
            Some((_, uids)) => uids.push(uid),
            None => folders.push((folder, vec![uid])),
        }
    }

    let bodies = folders
        .into_iter()
        .map(|(f, uids)| AwaitedPart::Bodies(f, uids));
    let attachments = request.attachments.into_iter().map(AwaitedPart::Attachment);
    bodies.chain(attachments).collect()
}

/// Bodies the server no longer has, or whose Folder is gone, are refused.
async fn fetch_awaited(
    server: &mut ImapSession,
    cache: &Cache,
    part: &AwaitedPart,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    let (folder, uids) = match part {
        AwaitedPart::Attachment(request) => {
            return attachment_fetch::fetch(server, request, send).await;
        }
        AwaitedPart::Bodies(folder, uids) => (folder, uids),
    };

    let (unanswered, reason) = if examine::open(server, folder).await? {
        let answered = body_fetch::fetch_each(server, cache, folder, uids.clone(), send).await?;
        let unanswered = uids.iter().filter(|u| !answered.contains(u)).copied();
        (unanswered.collect::<Vec<_>>(), part_fetch::NOT_HELD)
    } else {
        (uids.clone(), part_fetch::FOLDER_GONE)
    };

    if !unanswered.is_empty() {
        let gone = Fetched::Refused {
            uids: unanswered,
            reason: reason.to_owned(),
        };

        send(Update::Bodies(folder.clone(), gone));
    }

    Ok(())
}

async fn fetch_backfill(
    server: &mut ImapSession,
    cache: &Cache,
    folder: &FolderName,
    spare: &mut SpareWork,
    send: &impl Fn(Update),
) -> Result<(), Error> {
    let progress = |done, total| send(Update::Backfill(Backfill::Fetching { done, total }));
    let page = backfill::page(server, cache, folder, backfill::PAGE, progress).await?;
    if page.below.is_some() {
        send(Update::Inbox);
    }

    send(Update::Backfill(Backfill::Paged {
        folder: folder.clone(),
        nothing_below: page.nothing_below,
    }));
    spare.backfilled(folder.clone(), page.below);
    Ok(())
}
