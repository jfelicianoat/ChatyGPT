//! Turnos con busqueda semantica y su plan de investigacion congelado.
//!
//! El plan se congela al enviar para que la segunda etapa y una eventual
//! recuperacion apliquen exactamente las mismas herramientas.

use super::*;

impl Database {
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub fn prepare_semantic_chat_turn(
        &self,
        workflow_id: &str,
        conversation_id: &str,
        user_message_id: &str,
        assistant_message_id: &str,
        embedding_task_id: &str,
        idempotency_key: &str,
        user_text: &str,
        embedding_request: &Value,
        context: &[ContextMessage],
        attachment_ids: &[String],
        tools_enabled: bool,
        sandbox_enabled: bool,
        execution_preferences: &ConversationExecutionPreferences,
        research_plan: Option<&Value>,
    ) -> Result<BrokerTaskRecord, AppError> {
        self.prepare_semantic_chat_turn_with_project_instruction(
            workflow_id,
            conversation_id,
            user_message_id,
            assistant_message_id,
            embedding_task_id,
            idempotency_key,
            user_text,
            embedding_request,
            context,
            None,
            None,
            attachment_ids,
            tools_enabled,
            sandbox_enabled,
            execution_preferences,
            research_plan,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_semantic_chat_turn_with_project_instruction(
        &self,
        workflow_id: &str,
        conversation_id: &str,
        user_message_id: &str,
        assistant_message_id: &str,
        embedding_task_id: &str,
        idempotency_key: &str,
        user_text: &str,
        embedding_request: &Value,
        context: &[ContextMessage],
        project_instruction: Option<&ProjectInstructionContext>,
        custom_gpt_context: Option<&CustomGptContext>,
        attachment_ids: &[String],
        tools_enabled: bool,
        sandbox_enabled: bool,
        execution_preferences: &ConversationExecutionPreferences,
        research_plan: Option<&Value>,
    ) -> Result<BrokerTaskRecord, AppError> {
        let research_plan_json = research_plan
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let request_json = serde_json::to_string(embedding_request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let context_json = serde_json::to_string(context)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let project_instruction_json =
            project_instruction
                .map(serde_json::to_string)
                .transpose()
                .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let custom_gpt_context_json = custom_gpt_context
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let attachment_ids_json = serde_json::to_string(attachment_ids)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        validate_execution_preferences(execution_preferences)?;
        let execution_preferences_json = serde_json::to_string(execution_preferences)
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
            "INSERT INTO messages(id, conversation_id, role, status, sequence_no)
             VALUES (?1, ?2, 'user', 'complete', ?3)",
            params![user_message_id, conversation_id, next_sequence],
        )?;
        transaction.execute(
            "INSERT INTO message_parts(id, message_id, ordinal, kind, content_text)
             VALUES (?1, ?2, 0, 'text', ?3)",
            params![
                format!("part_{}", Uuid::new_v4().simple()),
                user_message_id,
                user_text
            ],
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
                    "uno de los adjuntos ya no está listo para enviar".to_owned(),
                ));
            }
            transaction.execute(
                "INSERT INTO message_attachments(message_id, attachment_id, ordinal)
                 VALUES (?1, ?2, ?3)",
                params![user_message_id, attachment_id, ordinal as i64],
            )?;
        }
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
                id, conversation_id, request_message_id, idempotency_key,
                request_json, remote_status, local_state
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'not_submitted', 'created')",
            params![
                embedding_task_id,
                conversation_id,
                user_message_id,
                idempotency_key,
                request_json
            ],
        )?;
        transaction.execute(
            "UPDATE messages SET broker_task_id = ?2 WHERE id = ?1",
            params![assistant_message_id, embedding_task_id],
        )?;
        transaction.execute(
            "INSERT INTO semantic_chat_workflows(
                id, conversation_id, user_message_id, assistant_message_id,
                embedding_task_id, user_text, context_json, attachment_ids_json,
                tools_enabled, sandbox_enabled, execution_preferences_json,
                project_instruction_json, custom_gpt_context_json,
                research_plan_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                workflow_id,
                conversation_id,
                user_message_id,
                assistant_message_id,
                embedding_task_id,
                user_text,
                context_json,
                attachment_ids_json,
                i64::from(tools_enabled),
                i64::from(sandbox_enabled),
                execution_preferences_json,
                project_instruction_json,
                custom_gpt_context_json,
                research_plan_json
            ],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'local.prepared', 'not_submitted', '{}', datetime('now'))",
            params![embedding_task_id],
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
        self.task_record(embedding_task_id)
    }

    pub fn semantic_chat_workflow_for_task(
        &self,
        task_id: &str,
    ) -> Result<Option<SemanticChatWorkflow>, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT id, conversation_id, user_message_id, assistant_message_id,
                        embedding_task_id, chat_task_id, user_text, context_json,
                        attachment_ids_json, tools_enabled, sandbox_enabled,
                        execution_preferences_json, status, project_instruction_json,
                        custom_gpt_context_json, research_plan_json
                 FROM semantic_chat_workflows
                 WHERE embedding_task_id = ?1 OR chat_task_id = ?1",
                params![task_id],
                |row| {
                    let context_json: String = row.get(7)?;
                    let attachment_ids_json: String = row.get(8)?;
                    let context = serde_json::from_str(&context_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            context_json.len(),
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    let attachment_ids =
                        serde_json::from_str(&attachment_ids_json).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                attachment_ids_json.len(),
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    let execution_preferences_json: String = row.get(11)?;
                    let execution_preferences = serde_json::from_str(&execution_preferences_json)
                        .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            execution_preferences_json.len(),
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    let project_instruction_json: Option<String> = row.get(13)?;
                    let project_instruction = project_instruction_json
                        .map(|value| {
                            serde_json::from_str(&value).map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    value.len(),
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })
                        })
                        .transpose()?;
                    let custom_gpt_context_json: Option<String> = row.get(14)?;
                    let custom_gpt_context = custom_gpt_context_json
                        .map(|value| {
                            serde_json::from_str(&value).map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    value.len(),
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })
                        })
                        .transpose()?;
                    let research_plan_json: Option<String> = row.get(15)?;
                    let research_plan = research_plan_json
                        .map(|value| {
                            serde_json::from_str(&value).map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    value.len(),
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })
                        })
                        .transpose()?;
                    Ok(SemanticChatWorkflow {
                        id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        user_message_id: row.get(2)?,
                        assistant_message_id: row.get(3)?,
                        embedding_task_id: row.get(4)?,
                        chat_task_id: row.get(5)?,
                        user_text: row.get(6)?,
                        context,
                        project_instruction,
                        custom_gpt_context,
                        attachment_ids,
                        tools_enabled: row.get(9)?,
                        sandbox_enabled: row.get(10)?,
                        execution_preferences,
                        research_plan,
                        status: row.get(12)?,
                    })
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    #[allow(dead_code)]
    pub fn semantic_memory_matches(
        &self,
        workflow_id: &str,
    ) -> Result<Vec<SemanticMemoryMatch>, AppError> {
        self.semantic_memory_matches_with_limit(workflow_id, 5)
    }

    pub fn semantic_memory_matches_with_limit(
        &self,
        workflow_id: &str,
        maximum_items: usize,
    ) -> Result<Vec<SemanticMemoryMatch>, AppError> {
        let overview = self.memory_overview()?;
        let connection = self.connect()?;
        let (project_id, custom_gpt_id, model, dimensions, query_blob): (
            Option<String>,
            Option<String>,
            String,
            i64,
            Vec<u8>,
        ) = connection
            .query_row(
                "SELECT c.project_id, c.custom_gpt_id, er.model, er.dimensions, er.vector_blob
                 FROM semantic_chat_workflows workflow
                 JOIN conversations c ON c.id = workflow.conversation_id
                 JOIN embedding_records er
                   ON er.source_type = 'chat_memory_search'
                  AND er.source_id = workflow.id
                 WHERE workflow.id = ?1
                 ORDER BY er.created_at DESC, er.rowid DESC
                 LIMIT 1",
                params![workflow_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Conflict(
                    "la consulta semántica todavía no tiene un vector utilizable".to_owned(),
                )
            })?;
        if !overview.enabled && custom_gpt_id.is_none() {
            return Ok(Vec::new());
        }
        let query_vector = decode_embedding(&query_blob, dimensions)?;
        let mut statement = connection.prepare(
            "SELECT m.id, er.dimensions, er.vector_blob
             FROM memory_items m
             JOIN embedding_records er
              ON er.source_type = 'memory' AND er.source_id = m.id
              AND er.model = ?1 AND er.dimensions = ?2
             WHERE m.enabled = 1
               AND (
                 (?4 = 1 AND m.custom_gpt_id IS NULL
                    AND (m.project_id IS NULL OR (?3 IS NOT NULL AND m.project_id = ?3)))
                 OR (?5 IS NOT NULL AND m.custom_gpt_id = ?5)
               )
             ORDER BY m.updated_at DESC",
        )?;
        let candidates = statement
            .query_map(
                params![
                    model,
                    dimensions,
                    project_id.as_deref(),
                    overview.enabled,
                    custom_gpt_id.as_deref()
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let mut visible_items = if overview.enabled {
            overview.items
        } else {
            Vec::new()
        };
        if let Some(custom_gpt_id) = custom_gpt_id.as_deref() {
            visible_items.extend(self.custom_gpt_knowledge(custom_gpt_id)?);
        }
        let items_by_id: HashMap<String, MemoryItemView> = visible_items
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect();
        let mut matches = Vec::new();
        for (memory_id, candidate_dimensions, candidate_blob) in candidates {
            let Some(memory) = items_by_id.get(&memory_id) else {
                continue;
            };
            let candidate = decode_embedding(&candidate_blob, candidate_dimensions)?;
            let score = cosine_similarity(&query_vector, &candidate);
            if !score.is_finite() || score < 0.25 {
                continue;
            }
            let reason = if score >= 0.75 {
                "Coincidencia semántica alta"
            } else if score >= 0.5 {
                "Coincidencia semántica media"
            } else {
                "Coincidencia semántica baja"
            };
            matches.push(SemanticMemoryMatch {
                memory: memory.clone(),
                score: (score * 1000.0).round() / 1000.0,
                reason: reason.to_owned(),
            });
        }
        matches.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matches.truncate(maximum_items);
        Ok(matches)
    }

    pub fn semantic_workflow_uses_memory(&self, workflow_id: &str) -> Result<bool, AppError> {
        self.connect()?
            .query_row(
                "SELECT json_extract(task.request_json, '$.content.metadata.source_type')
                         = 'chat_memory_search'
                 FROM semantic_chat_workflows workflow
                 JOIN broker_tasks task ON task.id = workflow.embedding_task_id
                 WHERE workflow.id = ?1",
                params![workflow_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("flujo semántico {workflow_id}")))
    }
}
