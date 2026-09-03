//! Memoria del usuario y del GPT: alta, edicion, indice y busqueda.
//!
//! La memoria es opcional y con ambito: nada entra en un turno sin que el
//! usuario lo haya activado, y el indice se invalida al editar el texto.

use super::*;

impl Database {
    pub fn memory_overview(&self) -> Result<MemoryOverview, AppError> {
        let connection = self.connect()?;
        let enabled = connection.query_row(
            "SELECT enabled FROM feature_flags WHERE key = 'memory'",
            [],
            |row| row.get(0),
        )?;
        let mut statement = connection.prepare(
            "SELECT m.id, m.project_id, p.name, m.category, m.content,
                    m.sensitivity, m.enabled, m.created_at, m.updated_at,
                    CASE
                      WHEN er.id IS NOT NULL THEN 'ready'
                       WHEN EXISTS(
                         SELECT 1 FROM broker_tasks bt
                         WHERE json_extract(bt.request_json, '$.content.metadata.source_type') = 'memory'
                           AND json_extract(bt.request_json, '$.content.metadata.source_id') = m.id
                           AND json_extract(bt.request_json, '$.content.prompt') = m.content
                           AND bt.local_state NOT IN ('terminal', 'orphaned')
                       ) THEN 'indexing'
                       WHEN EXISTS(
                         SELECT 1 FROM broker_tasks bt
                         WHERE json_extract(bt.request_json, '$.content.metadata.source_type') = 'memory'
                           AND json_extract(bt.request_json, '$.content.metadata.source_id') = m.id
                           AND json_extract(bt.request_json, '$.content.prompt') = m.content
                           AND (bt.remote_status = 'failed' OR bt.local_state = 'orphaned')
                       ) THEN 'failed'
                      ELSE 'missing'
                    END,
                    er.model,
                    (
                      SELECT substr(json_extract(failed.error_json, '$.message'), 1, 500)
                       FROM broker_tasks failed
                       WHERE json_extract(failed.request_json, '$.content.metadata.source_type') = 'memory'
                         AND json_extract(failed.request_json, '$.content.metadata.source_id') = m.id
                         AND json_extract(failed.request_json, '$.content.prompt') = m.content
                         AND failed.error_json IS NOT NULL
                      ORDER BY failed.updated_at DESC, failed.rowid DESC LIMIT 1
                    )
             FROM memory_items m
             LEFT JOIN projects p ON p.id = m.project_id
             LEFT JOIN embedding_records er ON er.id = (
                SELECT candidate.id FROM embedding_records candidate
                WHERE candidate.source_type = 'memory' AND candidate.source_id = m.id
                ORDER BY candidate.created_at DESC, candidate.rowid DESC LIMIT 1
             )
             WHERE m.custom_gpt_id IS NULL
             ORDER BY m.updated_at DESC, m.id DESC",
        )?;
        let items = statement
            .query_map([], |row| {
                Ok(MemoryItemView {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    project_name: row.get(2)?,
                    custom_gpt_id: None,
                    custom_gpt_name: None,
                    category: row.get(3)?,
                    content: row.get(4)?,
                    sensitivity: row.get(5)?,
                    enabled: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    embedding_status: row.get(9)?,
                    embedding_model: row.get(10)?,
                    embedding_error: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MemoryOverview { enabled, items })
    }

    pub fn custom_gpt_knowledge(
        &self,
        custom_gpt_id: &str,
    ) -> Result<Vec<MemoryItemView>, AppError> {
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
            "SELECT m.id, m.custom_gpt_id, g.name, m.category, m.content,
                    m.sensitivity, m.enabled, m.created_at, m.updated_at,
                    CASE
                      WHEN er.id IS NOT NULL THEN 'ready'
                      WHEN EXISTS(
                        SELECT 1 FROM broker_tasks bt
                        WHERE json_extract(bt.request_json, '$.content.metadata.source_type') = 'memory'
                          AND json_extract(bt.request_json, '$.content.metadata.source_id') = m.id
                          AND json_extract(bt.request_json, '$.content.prompt') = m.content
                          AND bt.local_state NOT IN ('terminal', 'orphaned')
                      ) THEN 'indexing'
                      WHEN EXISTS(
                        SELECT 1 FROM broker_tasks bt
                        WHERE json_extract(bt.request_json, '$.content.metadata.source_type') = 'memory'
                          AND json_extract(bt.request_json, '$.content.metadata.source_id') = m.id
                          AND json_extract(bt.request_json, '$.content.prompt') = m.content
                          AND (bt.remote_status = 'failed' OR bt.local_state = 'orphaned')
                      ) THEN 'failed'
                      ELSE 'missing'
                    END,
                    er.model,
                    (
                      SELECT substr(json_extract(failed.error_json, '$.message'), 1, 500)
                      FROM broker_tasks failed
                      WHERE json_extract(failed.request_json, '$.content.metadata.source_type') = 'memory'
                        AND json_extract(failed.request_json, '$.content.metadata.source_id') = m.id
                        AND json_extract(failed.request_json, '$.content.prompt') = m.content
                        AND failed.error_json IS NOT NULL
                      ORDER BY failed.updated_at DESC, failed.rowid DESC LIMIT 1
                    )
             FROM memory_items m
             JOIN custom_gpts g ON g.id = m.custom_gpt_id
             LEFT JOIN embedding_records er ON er.id = (
                SELECT candidate.id FROM embedding_records candidate
                WHERE candidate.source_type = 'memory' AND candidate.source_id = m.id
                ORDER BY candidate.created_at DESC, candidate.rowid DESC LIMIT 1
             )
             WHERE m.custom_gpt_id = ?1
             ORDER BY m.updated_at DESC, m.id DESC",
        )?;
        let items = statement
            .query_map(params![custom_gpt_id], |row| {
                Ok(MemoryItemView {
                    id: row.get(0)?,
                    project_id: None,
                    project_name: None,
                    custom_gpt_id: row.get(1)?,
                    custom_gpt_name: row.get(2)?,
                    category: row.get(3)?,
                    content: row.get(4)?,
                    sensitivity: row.get(5)?,
                    enabled: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    embedding_status: row.get(9)?,
                    embedding_model: row.get(10)?,
                    embedding_error: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        Ok(items)
    }

    pub fn create_custom_gpt_memory_item(
        &self,
        custom_gpt_id: &str,
        content: &str,
        category: &str,
        sensitivity: &str,
    ) -> Result<(String, Vec<MemoryItemView>), AppError> {
        let id = format!("memory_{}", Uuid::new_v4().simple());
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM custom_gpts WHERE id = ?1 AND archived_at IS NULL
             )",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::NotFound(format!("GPT personal {custom_gpt_id}")));
        }
        transaction.execute(
            "INSERT INTO memory_items(
                id, custom_gpt_id, category, content, sensitivity,
                enabled, provenance_type
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 'manual')",
            params![id, custom_gpt_id, category, content, sensitivity],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.knowledge_created', 'user', ?1)",
            params![serde_json::json!({
                "memory_id": id,
                "custom_gpt_id": custom_gpt_id,
                "category": category,
                "sensitivity": sensitivity
            })
            .to_string()],
        )?;
        transaction.commit()?;
        Ok((id, self.custom_gpt_knowledge(custom_gpt_id)?))
    }

    pub fn set_custom_gpt_memory_item_enabled(
        &self,
        custom_gpt_id: &str,
        id: &str,
        enabled: bool,
    ) -> Result<Vec<MemoryItemView>, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE memory_items
             SET enabled = ?3, updated_at = datetime('now')
             WHERE id = ?1 AND custom_gpt_id = ?2",
            params![id, custom_gpt_id, enabled],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!(
                "conocimiento {id} del GPT personal"
            )));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES (?1, 'user', ?2)",
            params![
                if enabled {
                    "custom_gpt.knowledge_enabled"
                } else {
                    "custom_gpt.knowledge_disabled"
                },
                serde_json::json!({
                    "memory_id": id,
                    "custom_gpt_id": custom_gpt_id
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        self.custom_gpt_knowledge(custom_gpt_id)
    }

    pub fn delete_custom_gpt_memory_item(
        &self,
        custom_gpt_id: &str,
        id: &str,
    ) -> Result<Vec<MemoryItemView>, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let owned: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM memory_items WHERE id = ?1 AND custom_gpt_id = ?2
             )",
            params![id, custom_gpt_id],
            |row| row.get(0),
        )?;
        if !owned {
            return Err(AppError::NotFound(format!(
                "conocimiento {id} del GPT personal"
            )));
        }
        transaction.execute(
            "DELETE FROM embedding_records WHERE source_type = 'memory' AND source_id = ?1",
            params![id],
        )?;
        transaction.execute(
            "DELETE FROM memory_items WHERE id = ?1 AND custom_gpt_id = ?2",
            params![id, custom_gpt_id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.knowledge_deleted', 'user', ?1)",
            params![serde_json::json!({
                "memory_id": id,
                "custom_gpt_id": custom_gpt_id
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.custom_gpt_knowledge(custom_gpt_id)
    }

    pub fn custom_gpt_memory_item(
        &self,
        custom_gpt_id: &str,
        id: &str,
    ) -> Result<MemoryItemView, AppError> {
        self.custom_gpt_knowledge(custom_gpt_id)?
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| AppError::NotFound(format!("conocimiento {id} del GPT personal")))
    }

    pub fn clear_custom_gpt_memory_embedding(
        &self,
        custom_gpt_id: &str,
        id: &str,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let owned: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM memory_items WHERE id = ?1 AND custom_gpt_id = ?2
             )",
            params![id, custom_gpt_id],
            |row| row.get(0),
        )?;
        if !owned {
            return Err(AppError::NotFound(format!(
                "conocimiento {id} del GPT personal"
            )));
        }
        connection.execute(
            "DELETE FROM embedding_records WHERE source_type = 'memory' AND source_id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn set_memory_enabled(&self, enabled: bool) -> Result<MemoryOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE feature_flags
             SET enabled = ?1, updated_at = datetime('now')
             WHERE key = 'memory'",
            params![enabled],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES (?1, 'user', '{}')",
            params![if enabled {
                "memory.enabled"
            } else {
                "memory.disabled"
            }],
        )?;
        transaction.commit()?;
        self.memory_overview()
    }

    pub fn create_memory_item(
        &self,
        content: &str,
        category: &str,
        sensitivity: &str,
        project_id: Option<&str>,
    ) -> Result<(String, MemoryOverview), AppError> {
        let id = format!("memory_{}", Uuid::new_v4().simple());
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        if let Some(project_id) = project_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND archived_at IS NULL)",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        transaction.execute(
            "INSERT INTO memory_items(
                id, project_id, category, content, sensitivity,
                enabled, provenance_type
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 'manual')",
            params![id, project_id, category, content, sensitivity],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('memory.created', 'user', ?1)",
            params![serde_json::json!({
                "memory_id": id,
                "category": category,
                "sensitivity": sensitivity,
                "project_id": project_id
            })
            .to_string()],
        )?;
        transaction.commit()?;
        Ok((id, self.memory_overview()?))
    }

    pub fn update_memory_item(
        &self,
        id: &str,
        content: &str,
        category: &str,
        sensitivity: &str,
        project_id: Option<&str>,
    ) -> Result<(bool, MemoryOverview), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let current = transaction
            .query_row(
                "SELECT content, category, sensitivity, project_id
                 FROM memory_items
                 WHERE id = ?1 AND custom_gpt_id IS NULL",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("recuerdo {id}")))?;
        if let Some(project_id) = project_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND archived_at IS NULL)",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        let content_changed = current.0 != content;
        let unchanged = !content_changed
            && current.1 == category
            && current.2 == sensitivity
            && current.3.as_deref() == project_id;
        if unchanged {
            transaction.commit()?;
            return Ok((false, self.memory_overview()?));
        }
        transaction.execute(
            "UPDATE memory_items
             SET content = ?2,
                 category = ?3,
                 sensitivity = ?4,
                 project_id = ?5,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id, content, category, sensitivity, project_id],
        )?;
        if content_changed {
            transaction.execute(
                "DELETE FROM embedding_records
                 WHERE source_type = 'memory' AND source_id = ?1",
                params![id],
            )?;
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('memory.updated', 'user', ?1)",
            params![serde_json::json!({
                "memory_id": id,
                "category": category,
                "sensitivity": sensitivity,
                "project_id": project_id,
                "content_changed": content_changed
            })
            .to_string()],
        )?;
        transaction.commit()?;
        Ok((content_changed, self.memory_overview()?))
    }

    pub fn set_memory_item_enabled(
        &self,
        id: &str,
        enabled: bool,
    ) -> Result<MemoryOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE memory_items
             SET enabled = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND custom_gpt_id IS NULL",
            params![id, enabled],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("recuerdo {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES (?1, 'user', ?2)",
            params![
                if enabled {
                    "memory.item_enabled"
                } else {
                    "memory.item_disabled"
                },
                serde_json::json!({"memory_id": id}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.memory_overview()
    }

    pub fn delete_memory_item(&self, id: &str) -> Result<MemoryOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM embedding_records WHERE source_type = 'memory' AND source_id = ?1",
            params![id],
        )?;
        let changed = transaction.execute(
            "DELETE FROM memory_items WHERE id = ?1 AND custom_gpt_id IS NULL",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("recuerdo {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('memory.deleted', 'user', ?1)",
            params![serde_json::json!({"memory_id": id}).to_string()],
        )?;
        transaction.commit()?;
        self.memory_overview()
    }

    pub fn memory_item(&self, id: &str) -> Result<MemoryItemView, AppError> {
        self.memory_overview()?
            .items
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| AppError::NotFound(format!("recuerdo {id}")))
    }

    pub fn clear_memory_embedding(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        connection.execute(
            "DELETE FROM embedding_records
             WHERE source_type = 'memory' AND source_id = ?1
               AND EXISTS(
                 SELECT 1 FROM memory_items
                 WHERE id = ?1 AND custom_gpt_id IS NULL
               )",
            params![id],
        )?;
        Ok(())
    }

    pub fn prepare_memory_search(
        &self,
        search_id: &str,
        query: &str,
        project_id: Option<&str>,
        task_id: &str,
        idempotency_key: &str,
        request: &Value,
    ) -> Result<BrokerTaskRecord, AppError> {
        let request_json = serde_json::to_string(request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        if let Some(project_id) = project_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND archived_at IS NULL)",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        transaction.execute(
            "INSERT INTO broker_tasks(
                id, idempotency_key, request_json, remote_status, local_state
             ) VALUES (?1, ?2, ?3, 'not_submitted', 'created')",
            params![task_id, idempotency_key, request_json],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'local.prepared', 'not_submitted', '{}', datetime('now'))",
            params![task_id],
        )?;
        transaction.execute(
            "INSERT INTO memory_searches(id, query_text, project_id, broker_task_id)
             VALUES (?1, ?2, ?3, ?4)",
            params![search_id, query, project_id, task_id],
        )?;
        transaction.commit()?;
        self.task_record(task_id)
    }

    pub fn memory_search(&self, id: &str) -> Result<MemorySearchView, AppError> {
        let connection = self.connect()?;
        let record = connection
            .query_row(
                "SELECT ms.query_text, ms.project_id, bt.remote_status, bt.local_state,
                        bt.error_json, er.model, er.dimensions, er.vector_blob, ms.created_at
                 FROM memory_searches ms
                 JOIN broker_tasks bt ON bt.id = ms.broker_task_id
                 LEFT JOIN embedding_records er ON er.id = (
                    SELECT candidate.id FROM embedding_records candidate
                    WHERE candidate.source_type = 'memory_search'
                      AND candidate.source_id = ms.id
                    ORDER BY candidate.created_at DESC, candidate.rowid DESC LIMIT 1
                 )
                 WHERE ms.id = ?1",
                params![id],
                |row| {
                    Ok(MemorySearchRecord {
                        query: row.get(0)?,
                        project_id: row.get(1)?,
                        remote_status: row.get(2)?,
                        local_state: row.get(3)?,
                        error_json: row.get(4)?,
                        model: row.get(5)?,
                        dimensions: row.get(6)?,
                        blob: row.get(7)?,
                        created_at: row.get(8)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("búsqueda de memoria {id}")))?;

        let error = record
            .error_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
        let status = if record.blob.is_some() {
            "completed"
        } else if record.remote_status == "failed"
            || record.local_state == "orphaned"
            || record.remote_status == "completed"
        {
            "failed"
        } else {
            "searching"
        };
        let mut results = Vec::new();
        if let (Some(model_name), Some(dimensions), Some(search_blob)) = (
            record.model.as_deref(),
            record.dimensions,
            record.blob.as_deref(),
        ) {
            let search_vector = decode_embedding(search_blob, dimensions)?;
            let mut statement = connection.prepare(
                "SELECT m.id, m.content, m.category, p.name, m.sensitivity,
                        er.dimensions, er.vector_blob
                 FROM memory_items m
                 JOIN embedding_records er
                   ON er.source_type = 'memory' AND er.source_id = m.id
                  AND er.model = ?1 AND er.dimensions = ?2
                 LEFT JOIN projects p ON p.id = m.project_id
                 WHERE m.enabled = 1 AND m.custom_gpt_id IS NULL
                   AND (m.project_id IS NULL OR (?3 IS NOT NULL AND m.project_id = ?3))
                 ORDER BY m.updated_at DESC",
            )?;
            let candidates = statement
                .query_map(params![model_name, dimensions, record.project_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, Vec<u8>>(6)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (memory_id, content, category, project_name, sensitivity, dims, candidate_blob) in
                candidates
            {
                let candidate = decode_embedding(&candidate_blob, dims)?;
                let score = cosine_similarity(&search_vector, &candidate);
                if score.is_finite() && score >= 0.25 {
                    let reason = if score >= 0.75 {
                        "Coincidencia semántica alta"
                    } else if score >= 0.5 {
                        "Coincidencia semántica media"
                    } else {
                        "Coincidencia semántica baja"
                    };
                    results.push(MemorySearchResultView {
                        memory_id,
                        content,
                        category,
                        project_name,
                        sensitivity,
                        score: (score * 1000.0).round() / 1000.0,
                        reason: reason.to_owned(),
                    });
                }
            }
            results.sort_by(|left, right| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            results.truncate(5);
        }
        Ok(MemorySearchView {
            id: id.to_owned(),
            query: record.query,
            project_id: record.project_id,
            status: status.to_owned(),
            model: record.model,
            error: error.or_else(|| {
                (record.remote_status == "completed" && status == "failed").then(|| {
                    "Broker AI completó la tarea sin devolver un vector utilizable".to_owned()
                })
            }),
            results,
            created_at: record.created_at,
        })
    }

    pub fn latest_memory_search(&self) -> Result<Option<MemorySearchView>, AppError> {
        let id = self
            .connect()?
            .query_row(
                "SELECT id FROM memory_searches ORDER BY created_at DESC, rowid DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        id.map(|id| self.memory_search(&id)).transpose()
    }

    #[allow(dead_code)]
    pub fn active_memories_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<MemoryItemView>, AppError> {
        self.active_memories_for_conversation_with_limits(conversation_id, 20, 8_000)
    }

    pub fn active_memories_for_conversation_with_limits(
        &self,
        conversation_id: &str,
        maximum_items: usize,
        character_budget: usize,
    ) -> Result<Vec<MemoryItemView>, AppError> {
        let overview = self.memory_overview()?;
        let (project_id, custom_gpt_id): (Option<String>, Option<String>) =
            self.connect()?.query_row(
                "SELECT project_id, custom_gpt_id FROM conversations
             WHERE id = ?1 AND deleted_at IS NULL",
                params![conversation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
        let mut candidates = if overview.enabled {
            overview
                .items
                .into_iter()
                .filter(|item| item.enabled)
                .filter(|item| item.project_id.is_none() || item.project_id == project_id)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if let Some(custom_gpt_id) = custom_gpt_id {
            let mut scoped = self
                .custom_gpt_knowledge(&custom_gpt_id)?
                .into_iter()
                .filter(|item| item.enabled)
                .collect::<Vec<_>>();
            scoped.append(&mut candidates);
            candidates = scoped;
        }
        let mut total_chars = 0_usize;
        Ok(candidates
            .into_iter()
            .filter(|item| {
                total_chars += item.content.chars().count();
                total_chars <= character_budget
            })
            .take(maximum_items)
            .collect())
    }
}
