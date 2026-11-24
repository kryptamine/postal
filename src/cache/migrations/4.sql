-- Bodies join the index: docs/adr/0032-every-message-is-downloaded-and-its-body-indexed.md.
-- `bodied` says the row's index entry holds its body; neither the text nor
-- the raw body is kept, so any rewrite of the entry clears it.
ALTER TABLE message ADD COLUMN bodied INTEGER NOT NULL DEFAULT 0;
-- An index entry ends in the rowid, so this one reads newest first.
CREATE INDEX message_unbodied ON message (account) WHERE bodied = 0;
-- A Message Gmail files under several labels is indexed once for all.
CREATE INDEX message_by_body ON message (account, body_key);
DROP TRIGGER message_replaced;
DROP TRIGGER message_inserted;
DROP TRIGGER message_retexted;
DROP TRIGGER message_deleted;
DROP TABLE message_text;
-- `prefix` holds one and two characters: three and four cost 1.5 GB at
-- 500,000 bodies, and a range scan reads them in milliseconds.
CREATE VIRTUAL TABLE message_text USING fts5(
    sender, subject, snippet, recipients, from_line, to_line, body,
    content = '', contentless_delete = 1,
    tokenize = 'unicode61 remove_diacritics 2', prefix = '1 2'
);
INSERT INTO message_text (rowid, sender, subject, snippet, recipients, from_line, to_line)
SELECT rowid, sender, subject, snippet, iif(from_me, to_line, NULL), from_line, to_line
FROM message;
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
    INSERT INTO message_text (rowid, sender, subject, snippet, recipients, from_line, to_line)
    SELECT rowid, sender, subject, snippet, iif(from_me, to_line, NULL), from_line, to_line
    FROM message
    WHERE account = new.account AND mailbox = new.mailbox AND uid = new.uid;
END;
CREATE TRIGGER message_retexted
    AFTER UPDATE OF sender, subject, snippet, to_line, from_me, from_line ON message BEGIN
    DELETE FROM message_text WHERE rowid = old.rowid;
    INSERT INTO message_text (rowid, sender, subject, snippet, recipients, from_line, to_line)
    VALUES (new.rowid, new.sender, new.subject, new.snippet,
            iif(new.from_me, new.to_line, NULL), new.from_line, new.to_line);
    UPDATE message SET bodied = 0 WHERE rowid = new.rowid;
END;
CREATE TRIGGER message_deleted AFTER DELETE ON message BEGIN
    DELETE FROM message_text WHERE rowid = old.rowid;
END;
