//! Resumenes de conversacion: borrador, revision y aprobacion.
//!
//! Un resumen aprobado compacta el contexto pero no borra mensajes: lo que
//! se resumio sigue estando para quien quiera mirarlo.

use super::*;

impl Database {
    pub fn prepare_conversation_summary(
        &self,
        conversation_id: &str,
        summary_id: &str,
        local_task_id: &str,
        idempotency_key: &str,
        request: &Value,
        source_through_sequence: i64,
    ) -> Result<BrokerTaskRecord, AppError> {
        let request_json = serde_json::to_string(request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
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
        transaction.execute(
            "INSERT INTO broker_tasks(
                id, conversation_id, idempotency_key, request_json,
                remote_status, local_state
             ) VALUES (?1, ?2, ?3, ?4, 'not_submitted', 'created')",
            params![
                local_task_id,
                conversation_id,
                idempotency_key,
                request_json
            ],
        )?;
        transaction.execute(
            "INSERT INTO conversation_summaries(
                id, conversation_id, broker_task_id,
                source_through_sequence, status
             ) VALUES (?1, ?2, ?3, ?4, 'generating')",
            params![
                summary_id,
                conversation_id,
                local_task_id,
                source_through_sequence
            ],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'local.prepared', 'not_submitted', '{}', datetime('now'))",
            params![local_task_id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('summary.generation_started', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({
                    "summary_id": summary_id,
                    "source_through_sequence": source_through_sequence
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        self.task_record(local_task_id)
    }

    pub fn conversation_summary_overview(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationSummaryOverview, AppError> {
        let connection = self.connect()?;
        let load = |status_filter: &str| -> Result<Option<ConversationSummaryRevision>, AppError> {
            connection
                .query_row(
                    "SELECT id, status, draft_text, approved_text,
                            source_through_sequence, broker_task_id, updated_at
                     FROM conversation_summaries
                     WHERE conversation_id = ?1 AND status = ?2
                     ORDER BY updated_at DESC, rowid DESC
                     LIMIT 1",
                    params![conversation_id, status_filter],
                    |row| {
                        Ok(ConversationSummaryRevision {
                            id: row.get(0)?,
                            status: row.get(1)?,
                            draft_text: row.get(2)?,
                            approved_text: row.get(3)?,
                            source_through_sequence: row.get(4)?,
                            broker_task_id: row.get(5)?,
                            updated_at: row.get(6)?,
                        })
                    },
                )
                .optional()
                .map_err(AppError::from)
        };
        let candidate = load("draft")?.or(load("generating")?);
        let active = load("approved")?;
        let total_message_count: i64 = connection.query_row(
            "SELECT COUNT(*)
             FROM messages
             WHERE conversation_id = ?1
               AND status = 'complete'
               AND role IN ('user', 'assistant')",
            params![conversation_id],
            |row| row.get(0),
        )?;
        let covered_count = |through_sequence: i64| -> Result<i64, AppError> {
            connection
                .query_row(
                    "SELECT COUNT(*)
                     FROM messages
                     WHERE conversation_id = ?1
                       AND status = 'complete'
                       AND role IN ('user', 'assistant')
                       AND sequence_no <= ?2",
                    params![conversation_id, through_sequence],
                    |row| row.get(0),
                )
                .map_err(AppError::from)
        };
        let active_covered_message_count = active
            .as_ref()
            .map(|revision| covered_count(revision.source_through_sequence))
            .transpose()?
            .unwrap_or(0);
        let candidate_covered_message_count = candidate
            .as_ref()
            .map(|revision| covered_count(revision.source_through_sequence))
            .transpose()?;
        Ok(ConversationSummaryOverview {
            candidate,
            active,
            total_message_count,
            active_covered_message_count,
            remaining_message_count: total_message_count
                .saturating_sub(active_covered_message_count),
            candidate_covered_message_count,
        })
    }

    pub fn update_conversation_summary_draft(
        &self,
        summary_id: &str,
        text: &str,
    ) -> Result<ConversationSummaryOverview, AppError> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > 10_000 {
            return Err(AppError::Validation(
                "el resumen debe contener entre 1 y 10.000 caracteres".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let conversation_id: String = transaction
            .query_row(
                "SELECT conversation_id FROM conversation_summaries
                 WHERE id = ?1 AND status = 'draft'",
                params![summary_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Conflict(
                    "solo se puede editar un resumen que esté en borrador".to_owned(),
                )
            })?;
        transaction.execute(
            "UPDATE conversation_summaries
             SET draft_text = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![summary_id, text],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('summary.draft_updated', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({"summary_id": summary_id}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_summary_overview(&conversation_id)
    }

    pub fn approve_conversation_summary(
        &self,
        summary_id: &str,
    ) -> Result<ConversationSummaryOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let (conversation_id, draft_text): (String, String) = transaction
            .query_row(
                "SELECT conversation_id, draft_text
                 FROM conversation_summaries
                 WHERE id = ?1 AND status = 'draft' AND draft_text IS NOT NULL",
                params![summary_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Conflict(
                    "solo se puede aprobar un resumen que esté en borrador".to_owned(),
                )
            })?;
        transaction.execute(
            "UPDATE conversation_summaries
             SET status = 'superseded', updated_at = datetime('now')
             WHERE conversation_id = ?1 AND status = 'approved'",
            params![conversation_id],
        )?;
        transaction.execute(
            "UPDATE conversation_summaries
             SET status = 'approved', approved_text = ?2,
                 approved_at = datetime('now'), updated_at = datetime('now')
             WHERE id = ?1",
            params![summary_id, draft_text],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('summary.approved', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({"summary_id": summary_id}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_summary_overview(&conversation_id)
    }
}
