use crate::{
    domain::listing::Listing, domain::message::Message, domain::message_ref::MessageRef,
    domain::per_account::AccountId, domain::thread::Thread,
};

/// The Messages `each` names of every Thread of `listing` that are on the
/// server, grouped by Account.
pub fn per_account<'a, I>(
    listing: &'a Listing,
    each: impl Fn(&'a Thread) -> I,
) -> Vec<(AccountId, Vec<MessageRef>)>
where
    I: Iterator<Item = &'a Message>,
{
    let mut accounts: Vec<(AccountId, Vec<MessageRef>)> = Vec::new();
    for thread in listing.threads() {
        let account = *thread.key.account();
        let messages = each(thread).filter_map(Message::on_server);
        match accounts.iter_mut().find(|(of, _)| *of == account) {
            Some((_, held)) => held.extend(messages),
            None => accounts.push((account, messages.collect())),
        }
    }

    accounts.retain(|(_, messages)| !messages.is_empty());
    accounts
}
