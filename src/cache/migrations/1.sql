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
    -- One `size\tname` line per attachment; empty with none.
    attachments TEXT NOT NULL,
    PRIMARY KEY (account, mailbox, uid)
);
CREATE INDEX message_by_arrival
    ON message (account, mailbox, received DESC, uid DESC, thread, body_key);
CREATE INDEX message_by_thread ON message (account, thread, mailbox);
-- The words a Search finds a row by: docs/adr/0023-search-finds-candidates-in-fts5-and-ranks-them-with-nucleo.md.
-- Keyed by the row's rowid, which the triggers below set to its arrival
-- second shifted left 24 bits, so `ORDER BY rowid DESC` reads newest first.
CREATE VIRTUAL TABLE message_text USING fts5(
    sender, subject, snippet, recipients,
    content = '', contentless_delete = 1,
    tokenize = 'unicode61 remove_diacritics 2', prefix = '1 2 3 4'
);
-- `INSERT OR REPLACE` fires no delete trigger for the row it replaces.
CREATE TRIGGER message_replaced BEFORE INSERT ON message BEGIN
    DELETE FROM message_text WHERE rowid =
        (SELECT rowid FROM message
         WHERE account = new.account AND mailbox = new.mailbox AND uid = new.uid);
END;
CREATE TRIGGER message_inserted AFTER INSERT ON message BEGIN
    UPDATE message SET rowid =
        (SELECT coalesce(max(rowid) + 1, new.received << 24) FROM message
         WHERE rowid BETWEEN new.received << 24 AND (new.received << 24) | 16777215)
    WHERE rowid = new.rowid;
    INSERT INTO message_text (rowid, sender, subject, snippet, recipients)
    SELECT rowid, sender, subject, snippet, iif(from_me, to_line, NULL) FROM message
    WHERE account = new.account AND mailbox = new.mailbox AND uid = new.uid;
END;
CREATE TRIGGER message_retexted AFTER UPDATE OF sender, subject, snippet, to_line, from_me
    ON message BEGIN
    DELETE FROM message_text WHERE rowid = old.rowid;
    INSERT INTO message_text (rowid, sender, subject, snippet, recipients)
    VALUES (new.rowid, new.sender, new.subject, new.snippet, iif(new.from_me, new.to_line, NULL));
END;
CREATE TRIGGER message_deleted AFTER DELETE ON message BEGIN
    DELETE FROM message_text WHERE rowid = old.rowid;
END;
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
    attachments TEXT NOT NULL,
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
-- A Message a move took out of `message`, with its row, while the server
-- or an Undo may still move it: docs/adr/0012-delete-and-undo.md.
CREATE TABLE moved (
    id INTEGER PRIMARY KEY,
    -- One per move, shared by its Messages.
    batch INTEGER NOT NULL,
    -- Where it goes: the Folder with the role `to_role`, else `to_folder`.
    to_role TEXT,
    to_folder TEXT,
    -- Whether the Message is wanted away from `mailbox` and whether the
    -- server has it away, in `away`, under `at_uid` there or in `mailbox`;
    -- NULL when never said.
    want_away INTEGER NOT NULL,
    at_away INTEGER NOT NULL,
    at_uid INTEGER,
    away TEXT,
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
    text_html INTEGER,
    attachments TEXT NOT NULL
);
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
-- What the server holds in `mailbox` that the Message List leaves out.
CREATE VIEW leaving AS
    SELECT account, mailbox, at_uid AS uid FROM moved WHERE want_away AND NOT at_away;
-- Rows an Undo put back under `uid`, with where the server has the Message.
CREATE VIEW undone AS
    SELECT account, mailbox, uid, at_away, at_uid FROM moved WHERE NOT want_away;
