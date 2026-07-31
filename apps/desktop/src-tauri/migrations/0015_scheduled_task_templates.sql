CREATE TABLE IF NOT EXISTS scheduled_task_templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    prompt TEXT NOT NULL,
    schedule_expression TEXT NOT NULL
        CHECK (schedule_expression IN ('once', 'daily', 'weekly')),
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_scheduled_task_templates_updated
    ON scheduled_task_templates(updated_at DESC);
