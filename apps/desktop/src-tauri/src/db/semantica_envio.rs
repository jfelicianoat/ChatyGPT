//! Cierre de la busqueda: preparacion del envio y del flujo sin resultados.

use super::*;

impl Database {
    pub fn prepare_semantic_chat_submission(
        &self,
        workflow_id: &str,
        local_task_id: &str,
        idempotency_key: &str,
        request: &Value,
        memories: &[SemanticMemoryMatch],
        document_chunks: &[SelectedAttachmentChunk],
    ) -> Result<BrokerTaskRecord, AppError> {
        let workflow = self
            .semantic_chat_workflow_for_id(workflow_id)?
            .ok_or_else(|| AppError::NotFound(format!("flujo semántico {workflow_id}")))?;
        if workflow.status != "searching" {
            if let Some(chat_task_id) = workflow.chat_task_id {
                return self.task_record(&chat_task_id);
            }
            return Err(AppError::Conflict(
                "el flujo semántico ya no admite preparar otra tarea".to_owned(),
            ));
        }
        let request_json = serde_json::to_string(request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let memory_items = memories
            .iter()
            .map(|item| item.memory.clone())
            .collect::<Vec<_>>();
        let final_context_json = serde_json::to_string(&serde_json::json!({
            "messages": workflow.context,
            "projectInstruction": workflow.project_instruction,
            "customGpt": workflow.custom_gpt_context,
            "memories": memory_items,
            "documentChunks": document_chunks
        }))
        .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let claimed = transaction.execute(
            "UPDATE semantic_chat_workflows
             SET status = 'preparing_chat', updated_at = datetime('now')
             WHERE id = ?1 AND status = 'searching'",
            params![workflow_id],
        )?;
        if claimed == 0 {
            let existing_task_id: Option<String> = transaction
                .query_row(
                    "SELECT chat_task_id FROM semantic_chat_workflows WHERE id = ?1",
                    params![workflow_id],
                    |row| row.get(0),
                )
                .optional()?
                .flatten();
            if let Some(existing_task_id) = existing_task_id {
                drop(transaction);
                return self.task_record(&existing_task_id);
            }
            return Err(AppError::Conflict(
                "el flujo semántico está siendo preparado por otra operación".to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO broker_tasks(
                id, conversation_id, request_message_id, response_message_id,
                idempotency_key, request_json, remote_status, local_state,
                gpt_version_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'not_submitted', 'created', ?7)",
            params![
                local_task_id,
                workflow.conversation_id,
                workflow.user_message_id,
                workflow.assistant_message_id,
                idempotency_key,
                request_json,
                workflow
                    .custom_gpt_context
                    .as_ref()
                    .map(|context| context.version_id.as_str())
            ],
        )?;
        transaction.execute(
            "UPDATE messages SET broker_task_id = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![workflow.assistant_message_id, local_task_id],
        )?;
        insert_research_run_if_needed(
            &transaction,
            request,
            &workflow.conversation_id,
            local_task_id,
            &workflow.user_text,
        )?;
        let snapshot_id = format!("ctx_{}", Uuid::new_v4().simple());
        transaction.execute(
            "INSERT INTO context_snapshots(
                id, broker_task_id, strategy_version, token_budget,
                estimated_tokens, final_context_json
             ) VALUES (?1, ?2, ?3, NULL, ?4, ?5)",
            params![
                snapshot_id,
                local_task_id,
                match (
                    workflow
                        .context
                        .iter()
                        .any(|source| source.role == "summary"),
                    workflow.project_instruction.is_some(),
                    document_chunks.is_empty(),
                ) {
                    (true, false, true) => "window-summary-semantic-memory-v1",
                    (true, false, false) => "window-summary-semantic-memory-document-v1",
                    (false, false, true) => "window-semantic-memory-v1",
                    (false, false, false) => "window-semantic-memory-document-v1",
                    (true, true, true) => "window-summary-project-semantic-memory-v1",
                    (true, true, false) => {
                        "window-summary-project-semantic-memory-document-v1"
                    }
                    (false, true, true) => "window-project-semantic-memory-v1",
                    (false, true, false) => "window-project-semantic-memory-document-v1",
                },
                (final_context_json.chars().count() as i64 + 3) / 4,
                final_context_json
            ],
        )?;
        for (ordinal, source) in workflow.context.iter().enumerate() {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    if source.role == "summary" {
                        "summary"
                    } else {
                        "message"
                    },
                    source.message_id,
                    ordinal as i64,
                    if source.role == "summary" {
                        "approved_conversation_summary"
                    } else if source.message_id == workflow.user_message_id {
                        "current_user_turn"
                    } else {
                        "recent_conversation_window"
                    },
                    (source.text.chars().count() as i64 + 3) / 4,
                    source.text
                ],
            )?;
        }
        if let Some(project_instruction) = workflow.project_instruction.as_ref() {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'project_instruction', ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    project_instruction.project_id,
                    workflow.context.len() as i64,
                    "Instrucciones configuradas para el proyecto",
                    (project_instruction.instructions.chars().count() as i64 + 3) / 4,
                    project_instruction.instructions
                ],
            )?;
        }
        if let Some(custom_gpt) = workflow.custom_gpt_context.as_ref() {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'custom_gpt', ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    custom_gpt.version_id,
                    (workflow.context.len() + usize::from(workflow.project_instruction.is_some()))
                        as i64,
                    "Versión del GPT personal seleccionada al enviar",
                    (custom_gpt.instructions.chars().count() as i64 + 3) / 4,
                    format!(
                        "{} · versión {}\n{}\nPermisos: código aislado = {}; renombrar chat = {}",
                        custom_gpt.name,
                        custom_gpt.version_no,
                        custom_gpt.instructions,
                        custom_gpt.tool_permissions.run_code,
                        custom_gpt.tool_permissions.rename_conversation
                    )
                ],
            )?;
        }
        for (index, selected) in memories.iter().enumerate() {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, score, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'memory', ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    selected.memory.id,
                    (workflow.context.len()
                        + usize::from(workflow.project_instruction.is_some())
                        + usize::from(workflow.custom_gpt_context.is_some())
                        + index) as i64,
                    selected.reason,
                    selected.score,
                    (selected.memory.content.chars().count() as i64 + 3) / 4,
                    selected.memory.content
                ],
            )?;
        }
        for (index, chunk) in document_chunks.iter().enumerate() {
            let from_custom_gpt = if let Some(custom_gpt) = workflow.custom_gpt_context.as_ref() {
                transaction.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM custom_gpt_files
                        WHERE custom_gpt_id = ?1 AND attachment_id = ?2
                     )",
                    params![custom_gpt.custom_gpt_id, chunk.attachment_id],
                    |row| row.get::<_, bool>(0),
                )?
            } else {
                false
            };
            let reason = if from_custom_gpt {
                format!(
                    "Archivo de conocimiento del GPT personal seleccionado · {}",
                    chunk.reason
                )
            } else {
                chunk.reason.clone()
            };
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, score, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'attachment_chunk', ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    chunk.id,
                    (workflow.context.len()
                        + usize::from(workflow.project_instruction.is_some())
                        + usize::from(workflow.custom_gpt_context.is_some())
                        + memories.len()
                        + index) as i64,
                    reason,
                    chunk.score,
                    (chunk.text.chars().count() as i64 + 3) / 4,
                    chunk.text
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
            "UPDATE semantic_chat_workflows
             SET chat_task_id = ?2, status = 'submitted', updated_at = datetime('now')
             WHERE id = ?1 AND status = 'preparing_chat'",
            params![workflow_id, local_task_id],
        )?;
        transaction.commit()?;
        self.task_record(local_task_id)
    }

    pub(super) fn semantic_chat_workflow_for_id(
        &self,
        workflow_id: &str,
    ) -> Result<Option<SemanticChatWorkflow>, AppError> {
        let task_id = self
            .connect()?
            .query_row(
                "SELECT embedding_task_id FROM semantic_chat_workflows WHERE id = ?1",
                params![workflow_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        task_id
            .map(|task_id| self.semantic_chat_workflow_for_task(&task_id))
            .transpose()
            .map(Option::flatten)
    }

    pub fn semantic_chat_workflows_ready_to_continue(&self) -> Result<Vec<String>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT workflow.embedding_task_id
             FROM semantic_chat_workflows workflow
             JOIN broker_tasks task ON task.id = workflow.embedding_task_id
             WHERE workflow.status = 'searching'
               AND (task.remote_status IN ('completed', 'failed', 'cancelled')
                    OR task.local_state = 'orphaned')
             ORDER BY workflow.created_at",
        )?;
        let task_ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(task_ids)
    }

    pub fn finish_semantic_chat_without_submission(
        &self,
        embedding_task_id: &str,
        cancelled: bool,
        message: &str,
    ) -> Result<(), AppError> {
        let Some(workflow) = self.semantic_chat_workflow_for_task(embedding_task_id)? else {
            return Ok(());
        };
        if workflow.status != "searching" {
            return Ok(());
        }
        let error = serde_json::json!({
            "code": if cancelled { "CANCELLED" } else { "SEMANTIC_MEMORY_FAILED" },
            "message": message
        });
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE semantic_chat_workflows
             SET status = ?2, error_json = ?3, updated_at = datetime('now')
             WHERE id = ?1 AND status = 'searching'",
            params![
                workflow.id,
                if cancelled { "cancelled" } else { "failed" },
                error.to_string()
            ],
        )?;
        transaction.execute(
            "UPDATE messages SET status = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![
                workflow.assistant_message_id,
                if cancelled { "cancelled" } else { "failed" }
            ],
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
                workflow.assistant_message_id,
                error.to_string()
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }
}
