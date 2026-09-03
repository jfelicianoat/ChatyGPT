//! Plantillas de tareas programadas y consultas de historial paginado.

use super::*;

impl Database {
    pub fn list_scheduled_task_templates(
        &self,
    ) -> Result<Vec<ScheduledTaskTemplateView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, name, prompt, schedule_expression, created_at, updated_at
             FROM scheduled_task_templates
             ORDER BY datetime(updated_at) DESC, name COLLATE NOCASE",
        )?;
        let templates = statement
            .query_map([], |row| {
                Ok(ScheduledTaskTemplateView {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    prompt: row.get(2)?,
                    schedule_expression: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        Ok(templates)
    }

    pub fn create_scheduled_task_template(
        &self,
        name: &str,
        prompt: &str,
        schedule_expression: &str,
    ) -> Result<ScheduledTaskTemplateView, AppError> {
        if !matches!(schedule_expression, "once" | "daily" | "weekly") {
            return Err(AppError::Validation(
                "la recurrencia de la plantilla no es válida".to_owned(),
            ));
        }
        let id = format!("scheduled_template_{}", Uuid::new_v4().simple());
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO scheduled_task_templates(
                id, name, prompt, schedule_expression, created_at, updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![id, name, prompt, schedule_expression],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('scheduled_template.created', 'user', ?1)",
            params![serde_json::json!({
                "scheduled_template_id": id,
                "schedule_expression": schedule_expression
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.list_scheduled_task_templates()?
            .into_iter()
            .find(|template| template.id == id)
            .ok_or_else(|| AppError::NotFound("plantilla programada recién creada".to_owned()))
    }

    pub fn delete_scheduled_task_template(
        &self,
        id: &str,
        confirmed: bool,
    ) -> Result<(), AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "eliminar una plantilla requiere confirmación explícita".to_owned(),
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let name = transaction
            .query_row(
                "SELECT name FROM scheduled_task_templates WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound("plantilla programada".to_owned()))?;
        transaction.execute(
            "DELETE FROM scheduled_task_templates WHERE id = ?1",
            params![id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('scheduled_template.deleted', 'user', ?1)",
            params![serde_json::json!({
                "scheduled_template_id": id,
                "name": name
            })
            .to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn list_scheduled_tasks(&self) -> Result<Vec<ScheduledTaskView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT st.id, st.name,
                    COALESCE(json_extract(st.payload_json, '$.target_kind'), 'conversation'),
                    json_extract(st.payload_json, '$.conversation_id'),
                    c.title,
                    json_extract(st.payload_json, '$.workflow_id'),
                    w.name,
                    version.version_no,
                    json_extract(st.payload_json, '$.prompt'),
                    st.schedule_expression, st.timezone, st.enabled,
                    st.confirmed_at, st.next_run_at, st.created_at, st.updated_at
             FROM scheduled_tasks st
             LEFT JOIN conversations c
               ON c.id = json_extract(st.payload_json, '$.conversation_id')
             LEFT JOIN workflows w
               ON w.id = json_extract(st.payload_json, '$.workflow_id')
             LEFT JOIN workflow_versions version
               ON version.id = json_extract(st.payload_json, '$.workflow_version_id')
             WHERE (
                    COALESCE(json_extract(st.payload_json, '$.target_kind'), 'conversation') = 'conversation'
                    AND c.id IS NOT NULL AND c.deleted_at IS NULL
               ) OR (
                    json_extract(st.payload_json, '$.target_kind') = 'workflow'
                    AND w.id IS NOT NULL AND w.archived_at IS NULL
               )
             ORDER BY st.created_at DESC",
        )?;
        let mut tasks = statement
            .query_map([], |row| {
                Ok(ScheduledTaskView {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    target_kind: row.get(2)?,
                    conversation_id: row.get(3)?,
                    conversation_title: row.get(4)?,
                    workflow_id: row.get(5)?,
                    workflow_name: row.get(6)?,
                    workflow_version_no: row.get(7)?,
                    prompt: row.get(8)?,
                    schedule_expression: row.get(9)?,
                    timezone: row.get(10)?,
                    enabled: row.get::<_, i64>(11)? != 0,
                    confirmed_at: row.get(12)?,
                    next_run_at: row.get(13)?,
                    created_at: row.get(14)?,
                    updated_at: row.get(15)?,
                    runs: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for task in &mut tasks {
            let mut runs = connection.prepare(
                "SELECT id, due_at, status, broker_task_id, workflow_run_id,
                        attempt, result_json,
                        created_at, updated_at
                 FROM scheduled_runs
                 WHERE scheduled_task_id = ?1
                 ORDER BY datetime(created_at) DESC, attempt DESC
                 LIMIT 10",
            )?;
            task.runs = runs
                .query_map(params![task.id], |row| {
                    let result_json: Option<String> = row.get(6)?;
                    Ok(ScheduledRunView {
                        id: row.get(0)?,
                        due_at: row.get(1)?,
                        status: row.get(2)?,
                        broker_task_id: row.get(3)?,
                        workflow_run_id: row.get(4)?,
                        attempt: row.get(5)?,
                        result: result_json.and_then(|value| serde_json::from_str(&value).ok()),
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
        }
        Ok(tasks)
    }

    pub fn scheduled_history_export_rows(
        &self,
        status_filter: &str,
        period_filter: &str,
    ) -> Result<Vec<ScheduledHistoryExportRow>, AppError> {
        if !matches!(
            status_filter,
            "all" | "active" | "completed" | "failed" | "cancelled"
        ) {
            return Err(AppError::Validation(
                "el filtro de estado del historial no es válido".to_owned(),
            ));
        }
        if !matches!(period_filter, "all" | "today" | "7d" | "30d") {
            return Err(AppError::Validation(
                "el filtro de fecha del historial no es válido".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT task.name, COALESCE(conversation.title, workflow.name),
                    json_extract(task.payload_json, '$.prompt'),
                    task.schedule_expression, task.timezone,
                    run.id, run.due_at, run.status, run.attempt, run.result_json,
                    run.created_at, run.updated_at
             FROM scheduled_runs run
             JOIN scheduled_tasks task ON task.id = run.scheduled_task_id
             LEFT JOIN conversations conversation
               ON conversation.id = json_extract(task.payload_json, '$.conversation_id')
             LEFT JOIN workflows workflow
               ON workflow.id = json_extract(task.payload_json, '$.workflow_id')
             WHERE (
                    (
                        COALESCE(json_extract(task.payload_json, '$.target_kind'), 'conversation') = 'conversation'
                        AND conversation.id IS NOT NULL AND conversation.deleted_at IS NULL
                    ) OR (
                        json_extract(task.payload_json, '$.target_kind') = 'workflow'
                        AND workflow.id IS NOT NULL AND workflow.archived_at IS NULL
                    )
               ) AND (
                    ?1 = 'all'
                    OR (?1 = 'active' AND run.status IN ('claimed', 'running'))
                    OR run.status = ?1
               )
               AND (
                    ?2 = 'all'
                    OR (?2 = 'today'
                        AND date(run.updated_at, 'localtime') = date('now', 'localtime'))
                    OR (?2 = '7d'
                        AND datetime(run.updated_at) >= datetime('now', '-7 days'))
                    OR (?2 = '30d'
                        AND datetime(run.updated_at) >= datetime('now', '-30 days'))
               )
             ORDER BY datetime(run.updated_at) DESC, run.attempt DESC",
        )?;
        let rows = statement
            .query_map(params![status_filter, period_filter], |row| {
                let result_json: Option<String> = row.get(9)?;
                Ok(ScheduledHistoryExportRow {
                    task_name: row.get(0)?,
                    conversation_title: row.get(1)?,
                    prompt: row.get(2)?,
                    schedule_expression: row.get(3)?,
                    timezone: row.get(4)?,
                    run_id: row.get(5)?,
                    due_at: row.get(6)?,
                    status: row.get(7)?,
                    attempt: row.get(8)?,
                    result: result_json.and_then(|value| serde_json::from_str(&value).ok()),
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        Ok(rows)
    }

    pub fn scheduled_run_page(
        &self,
        scheduled_task_id: &str,
        status_filter: &str,
        period_filter: &str,
        sort: &str,
        page: i64,
        page_size: i64,
    ) -> Result<ScheduledRunPageView, AppError> {
        if !matches!(
            status_filter,
            "all" | "active" | "completed" | "failed" | "cancelled"
        ) {
            return Err(AppError::Validation(
                "el filtro de estado del historial no es válido".to_owned(),
            ));
        }
        if !matches!(period_filter, "all" | "today" | "7d" | "30d") {
            return Err(AppError::Validation(
                "el filtro de fecha del historial no es válido".to_owned(),
            ));
        }
        if !matches!(sort, "newest" | "oldest") {
            return Err(AppError::Validation(
                "la ordenación del historial no es válida".to_owned(),
            ));
        }
        if page < 1 || !matches!(page_size, 10 | 25 | 50) {
            return Err(AppError::Validation(
                "la página o su tamaño no son válidos".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM scheduled_tasks WHERE id = ?1)",
            params![scheduled_task_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::NotFound("tarea programada".to_owned()));
        }
        let filters = "scheduled_task_id = ?1
            AND (
                ?2 = 'all'
                OR (?2 = 'active' AND status IN ('claimed', 'running'))
                OR status = ?2
            )
            AND (
                ?3 = 'all'
                OR (?3 = 'today'
                    AND date(updated_at, 'localtime') = date('now', 'localtime'))
                OR (?3 = '7d'
                    AND datetime(updated_at) >= datetime('now', '-7 days'))
                OR (?3 = '30d'
                    AND datetime(updated_at) >= datetime('now', '-30 days'))
            )";
        let total: i64 = connection.query_row(
            &format!("SELECT COUNT(*) FROM scheduled_runs WHERE {filters}"),
            params![scheduled_task_id, status_filter, period_filter],
            |row| row.get(0),
        )?;
        let maximum_page = std::cmp::max(1, (total + page_size - 1) / page_size);
        let page = std::cmp::min(page, maximum_page);
        let offset = (page - 1) * page_size;
        let direction = if sort == "oldest" { "ASC" } else { "DESC" };
        let query = format!(
            "SELECT id, due_at, status, broker_task_id, workflow_run_id,
                    attempt, result_json,
                    created_at, updated_at
             FROM scheduled_runs
             WHERE {filters}
             ORDER BY datetime(updated_at) {direction}, attempt {direction}, id {direction}
             LIMIT ?4 OFFSET ?5"
        );
        let mut statement = connection.prepare(&query)?;
        let items = statement
            .query_map(
                params![
                    scheduled_task_id,
                    status_filter,
                    period_filter,
                    page_size,
                    offset
                ],
                |row| {
                    let result_json: Option<String> = row.get(6)?;
                    Ok(ScheduledRunView {
                        id: row.get(0)?,
                        due_at: row.get(1)?,
                        status: row.get(2)?,
                        broker_task_id: row.get(3)?,
                        workflow_run_id: row.get(4)?,
                        attempt: row.get(5)?,
                        result: result_json.and_then(|value| serde_json::from_str(&value).ok()),
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ScheduledRunPageView {
            items,
            total,
            page,
            page_size,
            sort: sort.to_owned(),
        })
    }
}
