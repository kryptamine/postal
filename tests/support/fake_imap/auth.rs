//! Signing in, by `AUTHENTICATE XOAUTH2` or `LOGIN`, and the capabilities
//! the fake lists.

use imap_next::{
    imap_types::{
        auth::{AuthMechanism, AuthenticateData},
        core::{AString, Atom, Tag, Vec1},
        response::{Capability, CommandContinuationRequest, Data, Status},
    },
    server::Server,
    types::CommandAuthenticate,
};

use super::{ACCESS_TOKEN, APP_PASSWORD, EMAIL, Inbox, Outage, code};

/// Asks for the XOAUTH2 response and returns the tag to answer it under;
/// any other mechanism is refused at once.
pub(super) fn start_authenticate(
    server: &mut Server,
    command: CommandAuthenticate,
) -> Option<Tag<'static>> {
    let tag = Some(command.tag);
    if command.mechanism != AuthMechanism::XOAuth2 {
        let refused = Status::no(tag, None, "mechanism not faked").unwrap();
        server.authenticate_finish(refused).unwrap();
        return None;
    }

    let ready = CommandContinuationRequest::base64(Vec::new());
    server.authenticate_continue(ready).unwrap();
    tag
}

pub(super) fn finish_authenticate(
    server: &mut Server,
    tag: Option<Tag<'static>>,
    data: &AuthenticateData,
    outage: Option<Outage>,
) {
    let ok = matches!(data, AuthenticateData::Continue(data)
        if data.declassify().as_ref() == xoauth2().as_bytes());
    let status = match outage {
        Some(Outage::Throttled) => Status::no(tag, Some(code("THROTTLED")), "Too many logins"),
        Some(Outage::Revoked) => Status::no(
            tag,
            Some(code("AUTHENTICATIONFAILED")),
            "Invalid credentials",
        ),
        _ if ok => Status::ok(tag, None, "AUTHENTICATE completed"),
        _ => Status::no(
            tag,
            Some(code("AUTHENTICATIONFAILED")),
            "Invalid credentials",
        ),
    };

    server.authenticate_finish(status.unwrap()).unwrap();
}

/// Answers `LOGIN` as `AUTHENTICATE` is, taking `APP_PASSWORD` as the
/// user the test set.
pub(super) fn login(
    inbox: &Inbox,
    tag: Tag<'static>,
    (user, password): (&AString, &AString),
) -> Status<'static> {
    let text = |value: &AString| String::from_utf8(value.as_ref().to_vec()).unwrap();
    let ok = text(user) == inbox.login_user && text(password) == APP_PASSWORD;
    let tag = Some(tag);
    let status = match inbox.outage {
        Some(Outage::Throttled) => Status::no(tag, Some(code("THROTTLED")), "Too many logins"),
        Some(Outage::Revoked) => Status::no(
            tag,
            Some(code("AUTHENTICATIONFAILED")),
            "Invalid credentials",
        ),
        _ if ok => Status::ok(tag, None, "LOGIN completed"),
        _ => Status::no(
            tag,
            Some(code("AUTHENTICATIONFAILED")),
            "Invalid credentials",
        ),
    };

    status.unwrap()
}

/// The SASL XOAUTH2 response of the Account with `ACCESS_TOKEN`.
pub fn xoauth2() -> String {
    format!("user={EMAIL}\x01auth=Bearer {ACCESS_TOKEN}\x01\x01")
}

pub(super) fn capabilities(inbox: &Inbox) -> Data<'static> {
    let mut capabilities = vec![Capability::Imap4Rev1, Capability::UidPlus];
    if inbox.listed.moves {
        capabilities.push(Capability::Move);
    }

    if inbox.listed.idle {
        capabilities.push(Capability::Idle);
    }

    if inbox.gmail {
        capabilities.push(Capability::from(Atom::try_from("X-GM-EXT-1").unwrap()));
    }

    Data::Capability(Vec1::try_from(capabilities).unwrap())
}
