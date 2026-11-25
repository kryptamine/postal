use std::error::Error;

use crate::{
    account::config::Config,
    account::xdg,
    compose::accounts::Accounts,
    compose::draft_file,
    compose::draft_notes::DraftNotes,
    compose::editor,
    compose::mailto::Mailto,
    compose::notes,
    domain::per_account::PerAccount,
    mail::sync_lock::{Kind, SyncLock},
};

/// `postal <link>` with no TUI open: the lock the TUI it starts holds.
pub struct Linked {
    pub lock: SyncLock,
    pub link: Mailto,
}

/// `None` once `link` is composed beside the TUI already open, which holds
/// the lock (ADR 0033).
pub fn open(link: Mailto) -> Result<Option<Linked>, Box<dyn Error>> {
    match SyncLock::try_take(Kind::Tui)? {
        Some(lock) => Ok(Some(Linked { lock, link })),
        None => compose(&link).map(|()| None),
    }
}

/// The Draft `link` fills, from the first Account, written beside an open
/// TUI and edited in `$EDITOR`; the TUI lists it in Local Drafts. A Draft
/// needs no lock, as it sits outside the Cache (ADR 0013).
#[expect(
    clippy::print_stderr,
    reason = "beside the TUI, stderr reaches the user"
)]
fn compose(link: &Mailto) -> Result<(), Box<dyn Error>> {
    let configs: PerAccount<Config> = Config::load(&xdg::config_dir()?.join("config.toml"))?
        .into_iter()
        .collect();
    let accounts = Accounts(&configs);
    let from = accounts.place(None);
    let signature = from.map_or(Ok(None), |from| accounts.outgoing(from).signature());
    let block = signature.as_ref().ok().and_then(Option::as_deref);
    let path = draft_file::create(&draft_file::dir()?, &link.draft(block))?;
    let notes = DraftNotes {
        account: from
            .and_then(|from| accounts.noted(from))
            .map(str::to_owned),
        answering: None,
    };

    draft_file::note(&path, &notes)?;
    editor::edit_draft(&path, accounts.format(notes.account.as_deref()))?;
    if let Err(error) = &signature {
        eprintln!("postal: could not read the signature: {error}");
    }

    if link.dropped_bcc {
        eprintln!("postal: the link's Bcc is dropped: a Draft has none");
    }

    if draft_file::untouched(&path, &draft_file::template(block)) {
        notes::remove_message(&path)?;
        eprintln!("postal: empty Draft dropped");
    }

    Ok(())
}
