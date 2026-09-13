//! Tareas locales y su estado, y mediciones de rendimiento.

use crate::*;

// Los artefactos vienen del espejo del contrato del Broker (8.3).
use crate::broker::TaskArtifact;

/// Vista estable del contrato IPC. El Broker habla `snake_case`; React recibe
/// `camelCase`. Mantener ambos límites separados evita que un cambio de
/// serialización para Tauri rompa la deserialización de la API remota.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TaskArtifactView {
    artifact_id: String,
    artifact_type: String,
    filename: String,
    media_type: Option<String>,
    size_bytes: Option<u64>,
    sha256: Option<String>,
    download_url: Option<String>,
    available: bool,
    #[serde(rename = "final")]
    is_final: Option<bool>,
}

impl From<TaskArtifact> for TaskArtifactView {
    fn from(artifact: TaskArtifact) -> Self {
        Self {
            artifact_id: artifact.artifact_id,
            artifact_type: artifact.artifact_type,
            filename: artifact.filename,
            media_type: artifact.media_type,
            size_bytes: artifact.size_bytes,
            sha256: artifact.sha256,
            download_url: artifact.download_url,
            available: artifact.available,
            is_final: artifact.is_final,
        }
    }
}

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
) -> Result<Vec<TaskArtifactView>, AppError> {
    state
        .broker
        .artifacts(&remote_task_id)
        .await
        .map(|items| items.into_iter().map(TaskArtifactView::from).collect())
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
        artifact.sha256.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_artifact_ipc_view_uses_the_names_consumed_by_react() {
        let value = serde_json::to_value(TaskArtifactView::from(TaskArtifact {
            artifact_id: "artifact-1".to_owned(),
            artifact_type: "image_output".to_owned(),
            filename: "image.png".to_owned(),
            media_type: Some("image/png".to_owned()),
            size_bytes: Some(42),
            sha256: Some("abc123".to_owned()),
            download_url: Some("/artifact-1".to_owned()),
            available: true,
            is_final: Some(true),
        }))
        .expect("IPC artifact should serialize");

        assert_eq!(value["artifactId"], "artifact-1");
        assert_eq!(value["artifactType"], "image_output");
        assert_eq!(value["mediaType"], "image/png");
        assert_eq!(value["sizeBytes"], 42);
        assert_eq!(value["downloadUrl"], "/artifact-1");
        assert_eq!(value["final"], true);
        assert!(value.get("artifact_id").is_none());
        assert!(value.get("artifact_type").is_none());
    }
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
