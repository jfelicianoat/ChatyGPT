//! Estado de un adjunto: subida, preparacion de contexto, fallos y reintentos.

use super::*;

impl Database {
    pub fn attachment_view(&self, id: &str) -> Result<AttachmentView, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT id, display_name, media_type, size_bytes, sha256,
                        broker_file_id, ingestion_status, ingestion_error_json,
                        context_status, context_error_json,
                        (SELECT COUNT(*) FROM attachment_chunks chunk
                         WHERE chunk.attachment_id = attachments.id),
                        (SELECT COALESCE(SUM(length(chunk.content_text)), 0)
                         FROM attachment_chunks chunk
                         WHERE chunk.attachment_id = attachments.id),
                        (SELECT COUNT(*) FROM attachment_chunks chunk
                         WHERE chunk.attachment_id = attachments.id
                           AND EXISTS(
                             SELECT 1 FROM embedding_records embedding
                             WHERE embedding.source_type = 'attachment_chunk'
                               AND embedding.source_id = chunk.id
                               AND embedding.content_sha256 = chunk.content_sha256
                           )),
                        EXISTS(
                          SELECT 1 FROM broker_tasks task
                          JOIN attachment_chunks chunk
                            ON chunk.id = json_extract(task.request_json, '$.content.metadata.source_id')
                          WHERE chunk.attachment_id = attachments.id
                            AND json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'
                            AND task.local_state IN ('created', 'submitting', 'polling', 'recovery_pending')
                        ),
                        (SELECT COUNT(DISTINCT chunk.id)
                         FROM attachment_chunks chunk
                         JOIN broker_tasks task
                           ON json_extract(task.request_json, '$.content.metadata.source_id') = chunk.id
                         WHERE chunk.attachment_id = attachments.id
                           AND json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'
                           AND task.local_state IN ('terminal', 'orphaned')
                           AND task.remote_status != 'completed'),
                        (SELECT embedding.model
                         FROM attachment_chunks chunk
                         JOIN embedding_records embedding
                           ON embedding.source_type = 'attachment_chunk'
                          AND embedding.source_id = chunk.id
                          AND embedding.content_sha256 = chunk.content_sha256
                         WHERE chunk.attachment_id = attachments.id
                         ORDER BY embedding.created_at DESC, embedding.rowid DESC
                         LIMIT 1),
                        describe_images, updated_at
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
                        broker_file_id, ingestion_status, describe_images
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
                        describe_images: row.get(8)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("adjunto {id}")))
    }

    pub fn set_attachment_describe_images(
        &self,
        id: &str,
        describe_images: bool,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let changed = connection.execute(
            "UPDATE attachments
             SET describe_images = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![id, describe_images],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("adjunto {id}")));
        }
        Ok(())
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

    pub fn ready_attachments_without_chunks(&self) -> Result<Vec<AttachmentRecord>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT attachment.id
             FROM attachments attachment
             WHERE attachment.ingestion_status = 'ready'
               AND attachment.broker_file_id IS NOT NULL
               AND attachment.context_status IN ('pending', 'preparing')
               AND NOT EXISTS(
                   SELECT 1 FROM attachment_chunks chunk
                   WHERE chunk.attachment_id = attachment.id
               )
             ORDER BY attachment.updated_at",
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

    pub fn mark_attachment_context_preparing(&self, id: &str) -> Result<(), AppError> {
        let changed = self.connect()?.execute(
            "UPDATE attachments
             SET context_status = 'preparing',
                 context_error_json = NULL,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("adjunto {id}")));
        }
        Ok(())
    }

    pub fn record_attachment_context_failure(
        &self,
        id: &str,
        error: &Value,
    ) -> Result<(), AppError> {
        let error_json = serde_json::to_string(error)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let changed = self.connect()?.execute(
            "UPDATE attachments
             SET context_status = 'failed',
                 context_error_json = ?2,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id, error_json],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("adjunto {id}")));
        }
        Ok(())
    }

    pub fn mark_attachment_context_unavailable(&self, id: &str) -> Result<(), AppError> {
        let changed = self.connect()?.execute(
            "UPDATE attachments
             SET context_status = 'unavailable',
                 context_error_json = NULL,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("adjunto {id}")));
        }
        Ok(())
    }

    pub fn reset_attachment_context_for_retry(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE attachments
             SET context_status = 'pending',
                 context_error_json = NULL,
                 updated_at = datetime('now')
             WHERE id = ?1
               AND ingestion_status = 'ready'
               AND context_status IN ('failed', 'unavailable')",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::Conflict(
                "el contexto de este adjunto no admite reintento".to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('attachment.context_retry_requested', 'user', ?1)",
            params![serde_json::json!({"attachment_id": id}).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn reset_failed_attachment_for_retry(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE attachments
             SET broker_file_id = NULL,
                 ingestion_status = 'local',
                 ingestion_error_json = NULL,
                 context_status = 'pending',
                 context_error_json = NULL,
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
            let linked =
                Self::attachment_available_to_conversation(&connection, conversation_id, id)?;
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
}
