mod broker;
mod db;
mod error;

use std::sync::Mutex;

use broker::{BrokerClient, BrokerDiagnostic};
use db::Database;
use error::AppError;
use serde::Serialize;
use tauri::{Manager, State};

struct AppState {
    database: Mutex<Option<Database>>,
    broker: BrokerClient,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapReport {
    app_version: &'static str,
    database_path: String,
    schema_version: i64,
    recovered_tasks: usize,
}

#[tauri::command]
fn bootstrap_app(state: State<'_, AppState>) -> Result<BootstrapReport, AppError> {
    let database = state.database.lock().map_err(|_| AppError::NotInitialized)?;
    let database = database.as_ref().ok_or(AppError::NotInitialized)?;
    Ok(BootstrapReport {
        app_version: env!("CARGO_PKG_VERSION"),
        database_path: database.path().display().to_string(),
        schema_version: database.schema_version()?,
        recovered_tasks: database.recover_non_terminal_tasks()?,
    })
}

#[tauri::command]
async fn diagnose_broker(state: State<'_, AppState>) -> Result<BrokerDiagnostic, AppError> {
    Ok(state.broker.diagnose().await)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_local_data_dir()
                .map_err(|error| AppError::DataDirectory(error.to_string()))?;
            std::fs::create_dir_all(&data_dir)
                .map_err(|error| AppError::DataDirectory(error.to_string()))?;
            let database = Database::open(data_dir.join("chatygpt.db"))?;
            let broker = BrokerClient::from_environment()?;
            app.manage(AppState {
                database: Mutex::new(Some(database)),
                broker,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![bootstrap_app, diagnose_broker])
        .run(tauri::generate_context!())
        .expect("ChatyGPT no pudo iniciar");
}

