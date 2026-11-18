use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, mpsc},
    thread,
};

use futures::channel::mpsc::UnboundedSender;

use crate::{
    body_fetch::{BodyRequest, Fetched, Request},
    cache::{self, Cache},
    folder,
    inbox_view::InboxView,
    mail_colours::MailColours,
    message::Message,
    preview::Preview,
    preview_render::{self, Job},
    rendered_previews::{PreviewKey, RenderedPreviews},
    shown::Shown,
    update::Update,
};

/// Rows on each side of the selection whose Preview is made ready too.
const NEIGHBOURS: usize = 2;

const OFFLINE: &str = "Not downloaded yet · offline";

/// The connection bodies are fetched on.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Link {
    /// Not yet up since Postal started: bodies are asked for once it is.
    Starting,
    Live,
    Down,
}

/// Gets Previews ready: fetches bodies the Cache lacks, renders on a thread
/// of its own, and keeps what it rendered. Its UIDs are those of `folder`.
pub struct Previews {
    folder: Arc<str>,
    rendered: RenderedPreviews,
    rendering: HashSet<PreviewKey>,
    /// UIDs of the latest request not yet answered: the server serves only
    /// that one. One it does not answer for stays here.
    fetching: HashSet<u32>,
    /// The selection of the latest request.
    asked: Option<u32>,
    /// UIDs found without a body while the link is down.
    offline: HashSet<u32>,
    /// The placeholder of each body the server refused, until a reconnect.
    refused: HashMap<u32, String>,
    /// The header of the selected Message, while its body is unavailable.
    header: Option<(u32, Preview)>,
    link: Link,
    colours: MailColours,
    requests: UnboundedSender<Request>,
    jobs: mpsc::Sender<Job>,
}

impl Previews {
    /// Starts the render thread, which sends each Preview back to `updates`.
    pub fn start(
        requests: UnboundedSender<Request>,
        updates: UnboundedSender<Update>,
        colours: MailColours,
    ) -> Self {
        let (jobs, queue) = mpsc::channel::<Job>();
        thread::spawn(move || {
            for job in queue {
                let key = job.key;
                let preview = preview_render::render(job);
                if updates
                    .unbounded_send(Update::Rendered(key, preview))
                    .is_err()
                {
                    return;
                }
            }
        });

        Self {
            folder: folder::INBOX.into(),
            rendered: RenderedPreviews::default(),
            rendering: HashSet::new(),
            fetching: HashSet::new(),
            asked: None,
            offline: HashSet::new(),
            refused: HashMap::new(),
            header: None,
            link: Link::Starting,
            colours,
            requests,
            jobs,
        }
    }

    /// The selected Message's Preview at `width`, once rendered.
    pub fn get(&self, view: &InboxView, width: u16) -> Option<&Preview> {
        let message = view.selected_message()?;
        self.rendered.get(PreviewKey {
            body: message.body_key,
            width,
            view: view.html,
        })
    }

    pub fn shown(&self, view: &InboxView, width: u16) -> Shown<'_> {
        let Some(message) = view.selected_message() else {
            return Shown::Loading;
        };

        if let Some(preview) = self.get(view, width) {
            return Shown::Rendered(preview);
        }

        let why = match self.refused.get(&message.uid) {
            Some(why) => why.as_str(),
            None if self.offline.contains(&message.uid) => OFFLINE,
            None => return Shown::Loading,
        };

        match &self.header {
            Some((uid, header)) if *uid == message.uid => Shown::Unavailable { header, why },
            _ => Shown::Loading,
        }
    }

    /// Readies the Previews of the selection and its neighbours at `width`.
    /// When all are rendered already this does not allocate.
    pub fn want(
        &mut self,
        cache: &Cache,
        view: &InboxView,
        width: u16,
    ) -> Result<(), cache::Error> {
        if self.folder != view.folder.name {
            self.switch(&view.folder.name);
        }

        let selected = view.selected;
        let around = selected.saturating_sub(NEIGHBOURS)..=selected + NEIGHBOURS;
        let mut request = BodyRequest {
            folder: Arc::clone(&self.folder),
            selected: None,
            neighbours: Vec::new(),
        };

        for index in std::iter::once(selected).chain(around.filter(|&i| i != selected)) {
            let Some(message) = view.row(index) else {
                continue;
            };

            let key = PreviewKey {
                body: message.body_key,
                width,
                view: view.html,
            };

            if self.rendered.touch(key)
                || self.rendering.contains(&key)
                || self.unavailable(message.uid)
            {
                continue;
            }

            // A body on its way is not in the Cache yet.
            let body = if self.fetching.contains(&message.uid) {
                None
            } else {
                cache.body(&self.folder, message.uid)?
            };

            match body {
                Some(body) => self.render(key, body, message),
                None if self.link == Link::Live => {
                    if index == selected {
                        request.selected = Some(message.uid);
                    } else {
                        request.neighbours.push(message.uid);
                    }
                }
                None if self.link == Link::Down => {
                    self.offline.insert(message.uid);
                }
                None => {}
            }
        }

        self.ask(request);
        if let Some(message) = view.selected_message() {
            self.ready_header(message);
        }

        Ok(())
    }

    /// Forgets what it knew by UID: those were another Folder's. Rendered
    /// Previews are kept by body, shared across Folders.
    fn switch(&mut self, folder: &Arc<str>) {
        self.folder = Arc::clone(folder);
        self.fetching.clear();
        self.asked = None;
        self.offline.clear();
        self.refused.clear();
        self.header = None;
    }

    /// Whether the body of `uid` cannot come for now.
    fn unavailable(&self, uid: u32) -> bool {
        self.offline.contains(&uid) || self.refused.contains_key(&uid)
    }

    /// Sends `request` unless the latest one covers it, selection included.
    fn ask(&mut self, mut request: BodyRequest) {
        let covered = request.selected.is_none_or(|uid| self.asked == Some(uid))
            && request
                .neighbours
                .iter()
                .all(|uid| self.fetching.contains(uid));
        if covered {
            return;
        }

        request.neighbours.sort_unstable();
        self.asked = request.selected;
        self.fetching.clear();
        self.fetching
            .extend(request.selected.iter().chain(&request.neighbours));
        // The send fails only once the body fetch has stopped.
        let _ = self.requests.unbounded_send(Request::Bodies(request));
    }

    fn render(&mut self, key: PreviewKey, body: Vec<u8>, message: &Message) {
        self.rendering.insert(key);
        let job = Job {
            key,
            body,
            sender: message.sender.clone(),
            subject: message.subject.clone(),
            received: message.received,
            colours: self.colours,
        };

        // The send fails only once the render thread is gone.
        let _ = self.jobs.send(job);
    }

    /// Keeps the header of `selected` from its row while its body cannot come.
    fn ready_header(&mut self, selected: &Message) {
        let unavailable =
            self.offline.contains(&selected.uid) || self.refused.contains_key(&selected.uid);
        let ready = self
            .header
            .as_ref()
            .is_some_and(|(uid, _)| *uid == selected.uid);
        if unavailable && !ready {
            let header = preview_render::header(
                selected.sender.clone(),
                selected.subject.clone(),
                selected.received,
            );
            self.header = Some((selected.uid, header));
        }
    }

    /// Bodies fetched in `folder`; those of a Folder no longer shown are
    /// in the Cache, and no UID here waits on them.
    pub fn fetched(&mut self, folder: &str, fetched: Fetched) {
        if *self.folder != *folder {
            return;
        }

        match fetched {
            Fetched::Stored(uids) => {
                for uid in uids {
                    self.fetching.remove(&uid);
                    self.offline.remove(&uid);
                }
            }
            Fetched::Refused { uids, reason } => {
                for uid in uids {
                    self.fetching.remove(&uid);
                    self.refused.insert(uid, format!("Couldn't load: {reason}"));
                }
            }
        }
    }

    /// Down, what was asked for will not come; back up, every body that
    /// could not come is asked for again as it is wanted.
    pub fn link(&mut self, live: bool) {
        if live {
            self.link = Link::Live;
            self.offline.clear();
            self.refused.clear();
        } else {
            self.link = Link::Down;
            self.fetching.clear();
            self.asked = None;
        }
    }

    pub fn rendered(&mut self, key: PreviewKey, preview: Preview) {
        self.rendering.remove(&key);
        self.rendered.insert(key, preview);
    }
}
