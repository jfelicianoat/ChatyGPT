CREATE TABLE IF NOT EXISTS semantic_chat_workflows (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    user_message_id TEXT NOT NULL UNIQUE REFERENCES messages(id) ON DELETE CASCADE,
    assistant_message_id TEXT NOT NULL UNIQUE REFERENCES messages(id) ON DELETE CASCADE,
    embedding_task_id TEXT NOT NULL UNIQUE REFERENCES broker_tasks(id) ON DELETE CASCADE,
    chat_task_id TEXT UNIQUE REFERENCES broker_tasks(id) ON DELETE SET NULL,
    user_text TEXT NOT NULL,
    context_json TEXT NOT NULL CHECK (json_valid(context_json)),
    attachment_ids_json TEXT NOT NULL CHECK (json_valid(attachment_ids_json)),
    tools_enabled INTEGER NOT NULL CHECK (tools_enabled IN (0, 1)),
    sandbox_enabled INTEGER NOT NULL CHECK (sandbox_enabled IN (0, 1)),
    status TEXT NOT NULL DEFAULT 'searching'
        CHECK (status IN ('searching', 'preparing_chat', 'submitted', 'failed', 'cancelled')),
    error_json TEXT CHECK (error_json IS NULL OR json_valid(error_json)),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_semantic_chat_workflows_status
    ON semantic_chat_workflows(status, updated_at);
