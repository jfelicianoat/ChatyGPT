ALTER TABLE research_runs
    ADD COLUMN broker_task_id TEXT REFERENCES broker_tasks(id) ON DELETE CASCADE;

ALTER TABLE research_runs
    ADD COLUMN completed_at TEXT;

CREATE UNIQUE INDEX idx_research_runs_broker_task
    ON research_runs(broker_task_id)
    WHERE broker_task_id IS NOT NULL;

CREATE INDEX idx_research_runs_conversation
    ON research_runs(conversation_id, created_at DESC);

ALTER TABLE research_steps
    ADD COLUMN kind TEXT CHECK (kind IS NULL OR kind IN ('plan', 'research', 'synthesis'));

ALTER TABLE research_steps
    ADD COLUMN title TEXT;

ALTER TABLE research_steps
    ADD COLUMN started_at TEXT;

ALTER TABLE research_steps
    ADD COLUMN completed_at TEXT;
