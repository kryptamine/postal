//! `UID MOVE`, and the `UID COPY` and `UID EXPUNGE` standing in for it.

use std::num::NonZeroU32;

use imap_next::{
    imap_types::{
        command::{Command, CommandBody},
        core::Vec1,
        extensions::uidplus::{UidElement, UidSet},
        response::{Code, Status},
    },
    server::Server,
};

use super::{FakeMessage, Inbox, Opened, announce, code, fetched, mailbox_name};

/// Answers a `UID MOVE`, a `UID COPY` or a `UID EXPUNGE` in the Folder
/// `opened`, with `COPYUID` as UIDPLUS sends it, and announces the
/// expunges to `view`. Into Gmail's Trash a Message loses every label.
pub(super) fn transfer(
    server: &mut Server,
    command: Command<'static>,
    (inbox, opened): (&mut Inbox, &Opened),
    view: &mut Vec<(u32, bool)>,
) -> Status<'static> {
    let tag = Some(command.tag);
    if inbox.folder(&opened.0).is_none() || !opened.1 {
        return Status::no(tag, None, "needs a Folder selected read-write").unwrap();
    }

    let (set, to, moving) = match command.body {
        CommandBody::Move {
            sequence_set,
            mailbox,
            uid: true,
        } => (sequence_set, Some(mailbox_name(&mailbox)), true),
        CommandBody::Copy {
            sequence_set,
            mailbox,
            uid: true,
        } => (sequence_set, Some(mailbox_name(&mailbox)), false),
        CommandBody::ExpungeUid { sequence_set } => (sequence_set, None, true),
        _ => return Status::bad(tag, None, "not faked").unwrap(),
    };

    let source = inbox.folder(&opened.0).unwrap();
    let picked: Vec<(u32, u64, FakeMessage, bool)> = fetched(source, &set, true, None)
        .map(|(_, s)| (s.uid, s.msgid, s.message.clone(), s.deleted))
        .collect();
    if to.as_ref().is_some_and(|to| inbox.folder(to).is_none()) {
        return Status::no(tag, Some(code("TRYCREATE")), "no such Folder").unwrap();
    }

    let copyuid = to.and_then(|to| copy_into(inbox, &opened.0, &to, &picked));

    if moving {
        let gone: Vec<u32> = picked
            .iter()
            .filter(|(.., deleted)| copyuid.is_some() || *deleted)
            .map(|&(uid, ..)| uid)
            .collect();
        let source = inbox.folder_mut(&opened.0);
        source.bump();
        source.messages.retain(|m| !gone.contains(&m.uid));
        if let Some(code) = copyuid.clone() {
            // A move sends its COPYUID untagged, ahead of the expunges (RFC 6851).
            server.enqueue_status(Status::ok(None, Some(code), "moved").unwrap());
        }

        announce(server, view, inbox.folder(&opened.0).unwrap());
        return Status::ok(tag, None, "completed").unwrap();
    }

    Status::ok(tag, copyuid, "completed").unwrap()
}

/// The source and destination sets of a `COPYUID`; `None` for no Message.
fn uid_sets(pairs: &[(u32, u32)]) -> Option<(UidSet, UidSet)> {
    let set = |uids: Vec<u32>| {
        let elements = uids
            .into_iter()
            .map(|uid| UidElement::Single(NonZeroU32::new(uid).unwrap()));
        Vec1::try_from(elements.collect::<Vec<_>>())
            .ok()
            .map(UidSet)
    };

    Some((
        set(pairs.iter().map(|&(from, _)| from).collect())?,
        set(pairs.iter().map(|&(_, to)| to).collect())?,
    ))
}

/// Copies `picked` into the Folder named `to`, and returns the `COPYUID`
/// for them.
fn copy_into(
    inbox: &mut Inbox,
    from: &str,
    to: &str,
    picked: &[(u32, u64, FakeMessage, bool)],
) -> Option<Code<'static>> {
    let target = inbox.folder(to).unwrap();

    let (into_trash, mirrored) = (target.trash(), !target.apart());
    let mut pairs = Vec::new();
    for (uid, msgid, message, _) in picked {
        if inbox.gmail && into_trash {
            for folder in inbox.folders.iter_mut().filter(|f| f.name != from) {
                folder.bump();
                folder.messages.retain(|m| m.msgid != *msgid);
            }
        } else if inbox.gmail && mirrored {
            for all in inbox.folders.iter_mut().filter(|f| f.mirrors) {
                all.push(*msgid, message.clone());
            }
        }

        pairs.push((*uid, inbox.folder_mut(to).push(*msgid, message.clone())));
    }

    let uid_validity = NonZeroU32::new(inbox.folder(to).unwrap().uid_validity).unwrap();
    uid_sets(&pairs).map(|(source, destination)| Code::CopyUid {
        uid_validity,
        source,
        destination,
    })
}
