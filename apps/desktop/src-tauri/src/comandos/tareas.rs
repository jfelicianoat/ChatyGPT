//! Tareas locales y su estado, y mediciones de rendimiento.

use crate::*;

#[tauri::command]
pub(crate) async fn start_smoke_task(
    state: State<'_, AppState>,
) -> Result<LocalTaskSnapshot, AppError> {
    task_runtime::start_smoke_task(state.database.clone(), state.broker.clone()).await
}

#[tauri::command]
pub(crate) fn get_local_task(
    local_task_id: String,
    state: State<'_, AppState>,
) -> Result<LocalTaskSnapshot, AppError> {
    state.database.task_snapshot(&local_task_id)
}

#[tauri::command]
pub(crate) async fn cancel_local_task(
    local_task_id: String,
    state: State<'_, AppState>,
) -> Result<LocalTaskSnapshot, AppError> {
    task_runtime::cancel_task(state.database.clone(), state.broker.clone(), &local_task_id).await
}

#[tauri::command]
pub(crate) fn list_scheduled_tasks(
    state: State<'_, AppState>,
) -> Result<Vec<ScheduledTaskView>, AppError> {
    state.database.list_scheduled_tasks()
}

#[tauri::command]
pub(crate) fn list_scheduled_runs(
    scheduled_task_id: String,
    status_filter: String,
    period_filter: String,
    sort: String,
    page: i64,
    page_size: i64,
    state: State<'_, AppState>,
) -> Result<ScheduledRunPageView, AppError> {
    state.database.scheduled_run_page(
        &scheduled_task_id,
        status_filter.trim(),
        period_filter.trim(),
        sort.trim(),
        page,
        page_size,
    )
}

/// Registra duraciones observadas por la interfaz.
///
/// La orden acepta un lote porque la respuesta inmediata de la interfaz produce
/// una muestra por interacción: agruparlas evita que medir el rendimiento sea,
/// en sí mismo, un coste de rendimiento.
#[tauri::command]
pub(crate) fn record_performance_samples(
    metric: String,
    durations_ms: Vec<i64>,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let retained = state
        .database
        .record_performance_samples(&metric, &durations_ms)?;
    logging::info(
        "performance.samples_recorded",
        None,
        &[
            ("metric", logging::code(&metric)),
            ("recorded", logging::count(durations_ms.len() as i64)),
            ("retained", logging::count(retained)),
        ],
    );
    Ok(())
}

#[tauri::command]
pub(crate) fn get_performance_report(
    state: State<'_, AppState>,
) -> Result<metrics::PerformanceReportView, AppError> {
    state.database.performance_report()
}

#[tauri::command]
pub(crate) fn clear_performance_samples(
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<metrics::PerformanceReportView, AppError> {
    state.database.clear_performance_samples(confirmed)?;
    state.database.performance_report()
}
