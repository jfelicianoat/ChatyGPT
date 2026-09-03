//! Alta de adjuntos y de ficheros de proyecto, con deduplicado por hash.
//!
//! Dos conversaciones comparten el fichero sin compartir la decision de
//! si sus imagenes se describen: la politica viaja con el uso, no con el
//! contenido.

use super::*;

impl Database {
    #[allow(dead_code)]
    pub fn register_attachment(
        &self,
        conversation_id: &str,
        local_path: &str,
        display_name: &str,
        media_type: Option<&str>,
        size_bytes: i64,
        sha256: &str,
    ) -> Result<AttachmentView, AppError> {
        self.register_attachment_with_image_policy(
            conversation_id,
            local_path,
            display_name,
            media_type,
            size_bytes,
            sha256,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn register_attachment_with_image_policy(
        &self,
        conversation_id: &str,
        local_path: &str,
        display_name: &str,
        media_type: Option<&str>,
        size_bytes: i64,
        sha256: &str,
        describe_images: Option<bool>,
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
        let existing: Option<String> = match describe_images {
            Some(true) => transaction
                .query_row(
                    "SELECT id FROM attachments
                     WHERE sha256 = ?1 AND describe_images = 1
                     ORDER BY created_at, id LIMIT 1",
                    params![sha256],
                    |row| row.get(0),
                )
                .optional()?,
            Some(false) => transaction
                .query_row(
                    "SELECT id FROM attachments WHERE sha256 = ?1
                     ORDER BY CASE WHEN describe_images = 0 THEN 0 ELSE 1 END, created_at, id
                     LIMIT 1",
                    params![sha256],
                    |row| row.get(0),
                )
                .optional()?,
            None => transaction
                .query_row(
                    "SELECT id FROM attachments WHERE sha256 = ?1
                     ORDER BY created_at, id LIMIT 1",
                    params![sha256],
                    |row| row.get(0),
                )
                .optional()?,
        };
        let reused_attachment = existing.is_some();
        let attachment_id =
            existing.unwrap_or_else(|| format!("attachment_{}", Uuid::new_v4().simple()));
        transaction.execute(
            "INSERT OR IGNORE INTO attachments(
                id, local_path, display_name, media_type, size_bytes, sha256, describe_images
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                attachment_id,
                local_path,
                display_name,
                media_type,
                size_bytes,
                sha256,
                describe_images
            ],
        )?;
        if reused_attachment {
            let restarted = transaction.execute(
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
                params![attachment_id],
            )?;
            if restarted > 0 {
                transaction.execute(
                    "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
                     VALUES ('attachment.retry_requested', 'user', ?1, ?2)",
                    params![
                        conversation_id,
                        serde_json::json!({
                            "attachment_id": attachment_id,
                            "reason": "reattached_failed_file"
                        })
                        .to_string()
                    ],
                )?;
            }
        }
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

    #[allow(dead_code)]
    pub fn register_custom_gpt_attachment(
        &self,
        custom_gpt_id: &str,
        local_path: &str,
        display_name: &str,
        media_type: Option<&str>,
        size_bytes: i64,
        sha256: &str,
    ) -> Result<AttachmentView, AppError> {
        self.register_custom_gpt_attachment_with_image_policy(
            custom_gpt_id,
            local_path,
            display_name,
            media_type,
            size_bytes,
            sha256,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn register_custom_gpt_attachment_with_image_policy(
        &self,
        custom_gpt_id: &str,
        local_path: &str,
        display_name: &str,
        media_type: Option<&str>,
        size_bytes: i64,
        sha256: &str,
        describe_images: Option<bool>,
    ) -> Result<AttachmentView, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let active: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM custom_gpts WHERE id = ?1 AND archived_at IS NULL
             )",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        if !active {
            return Err(AppError::NotFound(format!("GPT personal {custom_gpt_id}")));
        }
        let current_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM custom_gpt_files WHERE custom_gpt_id = ?1",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        if current_count >= 20 {
            return Err(AppError::Conflict(
                "cada GPT personal admite hasta 20 archivos de conocimiento".to_owned(),
            ));
        }
        let existing: Option<String> = match describe_images {
            Some(true) => transaction
                .query_row(
                    "SELECT id FROM attachments
                     WHERE sha256 = ?1 AND describe_images = 1
                     ORDER BY created_at, id LIMIT 1",
                    params![sha256],
                    |row| row.get(0),
                )
                .optional()?,
            Some(false) => transaction
                .query_row(
                    "SELECT id FROM attachments WHERE sha256 = ?1
                     ORDER BY CASE WHEN describe_images = 0 THEN 0 ELSE 1 END, created_at, id
                     LIMIT 1",
                    params![sha256],
                    |row| row.get(0),
                )
                .optional()?,
            None => transaction
                .query_row(
                    "SELECT id FROM attachments WHERE sha256 = ?1
                     ORDER BY created_at, id LIMIT 1",
                    params![sha256],
                    |row| row.get(0),
                )
                .optional()?,
        };
        let reused_attachment = existing.is_some();
        let attachment_id =
            existing.unwrap_or_else(|| format!("attachment_{}", Uuid::new_v4().simple()));
        transaction.execute(
            "INSERT OR IGNORE INTO attachments(
                id, local_path, display_name, media_type, size_bytes, sha256, describe_images
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                attachment_id,
                local_path,
                display_name,
                media_type,
                size_bytes,
                sha256,
                describe_images
            ],
        )?;
        if reused_attachment {
            transaction.execute(
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
                params![attachment_id],
            )?;
        }
        transaction.execute(
            "INSERT OR IGNORE INTO custom_gpt_files(custom_gpt_id, attachment_id)
             VALUES (?1, ?2)",
            params![custom_gpt_id, attachment_id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.file_added', 'user', ?1)",
            params![serde_json::json!({
                "custom_gpt_id": custom_gpt_id,
                "attachment_id": attachment_id,
                "sha256": sha256,
                "size_bytes": size_bytes
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.attachment_view(&attachment_id)
    }

    pub fn list_custom_gpt_files(
        &self,
        custom_gpt_id: &str,
    ) -> Result<Vec<AttachmentView>, AppError> {
        let connection = self.connect()?;
        let exists: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM custom_gpts WHERE id = ?1 AND archived_at IS NULL
             )",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::NotFound(format!("GPT personal {custom_gpt_id}")));
        }
        let mut statement = connection.prepare(
            "SELECT attachment_id
             FROM custom_gpt_files
             WHERE custom_gpt_id = ?1
             ORDER BY added_at, attachment_id",
        )?;
        let ids = statement
            .query_map(params![custom_gpt_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|attachment_id| self.attachment_view(&attachment_id))
            .collect()
    }

    pub fn remove_custom_gpt_file(
        &self,
        custom_gpt_id: &str,
        attachment_id: &str,
    ) -> Result<Vec<AttachmentView>, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "DELETE FROM custom_gpt_files
             WHERE custom_gpt_id = ?1 AND attachment_id = ?2",
            params![custom_gpt_id, attachment_id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(
                "archivo de conocimiento del GPT personal".to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.file_removed', 'user', ?1)",
            params![serde_json::json!({
                "custom_gpt_id": custom_gpt_id,
                "attachment_id": attachment_id
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.list_custom_gpt_files(custom_gpt_id)
    }

    pub fn ready_custom_gpt_file_ids_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<String>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT file.attachment_id
             FROM conversations conversation
             JOIN custom_gpt_files file ON file.custom_gpt_id = conversation.custom_gpt_id
             JOIN attachments attachment ON attachment.id = file.attachment_id
             WHERE conversation.id = ?1
               AND conversation.archived_at IS NULL
               AND conversation.deleted_at IS NULL
               AND attachment.ingestion_status = 'ready'
               AND attachment.broker_file_id IS NOT NULL
             ORDER BY file.added_at, file.attachment_id",
        )?;
        let ids = statement
            .query_map(params![conversation_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        Ok(ids)
    }

    pub(super) fn attachment_available_to_conversation(
        connection: &Connection,
        conversation_id: &str,
        attachment_id: &str,
    ) -> Result<bool, AppError> {
        connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM conversation_attachments
                    WHERE conversation_id = ?1 AND attachment_id = ?2
                    UNION ALL
                    SELECT 1
                    FROM conversations conversation
                    JOIN custom_gpt_files file
                      ON file.custom_gpt_id = conversation.custom_gpt_id
                    WHERE conversation.id = ?1
                      AND conversation.archived_at IS NULL
                      AND conversation.deleted_at IS NULL
                      AND file.attachment_id = ?2
                 )",
                params![conversation_id, attachment_id],
                |row| row.get(0),
            )
            .map_err(AppError::from)
    }

    pub fn list_attachments(&self, conversation_id: &str) -> Result<Vec<AttachmentView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT a.id, a.display_name, a.media_type, a.size_bytes, a.sha256,
                    a.broker_file_id, a.ingestion_status, a.ingestion_error_json,
                    a.context_status, a.context_error_json,
                    (SELECT COUNT(*) FROM attachment_chunks chunk
                     WHERE chunk.attachment_id = a.id),
                    (SELECT COALESCE(SUM(length(chunk.content_text)), 0)
                     FROM attachment_chunks chunk
                     WHERE chunk.attachment_id = a.id),
                    (SELECT COUNT(*) FROM attachment_chunks chunk
                     WHERE chunk.attachment_id = a.id
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
                      WHERE chunk.attachment_id = a.id
                        AND json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'
                        AND task.local_state IN ('created', 'submitting', 'polling', 'recovery_pending')
                    ),
                    (SELECT COUNT(DISTINCT chunk.id)
                     FROM attachment_chunks chunk
                     JOIN broker_tasks task
                       ON json_extract(task.request_json, '$.content.metadata.source_id') = chunk.id
                     WHERE chunk.attachment_id = a.id
                       AND json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'
                       AND task.local_state IN ('terminal', 'orphaned')
                       AND task.remote_status != 'completed'),
                    (SELECT embedding.model
                     FROM attachment_chunks chunk
                     JOIN embedding_records embedding
                       ON embedding.source_type = 'attachment_chunk'
                      AND embedding.source_id = chunk.id
                      AND embedding.content_sha256 = chunk.content_sha256
                     WHERE chunk.attachment_id = a.id
                     ORDER BY embedding.created_at DESC, embedding.rowid DESC
                     LIMIT 1),
                    a.describe_images, a.updated_at
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

    pub fn list_project_files(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<AttachmentView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT pf.attachment_id
             FROM conversations c
             JOIN project_files pf ON pf.project_id = c.project_id
             JOIN attachments a ON a.id = pf.attachment_id
             WHERE c.id = ?1
               AND c.archived_at IS NULL
               AND c.deleted_at IS NULL
             ORDER BY pf.added_at, a.created_at",
        )?;
        let ids = statement
            .query_map(params![conversation_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|attachment_id| self.attachment_view(&attachment_id))
            .collect()
    }

    pub fn set_project_file(
        &self,
        conversation_id: &str,
        attachment_id: &str,
        enabled: bool,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let project_id = transaction
            .query_row(
                "SELECT project_id
                 FROM conversations
                 WHERE id = ?1
                   AND project_id IS NOT NULL
                   AND archived_at IS NULL
                   AND deleted_at IS NULL",
                params![conversation_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Conflict(
                    "la conversación debe pertenecer a un proyecto para compartir archivos"
                        .to_owned(),
                )
            })?;
        let changed = if enabled {
            let linked: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM conversation_attachments
                    WHERE conversation_id = ?1 AND attachment_id = ?2
                 )",
                params![conversation_id, attachment_id],
                |row| row.get(0),
            )?;
            if !linked {
                return Err(AppError::NotFound(format!(
                    "adjunto {attachment_id} en la conversación"
                )));
            }
            transaction.execute(
                "INSERT OR IGNORE INTO project_files(project_id, attachment_id)
                 VALUES (?1, ?2)",
                params![project_id, attachment_id],
            )?
        } else {
            transaction.execute(
                "DELETE FROM project_files
                 WHERE project_id = ?1 AND attachment_id = ?2",
                params![project_id, attachment_id],
            )?
        };
        if changed > 0 {
            transaction.execute(
                "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
                 VALUES (?1, 'user', ?2, ?3)",
                params![
                    if enabled {
                        "project.file_added"
                    } else {
                        "project.file_removed"
                    },
                    conversation_id,
                    serde_json::json!({
                        "project_id": project_id,
                        "attachment_id": attachment_id
                    })
                    .to_string()
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn use_project_file(
        &self,
        conversation_id: &str,
        attachment_id: &str,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "INSERT OR IGNORE INTO conversation_attachments(conversation_id, attachment_id)
             SELECT c.id, pf.attachment_id
             FROM conversations c
             JOIN project_files pf ON pf.project_id = c.project_id
             WHERE c.id = ?1
               AND pf.attachment_id = ?2
               AND c.archived_at IS NULL
               AND c.deleted_at IS NULL",
            params![conversation_id, attachment_id],
        )?;
        if changed == 0 {
            let already_linked: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1
                    FROM conversation_attachments ca
                    JOIN conversations c ON c.id = ca.conversation_id
                    JOIN project_files pf
                      ON pf.project_id = c.project_id
                     AND pf.attachment_id = ca.attachment_id
                    WHERE ca.conversation_id = ?1 AND ca.attachment_id = ?2
                 )",
                params![conversation_id, attachment_id],
                |row| row.get(0),
            )?;
            if !already_linked {
                return Err(AppError::NotFound(format!(
                    "archivo de proyecto {attachment_id}"
                )));
            }
        } else {
            transaction.execute(
                "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
                 VALUES ('project.file_used', 'user', ?1, ?2)",
                params![
                    conversation_id,
                    serde_json::json!({"attachment_id": attachment_id}).to_string()
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn conversation_attachment_records(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<AttachmentRecord>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT a.id, a.local_path, a.display_name, a.media_type, a.size_bytes,
                    a.sha256, a.broker_file_id, a.ingestion_status, a.describe_images
             FROM conversation_attachments ca
             JOIN attachments a ON a.id = ca.attachment_id
             WHERE ca.conversation_id = ?1
             ORDER BY ca.added_at, a.created_at",
        )?;
        let records = statement
            .query_map(params![conversation_id], |row| {
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
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(records)
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

    pub(super) fn map_attachment_view(row: &rusqlite::Row<'_>) -> rusqlite::Result<AttachmentView> {
        let error_json: Option<String> = row.get(7)?;
        let context_error_json: Option<String> = row.get(9)?;
        let chunk_count: i64 = row.get(10)?;
        let semantic_indexed_chunks: i64 = row.get(12)?;
        let semantic_active: bool = row.get(13)?;
        let semantic_failed_chunks: i64 = row.get(14)?;
        let semantic_index_status = if chunk_count == 0 {
            "unavailable"
        } else if semantic_indexed_chunks == chunk_count {
            "ready"
        } else if semantic_active {
            "indexing"
        } else if semantic_failed_chunks > 0 && semantic_indexed_chunks > 0 {
            "partial"
        } else if semantic_failed_chunks > 0 {
            "failed"
        } else {
            "pending"
        };
        Ok(AttachmentView {
            id: row.get(0)?,
            display_name: row.get(1)?,
            media_type: row.get(2)?,
            size_bytes: row.get(3)?,
            sha256: row.get(4)?,
            broker_file_id: row.get(5)?,
            ingestion_status: row.get(6)?,
            ingestion_error: error_json.and_then(|value| serde_json::from_str(&value).ok()),
            context_status: row.get(8)?,
            context_error: context_error_json.and_then(|value| serde_json::from_str(&value).ok()),
            chunk_count,
            indexed_characters: row.get(11)?,
            semantic_indexed_chunks,
            semantic_index_status: semantic_index_status.to_owned(),
            semantic_index_model: row.get(15)?,
            describe_images: row.get(16)?,
            updated_at: row.get(17)?,
        })
    }
}
