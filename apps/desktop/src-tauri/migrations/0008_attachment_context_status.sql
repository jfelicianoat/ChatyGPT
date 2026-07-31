ALTER TABLE attachments
ADD COLUMN context_status TEXT NOT NULL DEFAULT 'pending'
CHECK(context_status IN ('pending', 'preparing', 'ready', 'unavailable', 'failed'));

ALTER TABLE attachments
ADD COLUMN context_error_json TEXT
CHECK(context_error_json IS NULL OR json_valid(context_error_json));

UPDATE attachments
SET context_status = 'ready'
WHERE EXISTS(
    SELECT 1
    FROM attachment_chunks chunk
    WHERE chunk.attachment_id = attachments.id
);
