-- A Folder's total as the Folder Picker shows it: the newest count, by a
-- resync, IDLE or `STATUS`, and moved by local Deletes. `exists_count`
-- stays what the last resync saw, which the walk and the index rely on.
ALTER TABLE mailbox ADD COLUMN messages INTEGER;
UPDATE mailbox SET messages = exists_count;
