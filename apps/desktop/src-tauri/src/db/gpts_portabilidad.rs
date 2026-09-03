//! Exportacion e importacion portable de un GPT, y su contexto en un chat.

use super::*;

impl Database {
    pub(super) fn custom_gpt_view(&self, custom_gpt_id: &str) -> Result<CustomGptView, AppError> {
        self.list_custom_gpts()?
            .into_iter()
            .find(|item| item.id == custom_gpt_id)
            .ok_or_else(|| AppError::NotFound(format!("GPT personal {custom_gpt_id}")))
    }

    #[cfg(test)]
    pub fn export_custom_gpt_json(&self, custom_gpt_id: &str) -> Result<String, AppError> {
        Ok(self.export_custom_gpt_portable(custom_gpt_id, false)?.json)
    }

    pub fn export_custom_gpt_portable(
        &self,
        custom_gpt_id: &str,
        include_knowledge: bool,
    ) -> Result<CustomGptPortableExport, AppError> {
        let view = self.custom_gpt_view(custom_gpt_id)?;
        let knowledge_items = self.custom_gpt_knowledge(custom_gpt_id)?;
        let included = if include_knowledge {
            knowledge_items
                .iter()
                .filter(|item| item.enabled && item.sensitivity == "normal")
                .map(|item| PortableCustomGptKnowledge {
                    category: item.category.clone(),
                    content: item.content.clone(),
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let included_knowledge = included.len();
        let excluded_sensitive = knowledge_items
            .iter()
            .filter(|item| item.sensitivity == "sensitive")
            .count();
        let excluded_disabled = knowledge_items
            .iter()
            .filter(|item| !item.enabled && item.sensitivity != "sensitive")
            .count();
        let excluded_files = self.list_custom_gpt_files(custom_gpt_id)?.len();
        let json = serde_json::to_string_pretty(&PortableCustomGpt {
            schema_version: if include_knowledge { 2 } else { 1 },
            name: view.name,
            description: view.description,
            icon_ref: view.icon_ref,
            instructions: view.instructions,
            conversation_starters: view.conversation_starters,
            context_profile: view.context_profile,
            knowledge: included,
        })
        .map_err(|error| AppError::Validation(error.to_string()))?;
        Ok(CustomGptPortableExport {
            json,
            included_knowledge,
            excluded_sensitive,
            excluded_disabled,
            excluded_files,
        })
    }

    pub fn record_custom_gpt_exported(
        &self,
        custom_gpt_id: &str,
        included_knowledge: usize,
    ) -> Result<(), AppError> {
        let view = self.custom_gpt_view(custom_gpt_id)?;
        self.connect()?.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.exported', 'user', ?1)",
            params![serde_json::json!({
                "custom_gpt_id": custom_gpt_id,
                "version_no": view.version_no,
                "included_knowledge": included_knowledge
            })
            .to_string()],
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub fn import_custom_gpt_json(&self, source: &str) -> Result<CustomGptView, AppError> {
        Ok(self.import_custom_gpt_package_json(source)?.custom_gpt)
    }

    pub fn import_custom_gpt_package_json(
        &self,
        source: &str,
    ) -> Result<CustomGptImportReport, AppError> {
        if source.len() > 256_000 {
            return Err(AppError::Validation(
                "el archivo del GPT supera el límite de 256 KB".to_owned(),
            ));
        }
        let portable: PortableCustomGpt = serde_json::from_str(source)
            .map_err(|error| AppError::Validation(format!("JSON de GPT no válido: {error}")))?;
        if !matches!(portable.schema_version, 1 | 2) {
            return Err(AppError::Validation(format!(
                "versión de archivo de GPT no compatible: {}",
                portable.schema_version
            )));
        }
        if portable.schema_version == 1 && !portable.knowledge.is_empty() {
            return Err(AppError::Validation(
                "un archivo de GPT versión 1 no puede incluir conocimiento".to_owned(),
            ));
        }
        if portable.knowledge.len() > 100 {
            return Err(AppError::Validation(
                "el paquete del GPT supera el límite de 100 elementos de conocimiento".to_owned(),
            ));
        }
        let mut normalized_knowledge = Vec::new();
        let mut seen_knowledge = HashSet::new();
        for item in &portable.knowledge {
            if !matches!(
                item.category.as_str(),
                "preference" | "instruction" | "fact"
            ) {
                return Err(AppError::Validation(
                    "el paquete contiene una categoría de conocimiento no válida".to_owned(),
                ));
            }
            let content = item.content.trim();
            if content.is_empty() || content.chars().count() > 2_000 {
                return Err(AppError::Validation(
                    "cada conocimiento importado debe contener entre 1 y 2.000 caracteres"
                        .to_owned(),
                ));
            }
            let key = format!("{}\0{}", item.category, content.to_lowercase());
            if seen_knowledge.insert(key) {
                normalized_knowledge.push((item.category.clone(), content.to_owned()));
            }
        }
        let imported = self.create_custom_gpt_with_icon(
            &portable.name,
            portable.description.as_deref(),
            Some(&portable.icon_ref),
            &portable.instructions,
            &portable.conversation_starters,
            &CustomGptToolPermissions::default(),
            // Un paquete importado no impone modelo ni proyecto: ambos son
            // decisiones locales de quien lo recibe.
            None,
            None,
            None,
            Some(&portable.context_profile),
        )?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        for (category, content) in &normalized_knowledge {
            transaction.execute(
                "INSERT INTO memory_items(
                    id, custom_gpt_id, category, content, sensitivity,
                    enabled, provenance_type
                 ) VALUES (?1, ?2, ?3, ?4, 'normal', 0, 'import')",
                params![
                    format!("memory_{}", Uuid::new_v4().simple()),
                    imported.id,
                    category,
                    content
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.imported', 'user', ?1)",
            params![serde_json::json!({
                "custom_gpt_id": imported.id,
                "version_no": imported.version_no,
                "imported_knowledge": normalized_knowledge.len(),
                "knowledge_requires_review": !normalized_knowledge.is_empty()
            })
            .to_string()],
        )?;
        transaction.commit()?;
        Ok(CustomGptImportReport {
            custom_gpt: imported,
            imported_knowledge: normalized_knowledge.len(),
            knowledge_requires_review: !normalized_knowledge.is_empty(),
        })
    }

    /// Contexto congelable de un GPT por su identificador, sin conversación.
    ///
    /// Lo usa la vista previa para mostrar exactamente lo que se enviaría si se
    /// eligiera este GPT, sin crear ninguna tarea.
    pub fn custom_gpt_context(&self, custom_gpt_id: &str) -> Result<CustomGptContext, AppError> {
        let view = self.custom_gpt_view(custom_gpt_id)?;
        let version_id: String = self.connect()?.query_row(
            "SELECT active_version_id FROM custom_gpts
             WHERE id = ?1 AND archived_at IS NULL AND active_version_id IS NOT NULL",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        Ok(CustomGptContext {
            custom_gpt_id: view.id,
            version_id,
            name: view.name,
            icon_ref: view.icon_ref,
            version_no: view.version_no,
            instructions: view.instructions,
            tool_permissions: view.tool_permissions,
            preferred_model: view.preferred_model,
            execution_profile: view.execution_profile,
            context_profile: view.context_profile,
            api_actions: view.api_actions,
        })
    }

    pub fn custom_gpt_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Option<CustomGptContext>, AppError> {
        let row = self
            .connect()?
            .query_row(
                "SELECT gpt.id, version.id, gpt.name, version.version_no,
                        version.configuration_json,
                        COALESCE((
                          SELECT permission.effect FROM gpt_tool_permissions permission
                          WHERE permission.gpt_version_id = version.id
                            AND permission.tool_name = 'run_code'
                        ), 'deny'),
                        COALESCE((
                          SELECT permission.effect FROM gpt_tool_permissions permission
                          WHERE permission.gpt_version_id = version.id
                            AND permission.tool_name = 'rename_conversation'
                        ), 'deny'),
                        COALESCE((
                          SELECT permission.effect FROM gpt_tool_permissions permission
                          WHERE permission.gpt_version_id = version.id
                            AND permission.tool_name = 'read_authorized_folders'
                        ), 'deny'),
                        COALESCE((
                          SELECT permission.effect FROM gpt_tool_permissions permission
                          WHERE permission.gpt_version_id = version.id
                            AND permission.tool_name = 'modify_authorized_files'
                        ), 'deny'),
                        COALESCE((
                          SELECT permission.effect FROM gpt_tool_permissions permission
                          WHERE permission.gpt_version_id = version.id
                            AND permission.tool_name = 'create_scheduled_tasks'
                        ), 'deny'),
                        COALESCE((
                          SELECT permission.effect FROM gpt_tool_permissions permission
                          WHERE permission.gpt_version_id = version.id
                            AND permission.tool_name = 'call_external_apis'
                        ), 'deny')
                 FROM conversations conversation
                 JOIN custom_gpts gpt
                   ON gpt.id = conversation.custom_gpt_id
                  AND gpt.archived_at IS NULL
                 JOIN gpt_versions version ON version.id = gpt.active_version_id
                 WHERE conversation.id = ?1
                   AND conversation.archived_at IS NULL
                   AND conversation.deleted_at IS NULL",
                params![conversation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                    ))
                },
            )
            .optional()?;
        row.map(
            |(
                custom_gpt_id,
                version_id,
                name,
                version_no,
                configuration_json,
                run_code,
                rename_conversation,
                read_authorized_folders,
                modify_authorized_files,
                create_scheduled_tasks,
                call_external_apis,
            )| {
                let configuration: CustomGptConfiguration =
                    serde_json::from_str(&configuration_json).map_err(|error| {
                        AppError::Conflict(format!(
                            "la versión activa del GPT {custom_gpt_id} no es legible: {error}"
                        ))
                    })?;
                Ok(CustomGptContext {
                    custom_gpt_id,
                    version_id,
                    name,
                    icon_ref: configuration.icon_ref,
                    version_no,
                    instructions: configuration.instructions,
                    tool_permissions: CustomGptToolPermissions {
                        run_code,
                        rename_conversation,
                        read_authorized_folders,
                        modify_authorized_files,
                        create_scheduled_tasks,
                        call_external_apis,
                    },
                    preferred_model: configuration.preferred_model,
                    execution_profile: configuration.execution_profile,
                    context_profile: configuration.context_profile,
                    api_actions: configuration.api_actions,
                })
            },
        )
        .transpose()
    }
}
