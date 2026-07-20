CREATE TABLE conversation_attachments (
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    attachment_id TEXT NOT NULL REFERENCES attachments(id) ON DELETE CASCADE,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY(conversation_id, attachment_id)
) STRICT, WITHOUT ROWID;

INSERT OR IGNORE INTO conversation_attachments(conversation_id, attachment_id)
SELECT conversation_id, id FROM attachments WHERE conversation_id IS NOT NULL;

CREATE TABLE message_attachments (
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    attachment_id TEXT NOT NULL REFERENCES attachments(id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    PRIMARY KEY(message_id, attachment_id),
    UNIQUE(message_id, ordinal)
) STRICT, WITHOUT ROWID;

ALTER TABLE attachments ADD COLUMN kind TEXT;
ALTER TABLE attachments ADD COLUMN engine TEXT;
ALTER TABLE attachments ADD COLUMN ingestion_meta_json TEXT
    CHECK (ingestion_meta_json IS NULL OR json_valid(ingestion_meta_json));

CREATE INDEX idx_attachments_ingestion_recovery
    ON attachments(ingestion_status, updated_at)
    WHERE ingestion_status IN ('uploading', 'received', 'converting');
