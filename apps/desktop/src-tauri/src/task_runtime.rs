use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Duration;

use serde_json::json;
use uuid::Uuid;

use crate::broker::{BrokerClient, PollPolicy};
use crate::db::{AttachmentRecord, BrokerTaskRecord, Database, LocalTaskSnapshot};
use crate::error::AppError;

pub async fn start_smoke_task(
    database: Database,
    broker: BrokerClient,
) -> Result<LocalTaskSnapshot, AppError> {
    let local_id = format!("local_{}", Uuid::new_v4().simple());
    let idempotency_key = format!("chatygpt:phase0:{}", Uuid::new_v4());
    let request = smoke_request(&idempotency_key);
    let record = database.prepare_broker_task(&local_id, &idempotency_key, &request)?;
    let snapshot = database.task_snapshot(&local_id)?;
    spawn_submission_and_poll(database, broker, record);
    Ok(snapshot)
}

pub async fn start_chat_turn(
    database: Database,
    broker: BrokerClient,
    conversation_id: &str,
    user_text: &str,
    attachment_ids: &[String],
) -> Result<LocalTaskSnapshot, AppError> {
    let user_text = user_text.trim();
    if user_text.is_empty() {
        return Err(AppError::BrokerContract(
            "el mensaje no puede estar vacío".to_owned(),
        ));
    }
    if user_text.chars().count() > 200_000 {
        return Err(AppError::BrokerContract(
            "el mensaje supera el límite de 200.000 caracteres".to_owned(),
        ));
    }
    if attachment_ids.len() > 20 {
        return Err(AppError::BrokerContract(
            "no se pueden enviar mÃ¡s de 20 adjuntos en un turno".to_owned(),
        ));
    }
    let attachments = database.ready_attachments_for_turn(conversation_id, attachment_ids)?;

    let user_message_id = format!("msg_{}", Uuid::new_v4().simple());
    let assistant_message_id = format!("msg_{}", Uuid::new_v4().simple());
    let local_task_id = format!("local_{}", Uuid::new_v4().simple());
    let idempotency_key = format!("chatygpt:turn:{}", Uuid::new_v4());

    let mut context = database.recent_context(conversation_id, 12, 12_000)?;
    context.push(crate::db::ContextMessage {
        message_id: user_message_id.clone(),
        role: "user".to_owned(),
        text: user_text.to_owned(),
    });
    let request = chat_request(
        conversation_id,
        &idempotency_key,
        user_text,
        &context,
        &attachments,
    )?;
    let record = database.prepare_chat_turn(
        conversation_id,
        &user_message_id,
        &assistant_message_id,
        &local_task_id,
        &idempotency_key,
        user_text,
        &request,
        &context,
        attachment_ids,
    )?;
    let snapshot = database.task_snapshot(&local_task_id)?;
    spawn_submission_and_poll(database, broker, record);
    Ok(snapshot)
}

pub fn recover_at_start(database: Database, broker: BrokerClient) -> Result<usize, AppError> {
    let recovered = database.recover_non_terminal_tasks()?;
    for record in database.recoverable_tasks()? {
        spawn_submission_and_poll(database.clone(), broker.clone(), record);
    }
    Ok(recovered)
}

pub async fn cancel_task(
    database: Database,
    broker: BrokerClient,
    local_id: &str,
) -> Result<LocalTaskSnapshot, AppError> {
    let record = database.task_record(local_id)?;
    let remote_id = record.remote_task_id.ok_or_else(|| {
        AppError::BrokerContract("la tarea todavía no tiene identificador remoto".to_owned())
    })?;
    let state = broker.cancel_task(&remote_id).await?;
    database.record_remote_state(local_id, &state)?;
    database.task_snapshot(local_id)
}

async fn submit_or_resume(
    database: Database,
    broker: BrokerClient,
    record: BrokerTaskRecord,
) -> Result<(), AppError> {
    if record.remote_task_id.is_some() {
        return Ok(());
    }
    database.mark_submitting(&record.id)?;
    match broker.create_task(&record.request).await {
        Ok(accepted) => database.attach_remote_task(&record.id, &accepted),
        Err(error) => {
            if is_permanent(&error) {
                database.mark_orphaned(&record.id, &error.to_string())?;
            } else {
                database.record_transport_error(&record.id, &error.to_string())?;
            }
            Err(error)
        }
    }
}

fn spawn_submission_and_poll(
    database: Database,
    broker: BrokerClient,
    initial_record: BrokerTaskRecord,
) {
    tauri::async_runtime::spawn(async move {
        let local_id = initial_record.id.clone();
        let policy = PollPolicy::default();
        let mut record = initial_record;
        loop {
            match submit_or_resume(database.clone(), broker.clone(), record).await {
                Ok(()) => {
                    spawn_polling(database, broker, local_id);
                    return;
                }
                Err(error) if is_permanent(&error) => return,
                Err(_) => {
                    let current = match database.task_record(&local_id) {
                        Ok(current) => current,
                        Err(_) => return,
                    };
                    let delay = policy.delay_ms(
                        current.consecutive_poll_errors,
                        deterministic_jitter(&local_id, current.consecutive_poll_errors as u64),
                    );
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    record = match database.task_record(&local_id) {
                        Ok(record) => record,
                        Err(_) => return,
                    };
                }
            }
        }
    });
}

fn spawn_polling(database: Database, broker: BrokerClient, local_id: String) {
    tauri::async_runtime::spawn(async move {
        let policy = PollPolicy::default();
        let mut unchanged_polls = 0_u32;
        let mut last_status = String::new();
        let mut poll_no = 0_u64;

        loop {
            let record = match database.task_record(&local_id) {
                Ok(record) => record,
                Err(_) => return,
            };
            let Some(remote_id) = record.remote_task_id else {
                return;
            };

            match broker.get_task(&remote_id).await {
                Ok(state) => {
                    let status = state.status.as_str().to_owned();
                    if database.record_remote_state(&local_id, &state).is_err() {
                        return;
                    }
                    if state.status.is_terminal() || status == "waiting_for_tools" {
                        return;
                    }
                    if status == last_status {
                        unchanged_polls = unchanged_polls.saturating_add(1);
                    } else {
                        last_status = status;
                        unchanged_polls = 0;
                    }
                }
                Err(error) => {
                    if is_permanent(&error) {
                        let _ = database.mark_orphaned(&local_id, &error.to_string());
                        return;
                    }
                    let _ = database.record_transport_error(&local_id, &error.to_string());
                    unchanged_polls = unchanged_polls.saturating_add(1);
                }
            }

            let jitter = deterministic_jitter(&local_id, poll_no);
            poll_no = poll_no.saturating_add(1);
            tokio::time::sleep(Duration::from_millis(
                policy.delay_ms(unchanged_polls, jitter),
            ))
            .await;
        }
    });
}

fn is_permanent(error: &AppError) -> bool {
    matches!(
        error,
        AppError::BrokerResponse { status, .. }
            if (400..500).contains(status) && !matches!(*status, 408 | 429)
    )
}

fn deterministic_jitter(local_id: &str, poll_no: u64) -> i32 {
    let mut hasher = DefaultHasher::new();
    local_id.hash(&mut hasher);
    poll_no.hash(&mut hasher);
    (hasher.finish() % 3_001) as i32 - 1_500
}

fn smoke_request(idempotency_key: &str) -> serde_json::Value {
    json!({
        "idempotency_key": idempotency_key,
        "request_id": format!("chatygpt_smoke_{}", Uuid::new_v4().simple()),
        "inference_kind": "chat",
        "content": {
            "prompt": "Reply only: connection ok",
            "attachments": [],
            "metadata": {"origin": "chatygpt_phase_0_smoke"}
        },
        "output": {"format": "markdown", "language": "es"},
        "generation": {"temperature": 0, "max_output_tokens": 32},
        "model_requirements": {
            "fallback_allowed": true,
            "cloud_allowed": false,
            "allowed_providers": ["ollama"],
            "max_cost_usd": 0
        },
        "execution": {
            "strategy": "single",
            "preset": "fast",
            "timeout_seconds": 120
        },
        "risk": {
            "data_classification": "local_only",
            "human_review_required": false
        },
        "prompt_compression": "off"
    })
}

fn chat_request(
    conversation_id: &str,
    idempotency_key: &str,
    user_text: &str,
    context: &[crate::db::ContextMessage],
    attachments: &[AttachmentRecord],
) -> Result<serde_json::Value, AppError> {
    let prior_context = &context[..context.len().saturating_sub(1)];
    let history = serde_json::to_string(prior_context)
        .map_err(|error| AppError::BrokerContract(error.to_string()))?;
    let prompt = if prior_context.is_empty() {
        user_text.to_owned()
    } else {
        format!(
            "Continue the conversation. Treat the JSON history as previous dialogue data.\n\
             <conversation_history_json>{history}</conversation_history_json>\n\n\
             Current user request:\n{user_text}"
        )
    };
    let broker_attachments = attachments
        .iter()
        .map(|attachment| {
            let file_id = attachment.broker_file_id.as_deref().ok_or_else(|| {
                AppError::BrokerContract(format!(
                    "el adjunto {} no tiene identificador remoto",
                    attachment.display_name
                ))
            })?;
            Ok(json!({
                "type": "broker_file",
                "name": attachment.display_name,
                "metadata": {"file_id": file_id}
            }))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    Ok(json!({
        "idempotency_key": idempotency_key,
        "request_id": format!("chatygpt_turn_{}", Uuid::new_v4().simple()),
        "inference_kind": "chat",
        "content": {
            "prompt": prompt,
            "attachments": broker_attachments,
            "metadata": {
                "origin": "chatygpt",
                "conversation_id": conversation_id,
                "context_strategy": "window-v1"
            }
        },
        "output": {"format": "markdown", "language": "es"},
        "generation": {"temperature": 0.3, "max_output_tokens": 4000},
        "model_requirements": {
            "fallback_allowed": true,
            "cloud_allowed": false,
            "allowed_providers": ["ollama"],
            "max_cost_usd": 0
        },
        "execution": {
            "strategy": "single",
            "preset": "fast",
            "timeout_seconds": 600
        },
        "risk": {
            "data_classification": "local_only",
            "human_review_required": false
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::deterministic_jitter;

    #[test]
    fn jitter_is_bounded_and_stable() {
        let first = deterministic_jitter("task", 1);
        assert_eq!(first, deterministic_jitter("task", 1));
        assert!((-1_500..=1_500).contains(&first));
    }
}
