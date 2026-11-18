use std::{collections::HashSet, sync::mpsc, thread};

use futures::channel::mpsc::UnboundedSender;

use crate::{
    body_fetch::BodyRequest,
    cache::{self, Cache},
    inbox_view::InboxView,
    preview::Preview,
    preview_render::{self, Job},
    rendered_previews::{PreviewKey, RenderedPreviews},
    update::Update,
};

/// Rows on each side of the selection whose Preview is made ready too.
const NEIGHBOURS: usize = 2;

/// Gets Previews ready: fetches bodies the Cache lacks, renders on a thread
/// of its own, and keeps what it rendered.
pub struct Previews {
    rendered: RenderedPreviews,
    rendering: HashSet<PreviewKey>,
    /// UIDs asked of the server; one it does not answer for stays here.
    fetching: HashSet<u32>,
    requests: UnboundedSender<BodyRequest>,
    jobs: mpsc::Sender<Job>,
}

impl Previews {
    /// Starts the render thread, which sends each Preview back to `updates`.
    pub fn start(requests: UnboundedSender<BodyRequest>, updates: UnboundedSender<Update>) -> Self {
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
            rendered: RenderedPreviews::default(),
            rendering: HashSet::new(),
            fetching: HashSet::new(),
            requests,
            jobs,
        }
    }

    /// The selected Message's Preview at `width`, once rendered.
    pub fn get(&self, view: &InboxView, width: u16) -> Option<&Preview> {
        let message = view.messages.as_ref()?.get(view.selected)?;
        self.rendered.get(PreviewKey {
            body: message.body_key,
            width,
        })
    }

    /// Readies the Previews of the selection and its neighbours at `width`.
    /// When all are rendered already this does not allocate.
    pub fn want(
        &mut self,
        cache: &Cache,
        view: &InboxView,
        width: u16,
    ) -> Result<(), cache::Error> {
        let Some(messages) = view.messages.as_deref() else {
            return Ok(());
        };

        let selected = view.selected;
        let around = selected.saturating_sub(NEIGHBOURS)..=selected + NEIGHBOURS;
        let mut request = BodyRequest {
            selected: None,
            neighbours: Vec::new(),
        };

        for index in std::iter::once(selected).chain(around.filter(|&i| i != selected)) {
            let Some(message) = messages.get(index) else {
                continue;
            };

            let key = PreviewKey {
                body: message.body_key,
                width,
            };

            if self.rendered.touch(key)
                || self.rendering.contains(&key)
                || self.fetching.contains(&message.uid)
            {
                continue;
            }

            if let Some(body) = cache.body(message.uid)? {
                self.rendering.insert(key);
                let job = Job {
                    key,
                    body,
                    sender: message.sender.clone(),
                    subject: message.subject.clone(),
                    received: message.received,
                };

                // The send fails only once the render thread is gone.
                let _ = self.jobs.send(job);
            } else {
                self.fetching.insert(message.uid);
                if index == selected {
                    request.selected = Some(message.uid);
                } else {
                    request.neighbours.push(message.uid);
                }
            }
        }

        if request.selected.is_some() || !request.neighbours.is_empty() {
            request.neighbours.sort_unstable();
            // The send fails only once the body fetch has stopped.
            let _ = self.requests.unbounded_send(request);
        }

        Ok(())
    }

    pub fn stored(&mut self, uids: &[u32]) {
        for uid in uids {
            self.fetching.remove(uid);
        }
    }

    pub fn rendered(&mut self, key: PreviewKey, preview: Preview) {
        self.rendering.remove(&key);
        self.rendered.insert(key, preview);
    }
}
