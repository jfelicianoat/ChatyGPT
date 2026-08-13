CREATE TABLE workflows (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL CHECK(length(trim(name)) BETWEEN 1 AND 120),
    description TEXT,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    draft_definition_json TEXT NOT NULL,
    published_version_id TEXT,
    archived_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE workflow_versions (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    version_no INTEGER NOT NULL CHECK(version_no > 0),
    definition_json TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(workflow_id, version_no)
);

CREATE TABLE workflow_runs (
    id TEXT PRIMARY KEY,
    workflow_id TEXT NOT NULL REFERENCES workflows(id) ON DELETE CASCADE,
    workflow_version_id TEXT NOT NULL REFERENCES workflow_versions(id),
    status TEXT NOT NULL DEFAULT 'queued'
        CHECK(status IN ('queued', 'running', 'waiting_approval', 'completed', 'partial_failed', 'failed', 'cancelled')),
    input_text TEXT NOT NULL,
    output_json TEXT,
    error_json TEXT,
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE workflow_node_runs (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES workflow_runs(id) ON DELETE CASCADE,
    node_id TEXT NOT NULL,
    node_kind TEXT NOT NULL,
    node_label TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending'
        CHECK(status IN ('pending', 'running', 'waiting_approval', 'completed', 'failed', 'skipped', 'cancelled')),
    input_text TEXT,
    output_text TEXT,
    broker_task_id TEXT,
    error_json TEXT,
    started_at TEXT,
    completed_at TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(run_id, node_id)
);

CREATE INDEX idx_workflows_project_updated
    ON workflows(project_id, updated_at DESC)
    WHERE archived_at IS NULL;
CREATE INDEX idx_workflow_runs_workflow_created
    ON workflow_runs(workflow_id, created_at DESC);
CREATE INDEX idx_workflow_node_runs_run_status
    ON workflow_node_runs(run_id, status);
