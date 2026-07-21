INSERT OR IGNORE INTO citations(
    id, message_id, ordinal, title, source_attachment_id, metadata_json
)
SELECT
    'citation_' || lower(hex(randomblob(16))),
    bt.response_message_id,
    ma.ordinal,
    a.display_name,
    a.id,
    json_object(
        'kind', 'broker_file',
        'broker_file_id', a.broker_file_id,
        'media_type', a.media_type,
        'size_bytes', a.size_bytes,
        'attribution', 'turn_attachment'
    )
FROM broker_tasks bt
JOIN messages response_message
  ON response_message.id = bt.response_message_id
 AND response_message.status = 'complete'
JOIN message_attachments ma
  ON ma.message_id = bt.request_message_id
JOIN attachments a
  ON a.id = ma.attachment_id
WHERE bt.remote_status = 'completed'
  AND bt.response_message_id IS NOT NULL;
