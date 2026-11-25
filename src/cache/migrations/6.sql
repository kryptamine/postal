-- The fetched body now holds a calendar part even when it is attached, for
-- the invitation card; a body cached before lacks it, so it is fetched again.
DELETE FROM body WHERE (account, key) IN (
    SELECT account, body_key FROM message
    WHERE attachments LIKE '%.ics' || char(10) || '%'
        OR attachments LIKE '%' || char(9) || 'text/calendar' || char(10) || '%'
        OR attachments LIKE '%' || char(9) || 'application/ics' || char(10) || '%'
);
