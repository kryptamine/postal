use std::{
    panic,
    sync::mpsc::{self, Receiver},
    thread,
};

use futures::channel::mpsc::UnboundedSender;

use crate::{
    domain::per_account::AccountId,
    mail::update::Update,
    render::html_job::HtmlJob,
    render::preview_render::{self, Job},
    render::rendered_previews::PreviewKey,
};

/// What the render thread does: a Message's Preview, or HTML of Postal's
/// own laid out, sent back as an Update of `account`.
enum Work {
    Preview(Job),
    Html(AccountId, HtmlJob),
}

enum Order {
    Work(Work),
    Wanted(PreviewKey),
}

/// The render thread and its queue. Work waiting is taken in the order of
/// what is on screen: the wanted Preview, then the HTML of a Draft, then
/// the Preview asked last, which is nearest the selection.
pub struct RenderQueue {
    orders: mpsc::Sender<Order>,
    wanted: Option<PreviewKey>,
}

impl RenderQueue {
    /// Starts the render thread, which sends what it did back to `updates`.
    pub fn start(updates: UnboundedSender<(AccountId, Update)>) -> Self {
        let (orders, queue) = mpsc::channel();
        thread::spawn(move || serve(&queue, &updates));
        Self {
            orders,
            wanted: None,
        }
    }

    pub fn render(&self, job: Job) {
        self.send(Order::Work(Work::Preview(job)));
    }

    pub fn lay_out(&self, account: AccountId, job: HtmlJob) {
        self.send(Order::Work(Work::Html(account, job)));
    }

    /// Puts the Preview of `key`, once asked, ahead of all the others.
    pub fn want(&mut self, key: PreviewKey) {
        if self.wanted != Some(key) {
            self.wanted = Some(key);
            self.send(Order::Wanted(key));
        }
    }

    fn send(&self, order: Order) {
        // The send fails only once the render thread is gone.
        self.orders.send(order).ok();
    }
}

fn serve(queue: &Receiver<Order>, updates: &UnboundedSender<(AccountId, Update)>) {
    let mut waiting = Vec::new();
    let mut wanted = None;
    loop {
        if waiting.is_empty() {
            let Ok(order) = queue.recv() else {
                return;
            };

            take(order, &mut waiting, &mut wanted);
        }

        while let Ok(order) = queue.try_recv() {
            take(order, &mut waiting, &mut wanted);
        }

        let Some(work) = next(&mut waiting, wanted) else {
            continue;
        };

        if updates.unbounded_send(done(work)).is_err() {
            return;
        }
    }
}

fn take(order: Order, waiting: &mut Vec<Work>, wanted: &mut Option<PreviewKey>) {
    match order {
        Order::Work(work) => waiting.push(work),
        Order::Wanted(key) => *wanted = Some(key),
    }
}

fn next(waiting: &mut Vec<Work>, wanted: Option<PreviewKey>) -> Option<Work> {
    let at = waiting
        .iter()
        .position(|work| matches!(work, Work::Preview(job) if Some(job.key) == wanted))
        .or_else(|| {
            waiting
                .iter()
                .position(|work| matches!(work, Work::Html(..)))
        })
        .or_else(|| waiting.len().checked_sub(1))?;
    Some(waiting.remove(at))
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
