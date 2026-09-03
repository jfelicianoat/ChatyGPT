//! GPTs personalizados: alta, edicion y versiones inmutables.
//!
//! Cada edicion crea una version nueva en vez de pisar la anterior, que
//! es lo que permite restaurar sin haber perdido nada por el camino.

use super::*;

impl Database {
    #[cfg(test)]
    pub fn create_custom_gpt(
        &self,
        name: &str,
        description: Option<&str>,
        instructions: &str,
    ) -> Result<CustomGptView, AppError> {
        self.create_custom_gpt_with_starters(
            name,
            description,
            instructions,
            &[],
            &CustomGptToolPermissions::default(),
            None,
            None,
            None,
        )
    }

    #[allow(dead_code, clippy::too_many_arguments)]
    pub fn create_custom_gpt_with_starters(
        &self,
        name: &str,
        description: Option<&str>,
        instructions: &str,
        conversation_starters: &[String],
        tool_permissions: &CustomGptToolPermissions,
        preferred_model: Option<&str>,
        default_project_id: Option<&str>,
        execution_profile: Option<&ConversationExecutionPreferences>,
    ) -> Result<CustomGptView, AppError> {
        self.create_custom_gpt_with_icon(
            name,
            description,
            None,
            instructions,
            conversation_starters,
            tool_permissions,
            preferred_model,
            default_project_id,
            execution_profile,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_custom_gpt_with_icon(
        &self,
        name: &str,
        description: Option<&str>,
        icon_ref: Option<&str>,
        instructions: &str,
        conversation_starters: &[String],
        tool_permissions: &CustomGptToolPermissions,
        preferred_model: Option<&str>,
        default_project_id: Option<&str>,
        execution_profile: Option<&ConversationExecutionPreferences>,
        context_profile: Option<&str>,
    ) -> Result<CustomGptView, AppError> {
        self.create_custom_gpt_with_api_actions(
            name,
            description,
            icon_ref,
            instructions,
            conversation_starters,
            tool_permissions,
            preferred_model,
            default_project_id,
            execution_profile,
            context_profile,
            &[],
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_custom_gpt_with_api_actions(
        &self,
        name: &str,
        description: Option<&str>,
        icon_ref: Option<&str>,
        instructions: &str,
        conversation_starters: &[String],
        tool_permissions: &CustomGptToolPermissions,
        preferred_model: Option<&str>,
        default_project_id: Option<&str>,
        execution_profile: Option<&ConversationExecutionPreferences>,
        context_profile: Option<&str>,
        api_actions: &[CustomGptApiAction],
    ) -> Result<CustomGptView, AppError> {
        let (name, description, instructions) =
            validated_custom_gpt_fields(name, description, instructions)?;
        let icon_ref = validated_custom_gpt_icon(icon_ref)?;
        let conversation_starters = validated_conversation_starters(conversation_starters)?;
        let tool_permissions = validated_custom_gpt_tool_permissions(tool_permissions)?;
        let preferred_model = validated_preferred_model(preferred_model)?;
        if let Some(profile) = execution_profile {
            validate_execution_preferences(profile)?;
        }
        let context_profile = validated_custom_gpt_context_profile(context_profile)?;
        let api_actions = validated_custom_gpt_api_actions(api_actions)?;
        let custom_gpt_id = format!("gpt_{}", Uuid::new_v4().simple());
        let version_id = format!("gpt_version_{}", Uuid::new_v4().simple());
        let configuration = CustomGptConfiguration {
            schema_version: 2,
            icon_ref,
            instructions,
            conversation_starters,
            preferred_model,
            tools_enabled: tool_permissions.requires_confirmation("run_code")
                || tool_permissions.requires_confirmation("rename_conversation")
                || tool_permissions.requires_confirmation("read_authorized_file")
                || tool_permissions.requires_confirmation("replace_authorized_file")
                || tool_permissions.requires_confirmation("create_scheduled_task")
                || tool_permissions.requires_confirmation("call_external_api"),
            execution_profile: execution_profile.cloned(),
            context_profile,
            api_actions,
        };
        let configuration_json = serde_json::to_string(&configuration)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO custom_gpts(id, name, description, default_project_id)
             VALUES (?1, ?2, ?3, ?4)",
            params![custom_gpt_id, name, description, default_project_id],
        )?;
        transaction.execute(
            "INSERT INTO gpt_versions(
                id, custom_gpt_id, version_no, configuration_json
             ) VALUES (?1, ?2, 1, ?3)",
            params![version_id, custom_gpt_id, configuration_json],
        )?;
        transaction.execute(
            "UPDATE custom_gpts
             SET active_version_id = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![custom_gpt_id, version_id],
        )?;
        for (tool_name, effect) in [
            ("run_code", tool_permissions.run_code.as_str()),
            (
                "rename_conversation",
                tool_permissions.rename_conversation.as_str(),
            ),
            (
                "read_authorized_folders",
                tool_permissions.read_authorized_folders.as_str(),
            ),
            (
                "modify_authorized_files",
                tool_permissions.modify_authorized_files.as_str(),
            ),
            (
                "create_scheduled_tasks",
                tool_permissions.create_scheduled_tasks.as_str(),
            ),
            (
                "call_external_apis",
                tool_permissions.call_external_apis.as_str(),
            ),
        ] {
            transaction.execute(
                "INSERT INTO gpt_tool_permissions(
                    id, gpt_version_id, tool_name, effect, scope_json
                 ) VALUES (?1, ?2, ?3, ?4, '{}')",
                params![
                    format!("gpt_permission_{}", Uuid::new_v4().simple()),
                    version_id,
                    tool_name,
                    effect
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.created', 'user', ?1)",
            params![serde_json::json!({
                "custom_gpt_id": custom_gpt_id,
                "version_no": 1
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.custom_gpt_view(&custom_gpt_id)
    }

    #[cfg(test)]
    pub fn update_custom_gpt(
        &self,
        custom_gpt_id: &str,
        name: &str,
        description: Option<&str>,
        instructions: &str,
    ) -> Result<CustomGptView, AppError> {
        let current = self.custom_gpt_view(custom_gpt_id)?;
        self.update_custom_gpt_with_starters(
            custom_gpt_id,
            name,
            description,
            instructions,
            &current.conversation_starters,
            &current.tool_permissions,
            current.preferred_model.as_deref(),
            current.default_project_id.as_deref(),
            current.execution_profile.as_ref(),
        )
    }

    #[allow(dead_code, clippy::too_many_arguments)]
    pub fn update_custom_gpt_with_starters(
        &self,
        custom_gpt_id: &str,
        name: &str,
        description: Option<&str>,
        instructions: &str,
        conversation_starters: &[String],
        tool_permissions: &CustomGptToolPermissions,
        preferred_model: Option<&str>,
        default_project_id: Option<&str>,
        execution_profile: Option<&ConversationExecutionPreferences>,
    ) -> Result<CustomGptView, AppError> {
        let current = self.custom_gpt_view(custom_gpt_id)?;
        let icon_ref = current.icon_ref;
        self.update_custom_gpt_with_icon(
            custom_gpt_id,
            name,
            description,
            Some(&icon_ref),
            instructions,
            conversation_starters,
            tool_permissions,
            preferred_model,
            default_project_id,
            execution_profile,
            Some(&current.context_profile),
        )
    }

    #[allow(dead_code, clippy::too_many_arguments)]
    pub fn update_custom_gpt_with_icon(
        &self,
        custom_gpt_id: &str,
        name: &str,
        description: Option<&str>,
        icon_ref: Option<&str>,
        instructions: &str,
        conversation_starters: &[String],
        tool_permissions: &CustomGptToolPermissions,
        preferred_model: Option<&str>,
        default_project_id: Option<&str>,
        execution_profile: Option<&ConversationExecutionPreferences>,
        context_profile: Option<&str>,
    ) -> Result<CustomGptView, AppError> {
        self.update_custom_gpt_with_api_actions(
            custom_gpt_id,
            name,
            description,
            icon_ref,
            instructions,
            conversation_starters,
            tool_permissions,
            preferred_model,
            default_project_id,
            execution_profile,
            context_profile,
            &[],
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_custom_gpt_with_api_actions(
        &self,
        custom_gpt_id: &str,
        name: &str,
        description: Option<&str>,
        icon_ref: Option<&str>,
        instructions: &str,
        conversation_starters: &[String],
        tool_permissions: &CustomGptToolPermissions,
        preferred_model: Option<&str>,
        default_project_id: Option<&str>,
        execution_profile: Option<&ConversationExecutionPreferences>,
        context_profile: Option<&str>,
        api_actions: &[CustomGptApiAction],
    ) -> Result<CustomGptView, AppError> {
        let (name, description, instructions) =
            validated_custom_gpt_fields(name, description, instructions)?;
        let icon_ref = validated_custom_gpt_icon(icon_ref)?;
        let conversation_starters = validated_conversation_starters(conversation_starters)?;
        let tool_permissions = validated_custom_gpt_tool_permissions(tool_permissions)?;
        let preferred_model = validated_preferred_model(preferred_model)?;
        if let Some(profile) = execution_profile {
            validate_execution_preferences(profile)?;
        }
        let context_profile = validated_custom_gpt_context_profile(context_profile)?;
        let api_actions = validated_custom_gpt_api_actions(api_actions)?;
        let version_id = format!("gpt_version_{}", Uuid::new_v4().simple());
        let configuration = CustomGptConfiguration {
            schema_version: 2,
            icon_ref,
            instructions,
            conversation_starters,
            preferred_model,
            tools_enabled: tool_permissions.requires_confirmation("run_code")
                || tool_permissions.requires_confirmation("rename_conversation")
                || tool_permissions.requires_confirmation("read_authorized_file")
                || tool_permissions.requires_confirmation("replace_authorized_file")
                || tool_permissions.requires_confirmation("create_scheduled_task")
                || tool_permissions.requires_confirmation("call_external_api"),
            execution_profile: execution_profile.cloned(),
            context_profile,
            api_actions,
        };
        let configuration_json = serde_json::to_string(&configuration)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let next_version = transaction
            .query_row(
                "SELECT COALESCE(MAX(version.version_no), 0) + 1
                 FROM gpt_versions version
                 JOIN custom_gpts gpt ON gpt.id = version.custom_gpt_id
                 WHERE gpt.id = ?1 AND gpt.archived_at IS NULL",
                params![custom_gpt_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .filter(|version| *version > 1)
            .ok_or_else(|| AppError::NotFound(format!("GPT personal {custom_gpt_id}")))?;
        transaction.execute(
            "INSERT INTO gpt_versions(
                id, custom_gpt_id, version_no, configuration_json
             ) VALUES (?1, ?2, ?3, ?4)",
            params![version_id, custom_gpt_id, next_version, configuration_json],
        )?;
        transaction.execute(
            "UPDATE custom_gpts
             SET name = ?2, description = ?3, active_version_id = ?4,
                 default_project_id = ?5, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL",
            params![
                custom_gpt_id,
                name,
                description,
                version_id,
                default_project_id
            ],
        )?;
        for (tool_name, effect) in [
            ("run_code", tool_permissions.run_code.as_str()),
            (
                "rename_conversation",
                tool_permissions.rename_conversation.as_str(),
            ),
            (
                "read_authorized_folders",
                tool_permissions.read_authorized_folders.as_str(),
            ),
            (
                "modify_authorized_files",
                tool_permissions.modify_authorized_files.as_str(),
            ),
            (
                "create_scheduled_tasks",
                tool_permissions.create_scheduled_tasks.as_str(),
            ),
            (
                "call_external_apis",
                tool_permissions.call_external_apis.as_str(),
            ),
        ] {
            transaction.execute(
                "INSERT INTO gpt_tool_permissions(
                    id, gpt_version_id, tool_name, effect, scope_json
                 ) VALUES (?1, ?2, ?3, ?4, '{}')",
                params![
                    format!("gpt_permission_{}", Uuid::new_v4().simple()),
                    version_id,
                    tool_name,
                    effect
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.version_created', 'user', ?1)",
            params![serde_json::json!({
                "custom_gpt_id": custom_gpt_id,
                "version_no": next_version
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.custom_gpt_view(custom_gpt_id)
    }

    pub fn list_custom_gpts(&self) -> Result<Vec<CustomGptView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT gpt.id, gpt.name, gpt.description, version.configuration_json,
                    version.version_no, gpt.created_at, gpt.updated_at,
                    gpt.default_project_id,
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
             FROM custom_gpts gpt
             JOIN gpt_versions version ON version.id = gpt.active_version_id
             WHERE gpt.archived_at IS NULL
             ORDER BY gpt.updated_at DESC, gpt.name COLLATE NOCASE",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, String>(13)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(
                |(
                    id,
                    name,
                    description,
                    configuration_json,
                    version_no,
                    created_at,
                    updated_at,
                    default_project_id,
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
                                "la versión activa del GPT {id} no es legible: {error}"
                            ))
                        })?;
                    Ok(CustomGptView {
                        id,
                        name,
                        description,
                        icon_ref: configuration.icon_ref,
                        instructions: configuration.instructions,
                        conversation_starters: configuration.conversation_starters,
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
                        default_project_id,
                        version_no,
                        created_at,
                        updated_at,
                    })
                },
            )
            .collect()
    }

    /// Crea una copia independiente de un GPT a partir de su versión activa.
    ///
    /// Deliberadamente **no** arrastra permisos, conocimiento ni archivos: un
    /// duplicado nace tan restringido como un GPT importado, para que copiar un
    /// asistente no sea una vía silenciosa de propagar accesos o datos sensibles.
    pub fn duplicate_custom_gpt(
        &self,
        custom_gpt_id: &str,
        new_name: Option<&str>,
    ) -> Result<CustomGptView, AppError> {
        let source = self.custom_gpt_view(custom_gpt_id)?;
        let proposed = match new_name.map(str::trim).filter(|value| !value.is_empty()) {
            Some(name) => name.to_owned(),
            None => {
                // El sufijo se recorta para no superar el límite de nombre.
                let suffix = " (copia)";
                let room = 80 - suffix.chars().count();
                let base: String = source.name.chars().take(room).collect();
                format!("{base}{suffix}")
            }
        };
        let duplicate = self.create_custom_gpt_with_icon(
            &proposed,
            source.description.as_deref(),
            Some(&source.icon_ref),
            &source.instructions,
            &source.conversation_starters,
            // Permisos denegados por defecto, como en la importación.
            &CustomGptToolPermissions::default(),
            // El modelo y el proyecto sí se heredan: son preferencias, no accesos.
            source.preferred_model.as_deref(),
            source.default_project_id.as_deref(),
            source.execution_profile.as_ref(),
            Some(&source.context_profile),
        )?;
        self.connect()?.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.duplicated', 'user', ?1)",
            params![serde_json::json!({
                "source_custom_gpt_id": custom_gpt_id,
                "custom_gpt_id": duplicate.id
            })
            .to_string()],
        )?;
        Ok(duplicate)
    }

    /// Devuelve todas las revisiones guardadas, de la más reciente a la primera.
    ///
    /// El historial es solo lectura: ninguna versión anterior se modifica jamás,
    /// porque las tareas ya enviadas conservan congelada la que usaron.
    pub fn list_custom_gpt_versions(
        &self,
        custom_gpt_id: &str,
    ) -> Result<Vec<CustomGptVersionView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT version.id, version.version_no, version.configuration_json,
                    version.created_at,
                    version.id = COALESCE(gpt.active_version_id, ''),
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
                    ), 'deny'),
                    (SELECT COUNT(*) FROM broker_tasks task
                     WHERE task.gpt_version_id = version.id)
             FROM gpt_versions version
             JOIN custom_gpts gpt ON gpt.id = version.custom_gpt_id
             WHERE version.custom_gpt_id = ?1
             ORDER BY version.version_no DESC",
        )?;
        let versions = statement
            .query_map(params![custom_gpt_id], |row| {
                let configuration_json: String = row.get(2)?;
                let configuration: CustomGptConfiguration =
                    serde_json::from_str(&configuration_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            configuration_json.len(),
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                Ok(CustomGptVersionView {
                    id: row.get(0)?,
                    version_no: row.get(1)?,
                    icon_ref: configuration.icon_ref,
                    instructions: configuration.instructions,
                    conversation_starters: configuration.conversation_starters,
                    preferred_model: configuration.preferred_model,
                    execution_profile: configuration.execution_profile,
                    context_profile: configuration.context_profile,
                    api_actions: configuration.api_actions,
                    created_at: row.get(3)?,
                    active: row.get(4)?,
                    tool_permissions: CustomGptToolPermissions {
                        run_code: row.get(5)?,
                        rename_conversation: row.get(6)?,
                        read_authorized_folders: row.get(7)?,
                        modify_authorized_files: row.get(8)?,
                        create_scheduled_tasks: row.get(9)?,
                        call_external_apis: row.get(10)?,
                    },
                    task_count: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if versions.is_empty() {
            return Err(AppError::NotFound(format!("GPT personal {custom_gpt_id}")));
        }
        Ok(versions)
    }

    /// Restaura una revisión anterior **creando una versión nueva** con su
    /// contenido.
    ///
    /// No se reactiva la fila antigua ni se borra nada: así el historial sigue
    /// siendo un registro fiel de lo que ocurrió y las respuestas ya emitidas
    /// mantienen intacta la versión con la que se generaron.
    pub fn restore_custom_gpt_version(
        &self,
        custom_gpt_id: &str,
        version_id: &str,
        confirmed: bool,
    ) -> Result<CustomGptView, AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "restaurar una versión anterior requiere confirmación".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let (source_version_no, configuration_json): (i64, String) = transaction
            .query_row(
                "SELECT version.version_no, version.configuration_json
                 FROM gpt_versions version
                 JOIN custom_gpts gpt ON gpt.id = version.custom_gpt_id
                 WHERE version.id = ?1 AND version.custom_gpt_id = ?2
                   AND gpt.archived_at IS NULL",
                params![version_id, custom_gpt_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::NotFound(format!("la versión {version_id} no pertenece a este GPT"))
            })?;
        let active_version_id: Option<String> = transaction.query_row(
            "SELECT active_version_id FROM custom_gpts WHERE id = ?1",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        if active_version_id.as_deref() == Some(version_id) {
            return Err(AppError::Conflict("esa versión ya es la activa".to_owned()));
        }
        let next_version: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(version_no), 0) + 1 FROM gpt_versions WHERE custom_gpt_id = ?1",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        let new_version_id = format!("gpt_version_{}", Uuid::new_v4().simple());
        transaction.execute(
            "INSERT INTO gpt_versions(
                id, custom_gpt_id, version_no, configuration_json
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                new_version_id,
                custom_gpt_id,
                next_version,
                configuration_json
            ],
        )?;
        // Los permisos también se copian de la versión restaurada: restaurar sin
        // ellos daría un GPT que se comporta distinto al que se pidió recuperar.
        transaction.execute(
            "INSERT INTO gpt_tool_permissions(
                id, gpt_version_id, tool_name, effect, scope_json
             )
             SELECT lower(hex(randomblob(16))), ?1, tool_name, effect, scope_json
             FROM gpt_tool_permissions
             WHERE gpt_version_id = ?2",
            params![new_version_id, version_id],
        )?;
        transaction.execute(
            "UPDATE custom_gpts
             SET active_version_id = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL",
            params![custom_gpt_id, new_version_id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.version_restored', 'user', ?1)",
            params![serde_json::json!({
                "custom_gpt_id": custom_gpt_id,
                "restored_from_version_no": source_version_no,
                "version_no": next_version
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.custom_gpt_view(custom_gpt_id)
    }
}
