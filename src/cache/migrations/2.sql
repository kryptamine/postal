-- How far the pass over every Message's envelope has read, for Correspondents
-- the Message List never held: docs/adr/0016-expand-recipients-after-the-editor.md.
CREATE TABLE envelope_pass (
    account TEXT PRIMARY KEY,
    mailbox TEXT NOT NULL,
    uid_validity INTEGER NOT NULL,
    last_uid INTEGER NOT NULL,
    -- Envelopes below this UID are still to read; NULL once none are.
    below INTEGER
);
