//! Conversaciones: alta, busqueda, movimiento, archivado y contexto reciente.

use super::*;

impl Database {
    pub fn create_conversation(
        &self,
        title: &str,
        project_id: Option<&str>,
    ) -> Result<ConversationSummary, AppError> {
        let id = format!("conv_{}", Uuid::new_v4().simple());
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        if let Some(project_id) = project_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM projects
                    WHERE id = ?1 AND archived_at IS NULL
                 )",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        transaction.execute(
            "INSERT INTO conversations(id, project_id, title) VALUES (?1, ?2, ?3)",
            params![id, project_id, title],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.created', 'user', ?1, ?2)",
            params![
                id,
                serde_json::json!({"conversation_id": id, "project_id": project_id}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_summary(&id)
    }

    pub fn list_conversations(&self) -> Result<Vec<ConversationSummary>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, title, project_id, updated_at
             FROM conversations
             WHERE archived_at IS NULL AND deleted_at IS NULL
             ORDER BY updated_at DESC",
        )?;
        let conversations = statement
            .query_map([], |row| {
                Ok(ConversationSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    project_id: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(conversations)
    }

    pub fn search_conversations(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ConversationSummary>, AppError> {
        let connection = self.connect()?;
        let escaped = query
            .replace('!', "!!")
            .replace('%', "!%")
            .replace('_', "!_");
        let pattern = format!("%{escaped}%");
        let mut statement = connection.prepare(
            "SELECT c.id, c.title, c.project_id, c.updated_at
             FROM conversations c
             WHERE c.archived_at IS NULL
               AND c.deleted_at IS NULL
               AND (
                    c.title LIKE ?1 ESCAPE '!' COLLATE NOCASE
                    OR EXISTS(
                        SELECT 1
                        FROM messages m
                        JOIN message_parts mp ON mp.message_id = m.id
                        WHERE m.conversation_id = c.id
                          AND mp.content_text LIKE ?1 ESCAPE '!' COLLATE NOCASE
                    )
               )
             ORDER BY c.updated_at DESC
             LIMIT ?2",
        )?;
        let conversations = statement
            .query_map(params![pattern, limit as i64], |row| {
                Ok(ConversationSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    project_id: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(conversations)
    }

    pub(super) fn conversation_summary(&self, id: &str) -> Result<ConversationSummary, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT id, title, project_id, updated_at
                 FROM conversations
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |row| {
                    Ok(ConversationSummary {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        project_id: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("conversación {id}")))
    }

    pub fn rename_conversation(
        &self,
        id: &str,
        title: &str,
    ) -> Result<ConversationSummary, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE conversations
             SET title = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id, title],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("conversación {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.renamed', 'user', ?1, ?2)",
            params![
                id,
                serde_json::json!({"conversation_id": id, "title": title}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_summary(id)
    }

    pub fn move_conversation(
        &self,
        id: &str,
        project_id: Option<&str>,
    ) -> Result<ConversationSummary, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        if let Some(project_id) = project_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM projects
                    WHERE id = ?1 AND archived_at IS NULL
                 )",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        let changed = transaction.execute(
            "UPDATE conversations
             SET project_id = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL",
            params![id, project_id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("conversación activa {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.moved', 'user', ?1, ?2)",
            params![
                id,
                serde_json::json!({"conversation_id": id, "project_id": project_id}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_summary(id)
    }

    pub fn set_conversation_custom_gpt(
        &self,
        id: &str,
        custom_gpt_id: Option<&str>,
    ) -> Result<ConversationView, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        if let Some(custom_gpt_id) = custom_gpt_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM custom_gpts
                    WHERE id = ?1 AND archived_at IS NULL AND active_version_id IS NOT NULL
                 )",
                params![custom_gpt_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("GPT personal {custom_gpt_id}")));
            }
        }
        let changed = transaction.execute(
            "UPDATE conversations
             SET custom_gpt_id = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL",
            params![id, custom_gpt_id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("conversación activa {id}")));
        }
        // El proyecto predeterminado del GPT solo se aplica a una conversación que
        // todavía no pertenece a ninguno: nunca mueve un chat ya clasificado.
        if let Some(custom_gpt_id) = custom_gpt_id {
            let adopted = transaction.execute(
                "UPDATE conversations
                 SET project_id = (
                       SELECT gpt.default_project_id FROM custom_gpts gpt
                       WHERE gpt.id = ?2 AND gpt.default_project_id IS NOT NULL
                     ),
                     updated_at = datetime('now')
                 WHERE id = ?1 AND project_id IS NULL
                   AND EXISTS(
                     SELECT 1 FROM custom_gpts gpt
                     WHERE gpt.id = ?2 AND gpt.default_project_id IS NOT NULL
                   )",
                params![id, custom_gpt_id],
            )?;
            if adopted > 0 {
                transaction.execute(
                    "INSERT INTO audit_events(
                        event_type, actor, conversation_id, payload_json
                     ) VALUES ('conversation.default_project_applied', 'user', ?1, ?2)",
                    params![
                        id,
                        serde_json::json!({"custom_gpt_id": custom_gpt_id}).to_string()
                    ],
                )?;
            }
        }
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.custom_gpt_updated', 'user', ?1, ?2)",
            params![
                id,
                serde_json::json!({
                    "conversation_id": id,
                    "custom_gpt_id": custom_gpt_id
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_view(id)
    }

    pub(super) fn ensure_conversation_can_hide(
        transaction: &rusqlite::Transaction<'_>,
        id: &str,
    ) -> Result<(), AppError> {
        let active_tasks: i64 = transaction.query_row(
            "SELECT COUNT(*)
             FROM broker_tasks
             WHERE conversation_id = ?1
               AND local_state NOT IN ('terminal', 'orphaned')",
            params![id],
            |row| row.get(0),
        )?;
        if active_tasks > 0 {
            return Err(AppError::Conflict(
                "la conversación tiene una tarea en curso; cancélala o espera a que termine"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    pub fn archive_conversation(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        Self::ensure_conversation_can_hide(&transaction, id)?;
        let changed = transaction.execute(
            "UPDATE conversations
             SET archived_at = datetime('now'), updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("conversación activa {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.archived', 'user', ?1, ?2)",
            params![id, serde_json::json!({"conversation_id": id}).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn delete_conversation(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        Self::ensure_conversation_can_hide(&transaction, id)?;
        let changed = transaction.execute(
            "UPDATE conversations
             SET deleted_at = datetime('now'), updated_at = datetime('now')
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("conversación {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.deleted', 'user', ?1, ?2)",
            params![id, serde_json::json!({"conversation_id": id}).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn recent_context(
        &self,
        conversation_id: &str,
        message_limit: usize,
        character_limit: usize,
    ) -> Result<Vec<ContextMessage>, AppError> {
        let connection = self.connect()?;
        let approved_summary: Option<(String, String, i64)> = connection
            .query_row(
                "SELECT id, approved_text, source_through_sequence
                 FROM conversation_summaries
                 WHERE conversation_id = ?1 AND status = 'approved'
                 LIMIT 1",
                params![conversation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let source_through_sequence = approved_summary
            .as_ref()
            .map(|(_, _, sequence)| *sequence)
            .unwrap_or(0);
        let summary_characters = approved_summary
            .as_ref()
            .map(|(_, text, _)| text.chars().count())
            .unwrap_or(0);
        let message_character_limit = character_limit.saturating_sub(summary_characters);
        let mut statement = connection.prepare(
            "SELECT m.id, m.role, mp.content_text
             FROM messages m
             JOIN message_parts mp ON mp.message_id = m.id AND mp.ordinal = 0
             WHERE m.conversation_id = ?1
               AND m.status = 'complete'
               AND m.role IN ('user', 'assistant')
               AND mp.kind IN ('text', 'markdown')
               AND m.sequence_no > ?3
             ORDER BY m.sequence_no DESC
             LIMIT ?2",
        )?;
        let mut newest_first = statement
            .query_map(
                params![
                    conversation_id,
                    message_limit as i64,
                    source_through_sequence
                ],
                |row| {
                    Ok(ContextMessage {
                        message_id: row.get(0)?,
                        role: row.get(1)?,
                        text: row.get(2)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        newest_first.reverse();

        let mut selected = Vec::new();
        let mut used = 0_usize;
        for message in newest_first.into_iter().rev() {
            let remaining = message_character_limit.saturating_sub(used);
            if remaining == 0 {
                break;
            }
            let mut message = message;
            if message.text.chars().count() > remaining {
                message.text = message
                    .text
                    .chars()
                    .rev()
                    .take(remaining)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
            }
            used += message.text.chars().count();
            selected.push(message);
        }
        selected.reverse();
        if let Some((summary_id, summary_text, _)) = approved_summary {
            selected.insert(
                0,
                ContextMessage {
                    message_id: summary_id,
                    role: "summary".to_owned(),
                    text: summary_text,
                },
            );
        }
        Ok(selected)
    }

    pub fn conversation_summary_input(
        &self,
        conversation_id: &str,
        character_budget: usize,
    ) -> Result<ConversationSummaryInput, AppError> {
        let connection = self.connect()?;
        let approved_summary: Option<(String, String, i64)> = connection
            .query_row(
                "SELECT id, approved_text, source_through_sequence
                 FROM conversation_summaries
                 WHERE conversation_id = ?1 AND status = 'approved'
                 LIMIT 1",
                params![conversation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let previous_source_through_sequence = approved_summary
            .as_ref()
            .map(|(_, _, sequence)| *sequence)
            .unwrap_or(0);
        let mut statement = connection.prepare(
            "SELECT m.id, m.role, mp.content_text, m.sequence_no
             FROM messages m
             JOIN message_parts mp ON mp.message_id = m.id AND mp.ordinal = 0
             WHERE m.conversation_id = ?1
               AND m.status = 'complete'
               AND m.role IN ('user', 'assistant')
               AND mp.kind IN ('text', 'markdown')
               AND m.sequence_no > ?2
             ORDER BY m.sequence_no",
        )?;
        let rows = statement
            .query_map(
                params![conversation_id, previous_source_through_sequence],
                |row| {
                    Ok((
                        ContextMessage {
                            message_id: row.get(0)?,
                            role: row.get(1)?,
                            text: row.get(2)?,
                        },
                        row.get::<_, i64>(3)?,
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;

        let total_message_count = rows.len() as i64;
        let mut messages = Vec::new();
        let mut character_count = 0_usize;
        let mut source_through_sequence = previous_source_through_sequence;
        if let Some((summary_id, summary_text, _)) = approved_summary {
            character_count = summary_text.chars().count();
            messages.push(ContextMessage {
                message_id: summary_id,
                role: "summary".to_owned(),
                text: summary_text,
            });
        }
        let base_context_count = messages.len();
        for (message, sequence) in rows {
            let message_characters = message.text.chars().count();
            if character_count.saturating_add(message_characters) > character_budget {
                break;
            }
            character_count += message_characters;
            source_through_sequence = sequence;
            messages.push(message);
        }
        let included_message_count = (messages.len() - base_context_count) as i64;
        Ok(ConversationSummaryInput {
            messages,
            source_through_sequence,
            included_message_count,
            remaining_message_count: total_message_count - included_message_count,
            character_count,
        })
    }
}
