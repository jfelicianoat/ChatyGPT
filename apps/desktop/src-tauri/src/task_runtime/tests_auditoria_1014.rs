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

/// AI OS Core: un turno que Core no autoriza no llega al Broker y se cierra con
/// el motivo de Core a la vista, sin reintentarse solo.
#[test]
fn a_turn_denied_by_ai_os_core_is_closed_without_reaching_the_broker() {
    let broker_sim = SimulatedBroker::start();
    let core = SimulatedBroker::start();
    core.always(
        "POST /api/v1/policies/evaluate",
        ScriptedResponse::ok(json!({"authorization": "DENY", "executable": false})),
    );
    let database = database();
    let broker = BrokerClient::for_base_url(broker_sim.base_url())
        .and_then(|client| client.with_core(core.base_url(), None))
        .expect("cliente");
    let conversation = database.create_conversation("Core", None).expect("chat");
    let local_id = persisted_turn(&database, &conversation.id, "core-deny");
    let record = database.task_record(&local_id).expect("registro");
    super::spawn_submission_and_poll(database.clone(), broker, record);

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.local_state == "orphaned")),
        "la denegación de Core es definitiva"
    );
    assert!(broker_sim.requests_to("POST", "/api/v1/tasks").is_empty());
    let task = database.task_snapshot(&local_id).expect("tarea");
    assert!(
        task.error
            .as_ref()
            .is_some_and(|error| error.to_string().contains("AI OS Core")),
        "{:?}",
        task.error
    );
    cleanup(&database);
}

/// Recorrido real contra AI OS Core y Broker AI (no corre en la batería normal).
///
/// `cargo test --lib real_ai_os_core -- --ignored --nocapture` con
/// `CHATYGPT_VERIFY_CORE_URL`, `CHATYGPT_VERIFY_CORE_TOKEN` (token por app de
/// chatygpt), `CHATYGPT_VERIFY_DIRECTOR_TOKEN` (token común de Core),
/// `CHATYGPT_VERIFY_BROKER_URL` y `CHATYGPT_VERIFY_BROKER_TOKEN`. Usa el mismo
/// envío y sondeo que un turno de chat, sobre una base de usar y tirar.
#[test]
#[ignore = "necesita AI OS Core y Broker AI reales"]
fn real_ai_os_core_governs_a_chat_turn() {
    let env = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("falta {name}"));
    let core_url = env("CHATYGPT_VERIFY_CORE_URL");
    let director = env("CHATYGPT_VERIFY_DIRECTOR_TOKEN");
    let policy = |enabled: bool| {
        let response = reqwest::blocking::Client::new()
            .post(format!("{core_url}/api/v1/policies"))
            .bearer_auth(&director)
            .json(&json!({
                "policy_id": "verify_chatygpt_deny", "policy_key": "verify_chatygpt_deny",
                "scope": "chatygpt:broker", "decision_if_matched": "DENY", "enabled": enabled,
                "conditions": [{"field": "operation_type", "operator": "equals", "value": "chat"}]
            }))
            .send()
            .expect("Core responde");
        assert!(response.status().is_success(), "{}", response.status());
    };
    let broker = || {
        let client = BrokerClient::for_base_url(&env("CHATYGPT_VERIFY_BROKER_URL"))
            .and_then(|client| {
                client.with_core(&core_url, Some(&env("CHATYGPT_VERIFY_CORE_TOKEN")))
            })
            .expect("cliente");
        client
            .replace_admin_token(Some(&env("CHATYGPT_VERIFY_BROKER_TOKEN")))
            .expect("token");
        client
    };
    let database = database();
    let conversation = database
        .create_conversation("Core real", None)
        .expect("chat");
    let run = uuid::Uuid::new_v4().simple().to_string();

    policy(true);
    let denied = persisted_turn(&database, &conversation.id, &format!("deny-{run}"));
    super::spawn_submission_and_poll(
        database.clone(),
        broker(),
        database.task_record(&denied).expect("registro"),
    );
    assert!(SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
        .task_snapshot(&denied)
        .is_ok_and(
            |task| task.local_state == "orphaned" && task.remote_task_id.is_none()
        )));
    println!("[OK ] con DENY el turno se cierra sin tarea remota");
    policy(false);

    let allowed = persisted_turn(&database, &conversation.id, &format!("allow-{run}"));
    super::spawn_submission_and_poll(
        database.clone(),
        broker(),
        database.task_record(&allowed).expect("registro"),
    );
    assert!(
        SimulatedBroker::wait_until(Duration::from_secs(600), || database
            .task_snapshot(&allowed)
            .is_ok_and(|task| task.local_state == "terminal")),
        "el turno autorizado debía terminar en el Broker real"
    );
    let task = database.task_snapshot(&allowed).expect("tarea");
    assert_eq!(task.remote_status, "completed", "{:?}", task.error);
    println!(
        "[OK ] con ALLOW el turno termina en el Broker real ({:?})",
        task.remote_task_id
    );
    let receipt: serde_json::Value = reqwest::blocking::Client::new()
        .post(format!("{core_url}/api/v1/operations/status"))
        .bearer_auth(&director)
        .json(&json!({"operation_id": format!("chatygpt:turn:allow-{run}"), "target_service_id": "ai_broker"}))
        .send()
        .and_then(|response| response.json())
        .expect("estado del recibo");
    assert_eq!(receipt["status"], "succeeded", "{receipt}");
    println!("[OK ] Core tiene el recibo succeeded del turno");
    cleanup(&database);
}

/// Turno real con Knowledge Orchestrator activado en la conversación.
///
/// Variables: `KNOWLEDGE_ORCHESTRATOR_URL`, `KO_E2E_TOKEN` (consumidor «read»),
/// `CHATYGPT_VERIFY_BROKER_URL` y `CHATYGPT_VERIFY_PROFILE_DIR` (perfil del que
/// solo se lee el token cifrado del Broker). Pasa por `start_chat_turn`, el
/// mismo camino que un mensaje enviado desde la interfaz.
#[test]
#[ignore = "necesita Knowledge Orchestrator y Broker AI reales"]
fn real_knowledge_orchestrator_feeds_a_chat_turn() {
    let env = |name: &str| std::env::var(name).unwrap_or_else(|_| panic!("falta {name}"));
    let data_dir =
        std::env::temp_dir().join(format!("chatygpt-ko-e2e-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&data_dir).expect("directorio");
    let database = Database::open(data_dir.join("chatygpt.db")).expect("base");
    crate::secrets::store_api_credential(
        &data_dir,
        crate::knowledge_orchestrator::ALIAS_CREDENCIAL,
        &env("KO_E2E_TOKEN"),
    )
    .expect("credencial");
    let conversation = database.create_conversation("KO real", None).expect("chat");
    let mut preferences = database
        .conversation_execution_preferences(&conversation.id)
        .expect("preferencias");
    preferences.knowledge_orchestrator = true;
    // Prioridad «Alta»: el Broker real suele tener cola de otras aplicaciones.
    preferences.priority = 25;
    database
        .update_conversation_execution_preferences(&conversation.id, &preferences)
        .expect("activar KO");
    // `KO_E2E_QUESTION` permite probar una síntesis entre varias notas reales.
    let question = std::env::var("KO_E2E_QUESTION").ok();
    let custom_question = question.is_some();
    let question = question.unwrap_or_else(|| {
        "¿Cada cuánto tengo que descalcificar la cafetera y con qué producto?".to_owned()
    });
    let broker = BrokerClient::for_base_url(&env("CHATYGPT_VERIFY_BROKER_URL")).expect("cliente");
    broker
        .replace_admin_token(
            crate::secrets::load_broker_token(std::path::Path::new(&env(
                "CHATYGPT_VERIFY_PROFILE_DIR",
            )))
            .as_deref(),
        )
        .expect("token");

    let snapshot = tauri::async_runtime::block_on(super::start_chat_turn(
        database.clone(),
        broker,
        &conversation.id,
        &question,
        &[],
        false,
        false,
        false,
        false,
    ))
    .expect("turno");
    let request = database
        .task_record(&snapshot.id)
        .expect("registro")
        .request;
    let prompt = request["content"]["prompt"].as_str().expect("prompt");
    assert!(prompt.contains("Knowledge Orchestrator"));
    assert_eq!(request["execution"]["long_context"], "map_reduce");
    println!(
        "[OK ] la petición adjunta todo el conocimiento: documentos {} en {}",
        request["content"]["metadata"]["knowledge_orchestrator_document_ids"],
        request["content"]["attachments"]
    );
    assert!(
        SimulatedBroker::wait_until(Duration::from_secs(600), || database
            .task_snapshot(&snapshot.id)
            .is_ok_and(|task| task.local_state == "terminal")),
        "el turno debía terminar en el Broker real"
    );
    let task = database.task_snapshot(&snapshot.id).expect("tarea");
    assert_eq!(task.remote_status, "completed", "{:?}", task.error);
    let answer = database
        .conversation_view(&conversation.id)
        .expect("conversación")
        .messages
        .into_iter()
        .filter(|message| message.role == "assistant")
        .find_map(|message| message.text)
        .expect("respuesta");
    println!("[OK ] respuesta del modelo:\n{answer}");
    assert!(
        custom_question || answer.contains("90"),
        "la respuesta no usa el conocimiento"
    );
    let _ = std::fs::remove_dir_all(&data_dir);
}
