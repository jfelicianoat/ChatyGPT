CREATE TABLE attachment_chunks (
    id TEXT PRIMARY KEY,
    attachment_id TEXT NOT NULL REFERENCES attachments(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    content_text TEXT NOT NULL CHECK(length(content_text) > 0),
    content_sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(attachment_id, ordinal)
);

CREATE INDEX idx_attachment_chunks_attachment
    ON attachment_chunks(attachment_id, ordinal);
