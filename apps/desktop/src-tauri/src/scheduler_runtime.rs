use std::time::Duration;

use crate::broker::BrokerClient;
use crate::db::Database;

pub fn start(database: Database, broker: BrokerClient) {
    tauri::async_runtime::spawn(async move {
        loop {
            let _ = database.reconcile_scheduled_runs();
            match database.claim_due_scheduled_task() {
                Ok(Some(claim)) => {
                    match crate::task_runtime::start_chat_turn(
                        database.clone(),
                        broker.clone(),
                        &claim.conversation_id,
                        &claim.prompt,
                        &[],
                        false,
                        false,
                        false,
                        false,
                    )
                    .await
                    {
                        Ok(task) => {
                            let _ = database.start_scheduled_run(&claim.run_id, &task.id);
                        }
                        Err(error) => {
                            let _ = database.fail_scheduled_run(&claim.run_id, &error.to_string());
                        }
                    }
                }
                Ok(None) => {}
                Err(_) => {}
            }
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    });
}
