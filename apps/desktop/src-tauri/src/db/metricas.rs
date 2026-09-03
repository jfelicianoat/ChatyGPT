//! Mediciones de rendimiento locales, acotadas y sin contenido personal.

use super::*;

impl Database {
    /// Registra un lote de duraciones de una misma métrica y poda las antiguas.
    ///
    /// Insertar y podar en la misma transacción es lo que hace que el límite sea
    /// real: no existe un instante en el que la tabla supere las muestras
    /// conservadas, ni una tarea de mantenimiento que pueda no ejecutarse nunca.
    pub fn record_performance_samples(
        &self,
        metric: &str,
        durations_ms: &[i64],
    ) -> Result<i64, AppError> {
        let metric = PerformanceMetric::parse(metric).ok_or_else(|| {
            AppError::Validation("la métrica de rendimiento no es válida".to_owned())
        })?;
        if durations_ms.is_empty() {
            return Ok(0);
        }
        if durations_ms.len() > metrics::MAX_SAMPLES_PER_CALL {
            return Err(AppError::Validation(
                "demasiadas muestras de rendimiento en una sola llamada".to_owned(),
            ));
        }
        if let Some(invalid) = durations_ms
            .iter()
            .find(|duration| !metrics::is_reportable_sample(**duration))
        {
            return Err(AppError::Validation(format!(
                "la duración {invalid} ms está fuera del rango admitido"
            )));
        }

        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        {
            let mut insert = transaction
                .prepare("INSERT INTO performance_samples(metric, duration_ms) VALUES (?1, ?2)")?;
            for duration in durations_ms {
                insert.execute(params![metric.as_str(), duration])?;
            }
        }
        transaction.execute(
            "DELETE FROM performance_samples
             WHERE metric = ?1
               AND id NOT IN (
                   SELECT id FROM performance_samples
                   WHERE metric = ?1
                   ORDER BY id DESC
                   LIMIT ?2
               )",
            params![metric.as_str(), metrics::MAX_SAMPLES_PER_METRIC],
        )?;
        let retained: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM performance_samples WHERE metric = ?1",
            params![metric.as_str()],
            |row| row.get(0),
        )?;
        transaction.commit()?;
        Ok(retained)
    }

    /// Informe de rendimiento sobre las muestras conservadas.
    pub fn performance_report(&self) -> Result<PerformanceReportView, AppError> {
        let connection = self.connect()?;
        let mut summaries = Vec::with_capacity(PerformanceMetric::ALL.len());
        let mut total = 0_i64;
        for metric in PerformanceMetric::ALL {
            let mut statement = connection.prepare(
                "SELECT duration_ms FROM performance_samples
                 WHERE metric = ?1
                 ORDER BY id DESC
                 LIMIT ?2",
            )?;
            let durations = statement
                .query_map(
                    params![metric.as_str(), metrics::MAX_SAMPLES_PER_METRIC],
                    |row| row.get::<_, i64>(0),
                )?
                .collect::<Result<Vec<_>, _>>()?;
            let last_recorded_at = connection
                .query_row(
                    "SELECT recorded_at FROM performance_samples
                     WHERE metric = ?1
                     ORDER BY id DESC
                     LIMIT 1",
                    params![metric.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            total += durations.len() as i64;
            summaries.push(metrics::summarize(metric, &durations, last_recorded_at));
        }
        Ok(PerformanceReportView {
            metrics: summaries,
            sample_limit: metrics::MAX_SAMPLES_PER_METRIC,
            total_samples: total,
        })
    }

    /// Borra todas las mediciones. Exige confirmación y queda auditado.
    pub fn clear_performance_samples(&self, confirmed: bool) -> Result<(), AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "vaciar las mediciones requiere confirmación explícita".to_owned(),
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let removed = transaction.execute("DELETE FROM performance_samples", [])?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('performance.samples_cleared', 'user', ?1)",
            params![serde_json::json!({ "removed_samples": removed }).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }
}
