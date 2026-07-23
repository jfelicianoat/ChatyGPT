CREATE TABLE IF NOT EXISTS memory_searches (
    id TEXT PRIMARY KEY,
    query_text TEXT NOT NULL,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    broker_task_id TEXT NOT NULL UNIQUE REFERENCES broker_tasks(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_memory_searches_created
    ON memory_searches(created_at DESC);
