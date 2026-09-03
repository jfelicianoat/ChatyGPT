//! Workflows: definicion, publicacion y ejecuciones.

use crate::*;

#[tauri::command]
pub(crate) fn create_workflow(
    name: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<WorkflowView, AppError> {
    state.database.create_workflow(&name, project_id.as_deref())
}

#[tauri::command]
pub(crate) fn list_workflows(state: State<'_, AppState>) -> Result<Vec<WorkflowSummary>, AppError> {
    state.database.list_workflows()
}

#[tauri::command]
pub(crate) fn get_workflow(
    id: String,
    state: State<'_, AppState>,
) -> Result<WorkflowView, AppError> {
    state.database.workflow_view(&id)
}

#[tauri::command]
pub(crate) fn save_workflow(
    id: String,
    name: String,
    description: Option<String>,
    project_id: Option<String>,
    definition: WorkflowDefinition,
    state: State<'_, AppState>,
) -> Result<WorkflowView, AppError> {
    workflow_runtime::validate_definition(&definition)?;
    state.database.update_workflow(
        &id,
        &name,
        description.as_deref(),
        project_id.as_deref(),
        &definition,
    )
}

#[tauri::command]
pub(crate) fn publish_workflow(
    id: String,
    state: State<'_, AppState>,
) -> Result<WorkflowView, AppError> {
    let workflow = state.database.workflow_view(&id)?;
    workflow_runtime::validate_definition(&workflow.definition)?;
    for node in &workflow.definition.nodes {
        state
            .database
            .ready_workflow_attachments(&id, &node.attachment_ids)?;
    }
    state.database.publish_workflow(&id)
}

#[tauri::command]
pub(crate) fn run_workflow(
    id: String,
    input_text: String,
    state: State<'_, AppState>,
) -> Result<WorkflowRunView, AppError> {
    workflow_runtime::start(
        state.database.clone(),
        state.broker.clone(),
        &id,
        &input_text,
    )
}

#[tauri::command]
pub(crate) fn get_workflow_run(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<WorkflowRunView, AppError> {
    state.database.workflow_run(&run_id)
}

#[tauri::command]
pub(crate) fn list_workflow_runs(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<WorkflowRunView>, AppError> {
    state.database.list_workflow_runs(&workflow_id)
}

#[tauri::command]
pub(crate) fn retry_workflow_run(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<WorkflowRunView, AppError> {
    workflow_runtime::retry(state.database.clone(), state.broker.clone(), &run_id)
}

#[tauri::command]
pub(crate) async fn cancel_workflow_run(
    run_id: String,
    state: State<'_, AppState>,
) -> Result<WorkflowRunView, AppError> {
    workflow_runtime::cancel(state.database.clone(), state.broker.clone(), &run_id).await
}

#[tauri::command]
pub(crate) fn decide_workflow_approval(
    run_id: String,
    node_id: String,
    approved: bool,
    state: State<'_, AppState>,
) -> Result<WorkflowRunView, AppError> {
    workflow_runtime::decide_approval(
        state.database.clone(),
        state.broker.clone(),
        &run_id,
        &node_id,
        approved,
    )
}
