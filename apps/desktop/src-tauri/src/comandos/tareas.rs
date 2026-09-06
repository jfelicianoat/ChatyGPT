//! Tareas locales y su estado, y mediciones de rendimiento.

use crate::*;

// Los artefactos vienen del espejo del contrato del Broker (8.3).
use crate::broker::TaskArtifact;

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

/// Ficheros que produjo una tarea del Broker (contrato 2.10, 8.3).
///
/// Las imágenes que devuelve un modelo NO viajan en `result`: el resultado JSON
/// se lee entero en cada sondeo del estado, así que un PNG en base64 dentro lo
/// convertiría en megabytes por lectura. Van aparte, como artefactos.
#[tauri::command]
pub(crate) async fn list_task_artifacts(
    remote_task_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<TaskArtifact>, AppError> {
    state.broker.artifacts(&remote_task_id).await
}

/// Recoge un artefacto y lo guarda en el almacén gestionado de adjuntos.
///
/// Devuelve la ruta local, para que la interfaz pueda abrirlo. Un artefacto con
/// `available: false` existió y ya se borró —lo podó la retención del Broker—,
/// que no es lo mismo que no haber existido nunca: se dice tal cual en vez de
/// devolver un «no encontrado» que mandaría a buscar donde no hay nada.
#[tauri::command]
pub(crate) async fn save_task_artifact(
    remote_task_id: String,
    artifact_id: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let artifacts = state.broker.artifacts(&remote_task_id).await?;
    let artifact = artifacts
        .iter()
        .find(|item| item.artifact_id == artifact_id)
        .ok_or_else(|| AppError::Validation("la tarea no tiene ese artefacto".to_owned()))?;
    if !artifact.available {
        return Err(AppError::BrokerContract(
            "el artefacto existió y ya se ha borrado (retención del Broker)".to_owned(),
        ));
    }
    let bytes = state
        .broker
        .download_artifact(&remote_task_id, &artifact_id)
        .await?;
    let name = if artifact.filename.trim().is_empty() {
        artifact_id.as_str()
    } else {
        artifact.filename.as_str()
    };
    let stored = attachment_runtime::store_broker_artifact(
        state.attachments_dir.clone(),
        name.to_owned(),
        bytes,
    )
    .await?;
    Ok(stored)
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
