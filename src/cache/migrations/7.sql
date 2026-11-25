-- A plain part carrying HTML is now cut without its markup. Every plain
-- Snippet with a `<` is cleared, so the second fetch phase cuts it anew.
UPDATE message SET snippet = NULL WHERE NOT text_html AND snippet LIKE '%<%';
