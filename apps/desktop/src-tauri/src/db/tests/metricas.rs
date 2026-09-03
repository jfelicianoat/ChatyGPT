//! Mediciones de rendimiento: acotadas, tipadas y sin datos personales.

use super::comunes::{cleanup, test_database};
use crate::db::{PERFORMANCE_SAMPLES_MIGRATION, REMOTE_OPERATION_START_METRIC_MIGRATION};
use crate::error::AppError;
use rusqlite::Connection;

#[test]
fn performance_samples_are_bounded_typed_and_free_of_personal_content() {
    let database = test_database();

    // Una métrica desconocida no llega a tocar la tabla.
    let rejected = database.record_performance_samples("prompt del usuario", &[10]);
    assert!(matches!(rejected, Err(AppError::Validation(_))));
    // Tampoco una duración imposible.
    assert!(matches!(
        database.record_performance_samples("app_start", &[-1]),
        Err(AppError::Validation(_))
    ));
    assert!(matches!(
        database.record_performance_samples("app_start", &[600_001]),
        Err(AppError::Validation(_))
    ));
    // Ni un lote mayor que el máximo por llamada.
    let oversized = vec![1_i64; 101];
    assert!(matches!(
        database.record_performance_samples("ui_response", &oversized),
        Err(AppError::Validation(_))
    ));

    // La retención es real: 250 muestras dejan exactamente 200 conservadas.
    let durations: Vec<i64> = (1..=250).collect();
    for lote in durations.chunks(50) {
        database
            .record_performance_samples("conversation_open", lote)
            .expect("las muestras válidas deben registrarse");
    }
    let connection = database.connect().expect("connection should open");
    let retained: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM performance_samples WHERE metric = 'conversation_open'",
            [],
            |row| row.get(0),
        )
        .expect("count should succeed");
    assert_eq!(retained, 200);
    // Se conservan las últimas, no las primeras.
    let oldest: i64 = connection
        .query_row(
            "SELECT MIN(duration_ms) FROM performance_samples
             WHERE metric = 'conversation_open'",
            [],
            |row| row.get(0),
        )
        .expect("min should succeed");
    assert_eq!(oldest, 51);

    // La tabla no tiene ninguna columna capaz de guardar texto libre.
    let columns: Vec<String> = connection
        .prepare("SELECT name FROM pragma_table_info('performance_samples')")
        .expect("pragma should prepare")
        .query_map([], |row| row.get::<_, String>(0))
        .expect("pragma should run")
        .collect::<Result<Vec<_>, _>>()
        .expect("column names should be readable");
    assert_eq!(columns, vec!["id", "metric", "duration_ms", "recorded_at"]);
    drop(connection);

    cleanup(&database);
}

#[test]
fn remote_start_metric_migration_preserves_existing_measurements() {
    let connection = Connection::open_in_memory().expect("SQLite temporal debe abrir");
    connection
        .execute_batch(PERFORMANCE_SAMPLES_MIGRATION)
        .expect("el esquema anterior debe crearse");
    connection
        .execute(
            "INSERT INTO performance_samples(metric, duration_ms) VALUES ('app_start', 850)",
            [],
        )
        .expect("la muestra anterior debe guardarse");

    connection
        .execute_batch(REMOTE_OPERATION_START_METRIC_MIGRATION)
        .expect("la ampliación del vocabulario debe ser atómica");
    let preserved: i64 = connection
        .query_row(
            "SELECT duration_ms FROM performance_samples WHERE metric = 'app_start'",
            [],
            |row| row.get(0),
        )
        .expect("la muestra previa debe sobrevivir");
    assert_eq!(preserved, 850);
    connection
        .execute(
            "INSERT INTO performance_samples(metric, duration_ms) \
             VALUES ('remote_operation_start', 75)",
            [],
        )
        .expect("la métrica nueva debe quedar admitida");
}

#[test]
fn performance_report_only_judges_metrics_that_were_executed() {
    let database = test_database();

    // Sin ninguna muestra, ninguna métrica obtiene veredicto.
    let empty = database
        .performance_report()
        .expect("el informe vacío debe poder consultarse");
    assert_eq!(empty.metrics.len(), 5);
    assert_eq!(empty.total_samples, 0);
    assert_eq!(empty.sample_limit, 200);
    for summary in &empty.metrics {
        assert_eq!(summary.samples, 0);
        assert_eq!(summary.meets_budget, None);
        assert!(summary.last_recorded_at.is_none());
        assert!(summary.budget_ms > 0);
    }

    // Veinte aperturas rápidas cumplen el objetivo; la búsqueda lenta no.
    database
        .record_performance_samples("conversation_open", &[120; 20])
        .expect("las aperturas deben registrarse");
    database
        .record_performance_samples("conversation_search", &[900; 10])
        .expect("las búsquedas deben registrarse");

    let report = database
        .performance_report()
        .expect("el informe debe poder consultarse");
    assert_eq!(report.total_samples, 30);
    let open = report
        .metrics
        .iter()
        .find(|summary| summary.metric == "conversation_open")
        .expect("la apertura debe figurar");
    assert_eq!(open.samples, 20);
    assert_eq!(open.p95_ms, Some(120));
    assert_eq!(open.meets_budget, Some(true));
    assert!(open.last_recorded_at.is_some());
    let search = report
        .metrics
        .iter()
        .find(|summary| summary.metric == "conversation_search")
        .expect("la búsqueda debe figurar");
    assert_eq!(search.meets_budget, Some(false));
    // Las métricas nunca ejecutadas siguen sin veredicto en el mismo informe.
    let ui = report
        .metrics
        .iter()
        .find(|summary| summary.metric == "ui_response")
        .expect("la respuesta de interfaz debe figurar");
    assert_eq!(ui.meets_budget, None);

    // Vaciar exige confirmación y queda auditado.
    assert!(matches!(
        database.clear_performance_samples(false),
        Err(AppError::Validation(_))
    ));
    database
        .clear_performance_samples(true)
        .expect("vaciar confirmado debe funcionar");
    let cleared = database
        .performance_report()
        .expect("el informe debe seguir consultándose");
    assert_eq!(cleared.total_samples, 0);
    let connection = database.connect().expect("connection should open");
    let audited: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM audit_events
             WHERE event_type = 'performance.samples_cleared'",
            [],
            |row| row.get(0),
        )
        .expect("audit count should succeed");
    assert_eq!(audited, 1);
    drop(connection);

    cleanup(&database);
}
