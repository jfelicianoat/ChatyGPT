ALTER TABLE scheduled_runs
    ADD COLUMN workflow_run_id TEXT REFERENCES workflow_runs(id) ON DELETE SET NULL;

CREATE INDEX idx_scheduled_runs_workflow_run
    ON scheduled_runs(workflow_run_id)
    WHERE workflow_run_id IS NOT NULL;
