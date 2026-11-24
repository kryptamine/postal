use std::{cell::Cell, collections::VecDeque};

use crate::{
    cache::Cache,
    domain::folder::{Folder, Role},
    domain::folder_name::FolderName,
    mail::backfill::{self, Backfill},
    mail::imap::{MailServer, Mode},
    mail::sync::{self, Error},
    mail::update::Update,
};

/// Messages per page: a Preview asked for meanwhile waits on one page.
const PAGE: u32 = 100;

/// Brings every Folder but Trash and Spam whole into the Cache, a page a
/// step, so the body pass reaches every Message: ADR 0032.
pub struct FolderPass {
    /// The Folders not yet whole; `None` until listed.
    left: Option<VecDeque<FolderName>>,
}

impl FolderPass {
    pub const fn new() -> Self {
        Self { left: None }
    }

    pub fn due(&self) -> bool {
        self.left.as_ref().is_none_or(|left| !left.is_empty())
    }

    /// Lists the Folders on the first step, then fetches one page of the
    /// first not yet whole.
    pub async fn step(
        &mut self,
        server: &mut impl MailServer,
        cache: &Cache,
        send: &impl Fn(Update),
    ) -> Result<(), Error> {
        if self.left.is_none() {
            cache.list_folders(&server.folders().await?)?;
            send(Update::Folders);
            self.left = Some(walked(cache.folders()?));
        }

        let Some(left) = &mut self.left else {
            return Ok(());
        };

        if let Some(folder) = left.front()
            && !page(server, cache, folder, send).await?
        {
            left.pop_front();
        }

        Ok(())
    }
}

/// On Gmail All Mail alone, which holds every label's Messages once; else
/// every Folder but Trash and Spam.
fn walked(folders: Vec<Folder>) -> VecDeque<FolderName> {
    if let Some(all) = folders.iter().find(|f| f.role == Some(Role::All)) {
        return VecDeque::from([all.name.clone()]);
    }

    let kept = folders.into_iter();
    let kept = kept.filter(|f| !matches!(f.role, Some(Role::Trash | Role::Spam)));
    kept.map(|f| f.name).collect()
}

/// The newest Messages of `folder` when it was never synced, else the page
/// older than every cached one. Returns whether more is left of it.
async fn page(
    server: &mut impl MailServer,
    cache: &Cache,
    folder: &FolderName,
    send: &impl Fn(Update),
) -> Result<bool, Error> {
    let fetched = if cache.state(folder)?.is_none() {
        // Examined: the sync connection's Mark Read never comes this way.
        sync::resync_as(server, cache, folder, Mode::Examine, &|_| {})
            .await
            .map(|_| true)
    } else if cache.oldest_uid(folder)?.is_none() {
        return Ok(false);
    } else {
        older(server, cache, folder).await
    };

    match fetched {
        Ok(more) => {
            send(Update::Inbox);
            Ok(more && cache.oldest_uid(folder)?.is_some())
        }
        Err(Error::Gone(error)) => {
            tracing::warn!("{folder} is gone: {error}");
            Ok(false)
        }
        Err(Error::Imap(error)) if error.refusal().is_some() => {
            tracing::warn!("fetching {folder} whole was refused: {error}");
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

/// The page older than every cached one; whether more is left.
async fn older(
    server: &mut impl MailServer,
    cache: &Cache,
    folder: &FolderName,
) -> Result<bool, Error> {
    // `Paged` with a UID means the server has nothing older, or the Folder
    // is gone or renumbered: the sync connection's to refetch.
    let whole = Cell::new(false);
    let report = |backfill| {
        if let Backfill::Paged {
            nothing_below: Some(_),
            ..
        } = backfill
        {
            whole.set(true);
        }
    };

    backfill::page(server, cache, folder, PAGE, report).await?;
    Ok(!whole.get())
}
