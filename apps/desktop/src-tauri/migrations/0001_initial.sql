PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    default_gpt_id TEXT REFERENCES custom_gpts(id) ON DELETE SET NULL,
    archived_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE TABLE IF NOT EXISTS conversations (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    summary TEXT,
    summary_through_message_id TEXT,
    archived_at TEXT,
    deleted_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_conversations_project_updated
    ON conversations(project_id, updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_conversations_active_updated
    ON conversations(updated_at DESC) WHERE archived_at IS NULL AND deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    parent_message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,
    role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant', 'tool', 'error')),
    status TEXT NOT NULL CHECK (status IN ('draft', 'pending', 'complete', 'failed', 'cancelled')),
    sequence_no INTEGER NOT NULL,
    broker_task_id TEXT REFERENCES broker_tasks(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(conversation_id, sequence_no)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_messages_conversation_sequence
    ON messages(conversation_id, sequence_no);

CREATE TABLE IF NOT EXISTS message_parts (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('text', 'markdown', 'attachment', 'tool_call', 'tool_result', 'citation', 'error')),
    content_text TEXT,
    content_json TEXT CHECK (content_json IS NULL OR json_valid(content_json)),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(message_id, ordinal)
) STRICT;

CREATE TABLE IF NOT EXISTS attachments (
    id TEXT PRIMARY KEY,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE CASCADE,
    message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,
    local_path TEXT,
    display_name TEXT NOT NULL,
    media_type TEXT,
    size_bytes INTEGER CHECK (size_bytes IS NULL OR size_bytes >= 0),
    sha256 TEXT,
    broker_file_id TEXT,
    ingestion_status TEXT NOT NULL DEFAULT 'local'
        CHECK (ingestion_status IN ('local', 'uploading', 'received', 'converting', 'ready', 'failed')),
    ingestion_error_json TEXT CHECK (ingestion_error_json IS NULL OR json_valid(ingestion_error_json)),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_attachments_sha256
    ON attachments(sha256) WHERE sha256 IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_attachments_broker_file
    ON attachments(broker_file_id) WHERE broker_file_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS project_files (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    attachment_id TEXT NOT NULL REFERENCES attachments(id) ON DELETE CASCADE,
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY(project_id, attachment_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS broker_tasks (
    id TEXT PRIMARY KEY,
    remote_task_id TEXT UNIQUE,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    request_message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,
    response_message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    request_json TEXT NOT NULL CHECK (json_valid(request_json)),
    remote_status TEXT NOT NULL,
    local_state TEXT NOT NULL DEFAULT 'created'
        CHECK (local_state IN ('created', 'submitting', 'polling', 'waiting_for_tools', 'recovery_pending', 'terminal', 'orphaned')),
    attempt INTEGER NOT NULL DEFAULT 0 CHECK (attempt >= 0),
    consecutive_poll_errors INTEGER NOT NULL DEFAULT 0 CHECK (consecutive_poll_errors >= 0),
    next_poll_at TEXT,
    lease_owner TEXT,
    lease_expires_at TEXT,
    last_http_status INTEGER,
    result_json TEXT CHECK (result_json IS NULL OR json_valid(result_json)),
    error_json TEXT CHECK (error_json IS NULL OR json_valid(error_json)),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    terminal_at TEXT
) STRICT;

CREATE INDEX IF NOT EXISTS idx_broker_tasks_recovery
    ON broker_tasks(local_state, next_poll_at)
    WHERE local_state IN ('submitting', 'polling', 'waiting_for_tools', 'recovery_pending', 'orphaned');
CREATE INDEX IF NOT EXISTS idx_broker_tasks_conversation
    ON broker_tasks(conversation_id, created_at DESC);

CREATE TABLE IF NOT EXISTS broker_task_events (
    id INTEGER PRIMARY KEY,
    broker_task_id TEXT NOT NULL REFERENCES broker_tasks(id) ON DELETE CASCADE,
    remote_event_id TEXT,
    event_type TEXT NOT NULL,
    remote_status TEXT,
    payload_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload_json)),
    occurred_at TEXT NOT NULL,
    received_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(broker_task_id, remote_event_id)
) STRICT;

CREATE INDEX IF NOT EXISTS idx_broker_task_events_timeline
    ON broker_task_events(broker_task_id, occurred_at, id);

CREATE TABLE IF NOT EXISTS tool_calls (
    id TEXT PRIMARY KEY,
    broker_task_id TEXT NOT NULL REFERENCES broker_tasks(id) ON DELETE CASCADE,
    remote_tool_call_id TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    arguments_json TEXT NOT NULL CHECK (json_valid(arguments_json)),
    status TEXT NOT NULL CHECK (status IN ('requested', 'confirmation_required', 'approved', 'executing', 'completed', 'failed', 'cancelled')),
    requested_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT,
    UNIQUE(broker_task_id, remote_tool_call_id)
) STRICT;

CREATE TABLE IF NOT EXISTS tool_results (
    id TEXT PRIMARY KEY,
    tool_call_id TEXT NOT NULL UNIQUE REFERENCES tool_calls(id) ON DELETE CASCADE,
    content_text TEXT,
    content_json TEXT CHECK (content_json IS NULL OR json_valid(content_json)),
    is_error INTEGER NOT NULL DEFAULT 0 CHECK (is_error IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE TABLE IF NOT EXISTS citations (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    ordinal INTEGER NOT NULL,
    title TEXT,
    url TEXT,
    source_attachment_id TEXT REFERENCES attachments(id) ON DELETE SET NULL,
    quote_text TEXT,
    claim_text TEXT,
    metadata_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(metadata_json)),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(message_id, ordinal)
) STRICT;

CREATE TABLE IF NOT EXISTS memory_items (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE CASCADE,
    custom_gpt_id TEXT REFERENCES custom_gpts(id) ON DELETE CASCADE,
    category TEXT NOT NULL,
    content TEXT NOT NULL,
    sensitivity TEXT NOT NULL DEFAULT 'normal' CHECK (sensitivity IN ('normal', 'sensitive')),
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    provenance_type TEXT NOT NULL,
    provenance_id TEXT,
    expires_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_memory_scope_enabled
    ON memory_items(project_id, custom_gpt_id, enabled);

CREATE TABLE IF NOT EXISTS context_snapshots (
    id TEXT PRIMARY KEY,
    broker_task_id TEXT NOT NULL UNIQUE REFERENCES broker_tasks(id) ON DELETE CASCADE,
    strategy_version TEXT NOT NULL,
    token_budget INTEGER,
    estimated_tokens INTEGER,
    final_context_json TEXT NOT NULL CHECK (json_valid(final_context_json)),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE TABLE IF NOT EXISTS context_sources (
    id TEXT PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES context_snapshots(id) ON DELETE CASCADE,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    ordinal INTEGER NOT NULL,
    reason TEXT NOT NULL,
    score REAL,
    estimated_tokens INTEGER,
    excerpt TEXT,
    UNIQUE(snapshot_id, ordinal)
) STRICT;

CREATE TABLE IF NOT EXISTS embedding_records (
    id TEXT PRIMARY KEY,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    model TEXT NOT NULL,
    dimensions INTEGER NOT NULL CHECK (dimensions > 0),
    vector_blob BLOB NOT NULL,
    content_sha256 TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(source_type, source_id, chunk_index, model)
) STRICT;

CREATE TABLE IF NOT EXISTS custom_gpts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    icon_ref TEXT,
    active_version_id TEXT REFERENCES gpt_versions(id) ON DELETE SET NULL,
    default_project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    archived_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE TABLE IF NOT EXISTS gpt_versions (
    id TEXT PRIMARY KEY,
    custom_gpt_id TEXT NOT NULL REFERENCES custom_gpts(id) ON DELETE CASCADE,
    version_no INTEGER NOT NULL CHECK (version_no > 0),
    configuration_json TEXT NOT NULL CHECK (json_valid(configuration_json)),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(custom_gpt_id, version_no)
) STRICT;

CREATE TABLE IF NOT EXISTS gpt_tool_permissions (
    id TEXT PRIMARY KEY,
    gpt_version_id TEXT NOT NULL REFERENCES gpt_versions(id) ON DELETE CASCADE,
    tool_name TEXT NOT NULL,
    effect TEXT NOT NULL CHECK (effect IN ('allow', 'deny', 'confirm')),
    scope_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(scope_json)),
    UNIQUE(gpt_version_id, tool_name)
) STRICT;

CREATE TABLE IF NOT EXISTS scheduled_tasks (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    schedule_expression TEXT NOT NULL,
    timezone TEXT NOT NULL,
    payload_json TEXT NOT NULL CHECK (json_valid(payload_json)),
    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
    confirmed_at TEXT,
    next_run_at TEXT,
    last_claim_key TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE TABLE IF NOT EXISTS scheduled_runs (
    id TEXT PRIMARY KEY,
    scheduled_task_id TEXT NOT NULL REFERENCES scheduled_tasks(id) ON DELETE CASCADE,
    due_at TEXT NOT NULL,
    claim_key TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL CHECK (status IN ('claimed', 'running', 'completed', 'failed', 'cancelled', 'skipped')),
    broker_task_id TEXT REFERENCES broker_tasks(id) ON DELETE SET NULL,
    attempt INTEGER NOT NULL DEFAULT 0,
    result_json TEXT CHECK (result_json IS NULL OR json_valid(result_json)),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE TABLE IF NOT EXISTS research_runs (
    id TEXT PRIMARY KEY,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    objective TEXT NOT NULL,
    plan_json TEXT CHECK (plan_json IS NULL OR json_valid(plan_json)),
    status TEXT NOT NULL,
    synthesis_message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE TABLE IF NOT EXISTS research_steps (
    id TEXT PRIMARY KEY,
    research_run_id TEXT NOT NULL REFERENCES research_runs(id) ON DELETE CASCADE,
    parent_step_id TEXT REFERENCES research_steps(id) ON DELETE SET NULL,
    ordinal INTEGER NOT NULL,
    objective TEXT NOT NULL,
    status TEXT NOT NULL,
    broker_task_id TEXT REFERENCES broker_tasks(id) ON DELETE SET NULL,
    result_json TEXT CHECK (result_json IS NULL OR json_valid(result_json)),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(research_run_id, ordinal)
) STRICT;

CREATE TABLE IF NOT EXISTS authorized_folders (
    id TEXT PRIMARY KEY,
    canonical_path TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    permissions_json TEXT NOT NULL CHECK (json_valid(permissions_json)),
    granted_at TEXT NOT NULL DEFAULT (datetime('now')),
    revoked_at TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS confirmation_requests (
    id TEXT PRIMARY KEY,
    action_type TEXT NOT NULL,
    tool_name TEXT,
    resources_json TEXT NOT NULL CHECK (json_valid(resources_json)),
    disclosure_json TEXT NOT NULL CHECK (json_valid(disclosure_json)),
    consequences TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'allowed_once', 'cancelled', 'expired')),
    requested_at TEXT NOT NULL DEFAULT (datetime('now')),
    resolved_at TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS audit_events (
    id INTEGER PRIMARY KEY,
    event_type TEXT NOT NULL,
    actor TEXT NOT NULL,
    conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL,
    message_id TEXT REFERENCES messages(id) ON DELETE SET NULL,
    broker_task_id TEXT REFERENCES broker_tasks(id) ON DELETE SET NULL,
    payload_json TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(payload_json)),
    occurred_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT;

CREATE INDEX IF NOT EXISTS idx_audit_events_timeline ON audit_events(occurred_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS export_records (
    id TEXT PRIMARY KEY,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    stable_export_id TEXT NOT NULL,
    destination_path TEXT NOT NULL,
    source_hash TEXT NOT NULL,
    destination_hash_before TEXT,
    destination_hash_after TEXT,
    status TEXT NOT NULL CHECK (status IN ('pending', 'completed', 'conflict', 'failed')),
    error_json TEXT CHECK (error_json IS NULL OR json_valid(error_json)),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(stable_export_id, destination_path)
) STRICT;

CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL CHECK (json_valid(value_json)),
    sensitivity TEXT NOT NULL DEFAULT 'public' CHECK (sensitivity = 'public'),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT, WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS feature_flags (
    key TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
    rationale TEXT,
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
) STRICT, WITHOUT ROWID;

INSERT OR IGNORE INTO feature_flags(key, enabled, rationale) VALUES
    ('chat', 1, 'Turnos locales durables con contexto trazable y Broker AI'),
    ('memory', 0, 'Fase 2'),
    ('custom_gpts', 0, 'Fase 3'),
    ('deep_research', 0, 'Fase 4'),
    ('scheduled_tasks', 0, 'Fase 4');
