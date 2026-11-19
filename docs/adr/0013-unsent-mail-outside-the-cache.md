# Unsent mail lives in files outside the Cache, and a sent one is never sent twice

Drafts and the Outbox are the user's own writing, which the Cache cannot
hold: it is deleted whole when its schema changes, and its bodies are
evicted to stay under a byte budget. Each unsent Message is one RFC 5322
file under `$XDG_DATA_HOME/postal/`, in `drafts/` or `outbox/`, and
`$EDITOR` opens a Draft's file directly. Drafts stay on this machine; the
server's Drafts Folder is not synced. What the file cannot hold rides in a
JSON file of the same name beside it: the Message a Reply answers, the
server's last answer, and in the Outbox the Draft's file as written, for a
refusal to give back.

A Send moves the file from `drafts/` to `outbox/`, with its `Message-ID` and
`Date` fixed then, so every retry sends the same Message. Sending takes two
steps: SMTP accepts the Message, then a copy is saved to the Folder with the
Sent role. Gmail saves that copy itself, so there the second step does
nothing; another server needs an `APPEND`. Once SMTP accepts, the file is
marked sent before the second step starts, so a failed `APPEND` is retried
on its own and never sends the Message again. The mark is a rename, from
`<id>.eml` to `<id>.sent`. A crash between SMTP's answer and that mark can
still send it twice, under the same `Message-ID`.

The sync connection sends the Outbox, as it knows whether the server is
Gmail and holds the connection an `APPEND` needs: on each connect, on each
Send, on each Folder switch, and after each `IDLE` the server's news or its
period ends, while the Outbox is not empty. A network error or a `4xx`
reply keeps the Message in the Outbox, to be retried on the next sync, its
error kept in the JSON file for Local Drafts to show. A
`5xx` reply moves it back to `drafts/`, with the server's answer shown when
the Draft is opened; a `53x` refuses the sign-in, not the Message, and keeps
it.

## Considered options

- **A table in the Cache.** One store and one transaction, but a schema
  change would delete unsent mail, and the editor would need a temporary
  copy to edit.
- **Sync Drafts to the server's Drafts Folder.** A Draft would show on the
  phone, at the cost of an `APPEND` and an expunge on every save and of
  conflicts with other clients editing the same Draft.
- **Send at once, with no Outbox.** Simpler, but a Message written offline
  could not leave, and a failed send would have nowhere to wait.
