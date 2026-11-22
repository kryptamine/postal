# Stored data is migrated across releases, and the Cache is rebuilt only as a fallback

Postal keeps state the server cannot give back: the flag changes it has not
acknowledged (`flag_change`) and the Deletes still moving or open to an Undo
(`moved`), in the Cache; the user's own writing in `config.toml` and the
JSON notes beside each Draft and Outbox file. Deleting the Cache on every
schema change (issue #91) brought an offline Delete back and lost its Undo,
and cost a full resync of every Folder.

## The Cache

The schema is the list of steps in `cache/migrations.rs`, one SQL file each
under `cache/migrations/`, run in order from an empty file. `user_version`
counts the steps a Cache has run, on top of 17, the last schema before
migrations: step 1, the first released schema, reaches 18. Each step runs in
one transaction with the version it reaches, so a failed step leaves the
Cache whole at the version before, and Postal exits with the error rather
than delete what the step could not carry.

- **A released step never changes.** A schema change appends a step, which
  alters the tables in place, and adds a fixture under `tests/fixtures/cache/`
  of the schema before it, written by the ignored test in
  `tests/migrations.rs`, with a test that opens it.
- **Pending state is always carried over**: every row of `flag_change` and
  `moved`. A step may clear derived rows (bodies, Snippets, a Folder's sync
  state, `correspondence`, `sent_pass`) and let the sync refill them.
- **Rebuild is the fallback**, for a Cache that fails `quick_check` and one
  at 17 or under, from before migrations.
- **A downgrade is refused.** An older Postal finds a Cache newer than its
  schema and exits, naming the directory to delete; it never rebuilds it,
  so going back and forth does not churn the Cache. `postal correspondent
  list` and `postal thread show` read only a Cache of their own schema and
  answer that it is not ready.

## The files

`config.toml` and the Draft notes carry no version. An additive change, a
key with a default, needs none: it is read through `#[serde(default)]`.
A breaking change (a key renamed, its meaning changed) adds a `version`
key, its absence meaning the shape before, and reads the older shape into
the new one; the file is rewritten in the new shape only on the next save.

`config.toml` denies unknown keys, to catch a typo. An older Postal reading
a key a newer one added refuses the file and says which key; deleting that
key makes it readable again. The Draft notes ignore unknown keys, so an
older Postal reads a newer note and drops what it does not know on save.

## Considered options

- **Rebuild on every schema change.** No code, but every release that
  touches the schema loses pending state and resyncs every Folder.
- **`rusqlite_migration`.** The same loop with hooks, but each release pins
  one rusqlite minor (2.7 needs rusqlite 0.41 and Rust 1.89), and the parts
  that need care, the fallback and what is carried over, stay ours.
- **`refinery`.** Built for several databases, with macros, and needs Rust
  1.92.
- **Restart `user_version` at 1.** A Cache from before migrations, at 17,
  would read as newer than the binary and be refused instead of rebuilt.
