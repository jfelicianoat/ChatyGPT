//! Pasos de investigacion profunda y tareas remotas abandonadas.

use super::*;

impl Database {
    /// Tareas que aquí se dieron por perdidas pero siguen vivas en el Broker.
    ///
    /// Una tarea queda `orphaned` cuando un error permanente impide seguir
    /// atendiéndola —por ejemplo, si el envío de resultados de herramienta es
    /// rechazado por contrato—. La recuperación las excluye a propósito: no
    /// tiene sentido reintentar algo que no puede mejorar repitiéndolo.
    ///
    /// El problema es el otro lado. Si el Broker la dejó pausada esperando una
    /// herramienta, `waiting_for_tools` **no caduca**: seguiría esperando una
    /// respuesta que ChatyGPT ya no va a enviar. Estas son las que hay que
    /// cerrar explícitamente al arrancar.
    pub fn abandoned_remote_tasks(&self) -> Result<Vec<(String, String)>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, remote_task_id FROM broker_tasks
             WHERE local_state = 'orphaned'
               AND remote_task_id IS NOT NULL
               AND remote_status NOT IN ('completed', 'failed', 'cancelled')
             ORDER BY created_at",
        )?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Deja constancia de que se cerró una tarea abandonada.
    ///
    /// Se audita porque es trabajo del Broker que ChatyGPT decide descartar sin
    /// preguntar: conviene poder responder después a «¿quién canceló esto?».
    pub fn record_abandoned_cancellation(
        &self,
        local_task_id: &str,
        remote_task_id: &str,
        remote_status: &str,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             SELECT 'task.abandoned_cancelled', 'chatygpt', conversation_id, ?2
             FROM broker_tasks WHERE id = ?1",
            params![
                local_task_id,
                serde_json::json!({
                    "broker_task_id": local_task_id,
                    "remote_task_id": remote_task_id,
                    "remote_status": remote_status
                })
                .to_string()
            ],
        )?;
        Ok(())
    }

    /// Anota una herramienta ejecutada como paso real de la investigación.
    ///
    /// Sustituye a las tres etapas fijas por lo que de verdad ocurrió: cada
    /// llamada que el modelo pidió, con su parámetro visible —la URL—, su
    /// resultado y su marca de tiempo. El `tool_call_id` es la identidad: el
    /// mismo paso no se registra dos veces aunque una recuperación reejecute
    /// la herramienta.
    ///
    /// `kind` es `research` porque el CHECK de la tabla solo admite las tres
    /// clases originales; el detalle real vive en `objective` y `result_json`.
    pub fn record_research_tool_step(
        &self,
        broker_task_id: &str,
        tool_call_id: &str,
        tool_name: &str,
        argument: &str,
        status: &str,
        result: &Value,
    ) -> Result<(), AppError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let research_run_id: Option<String> = transaction
            .query_row(
                "SELECT id FROM research_runs WHERE broker_task_id = ?1",
                params![broker_task_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(research_run_id) = research_run_id else {
            return Ok(());
        };
        // La identidad del paso es la llamada, no su posición: reejecutar la
        // herramienta tras un reinicio actualiza el mismo registro.
        let existing: Option<String> = transaction
            .query_row(
                "SELECT id FROM research_steps
                 WHERE research_run_id = ?1
                   AND json_extract(result_json, '$.tool_call_id') = ?2",
                params![research_run_id, tool_call_id],
                |row| row.get(0),
            )
            .optional()?;
        let mut payload = result.clone();
        payload["tool_call_id"] = serde_json::json!(tool_call_id);
        payload["tool"] = serde_json::json!(tool_name);
        let payload_json = payload.to_string();
        match existing {
            Some(step_id) => {
                transaction.execute(
                    "UPDATE research_steps
                     SET status = ?2,
                         result_json = ?3,
                         completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                         updated_at = datetime('now')
                     WHERE id = ?1",
                    params![step_id, status, payload_json],
                )?;
            }
            None => {
                let next_ordinal: i64 = transaction.query_row(
                    "SELECT COALESCE(MAX(ordinal), -1) + 1 FROM research_steps
                     WHERE research_run_id = ?1",
                    params![research_run_id],
                    |row| row.get(0),
                )?;
                transaction.execute(
                    "INSERT INTO research_steps(
                        id, research_run_id, ordinal, objective, status,
                        broker_task_id, kind, title, started_at, completed_at,
                        result_json
                     ) VALUES (
                        ?1, ?2, ?3, ?4, ?5, ?6, 'research', ?7,
                        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                        ?8
                     )",
                    params![
                        format!("research_step_{}", Uuid::new_v4().simple()),
                        research_run_id,
                        next_ordinal,
                        argument,
                        status,
                        broker_task_id,
                        format!("{tool_name}: {argument}"),
                        payload_json
                    ],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }
}
