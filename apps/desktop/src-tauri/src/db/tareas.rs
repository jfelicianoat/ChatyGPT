//! Envio de una tarea al Broker y lectura de su estado remoto.

use super::*;

impl Database {
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
        let progress_json = serde_json::to_string(&state.progress)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let payload_json = serde_json::to_string(state)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE broker_tasks
             SET remote_status = ?2, local_state = ?3,
                 consecutive_poll_errors = 0, result_json = ?4, error_json = ?5,
                 progress_json = ?6,
                 terminal_at = CASE
                    WHEN ?3 = 'terminal'
                    THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                    ELSE NULL
                 END,
                 next_poll_at = CASE WHEN ?3 = 'polling' THEN datetime('now') ELSE NULL END,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![
                id,
                state.status.as_str(),
                local_state,
                result_json,
                error_json,
                progress_json
            ],
        )?;
        let research_phase = state
            .progress
            .get("phase")
            .and_then(Value::as_str)
            .unwrap_or_else(|| state.status.as_str());
        let research_run_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM research_runs WHERE broker_task_id = ?1)",
            params![id],
            |row| row.get(0),
        )?;
        if research_run_exists {
            let (run_status, _active_ordinal) = if state.status.is_terminal() {
                (
                    match state.status.as_str() {
                        "completed" => "completed",
                        "cancelled" => "cancelled",
                        _ => "failed",
                    },
                    None,
                )
            } else if matches!(research_phase, "synthesizing" | "verifying") {
                ("synthesizing", Some(2_i64))
            } else if matches!(
                research_phase,
                "queued" | "routing" | "planning" | "resource_planning"
            ) {
                ("planning", Some(0_i64))
            } else {
                ("researching", Some(1_i64))
            };
            transaction.execute(
                "UPDATE research_runs
                 SET status = ?2,
                     updated_at = datetime('now'),
                     completed_at = CASE WHEN ?3 = 1 THEN datetime('now') ELSE NULL END
                 WHERE broker_task_id = ?1",
                params![id, run_status, state.status.is_terminal()],
            )?;
            if state.status.is_terminal() {
                let step_status = match state.status.as_str() {
                    "completed" => "completed",
                    "cancelled" => "cancelled",
                    _ => "failed",
                };
                transaction.execute(
                    "UPDATE research_steps
                     SET status = CASE
                           -- Un paso que ya terminó conserva su desenlace: que
                           -- la investigación acabe bien no convierte en buena
                           -- una fuente que no se pudo abrir. Solo se cierran
                           -- los pasos que se quedaron a medias.
                           WHEN status IN ('completed', 'failed', 'cancelled') THEN status
                           ELSE ?2
                         END,
                         started_at = COALESCE(started_at, datetime('now')),
                         completed_at = COALESCE(completed_at, datetime('now'))
                     WHERE research_run_id = (
                       SELECT id FROM research_runs WHERE broker_task_id = ?1
                     )",
                    params![id, step_status],
                )?;
            }
            // Sin proyección por ordinal: antes se derivaba el estado de cada
            // etapa fija de la fase remota, porque las etapas eran una
            // plantilla. Los pasos reales llevan su propio estado, el de la
            // herramienta que se ejecutó, y sobrescribirlo desde la fase de la
            // tarea daría por «pendiente» una fuente ya abierta.
        }
        if previous != state.status.as_str() {
            transaction.execute(
                "INSERT INTO broker_task_events(
                    broker_task_id, event_type, remote_status, payload_json, occurred_at
                 ) VALUES (?1, 'remote.status_changed', ?2, ?3, datetime('now'))",
                params![id, state.status.as_str(), payload_json],
            )?;
        }
        let request_metadata = request
            .get("content")
            .and_then(|content| content.get("metadata"));
        let request_source_type = request_metadata
            .and_then(|value| value.get("source_type"))
            .and_then(Value::as_str);
        let request_source_id = request_metadata
            .and_then(|value| value.get("source_id"))
            .and_then(Value::as_str);
        if previous != state.status.as_str()
            && state.status.is_terminal()
            && request_source_type == Some("conversation_summary")
        {
            if let Some(summary_id) = request_source_id {
                if state.status.as_str() == "completed" {
                    let markdown = state
                        .result
                        .as_ref()
                        .and_then(assistant_result_text)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| {
                            AppError::BrokerContract(
                                "el resumen completado no incluye contenido Markdown".to_owned(),
                            )
                        })?;
                    transaction.execute(
                        "UPDATE conversation_summaries
                         SET status = 'draft', draft_text = ?2,
                             updated_at = datetime('now')
                         WHERE id = ?1 AND status = 'generating'",
                        params![summary_id, markdown],
                    )?;
                    transaction.execute(
                        "INSERT INTO audit_events(
                            event_type, actor, conversation_id, payload_json
                         ) SELECT 'summary.draft_ready', 'broker', conversation_id, ?2
                           FROM conversation_summaries WHERE id = ?1",
                        params![
                            summary_id,
                            serde_json::json!({"summary_id": summary_id}).to_string()
                        ],
                    )?;
                } else {
                    transaction.execute(
                        "UPDATE conversation_summaries
                         SET status = ?2, updated_at = datetime('now')
                         WHERE id = ?1 AND status = 'generating'",
                        params![
                            summary_id,
                            if state.status.as_str() == "cancelled" {
                                "cancelled"
                            } else {
                                "failed"
                            }
                        ],
                    )?;
                }
            }
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
                let source_is_current = if source_type == "memory" {
                    transaction
                        .query_row(
                            "SELECT content FROM memory_items WHERE id = ?1",
                            params![source_id],
                            |row| row.get::<_, String>(0),
                        )
                        .optional()?
                        .is_some_and(|content| {
                            format!("{:x}", Sha256::digest(content.as_bytes())) == content_sha256
                        })
                } else if source_type == "attachment_chunk" {
                    transaction
                        .query_row(
                            "SELECT content_sha256 FROM attachment_chunks WHERE id = ?1",
                            params![source_id],
                            |row| row.get::<_, String>(0),
                        )
                        .optional()?
                        .is_some_and(|current_sha256| current_sha256 == content_sha256)
                } else {
                    true
                };
                if source_is_current {
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
                // El expediente de confirmación nace junto a la llamada, antes de
                // que nadie pueda decidir: así queda constancia de qué se propuso
                // aunque la persona cierre la aplicación sin responder.
                let local_call_id: String = transaction.query_row(
                    "SELECT id FROM tool_calls
                     WHERE broker_task_id = ?1 AND remote_tool_call_id = ?2",
                    params![id, remote_tool_call_id],
                    |row| row.get(0),
                )?;
                let already_recorded: Option<String> = transaction
                    .query_row(
                        "SELECT id FROM confirmation_requests WHERE tool_call_id = ?1",
                        params![local_call_id],
                        |row| row.get(0),
                    )
                    .optional()?;
                if already_recorded.is_none() {
                    let conversation_id: Option<String> = transaction.query_row(
                        "SELECT conversation_id FROM broker_tasks WHERE id = ?1",
                        params![id],
                        |row| row.get(0),
                    )?;
                    let (action_type, resources, disclosure, consequences) =
                        confirmation_blueprint(tool_name, &arguments, conversation_id.as_deref());
                    transaction.execute(
                        "INSERT INTO confirmation_requests(
                            id, action_type, tool_name, resources_json, disclosure_json,
                            consequences, status, tool_call_id, conversation_id
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7, ?8)",
                        params![
                            format!("confirm_{}", Uuid::new_v4().simple()),
                            action_type,
                            tool_name,
                            resources.to_string(),
                            disclosure.to_string(),
                            consequences,
                            local_call_id,
                            conversation_id
                        ],
                    )?;
                }
            }
        }
        if previous != state.status.as_str() && state.status.is_terminal() {
            if let Some(message_id) = response_message_id {
                let (message_status, kind, content_text, content_json) =
                    if state.status.as_str() == "completed" {
                        let markdown = state
                            .result
                            .as_ref()
                            .and_then(assistant_result_text)
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
                    if request_metadata
                        .and_then(|metadata| metadata.get("workflow_kind"))
                        .and_then(Value::as_str)
                        == Some("deep_research")
                    {
                        let markdown = state
                            .result
                            .as_ref()
                            .and_then(assistant_result_text)
                            .unwrap_or_default();
                        let first_ordinal: i64 = transaction.query_row(
                            "SELECT COALESCE(MAX(ordinal), -1) + 1
                             FROM citations WHERE message_id = ?1",
                            params![message_id],
                            |row| row.get(0),
                        )?;
                        for (offset, (title, url)) in
                            markdown_web_sources(markdown).into_iter().enumerate()
                        {
                            transaction.execute(
                                "INSERT INTO citations(
                                    id, message_id, ordinal, title, url, metadata_json
                                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                                params![
                                    format!("citation_{}", Uuid::new_v4().simple()),
                                    message_id,
                                    first_ordinal + offset as i64,
                                    title,
                                    url,
                                    serde_json::json!({
                                        "kind": "web",
                                        "attribution": "deep_research_markdown"
                                    })
                                    .to_string()
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
}
