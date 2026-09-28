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
    /// Copia conservada en el equipo, si la persona la guardó (H23).
    local_path: Option<String>,
    saved_at: Option<String>,
}

impl TaskArtifactView {
    fn with_local_copy(mut self, copy: Option<&crate::db::LocalArtifactCopy>) -> Self {
        if let Some(copy) = copy {
            // Una copia que ya no está en disco no se ofrece como disponible.
            if std::path::Path::new(&copy.local_path).is_file() {
                self.local_path = Some(copy.local_path.clone());
                self.saved_at = Some(copy.saved_at.clone());
            }
        }
        self
    }

    /// Vista de un artefacto que solo existe ya como copia local.
    fn from_local_copy(copy: &crate::db::LocalArtifactCopy) -> Self {
        Self {
            artifact_id: copy.artifact_id.clone(),
            artifact_type: "local_copy".to_owned(),
            filename: copy.filename.clone(),
            media_type: copy.media_type.clone(),
            size_bytes: Some(copy.size_bytes),
            sha256: Some(copy.sha256.clone()),
            download_url: None,
            available: false,
            is_final: None,
            local_path: None,
            saved_at: None,
        }
        .with_local_copy(Some(copy))
    }
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
            local_path: None,
            saved_at: None,
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
///
/// Se combinan con las copias que la persona conservó: si el Broker ya podó
/// el artefacto, o no responde, la copia local sigue apareciendo (H23).
#[tauri::command]
pub(crate) async fn list_task_artifacts(
    remote_task_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<TaskArtifactView>, AppError> {
    let copies = state.database.task_artifact_copies(&remote_task_id)?;
    let remote = match state.broker.artifacts(&remote_task_id).await {
        Ok(items) => items,
        Err(error) if copies.is_empty() => return Err(error),
        Err(_) => Vec::new(),
    };
    let mut views = remote
        .into_iter()
        .map(|artifact| {
            let copy = copies
                .iter()
                .find(|copy| copy.artifact_id == artifact.artifact_id);
            TaskArtifactView::from(artifact).with_local_copy(copy)
        })
        .collect::<Vec<_>>();
    for copy in &copies {
        if !views
            .iter()
            .any(|view| view.artifact_id == copy.artifact_id)
        {
            views.push(TaskArtifactView::from_local_copy(copy));
        }
    }
    Ok(views)
}

/// Descarga un artefacto al almacén gestionado y registra la copia.
async fn keep_task_artifact(
    state: &State<'_, AppState>,
    remote_task_id: &str,
    artifact_id: &str,
) -> Result<crate::db::LocalArtifactCopy, AppError> {
    if let Some(copy) = state
        .database
        .task_artifact_copy(remote_task_id, artifact_id)?
        .filter(|copy| std::path::Path::new(&copy.local_path).is_file())
    {
        return Ok(copy);
    }
    let artifacts = state.broker.artifacts(remote_task_id).await?;
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
        .download_artifact(remote_task_id, artifact_id)
        .await?;
    let name = if artifact.filename.trim().is_empty() {
        artifact_id
    } else {
        artifact.filename.as_str()
    };
    let size_bytes = bytes.len() as u64;
    let sha256 = {
        use sha2::{Digest, Sha256};
        format!("{:x}", Sha256::digest(&bytes))
    };
    let stored = attachment_runtime::store_broker_artifact(
        state.attachments_dir.clone(),
        name.to_owned(),
        bytes,
        artifact.sha256.clone(),
    )
    .await?;
    state.database.record_task_artifact_copy(
        remote_task_id,
        artifact_id,
        name,
        artifact.media_type.as_deref(),
        size_bytes,
        &sha256,
        &stored,
    )
}

/// Conserva un artefacto en el equipo y devuelve su ruta local.
///
/// Un artefacto con `available: false` existió y ya se borró —lo podó la
/// retención del Broker—, que no es lo mismo que no haber existido nunca: se
/// dice tal cual en vez de devolver un «no encontrado».
#[tauri::command]
pub(crate) async fn save_task_artifact(
    remote_task_id: String,
    artifact_id: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    Ok(keep_task_artifact(&state, &remote_task_id, &artifact_id)
        .await?
        .local_path)
}

/// «Guardar como…»: copia el artefacto donde la persona elija.
///
/// Devuelve la ruta elegida, o `None` si cerró el diálogo sin guardar.
#[tauri::command]
pub(crate) async fn save_task_artifact_as(
    remote_task_id: String,
    artifact_id: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, AppError> {
    let copy = keep_task_artifact(&state, &remote_task_id, &artifact_id).await?;
    let source = crate::comandos::validated_managed_source_path(
        &state.attachments_dir,
        std::path::Path::new(&copy.local_path),
    )?;
    let Some(destination) = crate::comandos::pick_save_destination(&copy.filename)? else {
        return Ok(None);
    };
    std::fs::copy(&source, &destination).map_err(|error| {
        AppError::DataDirectory(format!("no se pudo guardar el fichero: {error}"))
    })?;
    Ok(Some(destination))
}

/// Muestra en el Explorador la copia conservada de un artefacto.
#[tauri::command]
pub(crate) fn reveal_task_artifact(
    remote_task_id: String,
    artifact_id: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let copy = state
        .database
        .task_artifact_copy(&remote_task_id, &artifact_id)?
        .ok_or_else(|| {
            AppError::NotFound("este fichero todavía no se ha conservado en el equipo".to_owned())
        })?;
    let path = crate::comandos::validated_managed_source_path(
        &state.attachments_dir,
        std::path::Path::new(&copy.local_path),
    )?;
    crate::comandos::reveal_in_explorer(&path)?;
    Ok(copy.filename)
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
