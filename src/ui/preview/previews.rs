use std::{
    collections::{HashMap, HashSet},
    panic,
    sync::{Arc, mpsc},
    thread,
};

use futures::channel::mpsc::UnboundedSender;

use crate::{
    account::config::Config,
    cache::{self, Cache},
    domain::folder,
    domain::folder_name::FolderName,
    domain::message::Message,
    domain::per_account::{AccountId, PerAccount},
    domain::uid::Uid,
    mail::body_fetch::{BodyRequest, Fetched, Request},
    mail::routes::Routes,
    mail::update::{BodyLink, Update},
    render::body::Body,
    render::html_job::HtmlJob,
    render::mail_look::MailLook,
    render::preview::Preview,
    render::preview_render::{self, Job},
    render::rendered_previews::{PreviewKey, RenderedPreviews},
    ui::list::inbox_view::InboxView,
    ui::preview::reading::Reading,
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

/// What the render thread does: a Message's Preview, or HTML of Postal's
/// own laid out, sent back as an Update of `account`.
enum Work {
    Preview(Job),
    Html(AccountId, HtmlJob),
}

/// Gets Previews ready: fetches bodies the Cache lacks, renders on a thread
/// of its own, and keeps what it rendered. Its UIDs are those of `folder`
/// of `account`, which is `None` until a Folder is asked of.
pub struct Previews {
    account: Option<AccountId>,
    folder: FolderName,
    rendered: RenderedPreviews,
    rendering: HashSet<PreviewKey>,
    /// UIDs of the latest request not yet answered: the server serves only
    /// that one. One it does not answer for stays here.
    fetching: HashSet<Uid>,
    /// The selection of the latest request.
    asked: Option<Uid>,
    /// UIDs found without a body while the link is down.
    offline: HashSet<Uid>,
    /// The placeholder of each body the server refused, until a reconnect.
    refused: HashMap<Uid, String>,
    links: PerAccount<Link>,
    /// Each Account's address, for the invitations its Previews show.
    emails: PerAccount<Arc<str>>,
    look: MailLook,
    requests: Routes<Request>,
    jobs: mpsc::Sender<Work>,
}

impl Previews {
    /// Starts the render thread, which sends each Preview back to `updates`.
    pub fn start(
        requests: Routes<Request>,
        accounts: &PerAccount<Config>,
        updates: UnboundedSender<(AccountId, Update)>,
        look: MailLook,
    ) -> Self {
        let (jobs, queue) = mpsc::channel::<Work>();
        thread::spawn(move || {
            for work in queue {
                if updates.unbounded_send(done(work)).is_err() {
                    return;
                }
            }
        });

        Self {
            account: None,
            folder: folder::INBOX.into(),
            rendered: RenderedPreviews::default(),
            rendering: HashSet::new(),
            fetching: HashSet::new(),
            asked: None,
            offline: HashSet::new(),
            refused: HashMap::new(),
            links: accounts.map(|_| Link::Starting),
            emails: accounts.map(|config| Arc::from(config.email.as_str())),
            look,
            requests,
            jobs,
        }
    }

    /// Renders every Preview again in `look`: the ends of buttons change
    /// with the font.
    pub fn set_look(&mut self, look: MailLook) {
        self.look = look;
        self.rendered = RenderedPreviews::default();
    }

    /// The selected Message's Preview at `width`, once rendered.
    pub fn get(&self, view: &InboxView, width: u16) -> Option<&Preview> {
        self.rendered
            .get(view.preview_key(view.selected_message()?, width))
    }

    pub fn body(&self, view: &InboxView, message: &Message, width: u16) -> Body<'_> {
        if let Some(preview) = self.rendered.get(view.preview_key(message, width)) {
            return Body::Rendered(preview);
        }

        // What is known by UID is of `folder` alone.
        if !self.holds(message) {
            return Body::Loading;
        }

        match self.refused.get(&message.uid) {
            Some(why) => Body::Unavailable(why),
            None if self.offline.contains(&message.uid) => Body::Unavailable(OFFLINE),
            None => Body::Loading,
        }
    }

    /// Where the selected Message's Preview stands; `Loading` with none.
    pub fn selected(&self, view: &InboxView, width: u16) -> Body<'_> {
        view.selected_message()
            .map_or(Body::Loading, |message| self.body(view, message, width))
    }

    /// Readies the Previews of the Messages `reading` unfolds, or else of the
    /// selection and its neighbours, at `width`.
    pub fn want(
        &mut self,
        caches: &PerAccount<Cache>,
        view: &InboxView,
        reading: Option<&Reading>,
        width: u16,
    ) -> Result<(), cache::Error> {
        match reading {
            Some(reading) => self.want_stack(caches, view, reading, width),
            None => self.want_around(caches, view, width),
        }
    }

    /// When all are rendered already this does not allocate.
    fn want_around(
        &mut self,
        caches: &PerAccount<Cache>,
        view: &InboxView,
        width: u16,
    ) -> Result<(), cache::Error> {
        let selected = view.selected;
        let around = selected.saturating_sub(NEIGHBOURS)..=selected + NEIGHBOURS;
        let rows = std::iter::once(selected).chain(around.filter(|&i| i != selected));
        let wanted = rows.filter_map(|index| Some((view.row(index)?, index == selected)));
        let folder = view
            .selected_message()
            .and_then(|m| Some((m.account, m.folder.as_ref()?)));
        let key = |m: &Message| view.preview_key(m, width);
        self.ready(caches, folder.or_else(|| shown(view)), wanted, key)?;
        Ok(())
    }

    /// The one at the cursor is asked for first.
    fn want_stack(
        &mut self,
        caches: &PerAccount<Cache>,
        view: &InboxView,
        reading: &Reading,
        width: u16,
    ) -> Result<(), cache::Error> {
        let cursor = reading.cursor;
        let stack = view.stack().filter(|m| reading.unfolded(m.body_key));
        let wanted = stack.clone().map(|m| (m, m.body_key == cursor));
        let at_cursor = stack
            .filter(|m| m.body_key == cursor)
            .find_map(|m| Some((m.account, m.folder.as_ref()?)));
        let key = |m: &Message| view.preview_key(m, width);
        self.ready(caches, at_cursor.or_else(|| shown(view)), wanted, key)
    }

    /// Renders each `(message, selected)` whose body is cached under its `key`
    /// and asks the server in `folder`, if any, for the rest of that Folder's.
    /// When all are rendered already this does not allocate.
    fn ready<'m>(
        &mut self,
        caches: &PerAccount<Cache>,
        folder: Option<(AccountId, &FolderName)>,
        wanted: impl Iterator<Item = (&'m Message, bool)>,
        key: impl Fn(&Message) -> PreviewKey,
    ) -> Result<(), cache::Error> {
        if let Some((account, folder)) = folder
            && (self.account != Some(account) || self.folder != *folder)
        {
            self.switch(account, folder);
        }

        let live = self.account.map(|account| self.links[account]);

        let mut request = BodyRequest {
            folder: self.folder.clone(),
            selected: None,
            neighbours: Vec::new(),
        };

        for (message, selected) in wanted {
            let key = key(message);
            let here = folder.is_some() && self.holds(message);
            if self.rendered.touch(key)
                || self.rendering.contains(&key)
                || (here && self.unavailable(message.uid))
            {
                continue;
            }

            // A body on its way is not in the Cache yet.
            let body = if here && self.fetching.contains(&message.uid) {
                None
            } else {
                caches[message.account].body(message.body_key)?
            };

            match body {
                Some(body) => self.render(key, body),
                // It came with the Thread, or comes once its Folder is asked.
                None if !here => {}
                None if live == Some(Link::Live) => {
                    if selected {
                        request.selected = Some(message.uid);
                    } else {
                        request.neighbours.push(message.uid);
                    }
                }
                None if live == Some(Link::Down) => {
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
    fn switch(&mut self, account: AccountId, folder: &FolderName) {
        self.account = Some(account);
        self.folder = folder.clone();
        self.fetching.clear();
        self.asked = None;
        self.offline.clear();
        self.refused.clear();
    }

    /// Whether `message`'s UID is one of `folder`'s.
    fn holds(&self, message: &Message) -> bool {
        self.account == Some(message.account) && message.folder.as_ref() == Some(&self.folder)
    }

    /// Whether the body of `uid` cannot come for now.
    fn unavailable(&self, uid: Uid) -> bool {
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
        if let Some(account) = self.account {
            self.requests.send(account, Request::Bodies(request));
        }
    }

    fn render(&mut self, key: PreviewKey, body: Vec<u8>) {
        self.rendering.insert(key);
        let job = Job {
            key,
            body,
            look: self.look,
            own: Arc::clone(&self.emails[key.account]),
        };

        // The send fails only once the render thread is gone.
        self.jobs.send(Work::Preview(job)).ok();
    }

    /// Lays `job` out on the render thread, behind the Previews asked
    /// before it; `account` names the Update it comes back as.
    pub fn lay_out(&self, account: AccountId, job: HtmlJob) {
        self.jobs.send(Work::Html(account, job)).ok();
    }

    /// Bodies fetched in `folder`; those of a Folder no longer shown are
    /// in the Cache, and no UID here waits on them.
    pub fn fetched(&mut self, account: AccountId, folder: &FolderName, fetched: Fetched) {
        if self.account != Some(account) || self.folder != *folder {
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
    pub fn link(&mut self, account: AccountId, link: BodyLink) {
        let live = link == BodyLink::Up;
        self.links[account] = if live { Link::Live } else { Link::Down };

        if Some(account) != self.account {
            return;
        }

        if live {
            self.offline.clear();
            self.refused.clear();
        } else {
            self.fetching.clear();
            self.asked = None;
        }
    }

    pub fn rendered(&mut self, key: PreviewKey, preview: Preview) {
        self.rendering.remove(&key);
        self.rendered.insert(key, preview);
    }
}

/// `work` done as the Update it comes back as. A panic on hostile mail
/// fails that one job and leaves the thread to render the rest.
fn done(work: Work) -> (AccountId, Update) {
    match work {
        Work::Preview(job) => {
            let key = job.key;
            let preview = panic::catch_unwind(|| preview_render::render(&job))
                .unwrap_or_else(|_| preview_render::failed(key));
            (key.account, Update::Rendered(key, preview))
        }
        Work::Html(account, job) => (account, Update::LaidOut(job.run())),
    }
}

/// The Folder the list shows, none in All Inboxes.
fn shown(view: &InboxView) -> Option<(AccountId, &FolderName)> {
    view.folder.folder().map(|f| (f.account, &f.name))
}
