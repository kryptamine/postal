//! `UID STORE` of the flags the fake keeps.

use imap_next::imap_types::{
    command::{Command, CommandBody},
    flag::Flag,
    flag::{StoreResponse, StoreType},
    response::Status,
};

use super::{Inbox, Opened, Stored, fetch::fetched};

/// Applies a `UID STORE` of `\Seen` or `\Answered` to the Folder `opened`, and
/// `\Seen` to each copy of its Messages in other Folders, as Gmail shares it;
/// refused unless it was `SELECT`ed.
pub(super) fn store(
    inbox: &mut Inbox,
    command: Command<'static>,
    opened: &Opened,
) -> Status<'static> {
    let tag = Some(command.tag);
    let CommandBody::Store {
        sequence_set,
        kind,
        response: StoreResponse::Silent,
        flags,
        uid: true,
        ..
    } = command.body
    else {
        return Status::bad(tag, None, "not faked").unwrap();
    };

    let Some(folder) = inbox.folder(&opened.0).filter(|_| opened.1) else {
        return Status::no(tag, None, "STORE needs a Folder selected read-write").unwrap();
    };

    let added = |flag| kind == StoreType::Add && flags.contains(&flag);
    let only_here: Option<fn(&mut Stored)> = if added(Flag::Deleted) {
        Some(|stored| stored.deleted = true)
    } else if added(Flag::Answered) {
        Some(|stored| stored.message.answered = true)
    } else {
        None
    };

    if let Some(mark) = only_here {
        let uids: Vec<u32> = fetched(folder, &sequence_set, true, None)
            .map(|(_, stored)| stored.uid)
            .collect();
        let folder = inbox.folder_mut(&opened.0);
        for uid in uids {
            mark(folder.stored(uid));
        }

        return Status::ok(tag, None, "STORE completed").unwrap();
    }

    let has_seen = flags.contains(&Flag::Seen);
    let seen = match kind {
        StoreType::Add if has_seen => true,
        StoreType::Remove if has_seen => false,
        StoreType::Replace => has_seen,
        StoreType::Add | StoreType::Remove => {
            return Status::ok(tag, None, "STORE completed").unwrap();
        }
    };

    let msgids: Vec<u64> = fetched(folder, &sequence_set, true, None)
        .map(|(_, stored)| stored.msgid)
        .collect();
    for folder in &mut inbox.folders {
        let copies: Vec<u32> = folder
            .messages
            .iter()
            .filter(|m| msgids.contains(&m.msgid) && m.message.seen != seen)
            .map(|m| m.uid)
            .collect();
        for uid in copies {
            let modseq = folder.bump();
            let stored = folder.stored(uid);
            stored.modseq = modseq;
            stored.message.seen = seen;
        }
    }

    Status::ok(tag, None, "STORE completed").unwrap()
}
