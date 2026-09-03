//! Que conocimiento entra en un nodo de workflow: adjuntos, memoria e instrucciones.

use super::*;

impl Database {
    pub fn ready_workflow_attachments(
        &self,
        workflow_id: &str,
        attachment_ids: &[String],
    ) -> Result<Vec<AttachmentRecord>, AppError> {
        if attachment_ids.len() > 20 {
            return Err(AppError::Validation(
                "cada nodo admite como máximo 20 archivos".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let project_id: Option<String> = connection
            .query_row(
                "SELECT project_id FROM workflows WHERE id = ?1 AND archived_at IS NULL",
                params![workflow_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("flujo {workflow_id}")))?;
        if !attachment_ids.is_empty() && project_id.is_none() {
            return Err(AppError::Conflict(
                "asocia el flujo a un proyecto para asignarle archivos".to_owned(),
            ));
        }
        let mut records = Vec::with_capacity(attachment_ids.len());
        for attachment_id in attachment_ids {
            let allowed: bool = connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM project_files
                    WHERE project_id = ?1 AND attachment_id = ?2
                 )",
                params![project_id, attachment_id],
                |row| row.get(0),
            )?;
            if !allowed {
                return Err(AppError::Conflict(format!(
                    "el archivo {attachment_id} no pertenece al proyecto del flujo"
                )));
            }
            let record = self.attachment_record(attachment_id)?;
            if record.ingestion_status != "ready" || record.broker_file_id.is_none() {
                return Err(AppError::Conflict(format!(
                    "el archivo {} todavía no está preparado",
                    record.display_name
                )));
            }
            records.push(record);
        }
        Ok(records)
    }

    /// Resuelve únicamente los archivos que siguen perteneciendo al GPT.
    ///
    /// La versión publicada conserva los identificadores que estaban activos,
    /// pero retirar después un archivo es una revocación efectiva: no se envía
    /// gracias a una copia histórica ni queda pegado al flujo.
    pub fn ready_custom_gpt_attachments_for_workflow(
        &self,
        custom_gpt_id: &str,
        attachment_ids: &[String],
    ) -> Result<Vec<AttachmentRecord>, AppError> {
        if attachment_ids.len() > 20 {
            return Err(AppError::Validation(
                "cada GPT de un flujo admite como máximo 20 archivos propios".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let mut records = Vec::with_capacity(attachment_ids.len());
        for attachment_id in attachment_ids {
            let still_linked: bool = connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM custom_gpt_files
                    WHERE custom_gpt_id = ?1 AND attachment_id = ?2
                 )",
                params![custom_gpt_id, attachment_id],
                |row| row.get(0),
            )?;
            if !still_linked {
                continue;
            }
            let record = self.attachment_record(attachment_id)?;
            if record.ingestion_status != "ready" || record.broker_file_id.is_none() {
                return Err(AppError::Conflict(format!(
                    "el archivo {} del GPT todavía no está preparado",
                    record.display_name
                )));
            }
            records.push(record);
        }
        Ok(records)
    }

    /// Recupera el conocimiento que estaba seleccionado al publicar y que aún
    /// continúa habilitado. El orden congelado se conserva y los elementos
    /// revocados simplemente dejan de formar parte del contexto.
    pub fn custom_gpt_memories_for_workflow(
        &self,
        custom_gpt_id: &str,
        memory_ids: &[String],
    ) -> Result<Vec<MemoryItemView>, AppError> {
        if memory_ids.len() > 20 {
            return Err(AppError::Validation(
                "cada GPT de un flujo admite como máximo 20 elementos de conocimiento".to_owned(),
            ));
        }
        let available = self
            .custom_gpt_knowledge(custom_gpt_id)?
            .into_iter()
            .filter(|item| item.enabled)
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();
        let mut used_characters = 0_usize;
        Ok(memory_ids
            .iter()
            .filter_map(|id| available.get(id))
            .filter(|item| {
                used_characters += item.content.chars().count();
                used_characters <= 8_000
            })
            .cloned()
            .collect())
    }

    /// Devuelve las instrucciones solo mientras el proyecto siga activo y el
    /// texto publicado continúe siendo el autorizado actualmente. Editarlas o
    /// retirarlas revoca versiones antiguas hasta que el flujo se publique otra vez.
    pub fn project_instruction_for_workflow(
        &self,
        context: &WorkflowProjectContext,
    ) -> Result<Option<ProjectInstructionContext>, AppError> {
        let current = self
            .connect()?
            .query_row(
                "SELECT name, instructions FROM projects
                 WHERE id = ?1 AND archived_at IS NULL",
                params![context.project_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let Some((project_name, instructions)) = current else {
            return Ok(None);
        };
        let current = instructions.filter(|value| !value.trim().is_empty());
        if current != context.instructions {
            return Ok(None);
        }
        Ok(current.map(|instructions| ProjectInstructionContext {
            project_id: context.project_id.clone(),
            project_name,
            instructions,
        }))
    }

    /// Resuelve los recuerdos del proyecto que estaban autorizados al publicar
    /// y que continúan activos. La memoria global apagada también los revoca.
    pub fn project_memories_for_workflow(
        &self,
        context: &WorkflowProjectContext,
    ) -> Result<Vec<MemoryItemView>, AppError> {
        if context.memory_ids.len() > 20 {
            return Err(AppError::Validation(
                "cada flujo admite como máximo 20 recuerdos del proyecto".to_owned(),
            ));
        }
        let active: bool = self.connect()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND archived_at IS NULL)",
            params![context.project_id],
            |row| row.get(0),
        )?;
        let overview = self.memory_overview()?;
        if !active || !overview.enabled {
            return Ok(Vec::new());
        }
        let available = overview
            .items
            .into_iter()
            .filter(|item| {
                item.enabled && item.project_id.as_deref() == Some(context.project_id.as_str())
            })
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();
        let mut used_characters = 0_usize;
        Ok(context
            .memory_ids
            .iter()
            .filter_map(|id| available.get(id))
            .filter(|item| {
                used_characters += item.content.chars().count();
                used_characters <= 8_000
            })
            .cloned()
            .collect())
    }
}
