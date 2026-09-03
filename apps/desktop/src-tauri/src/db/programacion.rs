//! Alta y edicion de tareas y workflows programados.

use super::*;

impl Database {
    #[allow(clippy::too_many_arguments)]
    pub fn create_scheduled_task(
        &self,
        name: &str,
        conversation_id: &str,
        prompt: &str,
        due_at: &str,
        timezone: &str,
        schedule_expression: &str,
        confirmed: bool,
    ) -> Result<ScheduledTaskView, AppError> {
        let id = format!("scheduled_{}", Uuid::new_v4().simple());
        self.create_scheduled_task_with_id(
            &id,
            name,
            conversation_id,
            prompt,
            due_at,
            timezone,
            schedule_expression,
            confirmed,
            "user",
        )
    }

    /// Crea una tarea propuesta por una herramienta de forma idempotente.
    /// Repetir la misma confirmación tras un cierre inesperado devuelve la tarea
    /// original en lugar de programar dos ejecuciones.
    #[allow(clippy::too_many_arguments)]
    pub fn create_scheduled_task_from_tool(
        &self,
        tool_call_id: &str,
        name: &str,
        conversation_id: &str,
        prompt: &str,
        due_at: &str,
        timezone: &str,
    ) -> Result<ScheduledTaskView, AppError> {
        let digest = format!("{:x}", Sha256::digest(tool_call_id.as_bytes()));
        let id = format!("scheduled_tool_{}", &digest[..32]);
        if let Some(existing) = self
            .list_scheduled_tasks()?
            .into_iter()
            .find(|task| task.id == id)
        {
            return Ok(existing);
        }
        self.create_scheduled_task_with_id(
            &id,
            name,
            conversation_id,
            prompt,
            due_at,
            timezone,
            "once",
            true,
            "tool",
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn create_scheduled_task_with_id(
        &self,
        id: &str,
        name: &str,
        conversation_id: &str,
        prompt: &str,
        due_at: &str,
        timezone: &str,
        schedule_expression: &str,
        confirmed: bool,
        actor: &str,
    ) -> Result<ScheduledTaskView, AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "activar una tarea programada requiere confirmación explícita".to_owned(),
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let conversation_exists: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM conversations
                WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL
             )",
            params![conversation_id],
            |row| row.get(0),
        )?;
        if !conversation_exists {
            return Err(AppError::NotFound(
                "la conversación seleccionada ya no está disponible".to_owned(),
            ));
        }
        let valid_due_at: bool = transaction.query_row(
            "SELECT datetime(?1) IS NOT NULL AND datetime(?1) > datetime('now')",
            params![due_at],
            |row| row.get(0),
        )?;
        if !valid_due_at {
            return Err(AppError::Validation(
                "la fecha programada debe estar en el futuro".to_owned(),
            ));
        }
        if !matches!(schedule_expression, "once" | "daily" | "weekly") {
            return Err(AppError::Validation(
                "la recurrencia programada no es válida".to_owned(),
            ));
        }
        let payload = serde_json::json!({
            "conversation_id": conversation_id,
            "prompt": prompt
        });
        transaction.execute(
            "INSERT INTO scheduled_tasks(
                id, name, schedule_expression, timezone, payload_json,
                enabled, confirmed_at, next_run_at, created_at, updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, 1,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?6,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                id,
                name,
                schedule_expression,
                timezone,
                payload.to_string(),
                due_at
            ],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES ('scheduled_task.created', ?1, ?2, ?3)",
            params![
                actor,
                conversation_id,
                serde_json::json!({
                    "scheduled_task_id": id,
                    "due_at": due_at,
                    "timezone": timezone,
                    "schedule_expression": schedule_expression
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        self.list_scheduled_tasks()?
            .into_iter()
            .find(|task| task.id == id)
            .ok_or_else(|| AppError::NotFound("tarea programada recién creada".to_owned()))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_scheduled_workflow(
        &self,
        name: &str,
        workflow_id: &str,
        input_text: &str,
        due_at: &str,
        timezone: &str,
        schedule_expression: &str,
        confirmed: bool,
    ) -> Result<ScheduledTaskView, AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "activar un flujo programado requiere confirmación explícita".to_owned(),
            ));
        }
        if !matches!(schedule_expression, "once" | "daily" | "weekly") {
            return Err(AppError::Validation(
                "la recurrencia programada no es válida".to_owned(),
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let workflow_version_id = transaction
            .query_row(
                "SELECT published_version_id FROM workflows
             WHERE id = ?1 AND archived_at IS NULL AND published_version_id IS NOT NULL",
                params![workflow_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Conflict("publica el flujo antes de programarlo".to_owned())
            })?;
        let valid_due_at: bool = transaction.query_row(
            "SELECT datetime(?1) IS NOT NULL AND datetime(?1) > datetime('now')",
            params![due_at],
            |row| row.get(0),
        )?;
        if !valid_due_at {
            return Err(AppError::Validation(
                "la fecha programada debe estar en el futuro".to_owned(),
            ));
        }
        let id = format!("scheduled_{}", Uuid::new_v4().simple());
        let payload = serde_json::json!({
            "target_kind": "workflow",
            "workflow_id": workflow_id,
            "workflow_version_id": workflow_version_id,
            "prompt": input_text
        });
        transaction.execute(
            "INSERT INTO scheduled_tasks(
                id, name, schedule_expression, timezone, payload_json,
                enabled, confirmed_at, next_run_at, created_at, updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, 1,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?6,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                id,
                name,
                schedule_expression,
                timezone,
                payload.to_string(),
                due_at
            ],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('scheduled_workflow.created', 'user', ?1)",
            params![serde_json::json!({
                "scheduled_task_id": id,
                "workflow_id": workflow_id,
                "due_at": due_at,
                "timezone": timezone,
                "schedule_expression": schedule_expression
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.list_scheduled_tasks()?
            .into_iter()
            .find(|task| task.id == id)
            .ok_or_else(|| AppError::NotFound("flujo programado recién creado".to_owned()))
    }

    pub fn set_scheduled_task_enabled(
        &self,
        id: &str,
        enabled: bool,
        confirmed: bool,
    ) -> Result<ScheduledTaskView, AppError> {
        if enabled && !confirmed {
            return Err(AppError::Validation(
                "reactivar una tarea programada requiere confirmación explícita".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let changed = connection.execute(
            "UPDATE scheduled_tasks
             SET enabled = ?2,
                 confirmed_at = CASE
                    WHEN ?2 = 1 THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                    ELSE confirmed_at
                 END,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1
               AND (schedule_expression != 'once' OR last_claim_key IS NULL)
               AND next_run_at IS NOT NULL",
            params![id, i64::from(enabled)],
        )?;
        if changed == 0 {
            return Err(AppError::Conflict(
                "la tarea ya se ejecutó o dejó de estar disponible".to_owned(),
            ));
        }
        self.list_scheduled_tasks()?
            .into_iter()
            .find(|task| task.id == id)
            .ok_or_else(|| AppError::NotFound("tarea programada".to_owned()))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_scheduled_task(
        &self,
        id: &str,
        name: &str,
        conversation_id: &str,
        prompt: &str,
        due_at: &str,
        timezone: &str,
        schedule_expression: &str,
        confirmed: bool,
    ) -> Result<ScheduledTaskView, AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "editar y reactivar una tarea requiere confirmación explícita".to_owned(),
            ));
        }
        if !matches!(schedule_expression, "once" | "daily" | "weekly") {
            return Err(AppError::Validation(
                "la recurrencia programada no es válida".to_owned(),
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let valid_due_at: bool = transaction.query_row(
            "SELECT datetime(?1) IS NOT NULL AND datetime(?1) > datetime('now')",
            params![due_at],
            |row| row.get(0),
        )?;
        if !valid_due_at {
            return Err(AppError::Validation(
                "la fecha programada debe estar en el futuro".to_owned(),
            ));
        }
        let payload = serde_json::json!({
            "conversation_id": conversation_id,
            "prompt": prompt
        });
        let changed = transaction.execute(
            "UPDATE scheduled_tasks
             SET name = ?2, payload_json = ?3, next_run_at = ?4, timezone = ?5,
                 schedule_expression = ?6, enabled = 1, last_claim_key = NULL,
                 confirmed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1
               AND COALESCE(json_extract(payload_json, '$.target_kind'), 'conversation') = 'conversation'
               AND EXISTS(
                    SELECT 1 FROM conversations
                    WHERE id = ?7 AND archived_at IS NULL AND deleted_at IS NULL
               )
               AND NOT EXISTS(
                    SELECT 1 FROM scheduled_runs
                    WHERE scheduled_task_id = ?1 AND status IN ('claimed', 'running')
               )",
            params![
                id,
                name,
                payload.to_string(),
                due_at,
                timezone,
                schedule_expression,
                conversation_id
            ],
        )?;
        if changed == 0 {
            return Err(AppError::Conflict(
                "la programación está ejecutándose o la conversación ya no está disponible"
                    .to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES ('scheduled_task.updated', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({
                    "scheduled_task_id": id,
                    "due_at": due_at,
                    "timezone": timezone,
                    "schedule_expression": schedule_expression
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        self.list_scheduled_tasks()?
            .into_iter()
            .find(|task| task.id == id)
            .ok_or_else(|| AppError::NotFound("tarea programada editada".to_owned()))
    }

    pub fn delete_scheduled_task(&self, id: &str, confirmed: bool) -> Result<(), AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "eliminar una tarea programada requiere confirmación explícita".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let changed = connection.execute(
            "DELETE FROM scheduled_tasks
             WHERE id = ?1
               AND NOT EXISTS(
                 SELECT 1 FROM scheduled_runs
                 WHERE scheduled_task_id = ?1 AND status IN ('claimed', 'running')
               )",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::Conflict(
                "no se puede eliminar una programación que se está ejecutando".to_owned(),
            ));
        }
        Ok(())
    }
}
