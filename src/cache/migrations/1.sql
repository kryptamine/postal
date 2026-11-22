-- Every row names its Account by address, lowercased.
-- One row per Folder, from `LIST` or a resync; the sync columns are NULL
-- until the Folder is first resynced.
CREATE TABLE mailbox (
    account TEXT NOT NULL,
    name TEXT NOT NULL,
    -- Its special use (RFC 6154), as `Role::label` names it; NULL for none.
    role TEXT,
    uid_validity INTEGER,
    uid_next INTEGER,
    highest_modseq INTEGER,
    -- The server's `EXISTS` as the last resync or IDLE update saw it.
    exists_count INTEGER,
    -- NULL until a resync or a `STATUS` counts it.
    unread INTEGER,
    PRIMARY KEY (account, name)
);
CREATE TABLE message (
    account TEXT NOT NULL,
    mailbox TEXT NOT NULL,
    uid INTEGER NOT NULL,
    -- The `body` row of this Message, once fetched: see docs/adr/0005-body-cache-key.md.
    body_key INTEGER NOT NULL,
    received INTEGER NOT NULL,
    received_offset INTEGER NOT NULL,
    unread INTEGER NOT NULL,
    answered INTEGER NOT NULL,
    sender TEXT NOT NULL,
    from_line TEXT NOT NULL,
    to_line TEXT NOT NULL,
    subject TEXT NOT NULL,
    -- Gmail's `X-GM-THRID`; NULL on another server.
    thread INTEGER,
    from_me INTEGER NOT NULL,
    -- NULL until the second fetch phase writes it.
    snippet TEXT,
    -- The part the Snippet and the Preview show; NULL with no text part.
    text_section TEXT,
    text_encoding TEXT,
    text_charset TEXT,
    text_html INTEGER,
    PRIMARY KEY (account, mailbox, uid)
);
CREATE INDEX message_by_arrival ON message (account, mailbox, received DESC, uid DESC);
-- A Thread's Messages as `[Gmail]/All Mail` held them when it was last read;
-- see docs/adr/0009-threads-from-gmail-thread-id.md.
CREATE TABLE thread_message (
    account TEXT NOT NULL,
    -- Its `X-GM-MSGID`, the key its body is cached under.
    key INTEGER NOT NULL,
    thread INTEGER NOT NULL,
    received INTEGER NOT NULL,
    received_offset INTEGER NOT NULL,
    unread INTEGER NOT NULL,
    answered INTEGER NOT NULL,
    sender TEXT NOT NULL,
    from_line TEXT NOT NULL,
    to_line TEXT NOT NULL,
    subject TEXT NOT NULL,
    from_me INTEGER NOT NULL,
    snippet TEXT NOT NULL,
    PRIMARY KEY (account, key)
);
CREATE INDEX thread_message_by_thread ON thread_message (account, thread);
-- Shared by every Folder, and not dropped with the rows on a UIDVALIDITY
-- change; a byte-bounded LRU.
CREATE TABLE body (
    account TEXT NOT NULL,
    key INTEGER NOT NULL,
    raw BLOB NOT NULL,
    size INTEGER NOT NULL,
    -- Bumped on every write and read; the lowest goes first.
    used INTEGER NOT NULL,
    PRIMARY KEY (account, key)
);
CREATE INDEX body_by_use ON body (used);
-- A flag change the server has not acknowledged yet; a resync leaves that
-- flag of the row alone. See docs/adr/0011-select-the-shown-folder.md.
CREATE TABLE flag_change (
    account TEXT NOT NULL,
    mailbox TEXT NOT NULL,
    uid INTEGER NOT NULL,
    -- As IMAP names it: `\Seen` or `\Answered`.
    flag TEXT NOT NULL,
    added INTEGER NOT NULL,
    PRIMARY KEY (account, mailbox, uid, flag)
);
-- A Message a Delete took out of `message`, with its row, while the
-- server or an Undo may still move it: docs/adr/0012-delete-and-undo.md.
CREATE TABLE moved (
    id INTEGER PRIMARY KEY,
    -- One per Delete, shared by its Messages.
    batch INTEGER NOT NULL,
    -- Whether the Message is wanted in Trash and whether the server has it
    -- there, under `at_uid` there or in `mailbox`; NULL when never said.
    want_trash INTEGER NOT NULL,
    at_trash INTEGER NOT NULL,
    at_uid INTEGER,
    trash TEXT,
    account TEXT NOT NULL,
    mailbox TEXT NOT NULL,
    uid INTEGER NOT NULL,
    body_key INTEGER NOT NULL,
    received INTEGER NOT NULL,
    received_offset INTEGER NOT NULL,
    unread INTEGER NOT NULL,
    answered INTEGER NOT NULL,
    sender TEXT NOT NULL,
    from_line TEXT NOT NULL,
    to_line TEXT NOT NULL,
    subject TEXT NOT NULL,
    thread INTEGER,
    from_me INTEGER NOT NULL,
    snippet TEXT,
    text_section TEXT,
    text_encoding TEXT,
    text_charset TEXT,
    text_html INTEGER
);
-- What the server holds in `mailbox` that the Message List leaves out.
-- An address a Message names, once per Message whichever Folders hold it;
-- none from Trash or Spam: docs/adr/0016-expand-recipients-after-the-editor.md.
CREATE TABLE correspondence (
    account TEXT NOT NULL,
    -- Lowercased.
    address TEXT NOT NULL,
    -- Its `Message-ID`, else a key of its own.
    message TEXT NOT NULL,
    name TEXT,
    -- Whether the Account wrote to it, rather than had mail from or with it.
    written INTEGER NOT NULL,
    at INTEGER NOT NULL,
    PRIMARY KEY (account, address, message)
);
-- How far the pass over the Folder with the Sent role has read.
CREATE TABLE sent_pass (
    account TEXT PRIMARY KEY,
    mailbox TEXT NOT NULL,
    uid_validity INTEGER NOT NULL,
    last_uid INTEGER NOT NULL
);
CREATE VIEW deleted AS
    SELECT account, mailbox, at_uid AS uid FROM moved WHERE want_trash AND NOT at_trash;
-- Rows an Undo put back under `uid`, with where the server has the Message.
CREATE VIEW undone AS
    SELECT account, mailbox, uid, at_trash, at_uid FROM moved WHERE NOT want_trash;
