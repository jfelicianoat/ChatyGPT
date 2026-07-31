CREATE TABLE custom_gpt_files (
    custom_gpt_id TEXT NOT NULL REFERENCES custom_gpts(id) ON DELETE CASCADE,
    attachment_id TEXT NOT NULL REFERENCES attachments(id) ON DELETE CASCADE,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY(custom_gpt_id, attachment_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX idx_custom_gpt_files_attachment
    ON custom_gpt_files(attachment_id, custom_gpt_id);
