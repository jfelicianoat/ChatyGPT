CREATE TABLE conversation_summaries (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    broker_task_id TEXT UNIQUE REFERENCES broker_tasks(id) ON DELETE SET NULL,
    source_through_sequence INTEGER NOT NULL CHECK(source_through_sequence >= 0),
    status TEXT NOT NULL CHECK(status IN (
        'generating', 'draft', 'approved', 'failed', 'cancelled', 'superseded'
    )),
    draft_text TEXT,
    approved_text TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    approved_at TEXT
);

CREATE INDEX idx_conversation_summaries_conversation
    ON conversation_summaries(conversation_id, updated_at DESC);

CREATE UNIQUE INDEX idx_conversation_summaries_candidate
    ON conversation_summaries(conversation_id)
    WHERE status IN ('generating', 'draft');

CREATE UNIQUE INDEX idx_conversation_summaries_active
    ON conversation_summaries(conversation_id)
    WHERE status = 'approved';
