# Recipients expand after the editor, from Correspondents learnt from mail

Superseded by ADR 0017: recipients are completed in the editor, and
nothing is expanded once it exits. The way Correspondents are learnt and
ranked, below, still holds.

A Draft is written in the user's own `$EDITOR`, which Postal cannot reach
while it runs, so there is no completion as one types. Instead a recipient
in `To:` or `Cc:` that is not an address, such as `маша` or `ivan@g`, is
expanded once the editor exits: every word of it must begin a word of a
Correspondent's name or address, case aside. One match replaces it in the
Draft's file at once; several open a picker over the Draft's preview; none
leave it as written, and the Draft cannot be sent until it is fixed. The
preview already stands between the editor and a Send, so it is where an
expansion is checked. Prefixes, not the fuzzy matcher of a Search (ADR
0007): a loose match here sends mail to the wrong person.

Correspondents are learnt from mail alone, into a table of the Cache, one
row per address and `Message-ID`, so a copy in another Folder counts once:
the sender and recipients of each Message fetched into a Folder but Trash
and Spam, those of each Message SMTP accepts from the Outbox, and once the
fetch connection is quiet after it connects, the envelopes alone of the
newest 1000 Messages in the Folder with the Sent role, which the Cache
otherwise holds only once that Folder is shown. Everything in Sent counts
as written by the Account, from whichever of its addresses, and its sender
is no Correspondent. Messages a Thread brings
from other Folders are not learnt from.

Every Account's Correspondents are offered, the Draft's Account's first.
Those the user wrote to rank above those who only wrote, then by how often
and how lately; the Accounts themselves, an address seen only in Trash or
Spam, and a no-reply address never written to are left out. The name used
is the one the user last wrote to, else the last one seen.

## Considered options

- **Completion inside the editor**, through a command that prints matches
  for the editor to call, as mutt's `query_command`. Needs set-up in each
  editor; may come later on top of the same table.
- **A recipient picker before the editor opens.** A step on every Compose,
  and no help with a Cc added to a Reply.
- **Google Contacts through the People API.** The address book the user
  keeps, at the cost of another OAuth scope, a second sync, and nothing for
  iCloud short of CardDAV.
- **Rows of the Message List for the Sent pass.** Would leave Sent's list
  with rows that have no Snippet or body.
