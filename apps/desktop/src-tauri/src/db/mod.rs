use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::broker::{TaskAccepted, TaskState};
use crate::error::AppError;

const INITIAL_MIGRATION: &str = include_str!("../../migrations/0001_initial.sql");
const RECOVER_NON_TERMINAL_TASKS: &str =
    include_str!("../../queries/recover_non_terminal_tasks.sql");
pub const SCHEMA_VERSION: i64 = 1;

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
    pub updated_at: String,
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
    pub created_at: String,
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
        if current < SCHEMA_VERSION {
            let transaction = connection.transaction()?;
            transaction.execute_batch(INITIAL_MIGRATION)?;
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
    ) -> Result<BrokerTaskRecord, AppError> {
        let request_json = serde_json::to_string(request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let context_json = serde_json::to_string(context)
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
        transaction.execute(
            "INSERT INTO context_snapshots(
                id, broker_task_id, strategy_version, token_budget,
                estimated_tokens, final_context_json
             ) VALUES (?1, ?2, 'window-v1', NULL, ?3, ?4)",
            params![
                snapshot_id,
                local_task_id,
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
                    mp.content_text, mp.content_json, m.created_at
             FROM messages m
             LEFT JOIN message_parts mp ON mp.message_id = m.id AND mp.ordinal = 0
             LEFT JOIN broker_tasks bt ON bt.id = m.broker_task_id
             WHERE m.conversation_id = ?1
             ORDER BY m.sequence_no",
        )?;
        let messages = statement
            .query_map(params![id], |row| {
                let error_json: Option<String> = row.get(8)?;
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
                    created_at: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
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
        let (previous, response_message_id, conversation_id): (
            String,
            Option<String>,
            Option<String>,
        ) = connection.query_row(
            "SELECT remote_status, response_message_id, conversation_id
             FROM broker_tasks WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
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

    pub fn task_snapshot(&self, id: &str) -> Result<LocalTaskSnapshot, AppError> {
        let connection = self.connect()?;
        connection
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
                        updated_at: row.get(7)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::BrokerContract(format!("tarea local no encontrada: {id}")))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::Database;
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
}
