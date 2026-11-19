use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, mpsc},
    thread,
};

use futures::channel::mpsc::UnboundedSender;

use crate::{
    body::Body,
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
    view::View,
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
        self.of(view.selected_message()?, view.html, width)
    }

    /// The Preview of `message` in `html` at `width`, once rendered.
    pub fn of(&self, message: &Message, html: View, width: u16) -> Option<&Preview> {
        self.rendered.get(PreviewKey {
            body: message.body_key,
            width,
            view: html,
        })
    }

    pub fn body(&self, message: &Message, html: View, width: u16) -> Body<'_> {
        if let Some(preview) = self.of(message, html, width) {
            return Body::Rendered(preview);
        }

        // A Message from elsewhere is never asked for: its UID is none of ours.
        if message.elsewhere {
            return Body::Loading;
        }

        match self.refused.get(&message.uid) {
            Some(why) => Body::Unavailable(why),
            None if self.offline.contains(&message.uid) => Body::Unavailable(OFFLINE),
            None => Body::Loading,
        }
    }

    pub fn shown(&self, view: &InboxView, width: u16) -> Shown<'_> {
        let Some(message) = view.selected_message() else {
            return Shown::Loading;
        };

        let why = match self.body(message, view.html, width) {
            Body::Rendered(preview) => return Shown::Rendered(preview),
            Body::Loading => return Shown::Loading,
            Body::Unavailable(why) => why,
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
        let selected = view.selected;
        let around = selected.saturating_sub(NEIGHBOURS)..=selected + NEIGHBOURS;
        let rows = std::iter::once(selected).chain(around.filter(|&i| i != selected));
        let wanted = rows.filter_map(|index| Some((view.row(index)?, index == selected)));
        self.ready(cache, view, wanted, width)?;
        if let Some(message) = view.selected_message() {
            self.ready_header(message);
        }

        Ok(())
    }

    /// Readies the Previews of the selected Thread's `unfolded` Messages at
    /// `width`, the one at `cursor` asked for first.
    pub fn want_stack(
        &mut self,
        cache: &Cache,
        view: &InboxView,
        (unfolded, cursor): (impl Fn(i64) -> bool, i64),
        width: u16,
    ) -> Result<(), cache::Error> {
        let stack = view.stack().filter(|m| unfolded(m.body_key));
        let wanted = stack.map(|m| (m, m.body_key == cursor));
        self.ready(cache, view, wanted, width)
    }

    /// Renders each `(message, selected)` whose body is cached and asks the
    /// server for the rest. When all are rendered already this does not allocate.
    fn ready<'m>(
        &mut self,
        cache: &Cache,
        view: &InboxView,
        wanted: impl Iterator<Item = (&'m Message, bool)>,
        width: u16,
    ) -> Result<(), cache::Error> {
        if self.folder != view.folder.name {
            self.switch(&view.folder.name);
        }

        let mut request = BodyRequest {
            folder: Arc::clone(&self.folder),
            selected: None,
            neighbours: Vec::new(),
        };

        for (message, selected) in wanted {
            let key = PreviewKey {
                body: message.body_key,
                width,
                view: view.html,
            };

            if self.rendered.touch(key)
                || self.rendering.contains(&key)
                || (!message.elsewhere && self.unavailable(message.uid))
            {
                continue;
            }

            // A body on its way is not in the Cache yet.
            let body = if !message.elsewhere && self.fetching.contains(&message.uid) {
                None
            } else {
                cache.body(message.body_key)?
            };

            match body {
                Some(body) => self.render(key, body, message),
                // It came with the Thread, or does not come at all.
                None if message.elsewhere => {}
                None if self.link == Link::Live => {
                    if selected {
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
        self.requests.unbounded_send(Request::Bodies(request)).ok();
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
        self.jobs.send(job).ok();
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
