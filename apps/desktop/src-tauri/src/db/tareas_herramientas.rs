//! Herramientas pedidas por el modelo: confirmacion, resultados y exportacion.
//!
//! Una confirmacion se declara, se persiste y no se puede repetir: el
//! usuario autoriza una vez, no cada vez que alguien reintente.

use super::*;

impl Database {
    pub fn pending_tool_calls(&self, local_task_id: &str) -> Result<Vec<ToolCallView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT call.remote_tool_call_id, call.tool_name, call.arguments_json, call.status,
                    request.id, request.action_type, request.tool_name, request.resources_json,
                    request.disclosure_json, request.consequences, request.status,
                    request.requested_at, request.resolved_at
             FROM tool_calls call
             LEFT JOIN confirmation_requests request ON request.tool_call_id = call.id
             WHERE call.broker_task_id = ?1 AND call.status = 'confirmation_required'
             ORDER BY call.requested_at, call.id",
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
                let confirmation_id: Option<String> = row.get(4)?;
                let confirmation = match confirmation_id {
                    Some(id) => {
                        let resources_json: String = row.get(7)?;
                        let disclosure_json: String = row.get(8)?;
                        Some(ConfirmationRequestView {
                            id,
                            action_type: row.get(5)?,
                            tool_name: row.get(6)?,
                            resources: serde_json::from_str(&resources_json).unwrap_or(Value::Null),
                            disclosure: serde_json::from_str(&disclosure_json)
                                .unwrap_or(Value::Null),
                            consequences: row.get(9)?,
                            status: row.get(10)?,
                            requested_at: row.get(11)?,
                            resolved_at: row.get(12)?,
                        })
                    }
                    None => None,
                };
                Ok(ToolCallView {
                    tool_call_id: row.get(0)?,
                    name: row.get(1)?,
                    arguments,
                    status: row.get(3)?,
                    confirmation,
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
            // La confirmación se resuelve en la misma transacción que ejecuta la
            // decisión: sin expediente pendiente no hay ejecución posible, y un
            // segundo intento sobre el mismo expediente se rechaza.
            let confirmation_status = if outcome.status == "approved" {
                "allowed_once"
            } else {
                "cancelled"
            };
            let resolved = transaction.execute(
                "UPDATE confirmation_requests
                 SET status = ?2, resolved_at = datetime('now')
                 WHERE tool_call_id = ?1 AND status = 'pending'",
                params![local_call_id, confirmation_status],
            )?;
            if resolved == 0 {
                let existing: Option<String> = transaction
                    .query_row(
                        "SELECT status FROM confirmation_requests WHERE tool_call_id = ?1",
                        params![local_call_id],
                        |row| row.get(0),
                    )
                    .optional()?;
                match existing {
                    Some(status) => {
                        return Err(AppError::Conflict(format!(
                            "esta confirmación ya se resolvió como {status}; \
                             vuelve a abrir la conversación para ver su estado"
                        )));
                    }
                    // Tarea heredada de un esquema anterior al expediente: se deja
                    // constancia de la decisión en lugar de bloquear la respuesta.
                    None => {
                        let conversation_id: Option<String> = transaction.query_row(
                            "SELECT conversation_id FROM broker_tasks WHERE id = ?1",
                            params![local_task_id],
                            |row| row.get(0),
                        )?;
                        let tool_name: String = transaction.query_row(
                            "SELECT tool_name FROM tool_calls WHERE id = ?1",
                            params![local_call_id],
                            |row| row.get(0),
                        )?;
                        let arguments_json: String = transaction.query_row(
                            "SELECT arguments_json FROM tool_calls WHERE id = ?1",
                            params![local_call_id],
                            |row| row.get(0),
                        )?;
                        let arguments: Value =
                            serde_json::from_str(&arguments_json).unwrap_or(Value::Null);
                        let (action_type, resources, disclosure, consequences) =
                            confirmation_blueprint(
                                &tool_name,
                                &arguments,
                                conversation_id.as_deref(),
                            );
                        transaction.execute(
                            "INSERT INTO confirmation_requests(
                                id, action_type, tool_name, resources_json, disclosure_json,
                                consequences, status, requested_at, resolved_at,
                                tool_call_id, conversation_id
                             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'),
                                       datetime('now'), ?8, ?9)",
                            params![
                                format!("confirm_{}", Uuid::new_v4().simple()),
                                action_type,
                                tool_name,
                                resources.to_string(),
                                disclosure.to_string(),
                                consequences,
                                confirmation_status,
                                local_call_id,
                                conversation_id
                            ],
                        )?;
                    }
                }
            }
            transaction.execute(
                "INSERT INTO audit_events(
                    event_type, actor, conversation_id, broker_task_id, payload_json
                 ) VALUES ('confirmation.resolved', 'user',
                           (SELECT conversation_id FROM broker_tasks WHERE id = ?1), ?1, ?2)",
                params![
                    local_task_id,
                    serde_json::json!({
                        "decision": confirmation_status,
                        "tool_call_id": local_call_id
                    })
                    .to_string()
                ],
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
                        consecutive_poll_errors, result_json, error_json, updated_at,
                        progress_json,
                        json_extract(request_json, '$.inference_kind'),
                        json_extract(request_json, '$.content.metadata.source_type')
                 FROM broker_tasks WHERE id = ?1",
                params![id],
                |row| {
                    let result_json: Option<String> = row.get(6)?;
                    let error_json: Option<String> = row.get(6)?;
                    let progress_json: String = row.get(8)?;
                    let progress_value: Value =
                        serde_json::from_str(&progress_json).unwrap_or(Value::Null);
                    let inference_kind: Option<String> = row.get(9)?;
                    let source_type: Option<String> = row.get(10)?;
                    let activity = match (inference_kind.as_deref(), source_type.as_deref()) {
                        (Some("chat"), Some("conversation_summary")) => {
                            "Preparando borrador del resumen"
                        }
                        (Some("embedding"), Some("chat_memory_search")) => {
                            "Buscando contexto relacionado"
                        }
                        (Some("embedding"), Some("chat_document_search")) => {
                            "Buscando fragmentos relacionados"
                        }
                        (Some("embedding"), Some("memory_search")) => "Buscando en la memoria",
                        (Some("embedding"), Some("attachment_chunk")) => {
                            "Preparando el índice documental"
                        }
                        (Some("embedding"), _) => "Preparando el índice de memoria",
                        (Some("chat"), _) | (Some("agent"), _) => "Generando respuesta",
                        _ => "Procesando tarea",
                    };
                    Ok(LocalTaskSnapshot {
                        id: row.get(0)?,
                        activity: activity.to_owned(),
                        remote_task_id: row.get(1)?,
                        remote_status: row.get(2)?,
                        local_state: row.get(3)?,
                        consecutive_poll_errors: row.get(4)?,
                        result: result_json.and_then(|value| serde_json::from_str(&value).ok()),
                        error: error_json.and_then(|value| serde_json::from_str(&value).ok()),
                        progress: TaskProgressView {
                            phase: progress_value
                                .get("phase")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                            invocations_completed: progress_value
                                .get("invocations_completed")
                                .and_then(Value::as_i64),
                            invocations_total: progress_value
                                .get("invocations_total")
                                .and_then(Value::as_i64),
                        },
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
}
