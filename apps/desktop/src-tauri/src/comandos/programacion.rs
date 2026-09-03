//! Tareas y workflows programados: alta, edicion, reintento y cancelacion.

use super::*;
use crate::*;

#[tauri::command]
pub(crate) fn list_scheduled_task_templates(
    state: State<'_, AppState>,
) -> Result<Vec<ScheduledTaskTemplateView>, AppError> {
    state.database.list_scheduled_task_templates()
}

#[tauri::command]
pub(crate) fn create_scheduled_task_template(
    name: String,
    prompt: String,
    schedule_expression: String,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskTemplateView, AppError> {
    let name = validated_text(&name, "el nombre", 120)?;
    let prompt = validated_text(&prompt, "la instrucción", 20_000)?;
    state
        .database
        .create_scheduled_task_template(&name, &prompt, schedule_expression.trim())
}

#[tauri::command]
pub(crate) fn delete_scheduled_task_template(
    scheduled_task_template_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state
        .database
        .delete_scheduled_task_template(&scheduled_task_template_id, confirmed)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_scheduled_task(
    name: String,
    conversation_id: String,
    prompt: String,
    due_at: String,
    timezone: String,
    schedule_expression: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskView, AppError> {
    let name = validated_text(&name, "el nombre", 120)?;
    let prompt = validated_text(&prompt, "la instrucción", 20_000)?;
    let timezone = validated_text(&timezone, "la zona horaria", 100)?;
    state.database.create_scheduled_task(
        &name,
        &conversation_id,
        &prompt,
        due_at.trim(),
        &timezone,
        schedule_expression.trim(),
        confirmed,
    )
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_scheduled_workflow(
    name: String,
    workflow_id: String,
    input_text: String,
    due_at: String,
    timezone: String,
    schedule_expression: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskView, AppError> {
    let name = validated_text(&name, "el nombre", 120)?;
    let input_text = validated_text(&input_text, "la entrada", 200_000)?;
    let timezone = validated_text(&timezone, "la zona horaria", 100)?;
    state.database.create_scheduled_workflow(
        &name,
        &workflow_id,
        &input_text,
        due_at.trim(),
        &timezone,
        schedule_expression.trim(),
        confirmed,
    )
}

#[tauri::command]
pub(crate) fn set_scheduled_task_enabled(
    scheduled_task_id: String,
    enabled: bool,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskView, AppError> {
    state
        .database
        .set_scheduled_task_enabled(&scheduled_task_id, enabled, confirmed)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_scheduled_task(
    scheduled_task_id: String,
    name: String,
    conversation_id: String,
    prompt: String,
    due_at: String,
    timezone: String,
    schedule_expression: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskView, AppError> {
    let name = validated_text(&name, "el nombre", 120)?;
    let prompt = validated_text(&prompt, "la instrucción", 20_000)?;
    let timezone = validated_text(&timezone, "la zona horaria", 100)?;
    state.database.update_scheduled_task(
        &scheduled_task_id,
        &name,
        &conversation_id,
        &prompt,
        due_at.trim(),
        &timezone,
        schedule_expression.trim(),
        confirmed,
    )
}

#[tauri::command]
pub(crate) fn delete_scheduled_task(
    scheduled_task_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state
        .database
        .delete_scheduled_task(&scheduled_task_id, confirmed)
}

#[tauri::command]
pub(crate) async fn retry_scheduled_run(
    scheduled_run_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskView, AppError> {
    let claim = state
        .database
        .retry_failed_scheduled_run(&scheduled_run_id, confirmed)?;
    if let Err(error) =
        scheduler_runtime::dispatch_claim(state.database.clone(), state.broker.clone(), &claim)
            .await
    {
        state
            .database
            .fail_scheduled_run(&claim.run_id, &error.to_string())?;
        return Err(error);
    }
    state
        .database
        .list_scheduled_tasks()?
        .into_iter()
        .find(|task| task.id == claim.scheduled_task_id)
        .ok_or_else(|| AppError::NotFound("tarea programada reintentada".to_owned()))
}

#[tauri::command]
pub(crate) async fn run_scheduled_task_now(
    scheduled_task_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskView, AppError> {
    let claim = state
        .database
        .claim_scheduled_task_now(&scheduled_task_id, confirmed)?;
    if let Err(error) =
        scheduler_runtime::dispatch_claim(state.database.clone(), state.broker.clone(), &claim)
            .await
    {
        state
            .database
            .fail_scheduled_run(&claim.run_id, &error.to_string())?;
        return Err(error);
    }
    state
        .database
        .list_scheduled_tasks()?
        .into_iter()
        .find(|task| task.id == claim.scheduled_task_id)
        .ok_or_else(|| AppError::NotFound("tarea programada iniciada manualmente".to_owned()))
}

#[tauri::command]
pub(crate) async fn cancel_scheduled_run(
    scheduled_run_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskView, AppError> {
    let target = state
        .database
        .scheduled_cancellation_target(&scheduled_run_id, confirmed)?;
    let execution_id = if let Some(workflow_run_id) = target.workflow_run_id.as_deref() {
        workflow_runtime::cancel(
            state.database.clone(),
            state.broker.clone(),
            workflow_run_id,
        )
        .await?;
        workflow_run_id
    } else if let Some(broker_task_id) = target.broker_task_id.as_deref() {
        let cancelled =
            task_runtime::cancel_task(state.database.clone(), state.broker.clone(), broker_task_id)
                .await?;
        if cancelled.remote_status != "cancelled" {
            return Err(AppError::Conflict(
                "el Broker no confirmó la cancelación porque la ejecución cambió de estado"
                    .to_owned(),
            ));
        }
        broker_task_id
    } else {
        return Err(AppError::Conflict(
            "la ejecución no tiene una tarea que se pueda cancelar".to_owned(),
        ));
    };
    state
        .database
        .finish_scheduled_cancellation(&scheduled_run_id, execution_id)?;
    state
        .database
        .list_scheduled_tasks()?
        .into_iter()
        .find(|task| task.id == target.scheduled_task_id)
        .ok_or_else(|| AppError::NotFound("tarea programada cancelada".to_owned()))
}
