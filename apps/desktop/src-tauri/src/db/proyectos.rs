//! Proyectos: alta, listado, instrucciones y conocimiento asociado.

use super::*;

impl Database {
    pub fn create_project(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<ProjectSummary, AppError> {
        let id = format!("project_{}", Uuid::new_v4().simple());
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO projects(id, name, description) VALUES (?1, ?2, ?3)",
            params![id, name, description],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('project.created', 'user', ?1)",
            params![serde_json::json!({"project_id": id, "name": name}).to_string()],
        )?;
        transaction.commit()?;
        self.project_summary(&id)
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT p.id, p.name, p.description, p.instructions, COUNT(c.id), p.updated_at
             FROM projects p
             LEFT JOIN conversations c
               ON c.project_id = p.id
              AND c.archived_at IS NULL
              AND c.deleted_at IS NULL
             WHERE p.archived_at IS NULL
             GROUP BY p.id, p.name, p.description, p.instructions, p.updated_at
             ORDER BY p.updated_at DESC, p.name COLLATE NOCASE",
        )?;
        let projects = statement
            .query_map([], |row| {
                Ok(ProjectSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    instructions: row.get(3)?,
                    conversation_count: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(projects)
    }

    pub(super) fn project_summary(&self, id: &str) -> Result<ProjectSummary, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT p.id, p.name, p.description, p.instructions, COUNT(c.id), p.updated_at
                 FROM projects p
                 LEFT JOIN conversations c
                   ON c.project_id = p.id
                  AND c.archived_at IS NULL
                  AND c.deleted_at IS NULL
                 WHERE p.id = ?1 AND p.archived_at IS NULL
                 GROUP BY p.id, p.name, p.description, p.instructions, p.updated_at",
                params![id],
                |row| {
                    Ok(ProjectSummary {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        instructions: row.get(3)?,
                        conversation_count: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("proyecto {id}")))
    }

    pub fn rename_project(&self, id: &str, name: &str) -> Result<ProjectSummary, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE projects
             SET name = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL",
            params![id, name],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("proyecto {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('project.renamed', 'user', ?1)",
            params![serde_json::json!({"project_id": id, "name": name}).to_string()],
        )?;
        transaction.commit()?;
        self.project_summary(id)
    }

    pub fn update_project_instructions(
        &self,
        id: &str,
        instructions: Option<&str>,
    ) -> Result<ProjectSummary, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE projects
             SET instructions = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL",
            params![id, instructions],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("proyecto {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('project.instructions_updated', 'user', ?1)",
            params![serde_json::json!({
                "project_id": id,
                "enabled": instructions.is_some()
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.project_summary(id)
    }

    pub fn project_instruction_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Option<ProjectInstructionContext>, AppError> {
        self.connect()?
            .query_row(
                "SELECT project.id, project.name, project.instructions
                 FROM conversations conversation
                 JOIN projects project ON project.id = conversation.project_id
                 WHERE conversation.id = ?1
                   AND conversation.deleted_at IS NULL
                   AND project.archived_at IS NULL
                   AND project.instructions IS NOT NULL
                   AND trim(project.instructions) != ''",
                params![conversation_id],
                |row| {
                    Ok(ProjectInstructionContext {
                        project_id: row.get(0)?,
                        project_name: row.get(1)?,
                        instructions: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn project_knowledge_overview(
        &self,
        project_id: &str,
    ) -> Result<ProjectKnowledgeOverview, AppError> {
        let project = self.project_summary(project_id)?;
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT project_file.attachment_id
             FROM project_files project_file
             JOIN attachments attachment ON attachment.id = project_file.attachment_id
             WHERE project_file.project_id = ?1
             ORDER BY attachment.display_name COLLATE NOCASE, project_file.attachment_id",
        )?;
        let attachment_ids = statement
            .query_map(params![project_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut statement = connection.prepare(
            "SELECT link.attachment_id, conversation.id, conversation.title,
                    conversation.project_id, conversation.updated_at
             FROM conversation_attachments link
             JOIN conversations conversation ON conversation.id = link.conversation_id
             JOIN project_files project_file
               ON project_file.attachment_id = link.attachment_id
              AND project_file.project_id = conversation.project_id
             WHERE project_file.project_id = ?1
               AND conversation.archived_at IS NULL
               AND conversation.deleted_at IS NULL
             ORDER BY link.attachment_id, conversation.updated_at DESC, conversation.id",
        )?;
        let usage_rows = statement
            .query_map(params![project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ConversationSummary {
                        id: row.get(1)?,
                        title: row.get(2)?,
                        project_id: row.get(3)?,
                        updated_at: row.get(4)?,
                    },
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        drop(connection);
        let mut conversations_by_attachment: HashMap<String, Vec<ConversationSummary>> =
            HashMap::new();
        for (attachment_id, conversation) in usage_rows {
            conversations_by_attachment
                .entry(attachment_id)
                .or_default()
                .push(conversation);
        }
        let files = attachment_ids
            .iter()
            .map(|attachment_id| self.attachment_view(attachment_id))
            .collect::<Result<Vec<_>, _>>()?;
        let file_usages = attachment_ids
            .iter()
            .map(|attachment_id| ProjectFileUsageView {
                attachment_id: attachment_id.clone(),
                conversations: conversations_by_attachment
                    .remove(attachment_id)
                    .unwrap_or_default(),
            })
            .collect();
        let memory = self.memory_overview()?;
        let memories = memory
            .items
            .into_iter()
            .filter(|item| item.project_id.as_deref() == Some(project_id))
            .collect();
        Ok(ProjectKnowledgeOverview {
            project,
            files,
            file_usages,
            memories,
            memory_enabled: memory.enabled,
        })
    }

    pub fn remove_project_file(
        &self,
        project_id: &str,
        attachment_id: &str,
    ) -> Result<ProjectKnowledgeOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "DELETE FROM project_files
             WHERE project_id = ?1
               AND attachment_id = ?2
               AND EXISTS(
                   SELECT 1 FROM projects
                   WHERE id = ?1 AND archived_at IS NULL
               )",
            params![project_id, attachment_id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(
                "archivo reutilizable del proyecto".to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('project.file_removed', 'user', ?1)",
            params![serde_json::json!({
                "project_id": project_id,
                "attachment_id": attachment_id
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.project_knowledge_overview(project_id)
    }

    pub fn set_project_memory_item_enabled(
        &self,
        project_id: &str,
        memory_id: &str,
        enabled: bool,
    ) -> Result<ProjectKnowledgeOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE memory_items
             SET enabled = ?3, updated_at = datetime('now')
             WHERE id = ?2
               AND project_id = ?1
               AND custom_gpt_id IS NULL",
            params![project_id, memory_id, enabled],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(
                "recuerdo limitado al proyecto".to_owned(),
            ));
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
                serde_json::json!({
                    "memory_id": memory_id,
                    "project_id": project_id
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        self.project_knowledge_overview(project_id)
    }

    pub fn archive_project(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE projects
             SET archived_at = datetime('now'), updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("proyecto {id}")));
        }
        transaction.execute(
            "UPDATE conversations
             SET project_id = NULL, updated_at = datetime('now')
             WHERE project_id = ?1 AND deleted_at IS NULL",
            params![id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('project.archived', 'user', ?1)",
            params![serde_json::json!({"project_id": id}).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }
}
