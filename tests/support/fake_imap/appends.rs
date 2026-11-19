//! `APPEND`, as a client saves the copy of a Message it sent.

use imap_next::imap_types::{
    command::{Command, CommandBody},
    extensions::binary::LiteralOrLiteral8,
    response::Status,
};

use super::{Inbox, code, mailbox_name};

/// Records the Message an `APPEND` carries, with its Folder; it is not
/// added to that Folder's Messages.
pub(super) fn append(command: Command<'static>, inbox: &mut Inbox) -> Status<'static> {
    let tag = Some(command.tag);
    let CommandBody::Append {
        mailbox, message, ..
    } = command.body
    else {
        return Status::bad(tag, None, "not faked").unwrap();
    };

    let folder = mailbox_name(&mailbox);
    if inbox.folder(&folder).is_none() {
        return Status::no(tag, Some(code("TRYCREATE")), "no such Folder").unwrap();
    }

    let data = match message {
        LiteralOrLiteral8::Literal(literal) => literal.data().to_vec(),
        LiteralOrLiteral8::Literal8(literal) => literal.data.to_vec(),
    };

    inbox
        .appended
        .push((folder, String::from_utf8(data).unwrap()));
    Status::ok(tag, None, "APPEND completed").unwrap()
}
