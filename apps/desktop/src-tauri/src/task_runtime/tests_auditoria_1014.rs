//! Regresiones de la auditoría QA del 14 de octubre de 2026 (INC-14).
//!
//! El Broker 2.11 publica un fallo terminal con `progress: {"phase": "failed"}`
//! y sin contadores. ChatyGPT lo rechazaba como contrato inválido y el turno se
//! quedaba «Generando» para siempre, también tras pedir la cancelación.

use super::cancel_task;
use crate::broker::simulated::{
    accepted_task, failed_task_state, task_state, ScriptedResponse, SimulatedBroker,
};
use crate::broker::BrokerClient;
use crate::db::Database;
use serde_json::json;
use std::time::Duration;

const SETTLE_TIMEOUT: Duration = Duration::from_secs(40);

fn database() -> Database {
    let path = std::env::temp_dir().join(format!(
        "chatygpt-auditoria-1014-{}.sqlite",
        uuid::Uuid::new_v4().simple()
    ));
    Database::open(path).expect("la base de pruebas debe abrirse")
}

fn cleanup(database: &Database) {
    let path = database.path().to_path_buf();
    for candidate in [
        path.clone(),
        path.with_extension("sqlite-wal"),
        path.with_extension("sqlite-shm"),
    ] {
        let _ = std::fs::remove_file(candidate);
    }
}

fn persisted_turn(database: &Database, conversation_id: &str, key: &str) -> String {
    let local_id = format!("local_{key}");
    database
        .prepare_chat_turn(
            conversation_id,
            &format!("user_{key}"),
            &format!("assistant_{key}"),
            &local_id,
            &format!("chatygpt:turn:{key}"),
            "Pregunta de prueba",
            &json!({
                "idempotency_key": format!("chatygpt:turn:{key}"),
                "inference_kind": "chat",
                "content": {"prompt": "Pregunta de prueba", "attachments": [], "metadata": {}},
                "risk": {"data_classification": "internal"}
            }),
            &[],
            &[],
            &[],
            &[],
        )
        .expect("el turno debe persistirse");
    local_id
}

fn assistant_status(database: &Database, conversation_id: &str) -> String {
    database
        .conversation_view(conversation_id)
        .expect("vista de la conversación")
        .messages
        .into_iter()
        .rev()
        .find(|message| message.role == "assistant")
        .expect("mensaje del asistente")
        .status
}

/// INC-14: cancelar un turno que el Broker acaba dando por fallido llega a un
/// desenlace en vez de repetir la cancelación para siempre.
#[test]
fn cancelling_a_turn_that_the_broker_fails_reaches_a_terminal_state() {
    let simulated = SimulatedBroker::start();
    simulated.always(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("r-inc14")),
    );
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("r-inc14", "generating", None)),
    );
    simulated.always(
        "DELETE /api/v1/tasks/{id}",
        ScriptedResponse::ok(failed_task_state(
            "r-inc14",
            "el razonamiento agotó la salida",
        )),
    );
    let database = database();
    let broker = BrokerClient::for_base_url(simulated.base_url()).expect("cliente");
    let conversation = database.create_conversation("INC-14", None).expect("chat");
    let local_id = persisted_turn(&database, &conversation.id, "inc14");
    let record = database.task_record(&local_id).expect("registro");
    super::spawn_submission_and_poll(database.clone(), broker.clone(), record);
    assert!(SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
        .task_snapshot(&local_id)
        .is_ok_and(|task| task.remote_task_id.is_some())));

    let snapshot = tauri::async_runtime::block_on(cancel_task(database.clone(), broker, &local_id))
        .expect("la cancelación devuelve el estado real");
    assert_eq!(snapshot.remote_status, "failed");
    assert_eq!(snapshot.local_state, "terminal");
    assert_eq!(assistant_status(&database, &conversation.id), "failed");
    cleanup(&database);
}

/// INC-14: una respuesta que nunca cumple el contrato cierra la tarea con un
/// motivo visible tras unos pocos intentos, en lugar de un bucle silencioso.
#[test]
fn a_persistent_contract_mismatch_closes_the_turn_with_a_visible_reason() {
    let simulated = SimulatedBroker::start();
    simulated.always(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("r-mismatch")),
    );
    let mut broken = task_state("r-mismatch", "generating", None);
    broken["progress"] = json!({"phase": "generating"});
    simulated.always("GET /api/v1/tasks/{id}", ScriptedResponse::ok(broken));
    let database = database();
    let broker = BrokerClient::for_base_url(simulated.base_url()).expect("cliente");
    let conversation = database.create_conversation("INC-14b", None).expect("chat");
    let local_id = persisted_turn(&database, &conversation.id, "inc14b");
    let record = database.task_record(&local_id).expect("registro");
    super::spawn_submission_and_poll(database.clone(), broker, record);

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.local_state == "orphaned")),
        "la tarea debía cerrarse en vez de sondear para siempre"
    );
    let polls = simulated
        .requests_to("GET", "/api/v1/tasks/r-mismatch")
        .len();
    assert_eq!(polls, 3, "se dejan de pedir lecturas al alcanzar el límite");
    assert_eq!(assistant_status(&database, &conversation.id), "failed");
    let message = database
        .task_snapshot(&local_id)
        .expect("tarea")
        .error
        .and_then(|error| {
            error
                .get("message")
                .and_then(|m| m.as_str().map(str::to_owned))
        })
        .unwrap_or_default();
    assert!(message.contains("no entiende la respuesta"), "{message}");
    cleanup(&database);
}
