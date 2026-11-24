-- The Query's operators: `from:` reads the From line, address and all, and
-- `to:`/`cc:` the recipients of every Message, not only of mail the Account
-- sent. FTS5 adds no column in place, so the index is built anew from `message`.
DROP TRIGGER message_replaced;
DROP TRIGGER message_inserted;
DROP TRIGGER message_retexted;
DROP TRIGGER message_deleted;
DROP TABLE message_text;
-- `recipients` is what a word without an operator matches: those of sent mail.
CREATE VIRTUAL TABLE message_text USING fts5(
    sender, subject, snippet, recipients, from_line, to_line,
    content = '', contentless_delete = 1,
    tokenize = 'unicode61 remove_diacritics 2', prefix = '1 2 3 4'
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
END;
CREATE TRIGGER message_deleted AFTER DELETE ON message BEGIN
    DELETE FROM message_text WHERE rowid = old.rowid;
END;
