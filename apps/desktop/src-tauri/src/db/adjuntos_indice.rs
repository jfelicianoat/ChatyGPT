//! Troceado de documentos, embeddings y seleccion de fragmentos.
//!
//! La seleccion mezcla coseno y estructura: una peticion global del
//! documento no puede resolverse solo con los fragmentos mas parecidos.

use super::*;

impl Database {
    pub fn replace_attachment_chunks(
        &self,
        attachment_id: &str,
        chunks: &[String],
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM attachments WHERE id = ?1)",
            params![attachment_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::NotFound(format!("adjunto {attachment_id}")));
        }
        transaction.execute(
            "DELETE FROM attachment_chunks WHERE attachment_id = ?1",
            params![attachment_id],
        )?;
        let mut stored_chunks = 0_i64;
        for (ordinal, text) in chunks.iter().enumerate() {
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            let content_sha256 = format!("{:x}", Sha256::digest(text.as_bytes()));
            transaction.execute(
                "INSERT INTO attachment_chunks(
                    id, attachment_id, ordinal, content_text, content_sha256
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    format!("chunk_{}_{}", attachment_id, ordinal),
                    attachment_id,
                    ordinal as i64,
                    text,
                    content_sha256
                ],
            )?;
            stored_chunks += 1;
        }
        transaction.execute(
            "UPDATE attachments
             SET context_status = ?2,
                 context_error_json = NULL,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![
                attachment_id,
                if stored_chunks > 0 {
                    "ready"
                } else {
                    "unavailable"
                }
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    #[cfg(test)]
    pub fn next_attachment_chunk_for_embedding(
        &self,
        attachment_id: &str,
        retry_failed: bool,
    ) -> Result<Option<AttachmentChunkEmbeddingInput>, AppError> {
        let connection = self.connect()?;
        let active: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM broker_tasks task
                JOIN attachment_chunks chunk
                  ON chunk.id = json_extract(task.request_json, '$.content.metadata.source_id')
                WHERE chunk.attachment_id = ?1
                  AND json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'
                  AND task.local_state IN ('created', 'submitting', 'polling', 'recovery_pending')
             )",
            params![attachment_id],
            |row| row.get(0),
        )?;
        if active {
            return Ok(None);
        }
        connection
            .query_row(
                "SELECT chunk.id, chunk.content_text, chunk.content_sha256
                 FROM attachment_chunks chunk
                 JOIN attachments attachment ON attachment.id = chunk.attachment_id
                 WHERE chunk.attachment_id = ?1
                   AND attachment.context_status = 'ready'
                   AND NOT EXISTS(
                     SELECT 1 FROM embedding_records embedding
                     WHERE embedding.source_type = 'attachment_chunk'
                       AND embedding.source_id = chunk.id
                       AND embedding.content_sha256 = chunk.content_sha256
                   )
                   AND (
                     ?2 = 1 OR NOT EXISTS(
                       SELECT 1 FROM broker_tasks task
                       WHERE json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'
                         AND json_extract(task.request_json, '$.content.metadata.source_id') = chunk.id
                         AND json_extract(task.request_json, '$.content.metadata.content_sha256') = chunk.content_sha256
                         AND task.local_state IN ('terminal', 'orphaned')
                     )
                   )
                 ORDER BY chunk.ordinal
                 LIMIT 1",
                params![attachment_id, retry_failed],
                |row| {
                    Ok(AttachmentChunkEmbeddingInput {
                        id: row.get(0)?,
                        text: row.get(1)?,
                        content_sha256: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    /// Devuelve de una vez todos los fragmentos que aún necesitan una tarea de
    /// embedding. Preparar el lote completo antes de enviar su primera tarea es
    /// una precondición del contrato 2.8: `depends_on_group` solo ve las tareas
    /// que ya existen cuando la dependiente es reclamada.
    pub fn attachment_chunks_for_embedding(
        &self,
        attachment_id: &str,
        retry_failed: bool,
    ) -> Result<Vec<AttachmentChunkEmbeddingInput>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT chunk.id, chunk.content_text, chunk.content_sha256
             FROM attachment_chunks chunk
             JOIN attachments attachment ON attachment.id = chunk.attachment_id
             WHERE chunk.attachment_id = ?1
               AND attachment.context_status = 'ready'
               AND NOT EXISTS(
                 SELECT 1 FROM embedding_records embedding
                 WHERE embedding.source_type = 'attachment_chunk'
                   AND embedding.source_id = chunk.id
                   AND embedding.content_sha256 = chunk.content_sha256
               )
               AND NOT EXISTS(
                 SELECT 1 FROM broker_tasks task
                 WHERE json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'
                   AND json_extract(task.request_json, '$.content.metadata.source_id') = chunk.id
                   AND json_extract(task.request_json, '$.content.metadata.content_sha256') = chunk.content_sha256
                   AND (
                     task.local_state IN ('created', 'submitting', 'polling', 'recovery_pending')
                     OR (?2 = 0 AND task.local_state IN ('terminal', 'orphaned'))
                   )
               )
             ORDER BY chunk.ordinal",
        )?;
        let chunks = statement
            .query_map(params![attachment_id, retry_failed], |row| {
                Ok(AttachmentChunkEmbeddingInput {
                    id: row.get(0)?,
                    text: row.get(1)?,
                    content_sha256: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(chunks)
    }

    pub fn attachment_embedding_tasks(
        &self,
        attachment_ids: &[String],
    ) -> Result<Vec<BrokerTaskRecord>, AppError> {
        let connection = self.connect()?;
        let mut task_ids = Vec::new();
        for attachment_id in attachment_ids {
            let mut statement = connection.prepare(
                "SELECT task.id
                 FROM broker_tasks task
                 JOIN attachment_chunks chunk
                   ON chunk.id = json_extract(task.request_json, '$.content.metadata.source_id')
                 WHERE chunk.attachment_id = ?1
                   AND json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'
                   AND json_extract(task.request_json, '$.content.metadata.content_sha256') = chunk.content_sha256
                   AND (
                     task.local_state IN ('created', 'submitting', 'polling', 'recovery_pending')
                     OR (
                       task.remote_task_id IS NOT NULL
                       AND task.remote_status IN ('failed', 'cancelled')
                     )
                   )
                 ORDER BY chunk.ordinal, task.created_at",
            )?;
            task_ids.extend(
                statement
                    .query_map(params![attachment_id], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        task_ids
            .iter()
            .map(|task_id| self.task_record(task_id))
            .collect()
    }

    pub fn attachments_needing_semantic_index(&self) -> Result<Vec<String>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT attachment.id
             FROM attachments attachment
             WHERE attachment.context_status = 'ready'
               AND EXISTS(
                 SELECT 1 FROM attachment_chunks chunk
                 WHERE chunk.attachment_id = attachment.id
                   AND NOT EXISTS(
                     SELECT 1 FROM embedding_records embedding
                     WHERE embedding.source_type = 'attachment_chunk'
                       AND embedding.source_id = chunk.id
                       AND embedding.content_sha256 = chunk.content_sha256
                   )
               )
             ORDER BY attachment.updated_at",
        )?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    pub fn attachments_have_semantic_index(
        &self,
        attachment_ids: &[String],
    ) -> Result<bool, AppError> {
        if attachment_ids.is_empty() {
            return Ok(false);
        }
        let connection = self.connect()?;
        for attachment_id in attachment_ids {
            let indexed: bool = connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM attachment_chunks chunk
                    JOIN embedding_records embedding
                      ON embedding.source_type = 'attachment_chunk'
                     AND embedding.source_id = chunk.id
                     AND embedding.content_sha256 = chunk.content_sha256
                    WHERE chunk.attachment_id = ?1
                 )",
                params![attachment_id],
                |row| row.get(0),
            )?;
            if indexed {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn select_attachment_chunks(
        &self,
        conversation_id: &str,
        attachment_ids: &[String],
        query: &str,
        maximum_chunks: usize,
        character_budget: usize,
    ) -> Result<Vec<SelectedAttachmentChunk>, AppError> {
        self.select_attachment_chunks_with_query(
            conversation_id,
            attachment_ids,
            query,
            maximum_chunks,
            character_budget,
            None,
        )
    }

    pub fn select_attachment_chunks_hybrid(
        &self,
        conversation_id: &str,
        attachment_ids: &[String],
        query: &str,
        maximum_chunks: usize,
        character_budget: usize,
        semantic_query_id: &str,
    ) -> Result<Vec<SelectedAttachmentChunk>, AppError> {
        self.select_attachment_chunks_with_query(
            conversation_id,
            attachment_ids,
            query,
            maximum_chunks,
            character_budget,
            Some(semantic_query_id),
        )
    }

    pub(super) fn select_attachment_chunks_with_query(
        &self,
        conversation_id: &str,
        attachment_ids: &[String],
        query: &str,
        maximum_chunks: usize,
        character_budget: usize,
        semantic_query_id: Option<&str>,
    ) -> Result<Vec<SelectedAttachmentChunk>, AppError> {
        if attachment_ids.is_empty() || maximum_chunks == 0 || character_budget == 0 {
            return Ok(Vec::new());
        }
        let connection = self.connect()?;
        let query_terms = lexical_terms(query);
        let mut semantic_scores = HashMap::new();
        if let Some(semantic_query_id) = semantic_query_id {
            let query_embedding = connection
                .query_row(
                    "SELECT model, dimensions, vector_blob
                     FROM embedding_records
                     WHERE source_type IN ('chat_memory_search', 'chat_document_search')
                       AND source_id = ?1
                     ORDER BY created_at DESC, rowid DESC
                     LIMIT 1",
                    params![semantic_query_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, Vec<u8>>(2)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((model, dimensions, query_blob)) = query_embedding {
                let query_vector = decode_embedding(&query_blob, dimensions)?;
                let allowed_attachments = attachment_ids.iter().collect::<HashSet<_>>();
                let mut statement = connection.prepare(
                    "SELECT chunk.id, chunk.attachment_id,
                            embedding.dimensions, embedding.vector_blob
                     FROM attachment_chunks chunk
                     JOIN embedding_records embedding
                       ON embedding.source_type = 'attachment_chunk'
                      AND embedding.source_id = chunk.id
                      AND embedding.content_sha256 = chunk.content_sha256
                     WHERE embedding.model = ?1 AND embedding.dimensions = ?2",
                )?;
                let rows = statement
                    .query_map(params![model, dimensions], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, Vec<u8>>(3)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                for (chunk_id, attachment_id, candidate_dimensions, candidate_blob) in rows {
                    if !allowed_attachments.contains(&attachment_id) {
                        continue;
                    }
                    let candidate = decode_embedding(&candidate_blob, candidate_dimensions)?;
                    let score = cosine_similarity(&query_vector, &candidate);
                    if score.is_finite() {
                        semantic_scores.insert(chunk_id, score.max(0.0));
                    }
                }
            }
        }
        let mut candidates = Vec::new();
        for attachment_id in attachment_ids {
            let linked = Self::attachment_available_to_conversation(
                &connection,
                conversation_id,
                attachment_id,
            )?;
            if !linked {
                return Err(AppError::Validation(format!(
                    "el adjunto {attachment_id} no pertenece a esta conversación"
                )));
            }
            let mut statement = connection.prepare(
                "SELECT chunk.id, chunk.attachment_id, attachment.display_name,
                        chunk.ordinal, chunk.content_text
                 FROM attachment_chunks chunk
                 JOIN attachments attachment ON attachment.id = chunk.attachment_id
                 WHERE chunk.attachment_id = ?1
                 ORDER BY chunk.ordinal",
            )?;
            let rows = statement
                .query_map(params![attachment_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (id, attachment_id, attachment_name, ordinal, text) in rows {
                let chunk_terms = lexical_terms(&text);
                let matched = query_terms.intersection(&chunk_terms).count();
                let lexical_score = if query_terms.is_empty() {
                    0.0
                } else {
                    matched as f64 / query_terms.len() as f64
                };
                let semantic_score = semantic_scores.get(&id).copied();
                let score = semantic_score
                    .map(|semantic| lexical_score * 0.35 + semantic * 0.65)
                    .unwrap_or(lexical_score);
                candidates.push(SelectedAttachmentChunk {
                    id,
                    attachment_id,
                    attachment_name,
                    ordinal,
                    text,
                    score,
                    reason: if matched > 0 && semantic_score.is_some() {
                        "Coincidencia léxica y semántica".to_owned()
                    } else if semantic_score.is_some_and(|semantic| semantic >= 0.25) {
                        "Coincidencia semántica".to_owned()
                    } else if matched > 0 {
                        "Coincidencia con la pregunta".to_owned()
                    } else {
                        "Inicio del documento".to_owned()
                    },
                });
            }
        }
        if is_global_document_request(query) {
            return select_global_document_chunks(candidates, maximum_chunks, character_budget);
        }
        let has_relevant = candidates.iter().any(|candidate| {
            candidate.score > 0.0
                && (candidate.reason != "Coincidencia semántica" || candidate.score >= 0.1625)
        });
        if has_relevant {
            let all_candidates = candidates;
            let mut relevant = all_candidates
                .iter()
                .filter(|candidate| {
                    candidate.score > 0.0
                        && (candidate.reason != "Coincidencia semántica"
                            || candidate.score >= 0.1625)
                })
                .cloned()
                .collect::<Vec<_>>();
            relevant.sort_by(|left, right| {
                right
                    .score
                    .total_cmp(&left.score)
                    .then_with(|| left.ordinal.cmp(&right.ordinal))
            });
            let mut expanded = relevant.clone();
            let mut included = relevant
                .iter()
                .map(|candidate| candidate.id.clone())
                .collect::<HashSet<_>>();
            for candidate in relevant {
                for neighbor_ordinal in [candidate.ordinal - 1, candidate.ordinal + 1] {
                    if let Some(neighbor) = all_candidates.iter().find(|other| {
                        other.attachment_id == candidate.attachment_id
                            && other.ordinal == neighbor_ordinal
                    }) {
                        if included.insert(neighbor.id.clone()) {
                            let mut neighbor = neighbor.clone();
                            neighbor.score = 0.0;
                            neighbor.reason = "Contexto próximo al fragmento relevante".to_owned();
                            expanded.push(neighbor);
                        }
                    }
                }
            }
            candidates = expanded;
        } else {
            candidates.sort_by(|left, right| {
                left.attachment_id
                    .cmp(&right.attachment_id)
                    .then_with(|| left.ordinal.cmp(&right.ordinal))
            });
        }
        let mut selected = Vec::new();
        let mut used_characters = 0_usize;
        for candidate in candidates {
            let candidate_characters = candidate.text.chars().count();
            if used_characters.saturating_add(candidate_characters) > character_budget {
                continue;
            }
            used_characters += candidate_characters;
            selected.push(candidate);
            if selected.len() == maximum_chunks {
                break;
            }
        }
        Ok(selected)
    }
}
