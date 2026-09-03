//! Apertura de la base, migraciones y recuperacion de arranque.
//!
//! La version de esquema es un numero y una sola escalera de migraciones:
//! abrir una base mas nueva que el binario falla en vez de adivinar.

use super::*;

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

    pub(super) fn migrate(connection: &mut Connection) -> Result<(), AppError> {
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
        if current < 4 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(MEMORY_SEARCHES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 4)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 5 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(SEMANTIC_CHAT_MEMORY_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 5)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 6 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(CONVERSATION_SUMMARIES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 6)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 7 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(ATTACHMENT_CHUNKS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 7)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 8 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(ATTACHMENT_CONTEXT_STATUS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 8)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 9 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(CONVERSATION_EXECUTION_PREFERENCES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 9)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 10 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(PROJECT_INSTRUCTIONS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 10)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 11 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(CUSTOM_GPTS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 11)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 12 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(CONVERSATION_CUSTOM_GPTS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 12)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 13 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(CUSTOM_GPT_FILES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 13)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 14 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(RESEARCH_RUNS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 14)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 15 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(SCHEDULED_TASK_TEMPLATES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 15)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 16 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(CONFIRMATION_REQUESTS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 16)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 17 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(PERFORMANCE_SAMPLES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 17)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 18 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(SEMANTIC_RESEARCH_WORKFLOW_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 18)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 19 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(ATTACHMENT_IMAGE_POLICY_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 19)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 21 {
            let transaction = connection.transaction()?;
            if current < 20 {
                transaction.execute_batch(WORKFLOWS_MIGRATION)?;
            }
            transaction.execute_batch(SCHEDULED_WORKFLOWS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 21)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 22 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(REMOTE_OPERATION_START_METRIC_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 22)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < SCHEMA_VERSION {
            let transaction = connection.transaction()?;
            transaction.execute_batch(ATHENA_RUNS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        Ok(())
    }

    pub(super) fn connect(&self) -> Result<Connection, AppError> {
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
                    label: if embedding_source.as_deref() == Some("conversation_summary") {
                        "Resumen de conversación pendiente".to_owned()
                    } else if embedding_source.as_deref() == Some("chat_memory_search") {
                        "Selección semántica de contexto pendiente".to_owned()
                    } else if embedding_source.as_deref() == Some("chat_document_search") {
                        "Búsqueda semántica documental pendiente".to_owned()
                    } else if embedding_source.as_deref() == Some("attachment_chunk") {
                        "Índice documental pendiente".to_owned()
                    } else if embedding_source.as_deref() == Some("memory_search") {
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

    pub fn path(&self) -> &Path {
        &self.path
    }
}
