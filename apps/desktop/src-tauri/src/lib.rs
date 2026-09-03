mod athena;
mod attachment_runtime;
mod broker;
mod db;
mod error;
mod export;
mod logging;
mod metrics;
mod research_tools;
mod scheduler_runtime;
mod secrets;
mod startup;
mod task_runtime;
mod workflow_runtime;

use broker::{BrokerClient, BrokerDiagnostic};
use db::{
    AttachmentView, AuditEventView, AuthorizedFolderView, ContextSnapshotView,
    ConversationExecutionPreferences, ConversationSummary, ConversationSummaryOverview,
    ConversationView, CustomGptImportReport, CustomGptToolPermissions, CustomGptView, Database,
    LocalTaskSnapshot, MemoryOverview, MemorySearchView, ProjectKnowledgeOverview, ProjectSummary,
    RecoveryItemView, ScheduledRunPageView, ScheduledTaskTemplateView, ScheduledTaskView,
    WorkflowDefinition, WorkflowRunView, WorkflowSummary, WorkflowView,
};
use error::AppError;
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

struct AppState {
    database: Database,
    broker: BrokerClient,
    /// Área de Athena: cliente del runtime y proyecciones vivas de sus runs.
    athena: athena::AreaAthena,
    recovered_at_start: usize,
    recovered_attachments_at_start: usize,
    recovered_workflows_at_start: usize,
    recovery_items_at_start: Vec<RecoveryItemView>,
    attachments_dir: std::path::PathBuf,
    data_dir: std::path::PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapReport {
    app_version: &'static str,
    database_path: String,
    /// Ruta del registro estructurado, para poder revisarlo sin buscarlo a mano.
    log_path: Option<String>,
    schema_version: i64,
    recovered_tasks: usize,
    recovered_attachments: usize,
    recovered_workflows: usize,
    recovery_items: Vec<RecoveryItemView>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportPathSelection {
    path: String,
    existed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CustomGptExportReport {
    path: String,
    included_knowledge: usize,
    excluded_sensitive: usize,
    excluded_disabled: usize,
    excluded_files: usize,
}

mod comandos;

// Sin calificar a proposito: `generate_handler!` nombra las ordenes tal
// cual, y asi la lista sigue siendo legible de un vistazo.
use comandos::*;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_local_data_dir()
                .map_err(|error| AppError::DataDirectory(error.to_string()))?;
            std::fs::create_dir_all(&data_dir)
                .map_err(|error| AppError::DataDirectory(error.to_string()))?;
            // El registro se prepara antes que nada para que un fallo posterior
            // de migración o de recuperación deje rastro observable.
            let _ = logging::init(&data_dir);
            let boot = logging::new_correlation_id();
            let _ = startup::refresh_protected_token_if_enabled(&data_dir);
            let database = Database::open(data_dir.join("chatygpt.db")).inspect_err(|error| {
                logging::error(
                    "app.database_failed",
                    Some(&boot),
                    &[("error_kind", logging::error_kind(error))],
                );
            })?;
            let broker = BrokerClient::bootstrap(&data_dir)?;
            // El área de Athena se prepara aunque el servicio no esté levantado:
            // su estado se consulta y se enseña, no bloquea el arranque.
            let cliente_athena = athena::AthenaClient::for_base_url(
                &std::env::var("ATHENA_BASE_URL")
                    .unwrap_or_else(|_| athena::URL_ATHENA_POR_DEFECTO.to_owned()),
            )?;
            cliente_athena.replace_token(secrets::resolve_athena_token(&data_dir).as_deref())?;
            let athena = athena::AreaAthena::nueva(cliente_athena);
            let recovery_items_at_start = database.recovery_candidates()?;
            let recovered_at_start =
                task_runtime::recover_at_start(database.clone(), broker.clone())?;
            let recovered_attachments_at_start =
                attachment_runtime::recover_at_start(database.clone(), broker.clone())?;
            let recovered_workflows_at_start =
                workflow_runtime::recover_at_start(database.clone(), broker.clone())?;
            logging::info(
                "app.started",
                Some(&boot),
                &[
                    (
                        "schema_version",
                        logging::count(database.schema_version().unwrap_or(-1)),
                    ),
                    ("recovered_tasks", logging::count(recovered_at_start as i64)),
                    (
                        "recovered_attachments",
                        logging::count(recovered_attachments_at_start as i64),
                    ),
                    (
                        "recovered_workflows",
                        logging::count(recovered_workflows_at_start as i64),
                    ),
                ],
            );
            scheduler_runtime::start(database.clone(), broker.clone());
            let attachments_dir = data_dir.join("attachments");
            app.manage(AppState {
                database,
                broker,
                athena,
                recovered_at_start,
                recovered_attachments_at_start,
                recovered_workflows_at_start,
                recovery_items_at_start,
                attachments_dir,
                data_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_athena_status,
            set_athena_credential,
            clear_athena_credential,
            start_athena_run,
            get_athena_run,
            list_athena_profiles,
            list_athena_models,
            list_athena_runs,
            get_athena_run_history,
            list_athena_memory,
            confirm_athena_memory,
            forget_athena_memory,
            get_athena_goal,
            revise_athena_goal,
            list_athena_recovery_runs,
            cancel_athena_run,
            resume_athena_run,
            acknowledge_athena_permission,
            resolve_athena_permission,
            list_athena_tracked_runs,
            fetch_athena_artifact,
            bootstrap_app,
            diagnose_broker,
            get_windows_startup_status,
            set_windows_startup_enabled,
            get_broker_credential,
            set_broker_credential,
            clear_broker_credential,
            list_authorized_folders,
            list_api_credentials,
            set_api_credential,
            clear_api_credential,
            pick_gpt_read_folder,
            pick_gpt_modify_folder,
            pick_athena_folder,
            revoke_authorized_folder,
            start_smoke_task,
            get_local_task,
            cancel_local_task,
            record_performance_samples,
            get_performance_report,
            clear_performance_samples,
            list_scheduled_tasks,
            list_scheduled_runs,
            list_scheduled_task_templates,
            create_scheduled_task_template,
            delete_scheduled_task_template,
            create_scheduled_task,
            create_scheduled_workflow,
            set_scheduled_task_enabled,
            update_scheduled_task,
            delete_scheduled_task,
            retry_scheduled_run,
            run_scheduled_task_now,
            cancel_scheduled_run,
            create_conversation,
            list_conversations,
            search_conversations,
            rename_conversation,
            move_conversation,
            set_conversation_custom_gpt,
            archive_conversation,
            delete_conversation,
            get_conversation,
            update_conversation_execution_preferences,
            get_task_context,
            reveal_context_source,
            get_conversation_summary,
            start_conversation_summary,
            update_conversation_summary,
            approve_conversation_summary,
            send_chat_turn,
            resolve_tool_calls,
            create_project,
            list_projects,
            create_workflow,
            list_workflows,
            get_workflow,
            save_workflow,
            publish_workflow,
            run_workflow,
            get_workflow_run,
            list_workflow_runs,
            retry_workflow_run,
            cancel_workflow_run,
            decide_workflow_approval,
            list_custom_gpts,
            create_custom_gpt,
            update_custom_gpt,
            list_custom_gpt_versions,
            restore_custom_gpt_version,
            duplicate_custom_gpt,
            preview_custom_gpt,
            preview_custom_gpt_api_action,
            test_custom_gpt_api_action,
            pick_custom_gpt_import_path,
            pick_custom_gpt_export_path,
            import_custom_gpt,
            export_custom_gpt,
            get_project_knowledge,
            remove_project_file,
            set_project_memory_item_enabled,
            list_audit_events,
            get_memory_overview,
            get_custom_gpt_knowledge,
            list_custom_gpt_files,
            import_custom_gpt_file,
            remove_custom_gpt_file,
            create_custom_gpt_knowledge_item,
            set_custom_gpt_knowledge_item_enabled,
            delete_custom_gpt_knowledge_item,
            reindex_custom_gpt_knowledge_item,
            set_memory_enabled,
            create_memory_item,
            update_memory_item,
            set_memory_item_enabled,
            delete_memory_item,
            reindex_memory_item,
            start_memory_search,
            get_memory_search,
            get_latest_memory_search,
            rename_project,
            update_project_instructions,
            archive_project,
            pick_attachment_paths,
            import_attachment,
            import_captured_image,
            list_attachments,
            list_project_files,
            set_project_file,
            use_project_file,
            remove_attachment,
            retry_attachment,
            retry_attachment_context,
            retry_attachment_semantic_index,
            pick_export_path,
            pick_scheduled_history_export_path,
            pick_scheduled_calendar_export_path,
            pick_obsidian_vault,
            export_conversation,
            export_scheduled_history,
            export_scheduled_calendar,
            export_conversation_to_obsidian
        ])
        .run(tauri::generate_context!())
        .expect("ChatyGPT no pudo iniciar");
}

#[cfg(test)]
mod tests;
