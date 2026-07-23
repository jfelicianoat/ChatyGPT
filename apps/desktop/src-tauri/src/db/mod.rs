use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::broker::{TaskAccepted, TaskState};
use crate::error::AppError;

const INITIAL_MIGRATION: &str = include_str!("../../migrations/0001_initial.sql");
const ATTACHMENTS_MIGRATION: &str = include_str!("../../migrations/0002_attachments.sql");
const ATTACHMENT_SOURCES_MIGRATION: &str =
    include_str!("../../migrations/0003_attachment_sources.sql");
const MEMORY_SEARCHES_MIGRATION: &str = include_str!("../../migrations/0004_memory_searches.sql");
const RECOVER_NON_TERMINAL_TASKS: &str =
    include_str!("../../queries/recover_non_terminal_tasks.sql");
pub const SCHEMA_VERSION: i64 = 4;

#[derive(Clone)]
pub struct Database {
    path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct BrokerTaskRecord {
    pub id: String,
    pub remote_task_id: Option<String>,
    pub request: Value,
    pub consecutive_poll_errors: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalTaskSnapshot {
    pub id: String,
    pub remote_task_id: Option<String>,
    pub remote_status: String,
    pub local_state: String,
    pub consecutive_poll_errors: u32,
    pub result: Option<Value>,
    pub error: Option<Value>,
    pub pending_tool_calls: Vec<ToolCallView>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallView {
    pub tool_call_id: String,
    pub name: String,
    pub arguments: Value,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct ToolOutcomeRecord {
    pub tool_call_id: String,
    pub status: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub project_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub conversation_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEventView {
    pub id: i64,
    pub category: String,
    pub summary: String,
    pub severity: String,
    pub actor: String,
    pub conversation_title: Option<String>,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryItemView {
    pub kind: String,
    pub label: String,
    pub status: String,
    pub conversation_id: Option<String>,
    pub conversation_title: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItemView {
    pub id: String,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub category: String,
    pub content: String,
    pub sensitivity: String,
    pub enabled: bool,
    pub embedding_status: String,
    pub embedding_model: Option<String>,
    pub embedding_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryOverview {
    pub enabled: bool,
    pub items: Vec<MemoryItemView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchResultView {
    pub memory_id: String,
    pub content: String,
    pub category: String,
    pub project_name: Option<String>,
    pub sensitivity: String,
    pub score: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchView {
    pub id: String,
    pub query: String,
    pub project_id: Option<String>,
    pub status: String,
    pub model: Option<String>,
    pub error: Option<String>,
    pub results: Vec<MemorySearchResultView>,
    pub created_at: String,
}

struct MemorySearchRecord {
    query: String,
    project_id: Option<String>,
    remote_status: String,
    local_state: String,
    error_json: Option<String>,
    model: Option<String>,
    dimensions: Option<i64>,
    blob: Option<Vec<u8>>,
    created_at: String,
}

fn audit_presentation(event_type: &str) -> (&'static str, &'static str, &'static str) {
    match event_type {
        "project.created" => ("project", "Proyecto creado", "info"),
        "project.renamed" => ("project", "Proyecto renombrado", "info"),
        "project.archived" => ("project", "Proyecto archivado", "warning"),
        "conversation.created" => ("conversation", "Conversación creada", "info"),
        "conversation.renamed" => ("conversation", "Conversación renombrada", "info"),
        "conversation.moved" => ("conversation", "Conversación movida", "info"),
        "conversation.archived" => ("conversation", "Conversación archivada", "warning"),
        "conversation.deleted" => ("conversation", "Conversación eliminada", "warning"),
        "attachment.added" => ("attachment", "Adjunto añadido", "info"),
        "attachment.removed" => ("attachment", "Adjunto retirado", "info"),
        "attachment.retry_requested" => ("attachment", "Reintento de adjunto solicitado", "info"),
        "local.prepared" => ("task", "Mensaje preparado para enviar", "info"),
        "remote.accepted" => ("task", "Broker AI aceptó la tarea", "info"),
        "remote.status_changed" => ("task", "Cambió el estado de una tarea", "info"),
        "transport.error" => ("task", "Error temporal de conexión", "error"),
        "local.orphaned" => ("task", "Tarea pendiente marcada para revisión", "warning"),
        "local.tool_decisions_prepared" => ("tool", "Decisiones de herramientas guardadas", "info"),
        "remote.tool_results_accepted" => ("tool", "Broker AI aceptó los resultados", "info"),
        "export.pending" => ("export", "Exportación iniciada", "info"),
        "export.completed" => ("export", "Exportación completada", "info"),
        "export.conflict" => ("export", "Exportación detenida por un conflicto", "warning"),
        "export.failed" => ("export", "Error durante la exportación", "error"),
        "memory.enabled" => ("memory", "Memoria activada", "info"),
        "memory.disabled" => ("memory", "Memoria desactivada", "warning"),
        "memory.created" => ("memory", "Recuerdo creado", "info"),
        "memory.item_enabled" => ("memory", "Recuerdo activado", "info"),
        "memory.item_disabled" => ("memory", "Recuerdo desactivado", "warning"),
        "memory.deleted" => ("memory", "Recuerdo eliminado", "warning"),
        _ => ("system", "Actividad registrada", "info"),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub id: String,
    pub role: String,
    pub status: String,
    pub sequence_no: i64,
    pub broker_task_id: Option<String>,
    pub task_remote_status: Option<String>,
    pub task_local_state: Option<String>,
    pub text: Option<String>,
    pub error: Option<Value>,
    pub model_used: Option<ModelUsedView>,
    pub sources: Vec<ConversationSource>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsedView {
    pub provider: String,
    pub deployment: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSource {
    pub id: String,
    pub title: String,
    pub source_attachment_id: Option<String>,
    pub media_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub url: Option<String>,
    pub quote_text: Option<String>,
    pub claim_text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationView {
    pub id: String,
    pub title: String,
    pub project_id: Option<String>,
    pub messages: Vec<ConversationMessage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextMessage {
    pub message_id: String,
    pub role: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentView {
    pub id: String,
    pub display_name: String,
    pub media_type: Option<String>,
    pub size_bytes: i64,
    pub sha256: String,
    pub broker_file_id: Option<String>,
    pub ingestion_status: String,
    pub ingestion_error: Option<Value>,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct AttachmentRecord {
    pub id: String,
    pub local_path: String,
    pub display_name: String,
    pub media_type: Option<String>,
    pub size_bytes: i64,
    pub sha256: String,
    pub broker_file_id: Option<String>,
    pub ingestion_status: String,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let path = path.as_ref().to_path_buf();
        let mut connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        Self::migrate(&mut connection)?;
        Ok(Self { path })
    }

    fn migrate(connection: &mut Connection) -> Result<(), AppError> {
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 1 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(INITIAL_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 1)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 2 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(ATTACHMENTS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 2)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 3 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(ATTACHMENT_SOURCES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 3)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < SCHEMA_VERSION {
            let transaction = connection.transaction()?;
            transaction.execute_batch(MEMORY_SEARCHES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        Ok(())
    }

    fn connect(&self) -> Result<Connection, AppError> {
        let connection = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        Ok(connection)
    }

    pub fn schema_version(&self) -> Result<i64, AppError> {
        Ok(self
            .connect()?
            .pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    pub fn recover_non_terminal_tasks(&self) -> Result<usize, AppError> {
        let connection = self.connect()?;
        let changed = connection.execute(RECOVER_NON_TERMINAL_TASKS, [])?;
        Ok(changed)
    }

    pub fn recovery_candidates(&self) -> Result<Vec<RecoveryItemView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT bt.remote_status, bt.conversation_id, c.title, bt.updated_at,
                    json_extract(bt.request_json, '$.inference_kind'),
                    json_extract(bt.request_json, '$.content.metadata.source_type')
             FROM broker_tasks bt
             LEFT JOIN conversations c ON c.id = bt.conversation_id
             WHERE bt.remote_status NOT IN ('completed', 'failed', 'cancelled')
               AND bt.local_state != 'orphaned'
             ORDER BY bt.updated_at DESC",
        )?;
        let items = statement
            .query_map([], |row| {
                let conversation_id: Option<String> = row.get(1)?;
                let inference_kind: Option<String> = row.get(4)?;
                let embedding_source: Option<String> = row.get(5)?;
                let is_embedding = inference_kind.as_deref() == Some("embedding");
                Ok(RecoveryItemView {
                    kind: if is_embedding { "embedding" } else { "task" }.to_owned(),
                    label: if embedding_source.as_deref() == Some("memory_search") {
                        "Búsqueda semántica pendiente".to_owned()
                    } else if is_embedding {
                        "Indexación de memoria pendiente".to_owned()
                    } else if conversation_id.is_some() {
                        "Respuesta pendiente".to_owned()
                    } else {
                        "Prueba de inferencia pendiente".to_owned()
                    },
                    status: row.get(0)?,
                    conversation_id,
                    conversation_title: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(items)
    }

    pub fn prepare_broker_task(
        &self,
        id: &str,
        idempotency_key: &str,
        request: &Value,
    ) -> Result<BrokerTaskRecord, AppError> {
        let request_json = serde_json::to_string(request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO broker_tasks(
                id, idempotency_key, request_json, remote_status, local_state
             ) VALUES (?1, ?2, ?3, 'not_submitted', 'created')",
            params![id, idempotency_key, request_json],
        )?;
        connection.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'local.prepared', 'not_submitted', '{}', datetime('now'))",
            params![id],
        )?;
        self.task_record(id)
    }

    pub fn create_project(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<ProjectSummary, AppError> {
        let id = format!("project_{}", Uuid::new_v4().simple());
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO projects(id, name, description) VALUES (?1, ?2, ?3)",
            params![id, name, description],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('project.created', 'user', ?1)",
            params![serde_json::json!({"project_id": id, "name": name}).to_string()],
        )?;
        transaction.commit()?;
        self.project_summary(&id)
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT p.id, p.name, p.description, COUNT(c.id), p.updated_at
             FROM projects p
             LEFT JOIN conversations c
               ON c.project_id = p.id
              AND c.archived_at IS NULL
              AND c.deleted_at IS NULL
             WHERE p.archived_at IS NULL
             GROUP BY p.id, p.name, p.description, p.updated_at
             ORDER BY p.updated_at DESC, p.name COLLATE NOCASE",
        )?;
        let projects = statement
            .query_map([], |row| {
                Ok(ProjectSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    conversation_count: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(projects)
    }

    pub fn list_audit_events(&self, limit: u32) -> Result<Vec<AuditEventView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT ae.id, ae.event_type, ae.actor, c.title, ae.occurred_at
             FROM audit_events ae
             LEFT JOIN conversations c ON c.id = ae.conversation_id
             ORDER BY ae.occurred_at DESC, ae.id DESC
             LIMIT ?1",
        )?;
        let events = statement
            .query_map(params![i64::from(limit.clamp(1, 100))], |row| {
                let event_type: String = row.get(1)?;
                let (category, summary, severity) = audit_presentation(&event_type);
                Ok(AuditEventView {
                    id: row.get(0)?,
                    category: category.to_owned(),
                    summary: summary.to_owned(),
                    severity: severity.to_owned(),
                    actor: row.get(2)?,
                    conversation_title: row.get(3)?,
                    occurred_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(events)
    }

    pub fn memory_overview(&self) -> Result<MemoryOverview, AppError> {
        let connection = self.connect()?;
        let enabled = connection.query_row(
            "SELECT enabled FROM feature_flags WHERE key = 'memory'",
            [],
            |row| row.get(0),
        )?;
        let mut statement = connection.prepare(
            "SELECT m.id, m.project_id, p.name, m.category, m.content,
                    m.sensitivity, m.enabled, m.created_at, m.updated_at,
                    CASE
                      WHEN er.id IS NOT NULL THEN 'ready'
                      WHEN EXISTS(
                        SELECT 1 FROM broker_tasks bt
                        WHERE json_extract(bt.request_json, '$.content.metadata.source_type') = 'memory'
                          AND json_extract(bt.request_json, '$.content.metadata.source_id') = m.id
                          AND bt.local_state NOT IN ('terminal', 'orphaned')
                      ) THEN 'indexing'
                      WHEN EXISTS(
                        SELECT 1 FROM broker_tasks bt
                        WHERE json_extract(bt.request_json, '$.content.metadata.source_type') = 'memory'
                          AND json_extract(bt.request_json, '$.content.metadata.source_id') = m.id
                          AND (bt.remote_status = 'failed' OR bt.local_state = 'orphaned')
                      ) THEN 'failed'
                      ELSE 'missing'
                    END,
                    er.model,
                    (
                      SELECT substr(json_extract(failed.error_json, '$.message'), 1, 500)
                      FROM broker_tasks failed
                      WHERE json_extract(failed.request_json, '$.content.metadata.source_type') = 'memory'
                        AND json_extract(failed.request_json, '$.content.metadata.source_id') = m.id
                        AND failed.error_json IS NOT NULL
                      ORDER BY failed.updated_at DESC, failed.rowid DESC LIMIT 1
                    )
             FROM memory_items m
             LEFT JOIN projects p ON p.id = m.project_id
             LEFT JOIN embedding_records er ON er.id = (
                SELECT candidate.id FROM embedding_records candidate
                WHERE candidate.source_type = 'memory' AND candidate.source_id = m.id
                ORDER BY candidate.created_at DESC, candidate.rowid DESC LIMIT 1
             )
             WHERE m.custom_gpt_id IS NULL
             ORDER BY m.updated_at DESC, m.id DESC",
        )?;
        let items = statement
            .query_map([], |row| {
                Ok(MemoryItemView {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    project_name: row.get(2)?,
                    category: row.get(3)?,
                    content: row.get(4)?,
                    sensitivity: row.get(5)?,
                    enabled: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    embedding_status: row.get(9)?,
                    embedding_model: row.get(10)?,
                    embedding_error: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MemoryOverview { enabled, items })
    }

    pub fn set_memory_enabled(&self, enabled: bool) -> Result<MemoryOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE feature_flags
             SET enabled = ?1, updated_at = datetime('now')
             WHERE key = 'memory'",
            params![enabled],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES (?1, 'user', '{}')",
            params![if enabled {
                "memory.enabled"
            } else {
                "memory.disabled"
            }],
        )?;
        transaction.commit()?;
        self.memory_overview()
    }

    pub fn create_memory_item(
        &self,
        content: &str,
        category: &str,
        sensitivity: &str,
        project_id: Option<&str>,
    ) -> Result<(String, MemoryOverview), AppError> {
        let id = format!("memory_{}", Uuid::new_v4().simple());
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        if let Some(project_id) = project_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND archived_at IS NULL)",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        transaction.execute(
            "INSERT INTO memory_items(
                id, project_id, category, content, sensitivity,
                enabled, provenance_type
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 'manual')",
            params![id, project_id, category, content, sensitivity],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('memory.created', 'user', ?1)",
            params![serde_json::json!({
                "memory_id": id,
                "category": category,
                "sensitivity": sensitivity,
                "project_id": project_id
            })
            .to_string()],
        )?;
        transaction.commit()?;
        Ok((id, self.memory_overview()?))
    }

    pub fn set_memory_item_enabled(
        &self,
        id: &str,
        enabled: bool,
    ) -> Result<MemoryOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE memory_items
             SET enabled = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![id, enabled],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("recuerdo {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES (?1, 'user', ?2)",
            params![
                if enabled {
                    "memory.item_enabled"
                } else {
                    "memory.item_disabled"
                },
                serde_json::json!({"memory_id": id}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.memory_overview()
    }

    pub fn delete_memory_item(&self, id: &str) -> Result<MemoryOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM embedding_records WHERE source_type = 'memory' AND source_id = ?1",
            params![id],
        )?;
        let changed = transaction.execute("DELETE FROM memory_items WHERE id = ?1", params![id])?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("recuerdo {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('memory.deleted', 'user', ?1)",
            params![serde_json::json!({"memory_id": id}).to_string()],
        )?;
        transaction.commit()?;
        self.memory_overview()
    }

    pub fn memory_item(&self, id: &str) -> Result<MemoryItemView, AppError> {
        self.memory_overview()?
            .items
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| AppError::NotFound(format!("recuerdo {id}")))
    }

    pub fn clear_memory_embedding(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        connection.execute(
            "DELETE FROM embedding_records WHERE source_type = 'memory' AND source_id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn prepare_memory_search(
        &self,
        search_id: &str,
        query: &str,
        project_id: Option<&str>,
        task_id: &str,
        idempotency_key: &str,
        request: &Value,
    ) -> Result<BrokerTaskRecord, AppError> {
        let request_json = serde_json::to_string(request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        if let Some(project_id) = project_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND archived_at IS NULL)",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        transaction.execute(
            "INSERT INTO broker_tasks(
                id, idempotency_key, request_json, remote_status, local_state
             ) VALUES (?1, ?2, ?3, 'not_submitted', 'created')",
            params![task_id, idempotency_key, request_json],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'local.prepared', 'not_submitted', '{}', datetime('now'))",
            params![task_id],
        )?;
        transaction.execute(
            "INSERT INTO memory_searches(id, query_text, project_id, broker_task_id)
             VALUES (?1, ?2, ?3, ?4)",
            params![search_id, query, project_id, task_id],
        )?;
        transaction.commit()?;
        self.task_record(task_id)
    }

    pub fn memory_search(&self, id: &str) -> Result<MemorySearchView, AppError> {
        let connection = self.connect()?;
        let record = connection
            .query_row(
                "SELECT ms.query_text, ms.project_id, bt.remote_status, bt.local_state,
                        bt.error_json, er.model, er.dimensions, er.vector_blob, ms.created_at
                 FROM memory_searches ms
                 JOIN broker_tasks bt ON bt.id = ms.broker_task_id
                 LEFT JOIN embedding_records er ON er.id = (
                    SELECT candidate.id FROM embedding_records candidate
                    WHERE candidate.source_type = 'memory_search'
                      AND candidate.source_id = ms.id
                    ORDER BY candidate.created_at DESC, candidate.rowid DESC LIMIT 1
                 )
                 WHERE ms.id = ?1",
                params![id],
                |row| {
                    Ok(MemorySearchRecord {
                        query: row.get(0)?,
                        project_id: row.get(1)?,
                        remote_status: row.get(2)?,
                        local_state: row.get(3)?,
                        error_json: row.get(4)?,
                        model: row.get(5)?,
                        dimensions: row.get(6)?,
                        blob: row.get(7)?,
                        created_at: row.get(8)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("búsqueda de memoria {id}")))?;

        let error = record
            .error_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
        let status = if record.blob.is_some() {
            "completed"
        } else if record.remote_status == "failed"
            || record.local_state == "orphaned"
            || record.remote_status == "completed"
        {
            "failed"
        } else {
            "searching"
        };
        let mut results = Vec::new();
        if let (Some(model_name), Some(dimensions), Some(search_blob)) = (
            record.model.as_deref(),
            record.dimensions,
            record.blob.as_deref(),
        ) {
            let search_vector = decode_embedding(search_blob, dimensions)?;
            let mut statement = connection.prepare(
                "SELECT m.id, m.content, m.category, p.name, m.sensitivity,
                        er.dimensions, er.vector_blob
                 FROM memory_items m
                 JOIN embedding_records er
                   ON er.source_type = 'memory' AND er.source_id = m.id
                  AND er.model = ?1 AND er.dimensions = ?2
                 LEFT JOIN projects p ON p.id = m.project_id
                 WHERE m.enabled = 1 AND m.custom_gpt_id IS NULL
                   AND (m.project_id IS NULL OR (?3 IS NOT NULL AND m.project_id = ?3))
                 ORDER BY m.updated_at DESC",
            )?;
            let candidates = statement
                .query_map(params![model_name, dimensions, record.project_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, Vec<u8>>(6)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (memory_id, content, category, project_name, sensitivity, dims, candidate_blob) in
                candidates
            {
                let candidate = decode_embedding(&candidate_blob, dims)?;
                let score = cosine_similarity(&search_vector, &candidate);
                if score.is_finite() && score >= 0.25 {
                    let reason = if score >= 0.75 {
                        "Coincidencia semántica alta"
                    } else if score >= 0.5 {
                        "Coincidencia semántica media"
                    } else {
                        "Coincidencia semántica baja"
                    };
                    results.push(MemorySearchResultView {
                        memory_id,
                        content,
                        category,
                        project_name,
                        sensitivity,
                        score: (score * 1000.0).round() / 1000.0,
                        reason: reason.to_owned(),
                    });
                }
            }
            results.sort_by(|left, right| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            results.truncate(5);
        }
        Ok(MemorySearchView {
            id: id.to_owned(),
            query: record.query,
            project_id: record.project_id,
            status: status.to_owned(),
            model: record.model,
            error: error.or_else(|| {
                (record.remote_status == "completed" && status == "failed").then(|| {
                    "Broker AI completó la tarea sin devolver un vector utilizable".to_owned()
                })
            }),
            results,
            created_at: record.created_at,
        })
    }

    pub fn latest_memory_search(&self) -> Result<Option<MemorySearchView>, AppError> {
        let id = self
            .connect()?
            .query_row(
                "SELECT id FROM memory_searches ORDER BY created_at DESC, rowid DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        id.map(|id| self.memory_search(&id)).transpose()
    }

    pub fn active_memories_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<MemoryItemView>, AppError> {
        let overview = self.memory_overview()?;
        if !overview.enabled {
            return Ok(Vec::new());
        }
        let project_id: Option<String> = self.connect()?.query_row(
            "SELECT project_id FROM conversations
             WHERE id = ?1 AND deleted_at IS NULL",
            params![conversation_id],
            |row| row.get(0),
        )?;
        let mut total_chars = 0_usize;
        Ok(overview
            .items
            .into_iter()
            .filter(|item| item.enabled)
            .filter(|item| item.project_id.is_none() || item.project_id == project_id)
            .filter(|item| {
                total_chars += item.content.chars().count();
                total_chars <= 8_000
            })
            .take(20)
            .collect())
    }

    fn project_summary(&self, id: &str) -> Result<ProjectSummary, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT p.id, p.name, p.description, COUNT(c.id), p.updated_at
                 FROM projects p
                 LEFT JOIN conversations c
                   ON c.project_id = p.id
                  AND c.archived_at IS NULL
                  AND c.deleted_at IS NULL
                 WHERE p.id = ?1 AND p.archived_at IS NULL
                 GROUP BY p.id, p.name, p.description, p.updated_at",
                params![id],
                |row| {
                    Ok(ProjectSummary {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        conversation_count: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("proyecto {id}")))
    }

    pub fn rename_project(&self, id: &str, name: &str) -> Result<ProjectSummary, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE projects
             SET name = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL",
            params![id, name],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("proyecto {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('project.renamed', 'user', ?1)",
            params![serde_json::json!({"project_id": id, "name": name}).to_string()],
        )?;
        transaction.commit()?;
        self.project_summary(id)
    }

    pub fn archive_project(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE projects
             SET archived_at = datetime('now'), updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("proyecto {id}")));
        }
        transaction.execute(
            "UPDATE conversations
             SET project_id = NULL, updated_at = datetime('now')
             WHERE project_id = ?1 AND deleted_at IS NULL",
            params![id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('project.archived', 'user', ?1)",
            params![serde_json::json!({"project_id": id}).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn create_conversation(
        &self,
        title: &str,
        project_id: Option<&str>,
    ) -> Result<ConversationSummary, AppError> {
        let id = format!("conv_{}", Uuid::new_v4().simple());
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        if let Some(project_id) = project_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM projects
                    WHERE id = ?1 AND archived_at IS NULL
                 )",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        transaction.execute(
            "INSERT INTO conversations(id, project_id, title) VALUES (?1, ?2, ?3)",
            params![id, project_id, title],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.created', 'user', ?1, ?2)",
            params![
                id,
                serde_json::json!({"conversation_id": id, "project_id": project_id}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_summary(&id)
    }

    pub fn list_conversations(&self) -> Result<Vec<ConversationSummary>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, title, project_id, updated_at
             FROM conversations
             WHERE archived_at IS NULL AND deleted_at IS NULL
             ORDER BY updated_at DESC",
        )?;
        let conversations = statement
            .query_map([], |row| {
                Ok(ConversationSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    project_id: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(conversations)
    }

    pub fn search_conversations(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ConversationSummary>, AppError> {
        let connection = self.connect()?;
        let escaped = query
            .replace('!', "!!")
            .replace('%', "!%")
            .replace('_', "!_");
        let pattern = format!("%{escaped}%");
        let mut statement = connection.prepare(
            "SELECT c.id, c.title, c.project_id, c.updated_at
             FROM conversations c
             WHERE c.archived_at IS NULL
               AND c.deleted_at IS NULL
               AND (
                    c.title LIKE ?1 ESCAPE '!' COLLATE NOCASE
                    OR EXISTS(
                        SELECT 1
                        FROM messages m
                        JOIN message_parts mp ON mp.message_id = m.id
                        WHERE m.conversation_id = c.id
                          AND mp.content_text LIKE ?1 ESCAPE '!' COLLATE NOCASE
                    )
               )
             ORDER BY c.updated_at DESC
             LIMIT ?2",
        )?;
        let conversations = statement
            .query_map(params![pattern, limit as i64], |row| {
                Ok(ConversationSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    project_id: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(conversations)
    }

    fn conversation_summary(&self, id: &str) -> Result<ConversationSummary, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT id, title, project_id, updated_at
                 FROM conversations
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |row| {
                    Ok(ConversationSummary {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        project_id: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("conversación {id}")))
    }

    pub fn rename_conversation(
        &self,
        id: &str,
        title: &str,
    ) -> Result<ConversationSummary, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE conversations
             SET title = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id, title],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("conversación {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.renamed', 'user', ?1, ?2)",
            params![
                id,
                serde_json::json!({"conversation_id": id, "title": title}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_summary(id)
    }

    pub fn move_conversation(
        &self,
        id: &str,
        project_id: Option<&str>,
    ) -> Result<ConversationSummary, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        if let Some(project_id) = project_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM projects
                    WHERE id = ?1 AND archived_at IS NULL
                 )",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        let changed = transaction.execute(
            "UPDATE conversations
             SET project_id = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL",
            params![id, project_id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("conversación activa {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.moved', 'user', ?1, ?2)",
            params![
                id,
                serde_json::json!({"conversation_id": id, "project_id": project_id}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_summary(id)
    }

    fn ensure_conversation_can_hide(
        transaction: &rusqlite::Transaction<'_>,
        id: &str,
    ) -> Result<(), AppError> {
        let active_tasks: i64 = transaction.query_row(
            "SELECT COUNT(*)
             FROM broker_tasks
             WHERE conversation_id = ?1
               AND local_state NOT IN ('terminal', 'orphaned')",
            params![id],
            |row| row.get(0),
        )?;
        if active_tasks > 0 {
            return Err(AppError::Conflict(
                "la conversación tiene una tarea en curso; cancélala o espera a que termine"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    pub fn archive_conversation(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        Self::ensure_conversation_can_hide(&transaction, id)?;
        let changed = transaction.execute(
            "UPDATE conversations
             SET archived_at = datetime('now'), updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("conversación activa {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.archived', 'user', ?1, ?2)",
            params![id, serde_json::json!({"conversation_id": id}).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn delete_conversation(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        Self::ensure_conversation_can_hide(&transaction, id)?;
        let changed = transaction.execute(
            "UPDATE conversations
             SET deleted_at = datetime('now'), updated_at = datetime('now')
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("conversación {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.deleted', 'user', ?1, ?2)",
            params![id, serde_json::json!({"conversation_id": id}).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn recent_context(
        &self,
        conversation_id: &str,
        message_limit: usize,
        character_limit: usize,
    ) -> Result<Vec<ContextMessage>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT m.id, m.role, mp.content_text
             FROM messages m
             JOIN message_parts mp ON mp.message_id = m.id AND mp.ordinal = 0
             WHERE m.conversation_id = ?1
               AND m.status = 'complete'
               AND m.role IN ('user', 'assistant')
               AND mp.kind IN ('text', 'markdown')
             ORDER BY m.sequence_no DESC
             LIMIT ?2",
        )?;
        let mut newest_first = statement
            .query_map(params![conversation_id, message_limit as i64], |row| {
                Ok(ContextMessage {
                    message_id: row.get(0)?,
                    role: row.get(1)?,
                    text: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        newest_first.reverse();

        let mut selected = Vec::new();
        let mut used = 0_usize;
        for message in newest_first.into_iter().rev() {
            let remaining = character_limit.saturating_sub(used);
            if remaining == 0 {
                break;
            }
            let mut message = message;
            if message.text.chars().count() > remaining {
                message.text = message
                    .text
                    .chars()
                    .rev()
                    .take(remaining)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
            }
            used += message.text.chars().count();
            selected.push(message);
        }
        selected.reverse();
        Ok(selected)
    }

    pub fn register_attachment(
        &self,
        conversation_id: &str,
        local_path: &str,
        display_name: &str,
        media_type: Option<&str>,
        size_bytes: i64,
        sha256: &str,
    ) -> Result<AttachmentView, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let active: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM conversations
                WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL
             )",
            params![conversation_id],
            |row| row.get(0),
        )?;
        if !active {
            return Err(AppError::NotFound(format!(
                "conversación activa {conversation_id}"
            )));
        }
        let existing: Option<String> = transaction
            .query_row(
                "SELECT id FROM attachments WHERE sha256 = ?1",
                params![sha256],
                |row| row.get(0),
            )
            .optional()?;
        let attachment_id =
            existing.unwrap_or_else(|| format!("attachment_{}", Uuid::new_v4().simple()));
        transaction.execute(
            "INSERT OR IGNORE INTO attachments(
                id, local_path, display_name, media_type, size_bytes, sha256
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                attachment_id,
                local_path,
                display_name,
                media_type,
                size_bytes,
                sha256
            ],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO conversation_attachments(conversation_id, attachment_id)
             VALUES (?1, ?2)",
            params![conversation_id, attachment_id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES ('attachment.added', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({
                    "attachment_id": attachment_id,
                    "sha256": sha256,
                    "size_bytes": size_bytes
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        self.attachment_view(&attachment_id)
    }

    pub fn list_attachments(&self, conversation_id: &str) -> Result<Vec<AttachmentView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT a.id, a.display_name, a.media_type, a.size_bytes, a.sha256,
                    a.broker_file_id, a.ingestion_status, a.ingestion_error_json, a.updated_at
             FROM conversation_attachments ca
             JOIN attachments a ON a.id = ca.attachment_id
             WHERE ca.conversation_id = ?1
             ORDER BY ca.added_at, a.created_at",
        )?;
        let attachments = statement
            .query_map(params![conversation_id], Self::map_attachment_view)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(attachments)
    }

    pub fn remove_conversation_attachment(
        &self,
        conversation_id: &str,
        attachment_id: &str,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let changed = connection.execute(
            "DELETE FROM conversation_attachments
             WHERE conversation_id = ?1 AND attachment_id = ?2",
            params![conversation_id, attachment_id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("adjunto {attachment_id}")));
        }
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES ('attachment.removed', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({"attachment_id": attachment_id}).to_string()
            ],
        )?;
        Ok(())
    }

    fn map_attachment_view(row: &rusqlite::Row<'_>) -> rusqlite::Result<AttachmentView> {
        let error_json: Option<String> = row.get(7)?;
        Ok(AttachmentView {
            id: row.get(0)?,
            display_name: row.get(1)?,
            media_type: row.get(2)?,
            size_bytes: row.get(3)?,
            sha256: row.get(4)?,
            broker_file_id: row.get(5)?,
            ingestion_status: row.get(6)?,
            ingestion_error: error_json.and_then(|value| serde_json::from_str(&value).ok()),
            updated_at: row.get(8)?,
        })
    }

    pub fn attachment_view(&self, id: &str) -> Result<AttachmentView, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT id, display_name, media_type, size_bytes, sha256,
                        broker_file_id, ingestion_status, ingestion_error_json, updated_at
                 FROM attachments WHERE id = ?1",
                params![id],
                Self::map_attachment_view,
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("adjunto {id}")))
    }

    pub fn attachment_record(&self, id: &str) -> Result<AttachmentRecord, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT id, local_path, display_name, media_type, size_bytes, sha256,
                        broker_file_id, ingestion_status
                 FROM attachments WHERE id = ?1",
                params![id],
                |row| {
                    Ok(AttachmentRecord {
                        id: row.get(0)?,
                        local_path: row.get(1)?,
                        display_name: row.get(2)?,
                        media_type: row.get(3)?,
                        size_bytes: row.get(4)?,
                        sha256: row.get(5)?,
                        broker_file_id: row.get(6)?,
                        ingestion_status: row.get(7)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("adjunto {id}")))
    }

    pub fn recoverable_attachments(&self) -> Result<Vec<AttachmentRecord>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id FROM attachments
             WHERE ingestion_status IN ('uploading', 'received', 'converting')
             ORDER BY updated_at",
        )?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| self.attachment_record(&id))
            .collect()
    }

    pub fn mark_attachment_uploading(&self, id: &str) -> Result<(), AppError> {
        self.update_attachment_ingestion(id, "uploading", None, None, None, None, None)
    }

    pub fn reset_failed_attachment_for_retry(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE attachments
             SET broker_file_id = NULL,
                 ingestion_status = 'local',
                 ingestion_error_json = NULL,
                 kind = NULL,
                 engine = NULL,
                 ingestion_meta_json = NULL,
                 updated_at = datetime('now')
             WHERE id = ?1 AND ingestion_status = 'failed'",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::Conflict(
                "solo se puede reintentar un adjunto fallido".to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('attachment.retry_requested', 'user', ?1)",
            params![serde_json::json!({"attachment_id": id}).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_attachment_ingestion(
        &self,
        id: &str,
        status: &str,
        broker_file_id: Option<&str>,
        kind: Option<&str>,
        engine: Option<&str>,
        meta: Option<&Value>,
        error: Option<&Value>,
    ) -> Result<(), AppError> {
        let meta_json = meta
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let error_json = error
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        let changed = connection.execute(
            "UPDATE attachments
             SET ingestion_status = ?2,
                 broker_file_id = COALESCE(?3, broker_file_id),
                 kind = COALESCE(?4, kind),
                 engine = COALESCE(?5, engine),
                 ingestion_meta_json = COALESCE(?6, ingestion_meta_json),
                 ingestion_error_json = ?7,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![
                id,
                status,
                broker_file_id,
                kind,
                engine,
                meta_json,
                error_json
            ],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("adjunto {id}")));
        }
        Ok(())
    }

    pub fn ready_attachments_for_turn(
        &self,
        conversation_id: &str,
        attachment_ids: &[String],
    ) -> Result<Vec<AttachmentRecord>, AppError> {
        let connection = self.connect()?;
        let mut result = Vec::with_capacity(attachment_ids.len());
        for id in attachment_ids {
            let record = self.attachment_record(id)?;
            let linked: bool = connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM conversation_attachments
                    WHERE conversation_id = ?1 AND attachment_id = ?2
                 )",
                params![conversation_id, id],
                |row| row.get(0),
            )?;
            if !linked {
                return Err(AppError::Validation(format!(
                    "el adjunto {} no pertenece a esta conversación",
                    record.display_name
                )));
            }
            if record.ingestion_status != "ready" || record.broker_file_id.is_none() {
                return Err(AppError::Conflict(format!(
                    "el adjunto {} todavía no está listo",
                    record.display_name
                )));
            }
            result.push(record);
        }
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_chat_turn(
        &self,
        conversation_id: &str,
        user_message_id: &str,
        assistant_message_id: &str,
        local_task_id: &str,
        idempotency_key: &str,
        user_text: &str,
        request: &Value,
        context: &[ContextMessage],
        memories: &[MemoryItemView],
        attachment_ids: &[String],
    ) -> Result<BrokerTaskRecord, AppError> {
        let request_json = serde_json::to_string(request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let context_json = serde_json::to_string(&serde_json::json!({
            "messages": context,
            "memories": memories
        }))
        .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let next_sequence: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence_no), 0) + 1
             FROM messages WHERE conversation_id = ?1",
            params![conversation_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO messages(
                id, conversation_id, role, status, sequence_no
             ) VALUES (?1, ?2, 'user', 'complete', ?3)",
            params![user_message_id, conversation_id, next_sequence],
        )?;
        for (ordinal, attachment_id) in attachment_ids.iter().enumerate() {
            let usable: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1
                    FROM conversation_attachments ca
                    JOIN attachments a ON a.id = ca.attachment_id
                    WHERE ca.conversation_id = ?1
                      AND ca.attachment_id = ?2
                      AND a.ingestion_status = 'ready'
                      AND a.broker_file_id IS NOT NULL
                 )",
                params![conversation_id, attachment_id],
                |row| row.get(0),
            )?;
            if !usable {
                return Err(AppError::Conflict(
                    "uno de los adjuntos ya no esta listo para enviar".to_owned(),
                ));
            }
            transaction.execute(
                "INSERT INTO message_attachments(message_id, attachment_id, ordinal)
                 VALUES (?1, ?2, ?3)",
                params![user_message_id, attachment_id, ordinal as i64],
            )?;
        }
        transaction.execute(
            "INSERT INTO message_parts(
                id, message_id, ordinal, kind, content_text
             ) VALUES (?1, ?2, 0, 'text', ?3)",
            params![
                format!("part_{}", Uuid::new_v4().simple()),
                user_message_id,
                user_text
            ],
        )?;
        transaction.execute(
            "INSERT INTO messages(
                id, conversation_id, role, status, sequence_no
             ) VALUES (?1, ?2, 'assistant', 'pending', ?3)",
            params![assistant_message_id, conversation_id, next_sequence + 1],
        )?;
        transaction.execute(
            "INSERT INTO broker_tasks(
                id, conversation_id, request_message_id, response_message_id,
                idempotency_key, request_json, remote_status, local_state
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'not_submitted', 'created')",
            params![
                local_task_id,
                conversation_id,
                user_message_id,
                assistant_message_id,
                idempotency_key,
                request_json
            ],
        )?;
        transaction.execute(
            "UPDATE messages SET broker_task_id = ?2 WHERE id = ?1",
            params![assistant_message_id, local_task_id],
        )?;
        let snapshot_id = format!("ctx_{}", Uuid::new_v4().simple());
        let strategy_version = if memories.is_empty() {
            "window-v1"
        } else {
            "window-memory-v1"
        };
        transaction.execute(
            "INSERT INTO context_snapshots(
                id, broker_task_id, strategy_version, token_budget,
                estimated_tokens, final_context_json
             ) VALUES (?1, ?2, ?3, NULL, ?4, ?5)",
            params![
                snapshot_id,
                local_task_id,
                strategy_version,
                (context_json.chars().count() as i64 + 3) / 4,
                context_json
            ],
        )?;
        for (ordinal, source) in context.iter().enumerate() {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'message', ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    source.message_id,
                    ordinal as i64,
                    if source.message_id == user_message_id {
                        "current_user_turn"
                    } else {
                        "recent_conversation_window"
                    },
                    (source.text.chars().count() as i64 + 3) / 4,
                    source.text
                ],
            )?;
        }
        for (index, memory) in memories.iter().enumerate() {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'memory', ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    memory.id,
                    (context.len() + index) as i64,
                    "Recuerdo activado explícitamente por el usuario",
                    (memory.content.chars().count() as i64 + 3) / 4,
                    memory.content
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'local.prepared', 'not_submitted', '{}', datetime('now'))",
            params![local_task_id],
        )?;
        transaction.execute(
            "UPDATE conversations
             SET title = CASE WHEN NOT EXISTS(
                    SELECT 1 FROM messages
                    WHERE conversation_id = ?1 AND sequence_no < ?2
                 ) THEN substr(?3, 1, 80) ELSE title END,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![conversation_id, next_sequence, user_text],
        )?;
        transaction.commit()?;
        self.task_record(local_task_id)
    }

    pub fn conversation_view(&self, id: &str) -> Result<ConversationView, AppError> {
        let summary = self.conversation_summary(id)?;
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT m.id, m.role, m.status, m.sequence_no,
                    m.broker_task_id, bt.remote_status, bt.local_state,
                    mp.content_text, mp.content_json, m.created_at,
                    json_extract(bt.result_json, '$.model_used.provider'),
                    json_extract(bt.result_json, '$.model_used.deployment'),
                    json_extract(bt.result_json, '$.model_used.model')
             FROM messages m
             LEFT JOIN message_parts mp ON mp.message_id = m.id AND mp.ordinal = 0
             LEFT JOIN broker_tasks bt ON bt.id = m.broker_task_id
             WHERE m.conversation_id = ?1
             ORDER BY m.sequence_no",
        )?;
        let messages = statement
            .query_map(params![id], |row| {
                let error_json: Option<String> = row.get(8)?;
                let model_provider: Option<String> = row.get(10)?;
                let model_deployment: Option<String> = row.get(11)?;
                let model_name: Option<String> = row.get(12)?;
                Ok(ConversationMessage {
                    id: row.get(0)?,
                    role: row.get(1)?,
                    status: row.get(2)?,
                    sequence_no: row.get(3)?,
                    broker_task_id: row.get(4)?,
                    task_remote_status: row.get(5)?,
                    task_local_state: row.get(6)?,
                    text: row.get(7)?,
                    error: error_json.and_then(|value| serde_json::from_str(&value).ok()),
                    model_used: match (model_provider, model_deployment, model_name) {
                        (Some(provider), Some(deployment), Some(model)) => Some(ModelUsedView {
                            provider,
                            deployment,
                            model,
                        }),
                        _ => None,
                    },
                    sources: Vec::new(),
                    created_at: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut source_statement = connection.prepare(
            "SELECT c.message_id, c.id,
                    COALESCE(c.title, a.display_name, 'Fuente'),
                    c.source_attachment_id, a.media_type, a.size_bytes,
                    c.url, c.quote_text, c.claim_text
             FROM citations c
             JOIN messages m ON m.id = c.message_id
             LEFT JOIN attachments a ON a.id = c.source_attachment_id
             WHERE m.conversation_id = ?1
             ORDER BY c.message_id, c.ordinal",
        )?;
        let source_rows = source_statement
            .query_map(params![id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ConversationSource {
                        id: row.get(1)?,
                        title: row.get(2)?,
                        source_attachment_id: row.get(3)?,
                        media_type: row.get(4)?,
                        size_bytes: row.get(5)?,
                        url: row.get(6)?,
                        quote_text: row.get(7)?,
                        claim_text: row.get(8)?,
                    },
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut sources_by_message: HashMap<String, Vec<ConversationSource>> = HashMap::new();
        for (message_id, source) in source_rows {
            sources_by_message
                .entry(message_id)
                .or_default()
                .push(source);
        }
        let messages = messages
            .into_iter()
            .map(|mut message| {
                message.sources = sources_by_message.remove(&message.id).unwrap_or_default();
                message
            })
            .collect();
        Ok(ConversationView {
            id: summary.id,
            title: summary.title,
            project_id: summary.project_id,
            messages,
        })
    }

    pub fn task_record(&self, id: &str) -> Result<BrokerTaskRecord, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT id, remote_task_id, request_json, consecutive_poll_errors
                 FROM broker_tasks WHERE id = ?1",
                params![id],
                |row| {
                    let request_json: String = row.get(2)?;
                    let request = serde_json::from_str(&request_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            request_json.len(),
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    Ok(BrokerTaskRecord {
                        id: row.get(0)?,
                        remote_task_id: row.get(1)?,
                        request,
                        consecutive_poll_errors: row.get(3)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::BrokerContract(format!("tarea local no encontrada: {id}")))
    }

    pub fn recoverable_tasks(&self) -> Result<Vec<BrokerTaskRecord>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id FROM broker_tasks
             WHERE local_state IN (
                'created', 'submitting', 'polling', 'recovery_pending'
             )
             ORDER BY created_at",
        )?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter().map(|id| self.task_record(&id)).collect()
    }

    pub fn mark_submitting(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        connection.execute(
            "UPDATE broker_tasks
             SET local_state = 'submitting', attempt = attempt + 1,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn attach_remote_task(&self, id: &str, accepted: &TaskAccepted) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE broker_tasks
             SET remote_task_id = ?2, remote_status = ?3, local_state = 'polling',
                 consecutive_poll_errors = 0, next_poll_at = datetime('now'),
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id, accepted.task_id, accepted.status.as_str()],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'remote.accepted', ?2, ?3, datetime('now'))",
            params![
                id,
                accepted.status.as_str(),
                serde_json::to_string(accepted)
                    .map_err(|error| AppError::BrokerContract(error.to_string()))?
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn record_remote_state(&self, id: &str, state: &TaskState) -> Result<(), AppError> {
        let connection = self.connect()?;
        let (previous, request_message_id, response_message_id, conversation_id, request): (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Value,
        ) = connection.query_row(
            "SELECT remote_status, request_message_id, response_message_id, conversation_id,
                    request_json
             FROM broker_tasks WHERE id = ?1",
            params![id],
            |row| {
                let request_json: String = row.get(4)?;
                let request = serde_json::from_str(&request_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        request_json.len(),
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, request))
            },
        )?;
        let local_state = if state.status.is_terminal() {
            "terminal"
        } else if state.status.as_str() == "waiting_for_tools" {
            "waiting_for_tools"
        } else {
            "polling"
        };
        let result_json = state
            .result
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let error_json = state
            .error
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let payload_json = serde_json::to_string(state)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE broker_tasks
             SET remote_status = ?2, local_state = ?3,
                 consecutive_poll_errors = 0, result_json = ?4, error_json = ?5,
                 terminal_at = CASE WHEN ?3 = 'terminal' THEN datetime('now') ELSE NULL END,
                 next_poll_at = CASE WHEN ?3 = 'polling' THEN datetime('now') ELSE NULL END,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![
                id,
                state.status.as_str(),
                local_state,
                result_json,
                error_json
            ],
        )?;
        if previous != state.status.as_str() {
            transaction.execute(
                "INSERT INTO broker_task_events(
                    broker_task_id, event_type, remote_status, payload_json, occurred_at
                 ) VALUES (?1, 'remote.status_changed', ?2, ?3, datetime('now'))",
                params![id, state.status.as_str(), payload_json],
            )?;
        }
        if state.status.as_str() == "completed"
            && request.get("inference_kind").and_then(Value::as_str) == Some("embedding")
        {
            let metadata = request
                .get("content")
                .and_then(|content| content.get("metadata"));
            let source_type = metadata
                .and_then(|value| value.get("source_type"))
                .and_then(Value::as_str);
            let source_id = metadata
                .and_then(|value| value.get("source_id"))
                .and_then(Value::as_str);
            let content_sha256 = metadata
                .and_then(|value| value.get("content_sha256"))
                .and_then(Value::as_str);
            let vector = state
                .result
                .as_ref()
                .and_then(|result| result.get("embedding"))
                .and_then(Value::as_array);
            if let (Some(source_type), Some(source_id), Some(content_sha256), Some(vector)) =
                (source_type, source_id, content_sha256, vector)
            {
                let values = vector
                    .iter()
                    .map(|value| {
                        value.as_f64().ok_or_else(|| {
                            AppError::BrokerContract(
                                "el embedding contiene un valor no numérico".to_owned(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if values.is_empty() {
                    return Err(AppError::BrokerContract(
                        "el embedding completado está vacío".to_owned(),
                    ));
                }
                let mut vector_blob = Vec::with_capacity(values.len() * 8);
                for value in &values {
                    vector_blob.extend_from_slice(&value.to_le_bytes());
                }
                let model_used = state
                    .result
                    .as_ref()
                    .and_then(|result| result.get("model_used"));
                let model = model_used
                    .map(|model| {
                        format!(
                            "{}/{}/{}",
                            model
                                .get("provider")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown"),
                            model
                                .get("deployment")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown"),
                            model
                                .get("model")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown")
                        )
                    })
                    .unwrap_or_else(|| "unknown/unknown/unknown".to_owned());
                transaction.execute(
                    "INSERT INTO embedding_records(
                        id, source_type, source_id, chunk_index, model,
                        dimensions, vector_blob, content_sha256
                     ) VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?7)
                     ON CONFLICT(source_type, source_id, chunk_index, model) DO UPDATE SET
                        dimensions = excluded.dimensions,
                        vector_blob = excluded.vector_blob,
                        content_sha256 = excluded.content_sha256,
                        created_at = datetime('now')",
                    params![
                        format!("embedding_{}", Uuid::new_v4().simple()),
                        source_type,
                        source_id,
                        model,
                        values.len() as i64,
                        vector_blob,
                        content_sha256
                    ],
                )?;
            }
        }
        if state.status.as_str() == "waiting_for_tools" {
            let pending = state
                .result
                .as_ref()
                .and_then(|result| result.get("pending_tool_calls"))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    AppError::BrokerContract(
                        "waiting_for_tools no incluye pending_tool_calls".to_owned(),
                    )
                })?;
            for call in pending {
                let remote_tool_call_id =
                    call.get("id").and_then(Value::as_str).ok_or_else(|| {
                        AppError::BrokerContract(
                            "una llamada de herramienta no incluye id".to_owned(),
                        )
                    })?;
                let tool_name = call.get("name").and_then(Value::as_str).ok_or_else(|| {
                    AppError::BrokerContract(
                        "una llamada de herramienta no incluye name".to_owned(),
                    )
                })?;
                let arguments = call
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                transaction.execute(
                    "INSERT INTO tool_calls(
                        id, broker_task_id, remote_tool_call_id, tool_name,
                        arguments_json, status
                     ) VALUES (?1, ?2, ?3, ?4, ?5, 'confirmation_required')
                     ON CONFLICT(broker_task_id, remote_tool_call_id) DO UPDATE SET
                        tool_name = excluded.tool_name,
                        arguments_json = excluded.arguments_json,
                        status = CASE
                            WHEN tool_calls.status IN ('requested', 'confirmation_required')
                            THEN 'confirmation_required'
                            ELSE tool_calls.status
                        END",
                    params![
                        format!("toolcall_{}", Uuid::new_v4().simple()),
                        id,
                        remote_tool_call_id,
                        tool_name,
                        arguments.to_string()
                    ],
                )?;
            }
        }
        if previous != state.status.as_str() && state.status.is_terminal() {
            if let Some(message_id) = response_message_id {
                let (message_status, kind, content_text, content_json) =
                    if state.status.as_str() == "completed" {
                        let markdown = state
                            .result
                            .as_ref()
                            .and_then(|result| result.get("result_markdown"))
                            .and_then(Value::as_str)
                            .unwrap_or("La tarea terminó sin contenido Markdown.")
                            .to_owned();
                        ("complete", "markdown", Some(markdown), None)
                    } else {
                        (
                            if state.status.as_str() == "cancelled" {
                                "cancelled"
                            } else {
                                "failed"
                            },
                            "error",
                            None,
                            Some(
                                state
                                    .error
                                    .clone()
                                    .unwrap_or_else(
                                        || serde_json::json!({"status": state.status.as_str()}),
                                    )
                                    .to_string(),
                            ),
                        )
                    };
                transaction.execute(
                    "UPDATE messages SET status = ?2, updated_at = datetime('now')
                     WHERE id = ?1",
                    params![message_id, message_status],
                )?;
                transaction.execute(
                    "INSERT INTO message_parts(
                        id, message_id, ordinal, kind, content_text, content_json
                     ) VALUES (?1, ?2, 0, ?3, ?4, ?5)
                     ON CONFLICT(message_id, ordinal) DO UPDATE SET
                        kind = excluded.kind,
                        content_text = excluded.content_text,
                        content_json = excluded.content_json",
                    params![
                        format!("part_{}", Uuid::new_v4().simple()),
                        message_id,
                        kind,
                        content_text,
                        content_json
                    ],
                )?;
                if state.status.as_str() == "completed" {
                    if let Some(request_message_id) = request_message_id.as_deref() {
                        let sources = {
                            let mut statement = transaction.prepare(
                                "SELECT a.id, a.display_name, a.broker_file_id,
                                        a.media_type, a.size_bytes, ma.ordinal
                                 FROM message_attachments ma
                                 JOIN attachments a ON a.id = ma.attachment_id
                                 WHERE ma.message_id = ?1
                                 ORDER BY ma.ordinal",
                            )?;
                            let rows = statement
                                .query_map(params![request_message_id], |row| {
                                    Ok((
                                        row.get::<_, String>(0)?,
                                        row.get::<_, String>(1)?,
                                        row.get::<_, Option<String>>(2)?,
                                        row.get::<_, Option<String>>(3)?,
                                        row.get::<_, Option<i64>>(4)?,
                                        row.get::<_, i64>(5)?,
                                    ))
                                })?
                                .collect::<Result<Vec<_>, _>>()?;
                            rows
                        };
                        for (
                            attachment_id,
                            title,
                            broker_file_id,
                            media_type,
                            size_bytes,
                            ordinal,
                        ) in sources
                        {
                            let metadata = serde_json::json!({
                                "kind": "broker_file",
                                "broker_file_id": broker_file_id,
                                "media_type": media_type,
                                "size_bytes": size_bytes,
                                "attribution": "turn_attachment"
                            });
                            transaction.execute(
                                "INSERT INTO citations(
                                    id, message_id, ordinal, title,
                                    source_attachment_id, metadata_json
                                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                                 ON CONFLICT(message_id, ordinal) DO UPDATE SET
                                    title = excluded.title,
                                    source_attachment_id = excluded.source_attachment_id,
                                    metadata_json = excluded.metadata_json",
                                params![
                                    format!("citation_{}", Uuid::new_v4().simple()),
                                    message_id,
                                    ordinal,
                                    title,
                                    attachment_id,
                                    metadata.to_string()
                                ],
                            )?;
                        }
                    }
                }
                if let Some(conversation_id) = conversation_id {
                    transaction.execute(
                        "UPDATE conversations SET updated_at = datetime('now') WHERE id = ?1",
                        params![conversation_id],
                    )?;
                }
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn record_transport_error(&self, id: &str, message: &str) -> Result<(), AppError> {
        let payload = serde_json::json!({"message": message});
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE broker_tasks
             SET consecutive_poll_errors = consecutive_poll_errors + 1,
                 next_poll_at = datetime('now', '+' ||
                    min(60, (consecutive_poll_errors + 1) * 2) || ' seconds'),
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) SELECT id, 'transport.error', remote_status, ?2, datetime('now')
               FROM broker_tasks WHERE id = ?1",
            params![id, payload.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn mark_orphaned(&self, id: &str, message: &str) -> Result<(), AppError> {
        let payload = serde_json::json!({"message": message});
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let (response_message_id, conversation_id): (Option<String>, Option<String>) = transaction
            .query_row(
                "SELECT response_message_id, conversation_id
                 FROM broker_tasks WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
        transaction.execute(
            "UPDATE broker_tasks
             SET local_state = 'orphaned', error_json = ?2, next_poll_at = NULL,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id, payload.to_string()],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) SELECT id, 'local.orphaned', remote_status, ?2, datetime('now')
               FROM broker_tasks WHERE id = ?1",
            params![id, payload.to_string()],
        )?;
        if let Some(message_id) = response_message_id {
            transaction.execute(
                "UPDATE messages
                 SET status = 'failed', updated_at = datetime('now')
                 WHERE id = ?1",
                params![message_id],
            )?;
            transaction.execute(
                "INSERT INTO message_parts(
                    id, message_id, ordinal, kind, content_json
                 ) VALUES (?1, ?2, 0, 'error', ?3)
                 ON CONFLICT(message_id, ordinal) DO UPDATE SET
                    kind = excluded.kind,
                    content_text = NULL,
                    content_json = excluded.content_json",
                params![
                    format!("part_{}", Uuid::new_v4().simple()),
                    message_id,
                    payload.to_string()
                ],
            )?;
        }
        if let Some(conversation_id) = conversation_id {
            transaction.execute(
                "UPDATE conversations SET updated_at = datetime('now') WHERE id = ?1",
                params![conversation_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn pending_tool_calls(&self, local_task_id: &str) -> Result<Vec<ToolCallView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT remote_tool_call_id, tool_name, arguments_json, status
             FROM tool_calls
             WHERE broker_task_id = ?1 AND status = 'confirmation_required'
             ORDER BY requested_at, id",
        )?;
        let calls = statement
            .query_map(params![local_task_id], |row| {
                let arguments_json: String = row.get(2)?;
                let arguments = serde_json::from_str(&arguments_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        arguments_json.len(),
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok(ToolCallView {
                    tool_call_id: row.get(0)?,
                    name: row.get(1)?,
                    arguments,
                    status: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(calls)
    }

    pub fn task_conversation_id(&self, local_task_id: &str) -> Result<String, AppError> {
        self.connect()?
            .query_row(
                "SELECT conversation_id FROM broker_tasks WHERE id = ?1",
                params![local_task_id],
                |row| row.get::<_, Option<String>>(0),
            )?
            .ok_or_else(|| AppError::BrokerContract("la tarea no pertenece a un chat".to_owned()))
    }

    pub fn prepare_tool_outcomes(
        &self,
        local_task_id: &str,
        outcomes: &[ToolOutcomeRecord],
    ) -> Result<(), AppError> {
        let expected = self.pending_tool_calls(local_task_id)?;
        let expected_ids: HashSet<&str> = expected
            .iter()
            .map(|call| call.tool_call_id.as_str())
            .collect();
        let provided_ids: HashSet<&str> = outcomes
            .iter()
            .map(|outcome| outcome.tool_call_id.as_str())
            .collect();
        if expected_ids != provided_ids || outcomes.len() != provided_ids.len() {
            return Err(AppError::Validation(
                "debe decidirse exactamente una vez sobre cada herramienta pendiente".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        for outcome in outcomes {
            if !matches!(outcome.status.as_str(), "approved" | "cancelled") {
                return Err(AppError::Validation(
                    "el resultado local de herramienta no es válido".to_owned(),
                ));
            }
            let local_call_id: String = transaction.query_row(
                "SELECT id FROM tool_calls
                 WHERE broker_task_id = ?1 AND remote_tool_call_id = ?2
                   AND status = 'confirmation_required'",
                params![local_task_id, outcome.tool_call_id],
                |row| row.get(0),
            )?;
            transaction.execute(
                "UPDATE tool_calls SET status = ?2 WHERE id = ?1",
                params![local_call_id, outcome.status],
            )?;
            transaction.execute(
                "INSERT INTO tool_results(id, tool_call_id, content_text, is_error)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(tool_call_id) DO UPDATE SET
                    content_text = excluded.content_text,
                    is_error = excluded.is_error",
                params![
                    format!("toolresult_{}", Uuid::new_v4().simple()),
                    local_call_id,
                    outcome.content,
                    i64::from(outcome.status == "cancelled")
                ],
            )?;
        }
        transaction.execute(
            "UPDATE broker_tasks
             SET local_state = 'polling', updated_at = datetime('now')
             WHERE id = ?1",
            params![local_task_id],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'local.tool_decisions_prepared', 'waiting_for_tools', ?2, datetime('now'))",
            params![
                local_task_id,
                serde_json::json!({"count": outcomes.len()}).to_string()
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn prepared_tool_results(&self, local_task_id: &str) -> Result<Value, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT tc.remote_tool_call_id, tr.content_text
             FROM tool_calls tc
             JOIN tool_results tr ON tr.tool_call_id = tc.id
             WHERE tc.broker_task_id = ?1
               AND tc.status IN ('approved', 'cancelled')
             ORDER BY tc.requested_at, tc.id",
        )?;
        let results = statement
            .query_map(params![local_task_id], |row| {
                Ok(serde_json::json!({
                    "tool_call_id": row.get::<_, String>(0)?,
                    "content": row.get::<_, Option<String>>(1)?.unwrap_or_default()
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(serde_json::json!({"tool_results": results}))
    }

    pub fn mark_tool_results_submitted(&self, local_task_id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE tool_calls
             SET status = 'completed', completed_at = datetime('now')
             WHERE broker_task_id = ?1 AND status IN ('approved', 'cancelled')",
            params![local_task_id],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'remote.tool_results_accepted', 'queued', '{}', datetime('now'))",
            params![local_task_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn last_completed_export_hash(
        &self,
        stable_export_id: &str,
        destination_path: &str,
    ) -> Result<Option<String>, AppError> {
        Ok(self
            .connect()?
            .query_row(
                "SELECT destination_hash_after
                 FROM export_records
                 WHERE stable_export_id = ?1 AND destination_path = ?2
                   AND status = 'completed'",
                params![stable_export_id, destination_path],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_export(
        &self,
        source_id: &str,
        stable_export_id: &str,
        destination_path: &str,
        source_hash: &str,
        destination_hash_before: Option<&str>,
        destination_hash_after: Option<&str>,
        status: &str,
        error: Option<&Value>,
    ) -> Result<(), AppError> {
        let error_json = error
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO export_records(
                id, source_type, source_id, stable_export_id, destination_path,
                source_hash, destination_hash_before, destination_hash_after,
                status, error_json
             ) VALUES (?1, 'conversation', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(stable_export_id, destination_path) DO UPDATE SET
                source_id = excluded.source_id,
                source_hash = excluded.source_hash,
                destination_hash_before = excluded.destination_hash_before,
                destination_hash_after = excluded.destination_hash_after,
                status = excluded.status,
                error_json = excluded.error_json,
                updated_at = datetime('now')",
            params![
                format!("export_{}", Uuid::new_v4().simple()),
                source_id,
                stable_export_id,
                destination_path,
                source_hash,
                destination_hash_before,
                destination_hash_after,
                status,
                error_json
            ],
        )?;
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES (?1, 'user', ?2, ?3)",
            params![
                format!("export.{status}"),
                source_id,
                serde_json::json!({
                    "stable_export_id": stable_export_id,
                    "destination_path": destination_path,
                    "source_hash": source_hash
                })
                .to_string()
            ],
        )?;
        Ok(())
    }

    pub fn task_snapshot(&self, id: &str) -> Result<LocalTaskSnapshot, AppError> {
        let connection = self.connect()?;
        let mut snapshot = connection
            .query_row(
                "SELECT id, remote_task_id, remote_status, local_state,
                        consecutive_poll_errors, result_json, error_json, updated_at
                 FROM broker_tasks WHERE id = ?1",
                params![id],
                |row| {
                    let result_json: Option<String> = row.get(5)?;
                    let error_json: Option<String> = row.get(6)?;
                    Ok(LocalTaskSnapshot {
                        id: row.get(0)?,
                        remote_task_id: row.get(1)?,
                        remote_status: row.get(2)?,
                        local_state: row.get(3)?,
                        consecutive_poll_errors: row.get(4)?,
                        result: result_json.and_then(|value| serde_json::from_str(&value).ok()),
                        error: error_json.and_then(|value| serde_json::from_str(&value).ok()),
                        pending_tool_calls: Vec::new(),
                        updated_at: row.get(7)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::BrokerContract(format!("tarea local no encontrada: {id}")))?;
        snapshot.pending_tool_calls = self.pending_tool_calls(id)?;
        Ok(snapshot)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn decode_embedding(blob: &[u8], dimensions: i64) -> Result<Vec<f64>, AppError> {
    let expected = usize::try_from(dimensions)
        .ok()
        .and_then(|value| value.checked_mul(8))
        .ok_or_else(|| AppError::BrokerContract("dimensiones de embedding inválidas".to_owned()))?;
    if blob.len() != expected {
        return Err(AppError::BrokerContract(
            "el vector almacenado no coincide con sus dimensiones".to_owned(),
        ));
    }
    Ok(blob
        .chunks_exact(8)
        .map(|chunk| f64::from_le_bytes(chunk.try_into().expect("chunk de ocho bytes")))
        .collect())
}

fn cosine_similarity(left: &[f64], right: &[f64]) -> f64 {
    if left.len() != right.len() || left.is_empty() {
        return f64::NAN;
    }
    let dot = left.iter().zip(right).map(|(a, b)| a * b).sum::<f64>();
    let left_norm = left.iter().map(|value| value * value).sum::<f64>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f64>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        f64::NAN
    } else {
        (dot / (left_norm * right_norm)).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::{ContextMessage, Database, ToolOutcomeRecord, INITIAL_MIGRATION};
    use crate::broker::TaskState;
    use crate::error::AppError;
    use rusqlite::params;
    use uuid::Uuid;

    fn test_database() -> Database {
        let path = std::env::temp_dir().join(format!(
            "chatygpt-db-test-{}.sqlite",
            Uuid::new_v4().simple()
        ));
        Database::open(path).expect("test database should open")
    }

    fn cleanup(database: &Database) {
        let path = database.path().to_path_buf();
        for candidate in [
            path.clone(),
            path.with_extension("sqlite-wal"),
            path.with_extension("sqlite-shm"),
        ] {
            let _ = std::fs::remove_file(candidate);
        }
    }

    #[test]
    fn projects_search_and_lifecycle_are_audited() {
        let database = test_database();
        let project = database
            .create_project("TFM", Some("Trabajo final"))
            .expect("project should be created");
        let conversation = database
            .create_conversation("Normativa", Some(&project.id))
            .expect("conversation should be created");

        let connection = database.connect().expect("connection should open");
        connection
            .execute(
                "INSERT INTO messages(
                    id, conversation_id, role, status, sequence_no
                 ) VALUES ('message-search', ?1, 'user', 'complete', 1)",
                params![conversation.id],
            )
            .expect("message should be inserted");
        connection
            .execute(
                "INSERT INTO message_parts(
                    id, message_id, ordinal, kind, content_text
                 ) VALUES (
                    'part-search', 'message-search', 0, 'text',
                    'consulta sobre contratación pública'
                 )",
                [],
            )
            .expect("message part should be inserted");
        drop(connection);

        let results = database
            .search_conversations("contratación", 10)
            .expect("search should succeed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, conversation.id);
        assert!(database
            .search_conversations("%", 10)
            .expect("wildcard should be treated literally")
            .is_empty());

        database
            .rename_conversation(&conversation.id, "Normativa española")
            .expect("rename should succeed");
        database
            .archive_project(&project.id)
            .expect("archive should succeed");

        let conversation_after = database
            .conversation_summary(&conversation.id)
            .expect("conversation should remain");
        assert!(conversation_after.project_id.is_none());
        assert!(database
            .list_projects()
            .expect("projects should list")
            .is_empty());

        let connection = database.connect().expect("connection should open");
        let audited: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM audit_events
                 WHERE event_type IN (
                    'project.created', 'conversation.created',
                    'conversation.renamed', 'project.archived'
                 )",
                [],
                |row| row.get(0),
            )
            .expect("audit count should load");
        assert_eq!(audited, 4);
        drop(connection);
        cleanup(&database);
    }

    #[test]
    fn conversation_with_active_task_cannot_be_hidden() {
        let database = test_database();
        let conversation = database
            .create_conversation("Tarea activa", None)
            .expect("conversation should be created");
        let connection = database.connect().expect("connection should open");
        connection
            .execute(
                "INSERT INTO broker_tasks(
                    id, conversation_id, idempotency_key, request_json,
                    remote_status, local_state
                 ) VALUES (
                    'active-task', ?1, 'active-key', '{}',
                    'generating', 'polling'
                 )",
                params![conversation.id],
            )
            .expect("task should be inserted");
        drop(connection);

        assert!(matches!(
            database.archive_conversation(&conversation.id),
            Err(AppError::Conflict(_))
        ));
        assert!(matches!(
            database.delete_conversation(&conversation.id),
            Err(AppError::Conflict(_))
        ));

        let connection = database.connect().expect("connection should open");
        connection
            .execute(
                "UPDATE broker_tasks
                 SET remote_status = 'completed', local_state = 'terminal'
                 WHERE id = 'active-task'",
                [],
            )
            .expect("task should become terminal");
        drop(connection);

        database
            .delete_conversation(&conversation.id)
            .expect("terminal conversation can be deleted");
        assert!(matches!(
            database.conversation_summary(&conversation.id),
            Err(AppError::NotFound(_))
        ));
        cleanup(&database);
    }

    #[test]
    fn attachment_is_deduplicated_and_reused_across_conversations() {
        let database = test_database();
        assert_eq!(database.schema_version().expect("version should load"), 4);
        let first_conversation = database
            .create_conversation("Primera", None)
            .expect("conversation should be created");
        let second_conversation = database
            .create_conversation("Segunda", None)
            .expect("conversation should be created");
        let first = database
            .register_attachment(
                &first_conversation.id,
                "C:/managed/document.pdf",
                "document.pdf",
                Some("application/pdf"),
                42,
                "abc123",
            )
            .expect("attachment should be registered");
        let second = database
            .register_attachment(
                &second_conversation.id,
                "C:/managed/document.pdf",
                "document.pdf",
                Some("application/pdf"),
                42,
                "abc123",
            )
            .expect("attachment should be reused");
        assert_eq!(first.id, second.id);
        assert_eq!(
            database
                .list_attachments(&first_conversation.id)
                .expect("first attachments should list")
                .len(),
            1
        );
        assert_eq!(
            database
                .list_attachments(&second_conversation.id)
                .expect("second attachments should list")
                .len(),
            1
        );

        database
            .update_attachment_ingestion(
                &first.id,
                "ready",
                Some("broker-file-1"),
                Some("document"),
                Some("test"),
                Some(&serde_json::json!({})),
                None,
            )
            .expect("attachment should become ready");
        let ready = database
            .ready_attachments_for_turn(&second_conversation.id, std::slice::from_ref(&first.id))
            .expect("reused attachment should be ready");
        assert_eq!(ready[0].broker_file_id.as_deref(), Some("broker-file-1"));

        database
            .remove_conversation_attachment(&first_conversation.id, &first.id)
            .expect("first association should be removed");
        assert!(database
            .list_attachments(&first_conversation.id)
            .expect("first attachments should list")
            .is_empty());
        assert_eq!(
            database
                .list_attachments(&second_conversation.id)
                .expect("second association should remain")
                .len(),
            1
        );
        cleanup(&database);
    }

    #[test]
    fn existing_schema_one_database_upgrades_without_losing_conversations() {
        let path = std::env::temp_dir().join(format!(
            "chatygpt-db-upgrade-test-{}.sqlite",
            Uuid::new_v4().simple()
        ));
        let connection = rusqlite::Connection::open(&path).expect("legacy database should open");
        connection
            .execute_batch(INITIAL_MIGRATION)
            .expect("initial migration should apply");
        connection
            .pragma_update(None, "user_version", 1)
            .expect("legacy version should be set");
        connection
            .execute(
                "INSERT INTO conversations(id, title) VALUES ('legacy-conversation', 'Legado')",
                [],
            )
            .expect("legacy conversation should exist");
        drop(connection);

        let database = Database::open(&path).expect("database should upgrade");
        assert_eq!(database.schema_version().expect("version should load"), 4);
        assert_eq!(
            database
                .list_conversations()
                .expect("conversations should survive")
                .first()
                .map(|conversation| conversation.id.as_str()),
            Some("legacy-conversation")
        );
        cleanup(&database);
    }

    #[test]
    fn retrying_failed_attachment_discards_terminal_broker_file_id() {
        let database = test_database();
        let conversation = database
            .create_conversation("Adjunto fallido", None)
            .expect("conversation should be created");
        let attachment = database
            .register_attachment(
                &conversation.id,
                "C:/managed/failed.pdf",
                "failed.pdf",
                Some("application/pdf"),
                100,
                "failed-sha",
            )
            .expect("attachment should be registered");
        database
            .update_attachment_ingestion(
                &attachment.id,
                "failed",
                Some("file-terminal-failure"),
                Some("document"),
                Some("docling"),
                Some(&serde_json::json!({"pages": 0})),
                Some(&serde_json::json!({"code": "ENGINE_MISSING"})),
            )
            .expect("attachment should fail");

        database
            .reset_failed_attachment_for_retry(&attachment.id)
            .expect("failed attachment should reset");
        let reset = database
            .attachment_record(&attachment.id)
            .expect("attachment should load");
        assert_eq!(reset.ingestion_status, "local");
        assert!(reset.broker_file_id.is_none());
        assert!(database
            .attachment_view(&attachment.id)
            .expect("attachment view should load")
            .ingestion_error
            .is_none());
        cleanup(&database);
    }

    #[test]
    fn completed_turn_materializes_attachment_sources_on_assistant_message() {
        let database = test_database();
        let conversation = database
            .create_conversation("Pregunta con fuente", None)
            .expect("conversation should be created");
        let attachment = database
            .register_attachment(
                &conversation.id,
                "C:/managed/source.pdf",
                "source.pdf",
                Some("application/pdf"),
                2048,
                "source-sha",
            )
            .expect("attachment should be registered");
        database
            .update_attachment_ingestion(
                &attachment.id,
                "ready",
                Some("broker-source-1"),
                Some("document"),
                Some("docling"),
                Some(&serde_json::json!({"pages": 2})),
                None,
            )
            .expect("attachment should become ready");
        let user_message_id = "message-source-user";
        let assistant_message_id = "message-source-assistant";
        let context = vec![ContextMessage {
            message_id: user_message_id.to_owned(),
            role: "user".to_owned(),
            text: "Resume el documento".to_owned(),
        }];
        database
            .prepare_chat_turn(
                &conversation.id,
                user_message_id,
                assistant_message_id,
                "local-source-task",
                "source-idempotency-key",
                "Resume el documento",
                &serde_json::json!({}),
                &context,
                &[],
                std::slice::from_ref(&attachment.id),
            )
            .expect("turn should be prepared");
        let state: TaskState = serde_json::from_value(serde_json::json!({
            "task_id": "remote-source-task",
            "status": "completed",
            "request_id": "request-source",
            "created_at": "2026-07-21T00:00:00Z",
            "updated_at": "2026-07-21T00:00:01Z",
            "execution_strategy": "single",
            "execution_preset": "fast",
            "selection_mode": "automatic",
            "progress": {},
            "result": {
                "result_markdown": "Resumen documentado",
                "model_used": {
                    "provider": "lmstudio",
                    "deployment": "local",
                    "model": "modelo-prueba"
                }
            },
            "error": null
        }))
        .expect("task state should deserialize");
        database
            .record_remote_state("local-source-task", &state)
            .expect("completed state should materialize");

        let view = database
            .conversation_view(&conversation.id)
            .expect("conversation should load");
        let assistant = view
            .messages
            .iter()
            .find(|message| message.id == assistant_message_id)
            .expect("assistant message should exist");
        assert_eq!(assistant.sources.len(), 1);
        assert_eq!(assistant.sources[0].title, "source.pdf");
        assert_eq!(
            assistant
                .model_used
                .as_ref()
                .map(|model| model.model.as_str()),
            Some("modelo-prueba")
        );
        assert_eq!(
            assistant.sources[0].source_attachment_id.as_deref(),
            Some(attachment.id.as_str())
        );
        cleanup(&database);
    }

    #[test]
    fn waiting_tool_call_is_persisted_and_decisions_are_durable() {
        let database = test_database();
        let conversation = database
            .create_conversation("Herramienta pendiente", None)
            .expect("conversation should be created");
        let context = vec![ContextMessage {
            message_id: "tool-user-message".to_owned(),
            role: "user".to_owned(),
            text: "Renombra este chat".to_owned(),
        }];
        database
            .prepare_chat_turn(
                &conversation.id,
                "tool-user-message",
                "tool-assistant-message",
                "local-tool-task",
                "tool-idempotency-key",
                "Renombra este chat",
                &serde_json::json!({}),
                &context,
                &[],
                &[],
            )
            .expect("turn should be prepared");
        let waiting: TaskState = serde_json::from_value(serde_json::json!({
            "task_id": "remote-tool-task",
            "status": "waiting_for_tools",
            "request_id": "request-tool",
            "created_at": "2026-07-21T00:00:00Z",
            "updated_at": "2026-07-21T00:00:01Z",
            "execution_strategy": "agent",
            "execution_preset": "fast",
            "selection_mode": "automatic",
            "progress": {},
            "result": {
                "status": "waiting_for_tools",
                "pending_tool_calls": [{
                    "id": "call-rename-1",
                    "name": "rename_conversation",
                    "arguments": {"title": "Título propuesto"}
                }]
            },
            "error": null
        }))
        .expect("waiting state should deserialize");
        database
            .record_remote_state("local-tool-task", &waiting)
            .expect("waiting state should persist");
        let waiting_snapshot = database
            .task_snapshot("local-tool-task")
            .expect("snapshot should load");
        assert_eq!(waiting_snapshot.local_state, "waiting_for_tools");
        assert_eq!(waiting_snapshot.pending_tool_calls.len(), 1);
        assert_eq!(
            waiting_snapshot.pending_tool_calls[0].arguments["title"],
            "Título propuesto"
        );

        database
            .prepare_tool_outcomes(
                "local-tool-task",
                &[ToolOutcomeRecord {
                    tool_call_id: "call-rename-1".to_owned(),
                    status: "approved".to_owned(),
                    content: serde_json::json!({"ok": true}).to_string(),
                }],
            )
            .expect("decision should persist before HTTP");
        let prepared = database
            .prepared_tool_results("local-tool-task")
            .expect("prepared results should load");
        assert_eq!(prepared["tool_results"][0]["tool_call_id"], "call-rename-1");
        assert!(database
            .task_snapshot("local-tool-task")
            .expect("snapshot should load")
            .pending_tool_calls
            .is_empty());
        cleanup(&database);
    }

    #[test]
    fn audit_inspector_exposes_only_safe_presentation_fields() {
        let database = test_database();
        let conversation = database
            .create_conversation("Auditoría segura", None)
            .expect("conversation should be created");
        let secret_path = r"C:\Users\private\Documents\conversation.md";
        let internal_hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        database
            .record_export(
                &conversation.id,
                "conversation:audit:markdown:v1",
                secret_path,
                internal_hash,
                None,
                Some(internal_hash),
                "completed",
                None,
            )
            .expect("export audit should be recorded");

        let events = database
            .list_audit_events(50)
            .expect("safe audit view should load");
        let serialized = serde_json::to_string(&events).expect("audit view should serialize");
        assert!(!serialized.contains(secret_path));
        assert!(!serialized.contains(internal_hash));
        assert!(events
            .iter()
            .any(|event| event.summary == "Exportación completada"));
        cleanup(&database);
    }

    #[test]
    fn pending_conversation_is_identified_for_visible_startup_recovery() {
        let database = test_database();
        let conversation = database
            .create_conversation("Conversación recuperable", None)
            .expect("conversation should be created");
        let context = vec![ContextMessage {
            message_id: "recovery-user-message".to_owned(),
            role: "user".to_owned(),
            text: "Continúa tras reiniciar".to_owned(),
        }];
        database
            .prepare_chat_turn(
                &conversation.id,
                "recovery-user-message",
                "recovery-assistant-message",
                "recovery-local-task",
                "recovery-idempotency",
                "Continúa tras reiniciar",
                &serde_json::json!({}),
                &context,
                &[],
                &[],
            )
            .expect("pending turn should be persisted");

        let candidates = database
            .recovery_candidates()
            .expect("recovery candidates should load");
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].conversation_id.as_deref(),
            Some(conversation.id.as_str())
        );
        assert_eq!(candidates[0].label, "Respuesta pendiente");
        cleanup(&database);
    }

    #[test]
    fn memory_is_opt_in_scoped_and_user_controllable() {
        let database = test_database();
        let general = database
            .create_conversation("Chat general", None)
            .expect("general conversation should exist");
        let project = database
            .create_project("Proyecto memoria", None)
            .expect("project should exist");
        let scoped = database
            .create_conversation("Chat de proyecto", Some(&project.id))
            .expect("scoped conversation should exist");
        database
            .create_memory_item("Responder en español", "preference", "normal", None)
            .expect("global memory should be created");
        database
            .create_memory_item("El proyecto usa Rust", "fact", "normal", Some(&project.id))
            .expect("project memory should be created");

        assert!(database
            .active_memories_for_conversation(&general.id)
            .expect("disabled memory should load")
            .is_empty());
        database
            .set_memory_enabled(true)
            .expect("memory should enable");
        let general_memories = database
            .active_memories_for_conversation(&general.id)
            .expect("global memory should load");
        assert_eq!(general_memories.len(), 1);
        let scoped_memories = database
            .active_memories_for_conversation(&scoped.id)
            .expect("scoped memories should load");
        assert_eq!(scoped_memories.len(), 2);
        let context = vec![ContextMessage {
            message_id: "memory-context-user".to_owned(),
            role: "user".to_owned(),
            text: "Usa mi memoria".to_owned(),
        }];
        database
            .prepare_chat_turn(
                &scoped.id,
                "memory-context-user",
                "memory-context-assistant",
                "memory-context-task",
                "memory-context-key",
                "Usa mi memoria",
                &serde_json::json!({}),
                &context,
                &scoped_memories,
                &[],
            )
            .expect("memory context should be traced");
        let connection = database.connect().expect("connection should open");
        let (strategy, memory_sources): (String, i64) = connection
            .query_row(
                "SELECT cs.strategy_version,
                        (SELECT COUNT(*) FROM context_sources src
                         WHERE src.snapshot_id = cs.id AND src.source_type = 'memory')
                 FROM context_snapshots cs
                 WHERE cs.broker_task_id = 'memory-context-task'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("memory snapshot should load");
        assert_eq!(strategy, "window-memory-v1");
        assert_eq!(memory_sources, 2);
        drop(connection);

        database
            .set_memory_item_enabled(&general_memories[0].id, false)
            .expect("item should disable");
        assert!(database
            .active_memories_for_conversation(&general.id)
            .expect("disabled item should be omitted")
            .is_empty());
        database
            .delete_memory_item(&general_memories[0].id)
            .expect("item should delete");
        assert_eq!(
            database
                .memory_overview()
                .expect("overview should load")
                .items
                .len(),
            1
        );
        cleanup(&database);
    }

    #[test]
    fn completed_memory_embedding_is_stored_with_model_and_dimensions() {
        let database = test_database();
        let (memory_id, _) = database
            .create_memory_item("Memoria vectorial", "fact", "normal", None)
            .expect("memory should be created");
        let request = serde_json::json!({
            "inference_kind": "embedding",
            "content": {
                "prompt": "Memoria vectorial",
                "metadata": {
                    "source_type": "memory",
                    "source_id": memory_id,
                    "content_sha256": "memory-content-hash"
                }
            }
        });
        database
            .prepare_broker_task("embedding-local-task", "embedding-key", &request)
            .expect("embedding task should persist");
        database
            .mark_orphaned(
                "embedding-local-task",
                "Broker AI devolvió HTTP 422: contrato inválido",
            )
            .expect("failed submission should be recorded");
        let failed_item = database
            .memory_item(&memory_id)
            .expect("memory should load");
        assert_eq!(failed_item.embedding_status, "failed");
        assert!(failed_item
            .embedding_error
            .as_deref()
            .is_some_and(|error| error.contains("HTTP 422")));
        let completed: TaskState = serde_json::from_value(serde_json::json!({
            "task_id": "embedding-remote-task",
            "status": "completed",
            "request_id": "embedding-request",
            "created_at": "2026-07-22T00:00:00Z",
            "updated_at": "2026-07-22T00:00:01Z",
            "execution_strategy": "single",
            "execution_preset": "fast",
            "selection_mode": "automatic",
            "progress": {},
            "result": {
                "inference_kind": "embedding",
                "embedding": [0.1, 0.2, 0.3],
                "model_used": {
                    "provider": "ollama",
                    "deployment": "local",
                    "model": "nomic-embed-text"
                }
            },
            "error": null
        }))
        .expect("completed embedding state should deserialize");
        database
            .record_remote_state("embedding-local-task", &completed)
            .expect("embedding should materialize");

        let item = database
            .memory_item(&memory_id)
            .expect("memory should load");
        assert_eq!(item.embedding_status, "ready");
        assert_eq!(
            item.embedding_model.as_deref(),
            Some("ollama/local/nomic-embed-text")
        );
        let connection = database.connect().expect("connection should open");
        let (dimensions, bytes): (i64, i64) = connection
            .query_row(
                "SELECT dimensions, length(vector_blob) FROM embedding_records
                 WHERE source_type = 'memory' AND source_id = ?1",
                params![memory_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("embedding record should exist");
        assert_eq!(dimensions, 3);
        assert_eq!(bytes, 24);
        drop(connection);
        cleanup(&database);
    }

    #[test]
    fn semantic_memory_search_ranks_compatible_vectors_and_respects_scope() {
        fn completed_embedding(task_id: &str, model: &str, vector: &[f64]) -> TaskState {
            serde_json::from_value(serde_json::json!({
                "task_id": task_id,
                "status": "completed",
                "request_id": format!("request-{task_id}"),
                "created_at": "2026-07-22T00:00:00Z",
                "updated_at": "2026-07-22T00:00:01Z",
                "execution_strategy": "single",
                "execution_preset": "fast",
                "selection_mode": "automatic",
                "progress": {},
                "result": {
                    "inference_kind": "embedding",
                    "embedding": vector,
                    "model_used": {
                        "provider": "ollama",
                        "deployment": "local",
                        "model": model
                    }
                },
                "error": null
            }))
            .expect("embedding state should deserialize")
        }

        fn store_memory_embedding(
            database: &Database,
            memory_id: &str,
            task_id: &str,
            model: &str,
            vector: &[f64],
        ) {
            let request = serde_json::json!({
                "inference_kind": "embedding",
                "content": {"metadata": {
                    "source_type": "memory",
                    "source_id": memory_id,
                    "content_sha256": format!("hash-{memory_id}")
                }}
            });
            database
                .prepare_broker_task(task_id, &format!("key-{task_id}"), &request)
                .expect("memory embedding task should persist");
            database
                .record_remote_state(task_id, &completed_embedding(task_id, model, vector))
                .expect("memory embedding should materialize");
        }

        let database = test_database();
        let project = database
            .create_project("TFM", None)
            .expect("project should be created");
        let other_project = database
            .create_project("Otro", None)
            .expect("other project should be created");
        let (global_id, _) = database
            .create_memory_item("Prefiero respuestas breves", "preference", "normal", None)
            .expect("global memory should be created");
        let (scoped_id, _) = database
            .create_memory_item(
                "El TFM usa arquitectura durable",
                "fact",
                "normal",
                Some(&project.id),
            )
            .expect("scoped memory should be created");
        let (other_id, _) = database
            .create_memory_item(
                "Recuerdo de otro proyecto",
                "fact",
                "normal",
                Some(&other_project.id),
            )
            .expect("other memory should be created");
        let (different_model_id, _) = database
            .create_memory_item("Modelo incompatible", "fact", "normal", None)
            .expect("incompatible memory should be created");
        store_memory_embedding(&database, &global_id, "task-global", "nomic", &[1.0, 0.0]);
        store_memory_embedding(&database, &scoped_id, "task-scoped", "nomic", &[0.8, 0.2]);
        store_memory_embedding(&database, &other_id, "task-other", "nomic", &[1.0, 0.0]);
        store_memory_embedding(
            &database,
            &different_model_id,
            "task-different-model",
            "other-model",
            &[1.0, 0.0],
        );

        let search_id = "memory-search-test";
        let search_task_id = "memory-search-task";
        let request = serde_json::json!({
            "inference_kind": "embedding",
            "content": {"metadata": {
                "source_type": "memory_search",
                "source_id": search_id,
                "content_sha256": "search-hash"
            }}
        });
        database
            .prepare_memory_search(
                search_id,
                "respuestas concisas",
                Some(&project.id),
                search_task_id,
                "memory-search-key",
                &request,
            )
            .expect("search should persist atomically");
        database
            .record_remote_state(
                search_task_id,
                &completed_embedding(search_task_id, "nomic", &[1.0, 0.0]),
            )
            .expect("search embedding should materialize");

        let search = database
            .memory_search(search_id)
            .expect("search should load");
        assert_eq!(search.status, "completed");
        assert_eq!(search.results.len(), 2);
        assert_eq!(search.results[0].memory_id, global_id);
        assert_eq!(search.results[1].memory_id, scoped_id);
        assert!(search.results[0].score > search.results[1].score);
        assert!(search
            .results
            .iter()
            .all(|result| result.memory_id != other_id && result.memory_id != different_model_id));
        cleanup(&database);
    }
}
