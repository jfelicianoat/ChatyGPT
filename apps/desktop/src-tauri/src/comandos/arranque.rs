//! Arranque, credenciales del Broker y de APIs, e inicio con Windows.

use crate::*;

#[tauri::command]
pub(crate) fn bootstrap_app(state: State<'_, AppState>) -> Result<BootstrapReport, AppError> {
    Ok(BootstrapReport {
        app_version: env!("CARGO_PKG_VERSION"),
        database_path: state.database.path().display().to_string(),
        log_path: logging::log_path().map(|path| path.display().to_string()),
        schema_version: state.database.schema_version()?,
        recovered_tasks: state.recovered_at_start,
        recovered_attachments: state.recovered_attachments_at_start,
        recovered_workflows: state.recovered_workflows_at_start,
        recovery_items: state.recovery_items_at_start.clone(),
        restore_notice: state.restore_notice.clone(),
    })
}

/// Elige la carpeta donde crear o desde la que leer una copia de seguridad.
#[tauri::command]
pub(crate) fn pick_backup_folder() -> Result<Option<String>, AppError> {
    crate::comandos::pick_folder("Elige la carpeta de las copias de seguridad de ChatyGPT")
}

/// Crea una copia completa y verificada (H16).
#[tauri::command]
pub(crate) async fn create_backup(
    destination_folder: String,
    state: State<'_, AppState>,
) -> Result<respaldo::BackupReport, AppError> {
    let database = state.database.clone();
    let data_dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        respaldo::create_backup(
            &database,
            &data_dir,
            std::path::Path::new(&destination_folder),
        )
    })
    .await
    .map_err(|error| AppError::DataDirectory(error.to_string()))?
}

/// Verifica una copia sin tocar los datos en uso.
#[tauri::command]
pub(crate) async fn inspect_backup(folder: String) -> Result<respaldo::BackupReport, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        respaldo::verify_backup(std::path::Path::new(&folder))
    })
    .await
    .map_err(|error| AppError::DataDirectory(error.to_string()))?
}

/// Programa la restauración de una copia para el próximo arranque.
#[tauri::command]
pub(crate) async fn schedule_backup_restore(
    folder: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<respaldo::BackupReport, AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "restaurar una copia requiere confirmación explícita".to_owned(),
        ));
    }
    let data_dir = state.data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        respaldo::schedule_restore(&data_dir, std::path::Path::new(&folder))
    })
    .await
    .map_err(|error| AppError::DataDirectory(error.to_string()))?
}

#[tauri::command]
pub(crate) async fn diagnose_broker(
    state: State<'_, AppState>,
) -> Result<BrokerDiagnostic, AppError> {
    Ok(state.broker.diagnose().await)
}

#[tauri::command]
pub(crate) fn get_broker_credential(
    state: State<'_, AppState>,
) -> Result<secrets::BrokerCredentialStatus, AppError> {
    Ok(secrets::credential_status(&state.data_dir))
}

/// Guarda el token del Broker cifrado para esta cuenta de Windows.
///
/// El valor nunca se devuelve al frontend ni se persiste en SQLite: solo se
/// informa del estado resultante.
#[tauri::command]
pub(crate) fn set_broker_credential(
    token: String,
    state: State<'_, AppState>,
) -> Result<secrets::BrokerCredentialStatus, AppError> {
    secrets::store_broker_token(&state.data_dir, &token)?;
    state.broker.replace_admin_token(Some(token.trim()))?;
    // Si el inicio con Windows está activo, su copia protegida se pone al día.
    let _ = startup::refresh_protected_token_if_enabled(&state.data_dir);
    state.database.record_broker_credential_changed(true)?;
    logging::info("broker.credential_stored", None, &[]);
    Ok(secrets::credential_status(&state.data_dir))
}

#[tauri::command]
pub(crate) fn clear_broker_credential(
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<secrets::BrokerCredentialStatus, AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "retirar la credencial requiere confirmación".to_owned(),
        ));
    }
    secrets::clear_broker_token(&state.data_dir)?;
    // Tras retirarla, solo queda lo que aporte el entorno de este proceso.
    state
        .broker
        .replace_admin_token(secrets::environment_token().as_deref())?;
    state.database.record_broker_credential_changed(false)?;
    logging::info("broker.credential_cleared", None, &[]);
    Ok(secrets::credential_status(&state.data_dir))
}

#[tauri::command]
pub(crate) fn list_api_credentials(
    state: State<'_, AppState>,
) -> Result<Vec<secrets::ApiCredentialStatus>, AppError> {
    secrets::list_api_credentials(&state.data_dir)
}

#[tauri::command]
pub(crate) fn set_api_credential(
    name: String,
    secret: String,
    state: State<'_, AppState>,
) -> Result<Vec<secrets::ApiCredentialStatus>, AppError> {
    secrets::store_api_credential(&state.data_dir, &name, &secret)?;
    logging::info("api.credential_stored", None, &[]);
    secrets::list_api_credentials(&state.data_dir)
}

#[tauri::command]
pub(crate) fn clear_api_credential(
    name: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<Vec<secrets::ApiCredentialStatus>, AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "retirar la credencial API requiere confirmación".to_owned(),
        ));
    }
    secrets::clear_api_credential(&state.data_dir, &name)?;
    logging::info("api.credential_cleared", None, &[]);
    secrets::list_api_credentials(&state.data_dir)
}

#[tauri::command]
pub(crate) fn get_windows_startup_status(
    state: State<'_, AppState>,
) -> Result<startup::WindowsStartupStatus, AppError> {
    startup::status(&state.data_dir)
}

#[tauri::command]
pub(crate) fn set_windows_startup_enabled(
    enabled: bool,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<startup::WindowsStartupStatus, AppError> {
    let status = startup::set_enabled(
        &state.data_dir,
        &state.broker.base_url(),
        enabled,
        confirmed,
    )?;
    state
        .database
        .record_windows_startup_changed(status.enabled, status.credential_protected)?;
    Ok(status)
}
