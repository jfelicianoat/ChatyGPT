//! Regresiones de la auditoría del 28 de septiembre de 2026 (H07, H08, H09).
//!
//! Cada prueba reproduce el fallo descrito en el informe y afirma ahora el
//! comportamiento correcto: un estado terminal no se reabre, una cancelación
//! sobrevive a la falta de identidad remota y un resultado que no puede
//! guardarse cierra la tarea con su motivo en vez de dejarla esperando.

use super::{cancel_task, recover_at_start, start_memory_embedding};
use crate::broker::simulated::{accepted_task, task_state, ScriptedResponse, SimulatedBroker};
use crate::broker::BrokerClient;
use crate::db::Database;
use serde_json::json;
use std::time::Duration;

const SETTLE_TIMEOUT: Duration = Duration::from_secs(20);

fn database() -> Database {
    let path = std::env::temp_dir().join(format!(
        "chatygpt-auditoria-{}.sqlite",
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

/// Persiste un turno sin lanzarlo: la prueba decide cuándo interviene cada bucle.
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

fn state(value: serde_json::Value) -> crate::broker::TaskState {
    serde_json::from_value(value).expect("estado remoto válido")
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

/// H07: una lectura antigua resuelta después de cancelar no reabre la tarea.
#[test]
fn a_late_poll_never_reopens_a_cancelled_task() {
    let database = database();
    let conversation = database.create_conversation("H07", None).expect("chat");
    let local_id = persisted_turn(&database, &conversation.id, "h07");

    database
        .record_remote_state(&local_id, &state(task_state("r-h07", "cancelled", None)))
        .expect("la cancelación se registra");
    let closed = database.task_snapshot(&local_id).expect("tarea");
    assert_eq!(closed.local_state, "terminal");

    // La respuesta del GET que salió antes del DELETE llega ahora.
    database
        .record_remote_state(&local_id, &state(task_state("r-h07", "generating", None)))
        .expect("la lectura tardía se descarta sin error");
    let after = database.task_snapshot(&local_id).expect("tarea");
    assert_eq!(after.remote_status, "cancelled");
    assert_eq!(after.local_state, "terminal");
    assert_eq!(assistant_status(&database, &conversation.id), "cancelled");

    // Tampoco un segundo desenlace distinto sustituye al primero.
    database
        .record_remote_state(
            &local_id,
            &state(task_state(
                "r-h07",
                "completed",
                Some(json!({"result_markdown": "no debe aparecer"})),
            )),
        )
        .expect("el desenlace tardío se descarta");
    assert_eq!(
        database
            .task_snapshot(&local_id)
            .expect("tarea")
            .remote_status,
        "cancelled"
    );
    cleanup(&database);
}

/// H08: cancelar antes de transmitir nada cierra la pregunta aquí, y ni
/// una recuperación posterior con el Broker ya disponible llega a enviarla.
#[test]
fn a_question_cancelled_before_submission_is_never_sent() {
    let simulated = SimulatedBroker::start();
    simulated.always(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("r-never")),
    );
    let database = database();
    let broker = BrokerClient::for_base_url(simulated.base_url()).expect("cliente");
    let conversation = database.create_conversation("H08a", None).expect("chat");
    let local_id = persisted_turn(&database, &conversation.id, "h08a");

    let snapshot =
        tauri::async_runtime::block_on(cancel_task(database.clone(), broker.clone(), &local_id))
            .expect("la cancelación no necesita identidad remota");
    assert_eq!(snapshot.remote_status, "cancelled");
    assert_eq!(snapshot.local_state, "terminal");
    assert_eq!(assistant_status(&database, &conversation.id), "cancelled");

    // Reinicio con el Broker disponible: no hay nada que recuperar.
    recover_at_start(database.clone(), broker).expect("recuperación");
    std::thread::sleep(Duration::from_millis(600));
    assert!(simulated.requests_to("POST", "/api/v1/tasks").is_empty());
    cleanup(&database);
}

/// H08: si un intento ya pudo llegar al Broker, la cancelación reconcilia con
/// la misma clave idempotente y cancela allí, sin crear otra tarea.
#[test]
fn a_possibly_transmitted_question_is_reconciled_and_cancelled_remotely() {
    let simulated = SimulatedBroker::start();
    simulated.script("POST /api/v1/tasks", ScriptedResponse::transient());
    simulated.always(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("r-reconciled")),
    );
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("r-reconciled", "generating", None)),
    );
    simulated.always(
        "DELETE /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("r-reconciled", "cancelled", None)),
    );
    let database = database();
    let broker = BrokerClient::for_base_url(simulated.base_url()).expect("cliente");
    let conversation = database.create_conversation("H08b", None).expect("chat");
    let local_id = persisted_turn(&database, &conversation.id, "h08b");
    let record = database.task_record(&local_id).expect("registro");
    super::spawn_submission_and_poll(database.clone(), broker.clone(), record);
    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || {
            simulated.requests_to("POST", "/api/v1/tasks").len() == 1
                && database
                    .task_record(&local_id)
                    .is_ok_and(|record| record.consecutive_poll_errors >= 1)
        }),
        "el primer envío debía fallar por transporte"
    );

    let snapshot =
        tauri::async_runtime::block_on(cancel_task(database.clone(), broker.clone(), &local_id))
            .expect("la cancelación se acepta aunque falte la identidad remota");
    assert!(snapshot.cancel_requested || snapshot.remote_status == "cancelled");

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.remote_status == "cancelled")),
        "la tarea debía quedar cancelada en el Broker"
    );
    assert!(!simulated
        .requests_to("DELETE", "/api/v1/tasks/r-reconciled")
        .is_empty());
    let keys = simulated
        .requests_to("POST", "/api/v1/tasks")
        .into_iter()
        .map(|request| {
            request.body["idempotency_key"]
                .as_str()
                .unwrap_or("")
                .to_owned()
        })
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        keys.len(),
        1,
        "todas las creaciones debían usar la misma clave"
    );
    assert_eq!(assistant_status(&database, &conversation.id), "cancelled");
    cleanup(&database);
}

/// H08: si el Broker no responde al cancelar, la intención queda guardada y
/// el sondeo la completa cuando vuelve.
#[test]
fn a_cancellation_the_broker_could_not_confirm_is_retried_by_polling() {
    let simulated = SimulatedBroker::start();
    simulated.always(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("r-retry")),
    );
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("r-retry", "generating", None)),
    );
    simulated.script("DELETE /api/v1/tasks/{id}", ScriptedResponse::transient());
    simulated.always(
        "DELETE /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("r-retry", "cancelled", None)),
    );
    let database = database();
    let broker = BrokerClient::for_base_url(simulated.base_url()).expect("cliente");
    let conversation = database.create_conversation("H08c", None).expect("chat");
    let local_id = persisted_turn(&database, &conversation.id, "h08c");
    let record = database.task_record(&local_id).expect("registro");
    super::spawn_submission_and_poll(database.clone(), broker.clone(), record);
    assert!(SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
        .task_snapshot(&local_id)
        .is_ok_and(|task| task.remote_task_id.is_some())));

    let snapshot =
        tauri::async_runtime::block_on(cancel_task(database.clone(), broker.clone(), &local_id))
            .expect("un fallo de transporte no pierde la cancelación");
    assert!(snapshot.cancel_requested);
    assert_ne!(snapshot.remote_status, "cancelled");

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.remote_status == "cancelled")),
        "el sondeo debía repetir la cancelación hasta confirmarla"
    );
    cleanup(&database);
}

/// H09: un resultado recibido que no puede materializarse cierra la tarea con
/// su motivo; el worker ya no desaparece dejándola «en curso».
#[test]
fn an_unmaterializable_result_closes_the_task_with_a_visible_reason() {
    let simulated = SimulatedBroker::start();
    simulated.always(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("r-bad-embedding")),
    );
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state(
            "r-bad-embedding",
            "completed",
            Some(json!({"embedding": ["no-es-un-número"]})),
        )),
    );
    let database = database();
    let broker = BrokerClient::for_base_url(simulated.base_url()).expect("cliente");
    let snapshot = start_memory_embedding(database.clone(), broker, "mem-h09", "texto", false)
        .expect("la tarea se lanza");

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&snapshot.id)
            .is_ok_and(|task| task.local_state == "orphaned")),
        "la tarea debía cerrarse en vez de quedar activa sin worker"
    );
    let task = database.task_snapshot(&snapshot.id).expect("tarea");
    assert_eq!(task.remote_status, "completed");
    let message = task
        .error
        .as_ref()
        .and_then(|error| error.get("message"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned();
    assert!(message.contains("no pudo guardar"), "{message}");
    // No se reintenta al arrancar: el Broker ya terminó y no hay que cancelar.
    assert!(database
        .abandoned_remote_tasks()
        .expect("consulta")
        .is_empty());
    cleanup(&database);
}

/// H05: si la aplicación se cierra entre crear el turno programado y enlazarlo
/// con su ejecución, la recuperación encuentra ese turno en vez de crear otro.
#[test]
fn a_scheduled_run_recovered_after_a_crash_does_not_duplicate_its_turn() {
    let simulated = SimulatedBroker::start();
    simulated.always(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("r-scheduled")),
    );
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("r-scheduled", "generating", None)),
    );
    let database = database();
    let broker = BrokerClient::for_base_url(simulated.base_url()).expect("cliente");
    let conversation = database.create_conversation("H05", None).expect("chat");
    let scheduled = database
        .create_scheduled_task(
            "Informe",
            &conversation.id,
            "Resume las novedades",
            "2099-01-01T10:00:00.000Z",
            "Atlantic/Canary",
            "daily",
            true,
        )
        .expect("programación");
    let claim = database
        .claim_scheduled_task_now(&scheduled.id, true)
        .expect("reclamo");
    tauri::async_runtime::block_on(crate::scheduler_runtime::dispatch_claim(
        database.clone(),
        broker.clone(),
        &claim,
    ))
    .expect("primer despacho");

    // El corte: el turno existe, pero el enlace con la ejecución se perdió.
    rusqlite::Connection::open(database.path())
        .expect("conexión")
        .execute(
            "UPDATE scheduled_runs SET status = 'claimed', broker_task_id = NULL WHERE id = ?1",
            rusqlite::params![claim.run_id],
        )
        .expect("simular el corte");
    let recovered = database
        .recover_claimed_scheduled_runs()
        .expect("recuperación");
    assert_eq!(recovered.len(), 1);
    tauri::async_runtime::block_on(crate::scheduler_runtime::dispatch_claim(
        database.clone(),
        broker,
        &recovered[0],
    ))
    .expect("segundo despacho");

    let tasks: i64 = rusqlite::Connection::open(database.path())
        .expect("conexión")
        .query_row(
            "SELECT COUNT(*) FROM broker_tasks WHERE conversation_id = ?1",
            rusqlite::params![conversation.id],
            |row| row.get(0),
        )
        .expect("recuento");
    assert_eq!(tasks, 1, "una sola tarea por ejecución programada");
    let linked: Option<String> = rusqlite::Connection::open(database.path())
        .expect("conexión")
        .query_row(
            "SELECT broker_task_id FROM scheduled_runs WHERE id = ?1",
            rusqlite::params![claim.run_id],
            |row| row.get(0),
        )
        .expect("enlace");
    assert!(linked.is_some(), "la ejecución vuelve a quedar enlazada");
    cleanup(&database);
}

/// H19: un documento grande no abre una petición por fragmento a la vez; la
/// cola local deja trabajar a unos pocos y el resto espera turno.
#[test]
fn document_indexing_keeps_a_bounded_number_of_tasks_in_flight() {
    let simulated = SimulatedBroker::start();
    simulated.always(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("r-index")),
    );
    // Ningún fragmento termina: si la cola no limitara, llegarían los 30.
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("r-index", "generating", None)),
    );
    let database = database();
    let broker = BrokerClient::for_base_url(simulated.base_url()).expect("cliente");
    let conversation = database.create_conversation("H19", None).expect("chat");
    let attachment = database
        .register_attachment(
            &conversation.id,
            "C:/managed/libro.pdf",
            "libro.pdf",
            Some("application/pdf"),
            1_000,
            "h19-hash",
        )
        .expect("adjunto");
    database
        .update_attachment_ingestion(
            &attachment.id,
            "ready",
            Some("broker-h19"),
            Some("document"),
            Some("test"),
            None,
            None,
        )
        .expect("adjunto listo");
    let chunks = (0..30)
        .map(|index| format!("Fragmento número {index} del libro."))
        .collect::<Vec<_>>();
    database
        .replace_attachment_chunks(&attachment.id, &chunks)
        .expect("fragmentos");

    let snapshot = super::start_attachment_semantic_index(
        database.clone(),
        broker,
        &attachment.id,
        false,
        false,
    )
    .expect("indexación")
    .expect("hay fragmentos que indexar");
    assert!(!snapshot.id.is_empty());
    let prepared: i64 = rusqlite::Connection::open(database.path())
        .expect("conexión")
        .query_row("SELECT COUNT(*) FROM broker_tasks", [], |row| row.get(0))
        .expect("recuento");
    assert_eq!(prepared, 30, "el lote entero queda persistido");

    std::thread::sleep(Duration::from_millis(1_500));
    let submitted = simulated.requests_to("POST", "/api/v1/tasks").len();
    assert_eq!(submitted, 4, "solo cuatro fragmentos pueden estar en curso");
    cleanup(&database);
}

/// H21: si el Broker no deja comprobar sus capacidades, el turno sigue por
/// compatibilidad pero lo declara, y la interfaz puede avisar.
#[test]
fn a_turn_sent_without_verified_capabilities_says_so() {
    let simulated = SimulatedBroker::start();
    simulated.always("GET /api/v1/capabilities", ScriptedResponse::transient());
    simulated.always(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("r-unverified")),
    );
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("r-unverified", "generating", None)),
    );
    let database = database();
    let broker = BrokerClient::for_base_url(simulated.base_url()).expect("cliente");
    let conversation = database.create_conversation("H21", None).expect("chat");
    let snapshot = tauri::async_runtime::block_on(super::start_chat_turn_with_identity(
        database.clone(),
        broker,
        &conversation.id,
        "Ejecuta este cálculo",
        &[],
        super::ChatTurnOptions {
            sandbox_enabled: true,
            ..Default::default()
        },
        None,
    ))
    .expect("el turno sale igualmente");
    assert_eq!(snapshot.unverified_capabilities, vec!["sandbox".to_owned()]);
    cleanup(&database);
}

/// H12: repetir un envío con el mismo identificador de operación devuelve la
/// tarea del primero; no crea otra con otra clave.
#[test]
fn repeating_a_send_with_the_same_operation_returns_the_first_task() {
    let simulated = SimulatedBroker::start();
    simulated.always(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("r-once")),
    );
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("r-once", "generating", None)),
    );
    let database = database();
    let broker = BrokerClient::for_base_url(simulated.base_url()).expect("cliente");
    let conversation = database.create_conversation("H12", None).expect("chat");
    let key = super::client_operation_key("draft-7f3a").expect("clave válida");
    let send = || {
        tauri::async_runtime::block_on(super::start_chat_turn_with_identity(
            database.clone(),
            broker.clone(),
            &conversation.id,
            "Pregunta única",
            &[],
            super::ChatTurnOptions::default(),
            Some(&key),
        ))
        .expect("envío")
    };
    let first = send();
    let second = send();
    assert_eq!(first.id, second.id);
    let view = database.conversation_view(&conversation.id).expect("vista");
    assert_eq!(
        view.messages
            .iter()
            .filter(|message| message.role == "user")
            .count(),
        1
    );
    assert!(super::client_operation_key("con espacios").is_err());
    assert!(super::client_operation_key("").is_err());
    cleanup(&database);
}
