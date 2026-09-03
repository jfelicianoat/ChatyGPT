//! Reclamo, ejecucion y reconciliacion de lo programado.
//!
//! El reclamo es exactamente una vez: dos procesos que despierten a la
//! vez no pueden lanzar la misma tarea dos veces.

use super::*;

impl Database {
    pub fn claim_due_scheduled_task(&self) -> Result<Option<ScheduledClaim>, AppError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let candidate = transaction
            .query_row(
                "SELECT id, next_run_at, schedule_expression,
                        COALESCE(json_extract(payload_json, '$.target_kind'), 'conversation'),
                        json_extract(payload_json, '$.conversation_id'),
                        json_extract(payload_json, '$.workflow_id'),
                        json_extract(payload_json, '$.workflow_version_id'),
                        json_extract(payload_json, '$.prompt')
                 FROM scheduled_tasks
                 WHERE enabled = 1
                   AND confirmed_at IS NOT NULL
                   AND next_run_at IS NOT NULL
                   AND datetime(next_run_at) <= datetime('now')
                 ORDER BY datetime(next_run_at), created_at
                 LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            scheduled_task_id,
            due_at,
            schedule_expression,
            target_kind,
            conversation_id,
            workflow_id,
            workflow_version_id,
            prompt,
        )) = candidate
        else {
            return Ok(None);
        };
        let claim_key = format!("{scheduled_task_id}:{due_at}");
        let run_id = format!("scheduled_run_{}", Uuid::new_v4().simple());
        let inserted = transaction.execute(
            "INSERT OR IGNORE INTO scheduled_runs(
                id, scheduled_task_id, due_at, claim_key, status, attempt
             ) VALUES (?1, ?2, ?3, ?4, 'claimed', 1)",
            params![run_id, scheduled_task_id, due_at, claim_key],
        )?;
        if inserted == 0 {
            transaction.commit()?;
            return Ok(None);
        }
        let next_run_at = match schedule_expression.as_str() {
            "daily" | "weekly" => {
                let modifier = if schedule_expression == "daily" {
                    "+1 day"
                } else {
                    "+7 days"
                };
                transaction.query_row(
                    "WITH RECURSIVE occurrences(value, step) AS (
                        SELECT ?1, 0
                        UNION ALL
                        SELECT strftime(
                                   '%Y-%m-%dT%H:%M:%fZ',
                                   datetime(value, 'localtime', ?2, 'utc')
                               ),
                               step + 1
                        FROM occurrences
                        WHERE datetime(value) <= datetime('now') AND step < 5000
                     )
                     SELECT value
                     FROM occurrences
                     WHERE datetime(value) > datetime('now')
                     ORDER BY step
                     LIMIT 1",
                    params![due_at, modifier],
                    |row| row.get::<_, String>(0),
                )?
            }
            _ => due_at.clone(),
        };
        transaction.execute(
            "UPDATE scheduled_tasks
             SET enabled = CASE WHEN schedule_expression = 'once' THEN 0 ELSE 1 END,
                 next_run_at = ?3,
                 last_claim_key = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![scheduled_task_id, claim_key, next_run_at],
        )?;
        transaction.commit()?;
        Ok(Some(ScheduledClaim {
            run_id,
            scheduled_task_id,
            target_kind,
            conversation_id,
            workflow_id,
            workflow_version_id,
            prompt,
        }))
    }

    pub fn retry_failed_scheduled_run(
        &self,
        run_id: &str,
        confirmed: bool,
    ) -> Result<ScheduledClaim, AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "reintentar una ejecución fallida requiere confirmación explícita".to_owned(),
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let source = transaction
            .query_row(
                "SELECT source.scheduled_task_id,
                        COALESCE(json_extract(task.payload_json, '$.target_kind'), 'conversation'),
                        json_extract(task.payload_json, '$.conversation_id'),
                        json_extract(task.payload_json, '$.workflow_id'),
                        json_extract(task.payload_json, '$.workflow_version_id'),
                        json_extract(task.payload_json, '$.prompt'),
                        COALESCE((
                            SELECT MAX(attempt) FROM scheduled_runs
                            WHERE scheduled_task_id = source.scheduled_task_id
                        ), 0)
                 FROM scheduled_runs source
                 JOIN scheduled_tasks task ON task.id = source.scheduled_task_id
                 WHERE source.id = ?1
                   AND source.status = 'failed'
                   AND (
                       (
                           COALESCE(json_extract(task.payload_json, '$.target_kind'), 'conversation') = 'conversation'
                           AND EXISTS(
                               SELECT 1 FROM conversations conversation
                               WHERE conversation.id = json_extract(task.payload_json, '$.conversation_id')
                                 AND conversation.archived_at IS NULL
                                 AND conversation.deleted_at IS NULL
                           )
                       ) OR (
                           json_extract(task.payload_json, '$.target_kind') = 'workflow'
                           AND EXISTS(
                               SELECT 1 FROM workflows workflow
                               WHERE workflow.id = json_extract(task.payload_json, '$.workflow_id')
                                 AND workflow.archived_at IS NULL
                                 AND workflow.published_version_id IS NOT NULL
                           )
                       )
                   )
                   AND NOT EXISTS(
                       SELECT 1 FROM scheduled_runs active
                       WHERE active.scheduled_task_id = source.scheduled_task_id
                         AND active.status IN ('claimed', 'running')
                   )",
                params![run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            scheduled_task_id,
            target_kind,
            conversation_id,
            workflow_id,
            workflow_version_id,
            prompt,
            maximum_attempt,
        )) = source
        else {
            return Err(AppError::Conflict(
                "esta ejecución ya no admite reintento o existe otra en curso".to_owned(),
            ));
        };
        let retry_run_id = format!("scheduled_run_{}", Uuid::new_v4().simple());
        let attempt = maximum_attempt + 1;
        let claim_key = format!(
            "{scheduled_task_id}:retry:{attempt}:{}",
            Uuid::new_v4().simple()
        );
        transaction.execute(
            "INSERT INTO scheduled_runs(
                id, scheduled_task_id, due_at, claim_key, status, attempt
             ) VALUES (
                ?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?3, 'claimed', ?4
             )",
            params![retry_run_id, scheduled_task_id, claim_key, attempt],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES ('scheduled_run.retry_requested', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({
                    "scheduled_task_id": scheduled_task_id,
                    "source_run_id": run_id,
                    "retry_run_id": retry_run_id,
                    "attempt": attempt
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        Ok(ScheduledClaim {
            run_id: retry_run_id,
            scheduled_task_id,
            target_kind,
            conversation_id,
            workflow_id,
            workflow_version_id,
            prompt,
        })
    }

    pub fn claim_scheduled_task_now(
        &self,
        scheduled_task_id: &str,
        confirmed: bool,
    ) -> Result<ScheduledClaim, AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "ejecutar una programación ahora requiere confirmación explícita".to_owned(),
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let source = transaction
            .query_row(
                "SELECT COALESCE(json_extract(task.payload_json, '$.target_kind'), 'conversation'),
                        json_extract(task.payload_json, '$.conversation_id'),
                        json_extract(task.payload_json, '$.workflow_id'),
                        json_extract(task.payload_json, '$.workflow_version_id'),
                        json_extract(task.payload_json, '$.prompt')
                 FROM scheduled_tasks task
                 WHERE task.id = ?1
                   AND (
                       (
                           COALESCE(json_extract(task.payload_json, '$.target_kind'), 'conversation') = 'conversation'
                           AND EXISTS(
                               SELECT 1 FROM conversations conversation
                               WHERE conversation.id = json_extract(task.payload_json, '$.conversation_id')
                                 AND conversation.archived_at IS NULL
                                 AND conversation.deleted_at IS NULL
                           )
                       ) OR (
                           json_extract(task.payload_json, '$.target_kind') = 'workflow'
                           AND EXISTS(
                               SELECT 1 FROM workflows workflow
                               WHERE workflow.id = json_extract(task.payload_json, '$.workflow_id')
                                 AND workflow.archived_at IS NULL
                                 AND workflow.published_version_id IS NOT NULL
                           )
                       )
                   )
                   AND NOT EXISTS(
                       SELECT 1 FROM scheduled_runs active
                       WHERE active.scheduled_task_id = task.id
                         AND active.status IN ('claimed', 'running')
                   )",
                params![scheduled_task_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((target_kind, conversation_id, workflow_id, workflow_version_id, prompt)) = source
        else {
            return Err(AppError::Conflict(
                "la programación ya tiene una ejecución en curso o su conversación no está disponible"
                    .to_owned(),
            ));
        };
        let manual_run_id = format!("scheduled_run_{}", Uuid::new_v4().simple());
        let claim_key = format!("{scheduled_task_id}:manual:{}", Uuid::new_v4().simple());
        transaction.execute(
            "INSERT INTO scheduled_runs(
                id, scheduled_task_id, due_at, claim_key, status, attempt
             ) VALUES (
                ?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?3, 'claimed', 1
             )",
            params![manual_run_id, scheduled_task_id, claim_key],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES ('scheduled_run.manual_requested', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({
                    "scheduled_task_id": scheduled_task_id,
                    "scheduled_run_id": manual_run_id
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        Ok(ScheduledClaim {
            run_id: manual_run_id,
            scheduled_task_id: scheduled_task_id.to_owned(),
            target_kind,
            conversation_id,
            workflow_id,
            workflow_version_id,
            prompt,
        })
    }

    pub fn start_scheduled_run(&self, run_id: &str, broker_task_id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        connection.execute(
            "UPDATE scheduled_runs
             SET status = 'running', broker_task_id = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND status = 'claimed'",
            params![run_id, broker_task_id],
        )?;
        Ok(())
    }

    pub fn start_scheduled_workflow_run(
        &self,
        run_id: &str,
        workflow_run_id: &str,
    ) -> Result<(), AppError> {
        self.connect()?.execute(
            "UPDATE scheduled_runs
             SET status = 'running', workflow_run_id = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND status = 'claimed'",
            params![run_id, workflow_run_id],
        )?;
        Ok(())
    }

    pub fn fail_scheduled_run(&self, run_id: &str, message: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        connection.execute(
            "UPDATE scheduled_runs
             SET status = 'failed', result_json = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND status IN ('claimed', 'running')",
            params![
                run_id,
                serde_json::json!({ "message": message }).to_string()
            ],
        )?;
        Ok(())
    }

    pub fn scheduled_cancellation_target(
        &self,
        run_id: &str,
        confirmed: bool,
    ) -> Result<ScheduledCancellationTarget, AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "cancelar una ejecución programada requiere confirmación explícita".to_owned(),
            ));
        }
        self.connect()?
            .query_row(
                "SELECT scheduled_task_id, broker_task_id, workflow_run_id
                 FROM scheduled_runs
                 WHERE id = ?1
                   AND status = 'running'
                   AND (broker_task_id IS NOT NULL OR workflow_run_id IS NOT NULL)",
                params![run_id],
                |row| {
                    Ok(ScheduledCancellationTarget {
                        scheduled_task_id: row.get(0)?,
                        broker_task_id: row.get(1)?,
                        workflow_run_id: row.get(2)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Conflict(
                    "la ejecución ya terminó o todavía no puede cancelarse".to_owned(),
                )
            })
    }

    pub fn finish_scheduled_cancellation(
        &self,
        run_id: &str,
        execution_id: &str,
    ) -> Result<(), AppError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE scheduled_runs
             SET status = 'cancelled',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1
               AND (broker_task_id = ?2 OR workflow_run_id = ?2)
               AND status IN ('running', 'cancelled')",
            params![run_id, execution_id],
        )?;
        if changed == 0 {
            return Err(AppError::Conflict(
                "la ejecución cambió de estado antes de completar la cancelación".to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             SELECT 'scheduled_run.cancelled', 'user',
                    json_extract(task.payload_json, '$.conversation_id'),
                    json_object(
                        'scheduled_task_id', run.scheduled_task_id,
                        'scheduled_run_id', run.id,
                        'broker_task_id', run.broker_task_id
                    )
             FROM scheduled_runs run
             JOIN scheduled_tasks task ON task.id = run.scheduled_task_id
             WHERE run.id = ?1",
            params![run_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn reconcile_scheduled_runs(&self) -> Result<usize, AppError> {
        let connection = self.connect()?;
        let broker_runs = connection.execute(
            "UPDATE scheduled_runs
             SET status = (
                    SELECT CASE bt.remote_status
                        WHEN 'completed' THEN 'completed'
                        WHEN 'cancelled' THEN 'cancelled'
                        ELSE 'failed'
                    END
                    FROM broker_tasks bt WHERE bt.id = scheduled_runs.broker_task_id
                 ),
                 result_json = (
                    SELECT COALESCE(bt.result_json, bt.error_json)
                    FROM broker_tasks bt WHERE bt.id = scheduled_runs.broker_task_id
                 ),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE status = 'running'
               AND broker_task_id IS NOT NULL
               AND EXISTS(
                    SELECT 1 FROM broker_tasks bt
                    WHERE bt.id = scheduled_runs.broker_task_id
                      AND bt.remote_status IN ('completed', 'failed', 'cancelled')
               )",
            [],
        )?;
        let workflow_runs = connection.execute(
            "UPDATE scheduled_runs
             SET status = (
                    SELECT CASE workflow.status
                        WHEN 'completed' THEN 'completed'
                        WHEN 'cancelled' THEN 'cancelled'
                        ELSE 'failed'
                    END
                    FROM workflow_runs workflow
                    WHERE workflow.id = scheduled_runs.workflow_run_id
                 ),
                 result_json = (
                    SELECT json_object(
                        'workflow_run_id', workflow.id,
                        'outputs', json(COALESCE(workflow.output_json, '{}')),
                        'error', CASE
                            WHEN workflow.error_json IS NULL THEN NULL
                            ELSE json(workflow.error_json)
                        END
                    )
                    FROM workflow_runs workflow
                    WHERE workflow.id = scheduled_runs.workflow_run_id
                 ),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE status = 'running'
               AND workflow_run_id IS NOT NULL
               AND EXISTS(
                    SELECT 1 FROM workflow_runs workflow
                    WHERE workflow.id = scheduled_runs.workflow_run_id
                      AND workflow.status IN ('completed', 'partial_failed', 'failed', 'cancelled')
               )",
            [],
        )?;
        Ok(broker_runs + workflow_runs)
    }
}
