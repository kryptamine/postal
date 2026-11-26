# The people in macOS Contacts are Correspondents, read and never kept

Correspondents are learnt from mail (ADR 0016), so a Draft's recipient
completes only to someone the user has already mailed from this machine,
under the name mail gave them (issue #191). On macOS the user's own address
book is at hand. With `contacts = "macos"` in `config.toml`, a switch on
the Contacts tab of Settings, off by default, each email address of a
person in Contacts is a Correspondent too.

**Read, never kept.** Postal still keeps no address book: nothing from
Contacts is written to the Cache. `CNContactStore` is read, for the given
name, the family name and the email addresses alone, each time the
Correspondents are: by every `postal correspondent list`, and so by the
MCP tool `correspondent_list`, and by each Search the TUI opens. An edit in
Contacts shows the next time, and turning the switch off leaves nothing
behind.

**In a process of its own.** The framework keeps about 10 MB after a
read, and a TUI that had opened one Search stood at 50 MB, past the 30 MB
of ADR 0001. So `postal` reads the store started again as a process of its
own (`account::contacts_process`), as the rendered View is laid out (ADR
0006), and writes the people to stdout; the memory is the system's again
when it ends. The process costs about 20 ms beside the read's 240.

**Ranking.** The name in Contacts wins over the one mail uses, since the
user chose it. An address that also has mail ranks as ADR 0016 says. One
known from Contacts alone ranks below every address with mail; those sort
by name, then address, the nameless last, and the Accounts' own addresses
stay out. In the CLI it is of the Account the Draft goes out from, never
written to.

**The permission.** macOS asks once, on the first read, and gives the
answer to the responsible process, usually the terminal app. A store read
off the main thread waits while macOS asks, so turning the switch on in
Settings reads at once on a thread of its own: the prompt comes then,
rather than in the middle of a completion. A refusal, then or later, leaves
the Correspondents of mail alone and one line in the log; a Toast says so
only for the read the switch started. Nothing that completes a recipient
fails for it.

**Off macOS** the tab is not shown, and the key is read but ignored with a
warning in the log.

**An agent sees the address book.** Through `correspondent_list` an agent
reads every person in Contacts with an email address, which the switch's
row says.

**The test seam.** The Contacts framework sits behind a trait, and
`POSTAL_CONTACTS_FAKE` names a JSON file of people for it, or `denied`.
The OS is faked here, not a mail server: an exception to ADR 0002.

## Considered options

- **Copy Contacts into the Cache on each sync.** Faster to read, but an
  address book Postal keeps, stale between syncs, and left behind once the
  switch is off.
- **Ask for access with `requestAccessForEntityType`.** Needs a callback
  and a run loop; the blocking read already asks.
- **CardDAV, the Google People API, vCard files.** Each a sync and a
  credential of its own; out of scope here.
