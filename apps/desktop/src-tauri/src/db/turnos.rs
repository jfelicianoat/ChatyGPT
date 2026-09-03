//! Preparacion de un turno de chat y la vista completa de la conversacion.

use super::*;

impl Database {
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
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
        document_chunks: &[SelectedAttachmentChunk],
        attachment_ids: &[String],
    ) -> Result<BrokerTaskRecord, AppError> {
        self.prepare_chat_turn_with_project_instruction(
            conversation_id,
            user_message_id,
            assistant_message_id,
            local_task_id,
            idempotency_key,
            user_text,
            request,
            context,
            None,
            None,
            memories,
            document_chunks,
            attachment_ids,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_chat_turn_with_project_instruction(
        &self,
        conversation_id: &str,
        user_message_id: &str,
        assistant_message_id: &str,
        local_task_id: &str,
        idempotency_key: &str,
        user_text: &str,
        request: &Value,
        context: &[ContextMessage],
        project_instruction: Option<&ProjectInstructionContext>,
        custom_gpt_context: Option<&CustomGptContext>,
        memories: &[MemoryItemView],
        document_chunks: &[SelectedAttachmentChunk],
        attachment_ids: &[String],
    ) -> Result<BrokerTaskRecord, AppError> {
        let request_json = serde_json::to_string(request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let context_json = serde_json::to_string(&serde_json::json!({
            "messages": context,
            "projectInstruction": project_instruction,
            "customGpt": custom_gpt_context,
            "memories": memories,
            "documentChunks": document_chunks
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
                    FROM attachments a
                    WHERE a.id = ?2
                      AND a.ingestion_status = 'ready'
                      AND a.broker_file_id IS NOT NULL
                      AND (
                        EXISTS(
                          SELECT 1 FROM conversation_attachments ca
                          WHERE ca.conversation_id = ?1 AND ca.attachment_id = a.id
                        )
                        OR EXISTS(
                          SELECT 1
                          FROM conversations conversation
                          JOIN custom_gpt_files file
                            ON file.custom_gpt_id = conversation.custom_gpt_id
                          WHERE conversation.id = ?1
                            AND conversation.archived_at IS NULL
                            AND conversation.deleted_at IS NULL
                            AND file.attachment_id = a.id
                        )
                      )
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
                id, conversation_id, role, status, sequence_no, created_at, updated_at
             ) VALUES (
                ?1, ?2, 'assistant', 'pending', ?3,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![assistant_message_id, conversation_id, next_sequence + 1],
        )?;
        transaction.execute(
            "INSERT INTO broker_tasks(
                id, conversation_id, request_message_id, response_message_id,
                idempotency_key, request_json, remote_status, local_state,
                gpt_version_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'not_submitted', 'created', ?7)",
            params![
                local_task_id,
                conversation_id,
                user_message_id,
                assistant_message_id,
                idempotency_key,
                request_json,
                custom_gpt_context.map(|context| context.version_id.as_str())
            ],
        )?;
        transaction.execute(
            "UPDATE messages SET broker_task_id = ?2 WHERE id = ?1",
            params![assistant_message_id, local_task_id],
        )?;
        insert_research_run_if_needed(
            &transaction,
            request,
            conversation_id,
            local_task_id,
            user_text,
        )?;
        let snapshot_id = format!("ctx_{}", Uuid::new_v4().simple());
        let has_summary = context.iter().any(|source| source.role == "summary");
        let strategy_version = match (
            has_summary,
            project_instruction.is_some(),
            memories.is_empty(),
            document_chunks.is_empty(),
        ) {
            (true, false, true, true) => "window-summary-v1",
            (true, false, false, true) => "window-summary-memory-v1",
            (false, false, true, true) => "window-v1",
            (false, false, false, true) => "window-memory-v1",
            (true, false, true, false) => "window-summary-document-v1",
            (true, false, false, false) => "window-summary-memory-document-v1",
            (false, false, true, false) => "window-document-v1",
            (false, false, false, false) => "window-memory-document-v1",
            (true, true, true, true) => "window-summary-project-v1",
            (true, true, false, true) => "window-summary-project-memory-v1",
            (false, true, true, true) => "window-project-v1",
            (false, true, false, true) => "window-project-memory-v1",
            (true, true, true, false) => "window-summary-project-document-v1",
            (true, true, false, false) => "window-summary-project-memory-document-v1",
            (false, true, true, false) => "window-project-document-v1",
            (false, true, false, false) => "window-project-memory-document-v1",
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
                    } else if source.message_id == user_message_id {
                        "current_user_turn"
                    } else {
                        "recent_conversation_window"
                    },
                    (source.text.chars().count() as i64 + 3) / 4,
                    source.text
                ],
            )?;
        }
        if let Some(project_instruction) = project_instruction {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'project_instruction', ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    project_instruction.project_id,
                    context.len() as i64,
                    "Instrucciones configuradas para el proyecto",
                    (project_instruction.instructions.chars().count() as i64 + 3) / 4,
                    project_instruction.instructions
                ],
            )?;
        }
        if let Some(custom_gpt) = custom_gpt_context {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'custom_gpt', ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    custom_gpt.version_id,
                    (context.len() + usize::from(project_instruction.is_some())) as i64,
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
                    (context.len()
                        + usize::from(project_instruction.is_some())
                        + usize::from(custom_gpt_context.is_some())
                        + index) as i64,
                    if memory.custom_gpt_id.is_some() {
                        "Conocimiento privado del GPT personal seleccionado"
                    } else {
                        "Recuerdo activado explícitamente por el usuario"
                    },
                    (memory.content.chars().count() as i64 + 3) / 4,
                    memory.content
                ],
            )?;
        }
        for (index, chunk) in document_chunks.iter().enumerate() {
            let from_custom_gpt = if let Some(custom_gpt) = custom_gpt_context {
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
                    (context.len()
                        + usize::from(project_instruction.is_some())
                        + usize::from(custom_gpt_context.is_some())
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
        let (execution_preferences_json, custom_gpt_id): (String, Option<String>) = connection
            .query_row(
                "SELECT execution_preferences_json, custom_gpt_id
             FROM conversations WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
        let execution_preferences = serde_json::from_str(&execution_preferences_json)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let mut statement = connection.prepare(
            "SELECT m.id, m.role, m.status, m.sequence_no,
                    m.broker_task_id, bt.remote_status, bt.local_state,
                    mp.content_text, mp.content_json, m.created_at,
                    json_extract(bt.result_json, '$.model_used.provider'),
                    json_extract(bt.result_json, '$.model_used.deployment'),
                    json_extract(bt.result_json, '$.model_used.model'),
                    CASE
                        WHEN bt.terminal_at IS NULL THEN NULL
                        ELSE CAST(ROUND(
                            MAX(
                                0,
                                (julianday(bt.terminal_at) - julianday(m.created_at))
                                    * 86400000.0
                            )
                        ) AS INTEGER)
                    END,
                    json_extract(bt.result_json, '$.usage'),
                    json_extract(bt.result_json, '$.fallback_used'),
                    json_extract(bt.result_json, '$.long_context'),
                    json_extract(bt.result_json, '$.consensus.synthesized'),
                    json_extract(bt.result_json, '$.consensus.warnings'),
                    json_extract(bt.result_json, '$.arbiter_failures'),
                    json_extract(bt.result_json, '$.warnings'),
                    json_extract(bt.result_json, '$.agent.citations.unsupported')
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
                let usage_json: Option<String> = row.get(14)?;
                let fallback_used: Option<i64> = row.get(15)?;
                let long_context_json: Option<String> = row.get(16)?;
                let consensus_synthesized: Option<i64> = row.get(17)?;
                let consensus_warnings_json: Option<String> = row.get(18)?;
                let arbiter_failures_json: Option<String> = row.get(19)?;
                let execution_warnings_json: Option<String> = row.get(20)?;
                let unsupported_citations_json: Option<String> = row.get(21)?;
                let consensus_warnings = consensus_warnings_json
                    .and_then(|value| serde_json::from_str::<Value>(&value).ok())
                    .and_then(|value| value.as_array().cloned())
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|warning| warning.as_str().map(str::to_owned))
                    .collect::<Vec<_>>();
                let arbiter_failure_count = arbiter_failures_json
                    .and_then(|value| serde_json::from_str::<Value>(&value).ok())
                    .and_then(|value| value.as_array().map(|failures| failures.len() as i64))
                    .unwrap_or(0);
                let string_list = |serialized: Option<String>| {
                    serialized
                        .and_then(|value| serde_json::from_str::<Value>(&value).ok())
                        .and_then(|value| value.as_array().cloned())
                        .unwrap_or_default()
                        .into_iter()
                        .filter_map(|value| value.as_str().map(str::to_owned))
                        .collect::<Vec<_>>()
                };
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
                    response_duration_ms: row.get(13)?,
                    usage: usage_json.and_then(|value| serde_json::from_str(&value).ok()),
                    fallback_used: fallback_used.map(|value| value != 0),
                    long_context: long_context_json
                        .and_then(|value| serde_json::from_str(&value).ok()),
                    consensus_synthesized: consensus_synthesized.map(|value| value != 0),
                    consensus_warnings,
                    arbiter_failure_count,
                    execution_warnings: string_list(execution_warnings_json),
                    unsupported_citation_urls: string_list(unsupported_citations_json),
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
        let mut research_statement = connection.prepare(
            "SELECT run.id, run.broker_task_id, run.objective, run.status,
                    COUNT(citation.id), run.created_at, run.updated_at
             FROM research_runs run
             JOIN broker_tasks task ON task.id = run.broker_task_id
             LEFT JOIN citations citation ON citation.message_id = task.response_message_id
             WHERE run.conversation_id = ?1
             GROUP BY run.id
             ORDER BY run.created_at DESC, run.id DESC",
        )?;
        let research_rows = research_statement
            .query_map(params![id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut research_runs = Vec::with_capacity(research_rows.len());
        for (run_id, broker_task_id, objective, status, source_count, created_at, updated_at) in
            research_rows
        {
            let mut step_statement = connection.prepare(
                "SELECT id, COALESCE(kind, 'research'),
                        COALESCE(title, objective), status
                 FROM research_steps
                 WHERE research_run_id = ?1
                 ORDER BY ordinal",
            )?;
            let steps = step_statement
                .query_map(params![run_id], |row| {
                    Ok(ResearchStepView {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        title: row.get(2)?,
                        status: row.get(3)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            research_runs.push(ResearchRunView {
                id: run_id,
                broker_task_id,
                objective,
                status,
                steps,
                source_count,
                created_at,
                updated_at,
            });
        }
        Ok(ConversationView {
            id: summary.id,
            title: summary.title,
            project_id: summary.project_id,
            custom_gpt_id,
            execution_preferences,
            messages,
            research_runs,
        })
    }

    pub fn conversation_export_metadata(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationExportMetadata, AppError> {
        self.connect()?
            .query_row(
                "SELECT c.created_at, c.updated_at, p.id, p.name
                 FROM conversations c
                 LEFT JOIN projects p ON p.id = c.project_id
                 WHERE c.id = ?1 AND c.deleted_at IS NULL",
                params![conversation_id],
                |row| {
                    let project_id: Option<String> = row.get(2)?;
                    let project_name: Option<String> = row.get(3)?;
                    Ok(ConversationExportMetadata {
                        created_at: row.get(0)?,
                        updated_at: row.get(1)?,
                        project: project_id
                            .zip(project_name)
                            .map(|(id, name)| ProjectExportMetadata { id, name }),
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("conversación {conversation_id}")))
    }

    pub fn conversation_execution_preferences(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationExecutionPreferences, AppError> {
        let connection = self.connect()?;
        let value: String = connection
            .query_row(
                "SELECT execution_preferences_json
                 FROM conversations
                 WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL",
                params![conversation_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("conversación activa {conversation_id}")))?;
        serde_json::from_str(&value).map_err(|error| AppError::BrokerContract(error.to_string()))
    }

    pub fn update_conversation_execution_preferences(
        &self,
        conversation_id: &str,
        preferences: &ConversationExecutionPreferences,
    ) -> Result<ConversationExecutionPreferences, AppError> {
        validate_execution_preferences(preferences)?;
        let value = serde_json::to_string(preferences)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE conversations
             SET execution_preferences_json = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL",
            params![conversation_id, value],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!(
                "conversación activa {conversation_id}"
            )));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES (
                'conversation.execution_preferences_updated',
                'user',
                ?1,
                json_object(
                    'data_classification', ?2,
                    'strategy', ?3,
                    'preset', ?4,
                    'max_cost_usd', ?5,
                    'long_context', ?6,
                    'priority', ?7
                )
             )",
            params![
                conversation_id,
                preferences.data_classification,
                preferences.strategy,
                preferences.preset,
                preferences.max_cost_usd,
                preferences.long_context,
                preferences.priority
            ],
        )?;
        transaction.commit()?;
        Ok(preferences.clone())
    }
}
