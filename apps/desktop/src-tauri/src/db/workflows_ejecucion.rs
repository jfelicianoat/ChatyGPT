//! Ejecuciones de workflow: alta, reintento, aprobacion y cancelacion.

use super::*;

impl Database {
    pub fn create_workflow_run(
        &self,
        workflow_id: &str,
        input_text: &str,
    ) -> Result<WorkflowExecutionRecord, AppError> {
        self.create_workflow_run_for_version(workflow_id, None, input_text)
    }

    pub fn create_workflow_run_from_version(
        &self,
        workflow_id: &str,
        workflow_version_id: &str,
        input_text: &str,
    ) -> Result<WorkflowExecutionRecord, AppError> {
        self.create_workflow_run_for_version(workflow_id, Some(workflow_version_id), input_text)
    }

    pub(super) fn create_workflow_run_for_version(
        &self,
        workflow_id: &str,
        workflow_version_id: Option<&str>,
        input_text: &str,
    ) -> Result<WorkflowExecutionRecord, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let (version_id, definition_json): (String, String) = transaction
            .query_row(
                "SELECT version.id, version.definition_json
                 FROM workflows workflow
                 JOIN workflow_versions version ON version.workflow_id = workflow.id
                 WHERE workflow.id = ?1 AND workflow.archived_at IS NULL
                   AND version.id = COALESCE(?2, workflow.published_version_id)",
                params![workflow_id, workflow_version_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| AppError::Conflict("publica el flujo antes de ejecutarlo".to_owned()))?;
        let definition: WorkflowDefinition = serde_json::from_str(&definition_json)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let run_id = format!("workflow_run_{}", Uuid::new_v4().simple());
        transaction.execute(
            "INSERT INTO workflow_runs(
                id, workflow_id, workflow_version_id, status, input_text, started_at
             ) VALUES (?1, ?2, ?3, 'queued', ?4, datetime('now'))",
            params![run_id, workflow_id, version_id, input_text],
        )?;
        for node in &definition.nodes {
            transaction.execute(
                "INSERT INTO workflow_node_runs(id, run_id, node_id, node_kind, node_label)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    format!("workflow_node_run_{}", Uuid::new_v4().simple()),
                    run_id,
                    node.id,
                    node.kind,
                    node.label
                ],
            )?;
        }
        transaction.commit()?;
        Ok(WorkflowExecutionRecord {
            run_id,
            workflow_id: workflow_id.to_owned(),
            version_id,
            definition,
            input_text: input_text.to_owned(),
        })
    }

    pub fn workflow_execution_record(
        &self,
        run_id: &str,
    ) -> Result<WorkflowExecutionRecord, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT run.id, run.workflow_id, run.workflow_version_id,
                        version.definition_json, run.input_text
                 FROM workflow_runs run
                 JOIN workflow_versions version ON version.id = run.workflow_version_id
                 WHERE run.id = ?1",
                params![run_id],
                |row| {
                    let definition_json: String = row.get(3)?;
                    let definition = serde_json::from_str(&definition_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    Ok(WorkflowExecutionRecord {
                        run_id: row.get(0)?,
                        workflow_id: row.get(1)?,
                        version_id: row.get(2)?,
                        definition,
                        input_text: row.get(4)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("ejecución de flujo {run_id}")))
    }

    pub fn retry_workflow_run(
        &self,
        previous_run_id: &str,
    ) -> Result<WorkflowExecutionRecord, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let (workflow_id, version_id, input_text, definition_json): (
            String,
            String,
            String,
            String,
        ) = transaction
            .query_row(
                "SELECT run.workflow_id, run.workflow_version_id, run.input_text,
                            version.definition_json
                     FROM workflow_runs run
                     JOIN workflow_versions version ON version.id = run.workflow_version_id
                     WHERE run.id = ?1 AND run.status IN ('failed', 'partial_failed')",
                params![previous_run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Conflict("solo se pueden reintentar ejecuciones fallidas".to_owned())
            })?;
        let definition: WorkflowDefinition = serde_json::from_str(&definition_json)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let run_id = format!("workflow_run_{}", Uuid::new_v4().simple());
        transaction.execute(
            "INSERT INTO workflow_runs(
                id, workflow_id, workflow_version_id, status, input_text, started_at
             ) VALUES (?1, ?2, ?3, 'queued', ?4, datetime('now'))",
            params![run_id, workflow_id, version_id, input_text],
        )?;
        for node in &definition.nodes {
            let reusable: Option<(String, String)> = if node.kind == "result" {
                None
            } else {
                transaction
                    .query_row(
                        "SELECT input_text, output_text FROM workflow_node_runs
                         WHERE run_id = ?1 AND node_id = ?2 AND status = 'completed'
                           AND input_text IS NOT NULL AND output_text IS NOT NULL",
                        params![previous_run_id, node.id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?
            };
            let (status, previous_input, previous_output) = reusable
                .map(|(input, output)| ("completed", Some(input), Some(output)))
                .unwrap_or(("pending", None, None));
            transaction.execute(
                "INSERT INTO workflow_node_runs(
                    id, run_id, node_id, node_kind, node_label, status,
                    input_text, output_text, completed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                           CASE WHEN ?6 = 'completed' THEN datetime('now') END)",
                params![
                    format!("workflow_node_run_{}", Uuid::new_v4().simple()),
                    run_id,
                    node.id,
                    node.kind,
                    node.label,
                    status,
                    previous_input,
                    previous_output
                ],
            )?;
        }
        transaction.commit()?;
        Ok(WorkflowExecutionRecord {
            run_id,
            workflow_id,
            version_id,
            definition,
            input_text,
        })
    }

    pub fn workflow_run(&self, run_id: &str) -> Result<WorkflowRunView, AppError> {
        let connection = self.connect()?;
        let mut run = connection
            .query_row(
                "SELECT run.id, run.workflow_id, run.workflow_version_id, version.version_no,
                        run.status, run.input_text, run.output_json, run.error_json,
                        run.started_at, run.completed_at, run.updated_at
                 FROM workflow_runs run
                 JOIN workflow_versions version ON version.id = run.workflow_version_id
                 WHERE run.id = ?1",
                params![run_id],
                |row| {
                    let output_json: Option<String> = row.get(6)?;
                    let error_json: Option<String> = row.get(7)?;
                    Ok(WorkflowRunView {
                        id: row.get(0)?,
                        workflow_id: row.get(1)?,
                        workflow_version_id: row.get(2)?,
                        version_no: row.get(3)?,
                        status: row.get(4)?,
                        input_text: row.get(5)?,
                        outputs: output_json
                            .and_then(|value| serde_json::from_str(&value).ok())
                            .unwrap_or_else(|| Value::Object(Default::default())),
                        error: error_json.and_then(|value| serde_json::from_str(&value).ok()),
                        node_runs: Vec::new(),
                        started_at: row.get(8)?,
                        completed_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("ejecución de flujo {run_id}")))?;
        let mut statement = connection.prepare(
            "SELECT id, node_id, node_kind, node_label, status, input_text, output_text,
                    broker_task_id, error_json, updated_at
             FROM workflow_node_runs WHERE run_id = ?1 ORDER BY rowid",
        )?;
        run.node_runs = statement
            .query_map(params![run_id], |row| {
                let error_json: Option<String> = row.get(8)?;
                Ok(WorkflowNodeRunView {
                    id: row.get(0)?,
                    node_id: row.get(1)?,
                    node_kind: row.get(2)?,
                    node_label: row.get(3)?,
                    status: row.get(4)?,
                    input_text: row.get(5)?,
                    output_text: row.get(6)?,
                    broker_task_id: row.get(7)?,
                    error: error_json.and_then(|value| serde_json::from_str(&value).ok()),
                    updated_at: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(run)
    }

    pub fn list_workflow_runs(&self, workflow_id: &str) -> Result<Vec<WorkflowRunView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id FROM workflow_runs WHERE workflow_id = ?1 ORDER BY created_at DESC LIMIT 25",
        )?;
        let ids = statement
            .query_map(params![workflow_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter().map(|id| self.workflow_run(&id)).collect()
    }

    pub fn update_workflow_run_status(
        &self,
        run_id: &str,
        status: &str,
        outputs: Option<&Value>,
        error: Option<&Value>,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let terminal = matches!(
            status,
            "completed" | "partial_failed" | "failed" | "cancelled"
        );
        connection.execute(
            "UPDATE workflow_runs
             SET status = ?2, output_json = COALESCE(?3, output_json),
                 error_json = ?4,
                 started_at = COALESCE(started_at, datetime('now')),
                 completed_at = CASE WHEN ?5 THEN datetime('now') ELSE completed_at END,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![
                run_id,
                status,
                outputs.map(Value::to_string),
                error.map(Value::to_string),
                terminal
            ],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_workflow_node_run(
        &self,
        run_id: &str,
        node_id: &str,
        status: &str,
        input_text: Option<&str>,
        output_text: Option<&str>,
        broker_task_id: Option<&str>,
        error: Option<&Value>,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let terminal = matches!(status, "completed" | "failed" | "skipped" | "cancelled");
        connection.execute(
            "UPDATE workflow_node_runs
             SET status = ?3, input_text = COALESCE(?4, input_text),
                 output_text = COALESCE(?5, output_text),
                 broker_task_id = COALESCE(?6, broker_task_id), error_json = ?7,
                 started_at = CASE WHEN ?3 = 'running' THEN COALESCE(started_at, datetime('now')) ELSE started_at END,
                 completed_at = CASE WHEN ?8 THEN datetime('now') ELSE completed_at END,
                 updated_at = datetime('now')
             WHERE run_id = ?1 AND node_id = ?2",
            params![
                run_id,
                node_id,
                status,
                input_text,
                output_text,
                broker_task_id,
                error.map(Value::to_string),
                terminal
            ],
        )?;
        Ok(())
    }

    pub fn workflow_run_cancelled(&self, run_id: &str) -> Result<bool, AppError> {
        Ok(self.connect()?.query_row(
            "SELECT status = 'cancelled' FROM workflow_runs WHERE id = ?1",
            params![run_id],
            |row| row.get(0),
        )?)
    }

    pub fn decide_workflow_approval(
        &self,
        run_id: &str,
        node_id: &str,
        approved: bool,
    ) -> Result<WorkflowExecutionRecord, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = if approved {
            transaction.execute(
                "UPDATE workflow_node_runs
                 SET status = 'completed', output_text = input_text, error_json = NULL,
                     completed_at = datetime('now'), updated_at = datetime('now')
                 WHERE run_id = ?1 AND node_id = ?2 AND node_kind = 'approval'
                   AND status = 'waiting_approval'",
                params![run_id, node_id],
            )?
        } else {
            transaction.execute(
                "UPDATE workflow_node_runs
                 SET status = 'failed', error_json = ?3,
                     completed_at = datetime('now'), updated_at = datetime('now')
                 WHERE run_id = ?1 AND node_id = ?2 AND node_kind = 'approval'
                   AND status = 'waiting_approval'",
                params![
                    run_id,
                    node_id,
                    json!({"message": "La persona responsable rechazó esta rama"}).to_string()
                ],
            )?
        };
        if changed == 0 {
            return Err(AppError::Conflict(
                "esta aprobación ya fue resuelta o no está pendiente".to_owned(),
            ));
        }
        transaction.execute(
            "UPDATE workflow_runs SET status = 'queued', error_json = NULL,
                    updated_at = datetime('now')
             WHERE id = ?1 AND status = 'waiting_approval'",
            params![run_id],
        )?;
        transaction.commit()?;
        self.workflow_execution_record(run_id)
    }

    pub fn cancel_workflow_run_locally(&self, run_id: &str) -> Result<Vec<String>, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE workflow_runs SET status = 'cancelled', completed_at = datetime('now'),
                    updated_at = datetime('now')
             WHERE id = ?1 AND status IN ('queued', 'running', 'waiting_approval')",
            params![run_id],
        )?;
        transaction.execute(
            "UPDATE workflow_node_runs SET status = 'cancelled', completed_at = datetime('now'),
                    updated_at = datetime('now')
             WHERE run_id = ?1 AND status IN ('pending', 'running', 'waiting_approval')",
            params![run_id],
        )?;
        let mut statement = transaction.prepare(
            "SELECT broker_task_id FROM workflow_node_runs
             WHERE run_id = ?1 AND broker_task_id IS NOT NULL",
        )?;
        let task_ids = statement
            .query_map(params![run_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        transaction.commit()?;
        Ok(task_ids)
    }

    pub fn recoverable_workflow_run_ids(&self) -> Result<Vec<String>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id FROM workflow_runs WHERE status IN ('queued', 'running') ORDER BY created_at",
        )?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }
}
