use std::time::Duration;

use crate::broker::BrokerClient;
use crate::db::{Database, ScheduledClaim};
use crate::error::AppError;

pub async fn dispatch_claim(
    database: Database,
    broker: BrokerClient,
    claim: &ScheduledClaim,
) -> Result<(), AppError> {
    if claim.target_kind == "workflow" {
        let workflow_id = claim.workflow_id.as_deref().ok_or_else(|| {
            AppError::BrokerContract("la programación no identifica su flujo".to_owned())
        })?;
        let workflow_version_id = claim.workflow_version_id.as_deref().ok_or_else(|| {
            AppError::BrokerContract(
                "la programación no identifica la versión del flujo".to_owned(),
            )
        })?;
        // La clave deriva de la ejecución programada: si la aplicación se
        // cierra entre crear el flujo y enlazarlo, la recuperación encuentra
        // el mismo flujo en vez de lanzar otro.
        let run = crate::workflow_runtime::start_version_for_operation(
            database.clone(),
            broker,
            workflow_id,
            workflow_version_id,
            &claim.prompt,
            &scheduled_operation_key(&claim.run_id),
        )?;
        database.start_scheduled_workflow_run(&claim.run_id, &run.id)
    } else {
        let conversation_id = claim.conversation_id.as_deref().ok_or_else(|| {
            AppError::BrokerContract("la programación no identifica su conversación".to_owned())
        })?;
        let task = crate::task_runtime::start_chat_turn_for_operation(
            database.clone(),
            broker,
            conversation_id,
            &claim.prompt,
            &scheduled_operation_key(&claim.run_id),
        )
        .await?;
        database.start_scheduled_run(&claim.run_id, &task.id)
    }
}

/// Identidad estable del trabajo que lanza una ejecución programada.
pub(crate) fn scheduled_operation_key(run_id: &str) -> String {
    format!("chatygpt:scheduled-run:{run_id}")
}

pub fn start(database: Database, broker: BrokerClient) {
    tauri::async_runtime::spawn(async move {
        if let Ok(claims) = database.recover_claimed_scheduled_runs() {
            for claim in claims {
                if let Err(error) = dispatch_claim(database.clone(), broker.clone(), &claim).await {
                    let _ = database.fail_scheduled_run(&claim.run_id, &error.to_string());
                }
            }
        }
        loop {
            let _ = database.reconcile_scheduled_runs();
            match database.claim_due_scheduled_task() {
                Ok(Some(claim)) => {
                    if let Err(error) =
                        dispatch_claim(database.clone(), broker.clone(), &claim).await
                    {
                        let _ = database.fail_scheduled_run(&claim.run_id, &error.to_string());
                    }
                }
                Ok(None) => {}
                Err(_) => {}
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
}
