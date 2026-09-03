//! Pruebas de `task_runtime`.
//!
//! Viven aparte desde que el fichero paso de mil lineas: separarlas deja
//! la logica a la vista sin cambiar una sola linea de codigo.

use super::{
    apply_deep_research_plan, apply_document_index_dependency, chat_request,
    chat_request_with_project_instruction, configured_api_url, custom_gpt_context_budget,
    custom_gpt_prompt_block, deep_research_plan, deterministic_jitter, embedding_request,
    is_tabular_attachment, memory_embedding_request, persisted_custom_gpt_allows_tool,
    replace_bounded_authorized_text, validate_sandbox_capability, ChatExecutionOptions,
    ResearchPlan,
};
use super::{cancel_task, recover_at_start, resolve_tool_calls, start_chat_turn, ToolDecision};
use crate::broker::simulated::{
    accepted_task, completed_chat_result, failed_task_state, task_state, waiting_for_tools_state,
    ScriptedResponse, SimulatedBroker,
};
use crate::broker::{BrokerCapabilities, BrokerClient};
use crate::db::{
    AttachmentRecord, ContextMessage, ConversationExecutionPreferences, CustomGptContext,
    CustomGptToolPermissions, Database, MemoryItemView, ProjectInstructionContext,
    SelectedAttachmentChunk,
};
use crate::error::AppError;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;
use std::time::Duration;
use uuid::Uuid;

/// Tiempo máximo que una prueba espera a que un bucle asíncrono se asiente.
///
/// El sondeo arranca en 750 ms y crece; este margen cubre varias vueltas sin
/// convertir un fallo real en una prueba que cuelga la suite.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(20);

fn integration_database() -> Database {
    let path = std::env::temp_dir().join(format!(
        "chatygpt-runtime-test-{}.sqlite",
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

/// Envía un turno de chat corriente y devuelve el identificador local.
fn send_turn(database: &Database, broker: &BrokerClient, conversation_id: &str) -> String {
    tauri::async_runtime::block_on(start_chat_turn(
        database.clone(),
        broker.clone(),
        conversation_id,
        "¿Qué dice la normativa sobre esto?",
        &[],
        false,
        false,
        false,
        false,
    ))
    .expect("el turno debe persistirse y lanzarse")
    .id
}

/// Al arrancar se cierra lo que quedó pausado y aquí ya se dio por perdido.
///
/// `waiting_for_tools` no caduca: sin esto, una investigación huérfana
/// seguiría esperando en el Broker una respuesta que nadie va a enviar.
#[test]
fn a_startup_closes_abandoned_tasks_that_are_still_paused_in_the_broker() {
    let simulated = SimulatedBroker::start();
    simulated.script(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("remote-abandoned")),
    );
    let mut paused = task_state("remote-abandoned", "waiting_for_tools", None);
    paused["result"] = json!({
        "status": "waiting_for_tools",
        "pending_tool_calls": [{
            "id": "call_1",
            "name": "rename_conversation",
            "arguments": {"title": "Otro título"}
        }]
    });
    simulated.always("GET /api/v1/tasks/{id}", ScriptedResponse::ok(paused));
    simulated.always(
        "DELETE /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("remote-abandoned", "cancelled", None)),
    );

    let database = integration_database();
    let broker =
        BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
    let conversation = database
        .create_conversation("Abandonada", None)
        .expect("la conversación debe crearse");
    let local_id = send_turn(&database, &broker, &conversation.id);
    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.local_state == "waiting_for_tools")),
        "la tarea debía quedar pausada esperando una decisión"
    );

    // Se da por perdida: un error permanente impidió seguir atendiéndola.
    database
        .mark_orphaned(&local_id, "el envío de resultados fue rechazado")
        .expect("la tarea debe poder marcarse como huérfana");

    recover_at_start(database.clone(), broker.clone()).expect("la recuperación debe correr");

    // Se espera al efecto persistido, no a que asome la petición: el
    // `DELETE` queda registrado en el simulador antes de que ChatyGPT haya
    // procesado su respuesta, y comprobar ahí la base es una carrera.
    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .abandoned_remote_tasks()
            .is_ok_and(|pending| pending.is_empty())),
        "el arranque debía cerrar la tarea abandonada en el Broker"
    );
    assert!(!simulated
        .requests_to("DELETE", "/api/v1/tasks/remote-abandoned")
        .is_empty());
    // Se consulta antes de cancelar: no se descarta trabajo a ciegas.
    assert!(!simulated
        .requests_to("GET", "/api/v1/tasks/remote-abandoned")
        .is_empty());
    assert_eq!(
        simulated
            .requests_to("DELETE", "/api/v1/tasks/remote-abandoned")
            .len(),
        1,
        "cancelar una vez basta"
    );

    // Y queda auditado: es trabajo del Broker que se descarta sin preguntar.
    let audited = database
        .list_audit_events(200)
        .expect("la auditoría debe poder consultarse")
        .into_iter()
        .filter(|event| event.summary == "Tarea abandonada cerrada en Broker AI")
        .collect::<Vec<_>>();
    assert_eq!(audited.len(), 1);
    // No se presenta como una anotación más: cerrar trabajo del Broker sin
    // preguntar merece verse como aviso.
    assert_eq!(audited[0].severity, "warning");
    assert_eq!(audited[0].actor, "chatygpt");

    // Su estado remoto queda anotado, así que un segundo arranque no la
    // vuelve a cancelar.
    assert_eq!(
        database
            .task_snapshot(&local_id)
            .expect("la tarea existe")
            .remote_status,
        "cancelled"
    );

    cleanup(&database);
}

/// Una tarea que terminó sola no se cancela: solo se anota su desenlace.
#[test]
fn an_abandoned_task_that_finished_on_its_own_is_not_cancelled() {
    let simulated = SimulatedBroker::start();
    simulated.script(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("remote-finished")),
    );
    simulated.script(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("remote-finished", "generating", None)),
    );
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state(
            "remote-finished",
            "completed",
            Some(completed_chat_result("Terminó por su cuenta.")),
        )),
    );

    let database = integration_database();
    let broker =
        BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
    let conversation = database
        .create_conversation("Terminó sola", None)
        .expect("la conversación debe crearse");
    let local_id = send_turn(&database, &broker, &conversation.id);
    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.remote_task_id.is_some())),
        "la tarea debía enlazarse con su identidad remota"
    );
    database
        .mark_orphaned(&local_id, "se dio por perdida mientras trabajaba")
        .expect("la tarea debe poder marcarse como huérfana");

    recover_at_start(database.clone(), broker.clone()).expect("la recuperación debe correr");

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.remote_status == "completed")),
        "debía anotarse el desenlace real"
    );
    assert!(
        simulated
            .requests_to("DELETE", "/api/v1/tasks/remote-finished")
            .is_empty(),
        "no se cancela algo que ya había terminado"
    );

    cleanup(&database);
}

/// Capacidades mínimas que admiten una investigación.
fn research_capabilities() -> BrokerCapabilities {
    BrokerCapabilities {
        contract_version: "2.7".to_owned(),
        strategies: vec!["single".to_owned(), "agent".to_owned()],
        agent_skills: vec!["web_search".to_owned()],
        client_tool_passthrough: Some(true),
        ..BrokerCapabilities::default()
    }
}

/// Lanza una investigación contra el simulador.
fn send_research_turn(database: &Database, broker: &BrokerClient, conversation_id: &str) -> String {
    tauri::async_runtime::block_on(start_chat_turn(
        database.clone(),
        broker.clone(),
        conversation_id,
        "Contrasta la normativa europea con fuentes públicas",
        &[],
        false,
        false,
        false,
        true,
    ))
    .expect("la investigación debe persistirse y lanzarse")
    .id
}

/// El bucle de herramientas se resuelve solo y deja un paso real.
///
/// La URL que pide el modelo apunta al propio equipo, que es justo lo que
/// `validate_fetch_url` rechaza. Sirve para dos cosas a la vez: comprobar
/// que la guarda aguanta de extremo a extremo —un modelo no puede hacer
/// que ChatyGPT llame a la puerta de su propio Broker— y que un fallo de
/// herramienta viaja como resultado, no como silencio, de modo que la
/// tarea continúa en lugar de quedarse esperando para siempre.
#[test]
fn a_research_resolves_its_own_tools_and_records_each_one_as_a_step() {
    let simulated = SimulatedBroker::start();
    simulated.always(
        "GET /api/v1/capabilities",
        ScriptedResponse::ok(serde_json::to_value(research_capabilities()).unwrap()),
    );
    simulated.script(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("remote-research")),
    );
    let mut paused = task_state("remote-research", "waiting_for_tools", None);
    paused["execution_strategy"] = json!("agent");
    paused["progress"] = json!({
        "phase": "generating",
        "invocations_completed": 1,
        "invocations_total": 1,
        "agent_iteration": 2,
        "agent_max_iterations": 12
    });
    paused["result"] = json!({
        "status": "waiting_for_tools",
        "pending_tool_calls": [{
            "id": "call_1",
            "name": "fetch_url",
            "arguments": {"url": "http://127.0.0.1:8765/api/v1/tasks"}
        }]
    });
    simulated.always("GET /api/v1/tasks/{id}", ScriptedResponse::ok(paused));
    let resolved = task_state(
        "remote-research",
        "completed",
        Some(completed_chat_result("Informe con las fuentes accesibles.")),
    );
    simulated.always(
        "POST /api/v1/tasks/{id}/tool_results",
        ScriptedResponse::ok(resolved.clone()),
    );
    // Recibir los resultados es lo que reanuda la tarea.
    simulated.after(
        "POST /api/v1/tasks/{id}/tool_results",
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(resolved),
    );

    let database = integration_database();
    let broker =
        BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
    let conversation = database
        .create_conversation("Investigación", None)
        .expect("la conversación debe crearse");
    let local_id = send_research_turn(&database, &broker, &conversation.id);

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.remote_status == "completed")),
        "la investigación debía resolver su herramienta y continuar sola"
    );

    // La decisión se envió una sola vez, con el identificador de la llamada.
    let submissions = simulated.requests_to("POST", "/api/v1/tasks/remote-research/tool_results");
    assert_eq!(submissions.len(), 1);
    let results = submissions[0].body["tool_results"]
        .as_array()
        .expect("el contrato exige una lista de resultados");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["tool_call_id"], "call_1");

    // La guarda aguantó: no se abrió ninguna dirección del propio equipo.
    assert!(
        results[0]["content"]
            .as_str()
            .is_some_and(|content| content.contains("propio equipo")),
        "el resultado debía explicar por qué no se abrió la URL"
    );

    // Y quedó como paso real, con su parámetro visible.
    let view = database
        .conversation_view(&conversation.id)
        .expect("la conversación debe cargarse");
    let steps = &view.research_runs[0].steps;
    assert_eq!(steps.len(), 1);
    assert_eq!(
        steps[0].title,
        "fetch_url: http://127.0.0.1:8765/api/v1/tasks"
    );
    assert_eq!(steps[0].status, "failed");

    cleanup(&database);
}

/// El recorrido completo termina en estado terminal y materializa la respuesta.
///
/// Es el criterio de aceptación «polling no bloquea la interfaz, aplica
/// límites y termina en estados terminales» comprobado contra un servidor,
/// no contra una función pura.
#[test]
fn chat_turn_polls_until_terminal_and_materializes_the_answer() {
    let simulated = SimulatedBroker::start();
    simulated.script(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("remote-happy")),
    );
    // Una fase intermedia antes del estado terminal: el sondeo debe seguir.
    simulated.script(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("remote-happy", "generating", None)),
    );
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state(
            "remote-happy",
            "completed",
            Some(completed_chat_result("La normativa exige contrato previo.")),
        )),
    );

    let database = integration_database();
    let broker =
        BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
    let conversation = database
        .create_conversation("Consulta normativa", None)
        .expect("la conversación debe crearse");
    let local_id = send_turn(&database, &broker, &conversation.id);

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.remote_status == "completed")),
        "la tarea debía alcanzar un estado terminal"
    );

    let task = database.task_snapshot(&local_id).expect("la tarea existe");
    assert_eq!(task.remote_task_id.as_deref(), Some("remote-happy"));
    assert!(task.error.is_none());
    // El sondeo se detiene: no sigue preguntando tras el estado terminal.
    let polls_at_settle = simulated
        .requests_to("GET", "/api/v1/tasks/remote-happy")
        .len();
    std::thread::sleep(Duration::from_millis(1_500));
    assert_eq!(
        simulated
            .requests_to("GET", "/api/v1/tasks/remote-happy")
            .len(),
        polls_at_settle,
        "tras el estado terminal no debe haber más sondeos"
    );

    // La respuesta queda materializada como mensaje del asistente.
    let view = database
        .conversation_view(&conversation.id)
        .expect("la conversación debe cargarse");
    let answer = view
        .messages
        .iter()
        .find(|message| message.role == "assistant")
        .expect("debe existir la respuesta");
    assert_eq!(answer.status, "complete");
    assert!(answer
        .text
        .as_deref()
        .is_some_and(|text| text.contains("contrato previo")));

    cleanup(&database);
}

/// Un fallo transitorio se reintenta y no crea una segunda tarea remota.
///
/// Es el criterio «la misma operación reintentada no duplica la tarea»:
/// aunque el cliente envíe dos veces, la clave idempotente es la misma y
/// localmente solo existe un identificador remoto.
#[test]
fn transient_failure_is_retried_with_the_same_idempotency_key() {
    let simulated = SimulatedBroker::start();
    simulated.script("POST /api/v1/tasks", ScriptedResponse::transient());
    simulated.script(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("remote-retry")),
    );
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state(
            "remote-retry",
            "completed",
            Some(completed_chat_result("Respuesta tras el reintento.")),
        )),
    );

    let database = integration_database();
    let broker =
        BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
    let conversation = database
        .create_conversation("Reintento", None)
        .expect("la conversación debe crearse");
    let local_id = send_turn(&database, &broker, &conversation.id);

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.remote_status == "completed")),
        "el reintento debía completar la tarea"
    );

    let submissions = simulated.requests_to("POST", "/api/v1/tasks");
    assert_eq!(
        submissions.len(),
        2,
        "debía reintentarse exactamente una vez"
    );
    let first_key = submissions[0].body["idempotency_key"]
        .as_str()
        .expect("la petición lleva clave idempotente");
    let second_key = submissions[1].body["idempotency_key"]
        .as_str()
        .expect("el reintento lleva clave idempotente");
    assert_eq!(
        first_key, second_key,
        "el reintento debe reutilizar la clave para que el Broker deduplique"
    );
    // Localmente tampoco hay duplicado: una tarea, un identificador remoto.
    let task = database.task_snapshot(&local_id).expect("la tarea existe");
    assert_eq!(task.remote_task_id.as_deref(), Some("remote-retry"));

    cleanup(&database);
}

/// Un rechazo permanente huérfana la tarea y no se reintenta jamás.
#[test]
fn permanent_rejection_orphans_the_task_without_retrying() {
    let simulated = SimulatedBroker::start();
    simulated.always("POST /api/v1/tasks", ScriptedResponse::permanent());

    let database = integration_database();
    let broker =
        BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
    let conversation = database
        .create_conversation("Contrato inválido", None)
        .expect("la conversación debe crearse");
    let local_id = send_turn(&database, &broker, &conversation.id);

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.local_state == "orphaned")),
        "un rechazo de contrato debía dejar la tarea huérfana"
    );

    // Lo esencial: un error permanente no entra en el bucle de reintento.
    std::thread::sleep(Duration::from_millis(1_500));
    assert_eq!(
        simulated.requests_to("POST", "/api/v1/tasks").len(),
        1,
        "un error permanente no debe reintentarse"
    );
    let task = database.task_snapshot(&local_id).expect("la tarea existe");
    assert!(task.remote_task_id.is_none());

    cleanup(&database);
}

/// La cancelación refleja la respuesta real del Broker, no una suposición.
#[test]
fn cancellation_reflects_the_real_broker_response() {
    let simulated = SimulatedBroker::start();
    simulated.script(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("remote-cancel")),
    );
    // Mientras no se cancele, la tarea sigue trabajando.
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("remote-cancel", "generating", None)),
    );
    simulated.always(
        "DELETE /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("remote-cancel", "cancelled", None)),
    );
    // Aceptar la cancelación es lo que cambia el estado: a partir de ahí el
    // sondeo tampoco puede volver a verla trabajando.
    simulated.after(
        "DELETE /api/v1/tasks/{id}",
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("remote-cancel", "cancelled", None)),
    );

    let database = integration_database();
    let broker =
        BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
    let conversation = database
        .create_conversation("Cancelación", None)
        .expect("la conversación debe crearse");
    let local_id = send_turn(&database, &broker, &conversation.id);
    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.remote_task_id.is_some())),
        "la tarea debía enlazarse con su identidad remota"
    );

    let cancelled =
        tauri::async_runtime::block_on(cancel_task(database.clone(), broker.clone(), &local_id))
            .expect("la cancelación debe resolverse");
    assert_eq!(cancelled.remote_status, "cancelled");
    assert_eq!(
        simulated
            .requests_to("DELETE", "/api/v1/tasks/remote-cancel")
            .len(),
        1
    );

    cleanup(&database);
}

/// Un fallo remoto se traslada al mensaje sin inventar una respuesta.
#[test]
fn remote_failure_is_reported_instead_of_being_answered() {
    let simulated = SimulatedBroker::start();
    simulated.script(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("remote-failed")),
    );
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(failed_task_state(
            "remote-failed",
            "ningún proveedor local respondió",
        )),
    );

    let database = integration_database();
    let broker =
        BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
    let conversation = database
        .create_conversation("Fallo remoto", None)
        .expect("la conversación debe crearse");
    let local_id = send_turn(&database, &broker, &conversation.id);

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.remote_status == "failed")),
        "la tarea debía terminar como fallida"
    );

    let view = database
        .conversation_view(&conversation.id)
        .expect("la conversación debe cargarse");
    let answer = view
        .messages
        .iter()
        .find(|message| message.role == "assistant")
        .expect("debe existir el mensaje del asistente");
    // No se fabrica contenido: el mensaje queda fallido y conserva el error.
    assert_eq!(answer.status, "failed");
    assert!(answer.text.is_none());
    assert_eq!(
        answer
            .error
            .as_ref()
            .and_then(|error| error["code"].as_str()),
        Some("PROVIDER_UNAVAILABLE")
    );

    cleanup(&database);
}

/// El sondeo se detiene en `waiting_for_tools` y reanuda tras la decisión.
///
/// Es la garantía de que ninguna herramienta se ejecuta sin confirmación:
/// el bucle no avanza solo, espera a que la persona decida.
#[test]
fn polling_waits_for_a_tool_decision_and_resumes_after_it() {
    let simulated = SimulatedBroker::start();
    simulated.script(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("remote-tools")),
    );
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(waiting_for_tools_state(
            "remote-tools",
            "call-rename-1",
            "rename_conversation",
        )),
    );
    let resolved_state = task_state(
        "remote-tools",
        "completed",
        Some(completed_chat_result("Listo, he aplicado la decisión.")),
    );
    simulated.always(
        "POST /api/v1/tasks/{id}/tool_results",
        ScriptedResponse::ok(resolved_state.clone()),
    );
    // Recibir la decisión es lo que completa la tarea: a partir de ahí, el
    // sondeo ya no puede volver a verla esperando herramientas.
    simulated.after(
        "POST /api/v1/tasks/{id}/tool_results",
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(resolved_state),
    );

    let database = integration_database();
    let broker =
        BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
    let conversation = database
        .create_conversation("Herramientas", None)
        .expect("la conversación debe crearse");
    let local_id = send_turn(&database, &broker, &conversation.id);

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.local_state == "waiting_for_tools")),
        "la tarea debía detenerse a esperar la decisión"
    );
    // El bucle no avanza solo: sin decisión no se envían resultados.
    std::thread::sleep(Duration::from_millis(1_500));
    assert!(
        simulated
            .requests_to("POST", "/api/v1/tasks/remote-tools/tool_results")
            .is_empty(),
        "no debe enviarse ningún resultado antes de que la persona decida"
    );
    let waiting = database.task_snapshot(&local_id).expect("la tarea existe");
    assert_eq!(waiting.pending_tool_calls.len(), 1);

    let resolved = tauri::async_runtime::block_on(resolve_tool_calls(
        database.clone(),
        broker.clone(),
        &std::env::temp_dir(),
        &local_id,
        &[ToolDecision {
            tool_call_id: waiting.pending_tool_calls[0].tool_call_id.clone(),
            approved: false,
        }],
    ))
    .expect("la decisión debe resolverse");
    assert!(resolved.pending_tool_calls.is_empty());

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.remote_status == "completed")),
        "tras la decisión la tarea debía continuar hasta completarse"
    );
    assert_eq!(
        simulated
            .requests_to("POST", "/api/v1/tasks/remote-tools/tool_results")
            .len(),
        1,
        "la decisión se envía una sola vez"
    );

    cleanup(&database);
}

/// Un corte transitorio durante el sondeo no da la tarea por perdida.
///
/// Es la diferencia entre «el Broker no responde ahora» y «esta tarea no
/// existe»: lo primero se reintenta conservando la identidad remota.
#[test]
fn transient_polling_errors_are_retried_without_losing_the_task() {
    let simulated = SimulatedBroker::start();
    simulated.script(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("remote-flaky")),
    );
    simulated.script("GET /api/v1/tasks/{id}", ScriptedResponse::transient());
    simulated.script("GET /api/v1/tasks/{id}", ScriptedResponse::transient());
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state(
            "remote-flaky",
            "completed",
            Some(completed_chat_result("Respuesta pese al corte.")),
        )),
    );

    let database = integration_database();
    let broker =
        BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
    let conversation = database
        .create_conversation("Corte transitorio", None)
        .expect("la conversación debe crearse");
    let local_id = send_turn(&database, &broker, &conversation.id);

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.remote_status == "completed")),
        "el sondeo debía superar los cortes y completar la tarea"
    );

    let task = database.task_snapshot(&local_id).expect("la tarea existe");
    // La identidad remota nunca se pierde ni se reenvía la tarea.
    assert_eq!(task.remote_task_id.as_deref(), Some("remote-flaky"));
    assert_eq!(task.local_state, "terminal");
    assert_eq!(simulated.requests_to("POST", "/api/v1/tasks").len(), 1);
    assert!(
        simulated
            .requests_to("GET", "/api/v1/tasks/remote-flaky")
            .len()
            >= 3,
        "debían registrarse los dos cortes y el sondeo con éxito"
    );

    cleanup(&database);
}

/// Un error de contrato durante el sondeo huérfana la tarea en lugar de
/// reintentar indefinidamente contra algo que no puede mejorar.
#[test]
fn permanent_polling_error_orphans_the_task_instead_of_looping() {
    let simulated = SimulatedBroker::start();
    simulated.script(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("remote-broken")),
    );
    simulated.always("GET /api/v1/tasks/{id}", ScriptedResponse::permanent());

    let database = integration_database();
    let broker =
        BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
    let conversation = database
        .create_conversation("Contrato roto al sondear", None)
        .expect("la conversación debe crearse");
    let local_id = send_turn(&database, &broker, &conversation.id);

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.local_state == "orphaned")),
        "un error permanente al sondear debía dejar la tarea huérfana"
    );

    let polls_at_settle = simulated
        .requests_to("GET", "/api/v1/tasks/remote-broken")
        .len();
    std::thread::sleep(Duration::from_millis(1_500));
    assert_eq!(
        simulated
            .requests_to("GET", "/api/v1/tasks/remote-broken")
            .len(),
        polls_at_settle,
        "el bucle debe detenerse, no seguir preguntando"
    );
    // La tarea conserva su identidad remota: queda trazada, no borrada.
    let task = database.task_snapshot(&local_id).expect("la tarea existe");
    assert_eq!(task.remote_task_id.as_deref(), Some("remote-broken"));

    cleanup(&database);
}

/// Un reinicio reanuda una tarea activa sin crear una segunda en el Broker.
///
/// Es el criterio «un reinicio recupera tareas activas sin pérdida»: la
/// tarea ya tenía identidad remota, así que recuperarla debe sondearla, no
/// volver a enviarla.
#[test]
fn restart_resumes_an_active_task_without_submitting_it_again() {
    let simulated = SimulatedBroker::start();
    simulated.script(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("remote-recovered")),
    );
    // Durante la primera vida de la aplicación la tarea sigue trabajando.
    simulated.script(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("remote-recovered", "generating", None)),
    );

    let database = integration_database();
    let broker =
        BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
    let conversation = database
        .create_conversation("Recuperación", None)
        .expect("la conversación debe crearse");
    let local_id = send_turn(&database, &broker, &conversation.id);
    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.remote_task_id.is_some())),
        "la tarea debía enlazarse antes de simular el reinicio"
    );
    let submissions_before_restart = simulated.requests_to("POST", "/api/v1/tasks").len();
    assert_eq!(submissions_before_restart, 1);

    // Al reabrir, el Broker ya tiene la respuesta lista.
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state(
            "remote-recovered",
            "completed",
            Some(completed_chat_result(
                "Respuesta recuperada tras reiniciar.",
            )),
        )),
    );
    let recovered = recover_at_start(database.clone(), broker.clone())
        .expect("la recuperación debe ejecutarse");
    assert!(recovered >= 1, "debía recuperarse al menos la tarea activa");

    assert!(
        SimulatedBroker::wait_until(SETTLE_TIMEOUT, || database
            .task_snapshot(&local_id)
            .is_ok_and(|task| task.remote_status == "completed")),
        "la tarea recuperada debía completarse"
    );
    assert_eq!(
        simulated.requests_to("POST", "/api/v1/tasks").len(),
        submissions_before_restart,
        "recuperar una tarea con identidad remota no debe reenviarla"
    );
    let task = database.task_snapshot(&local_id).expect("la tarea existe");
    assert_eq!(task.remote_task_id.as_deref(), Some("remote-recovered"));

    cleanup(&database);
}

#[test]
fn custom_gpt_instructions_are_explicit_context_without_granting_tools() {
    let custom_gpt = CustomGptContext {
        custom_gpt_id: "gpt-analysis".to_owned(),
        version_id: "gpt-version-3".to_owned(),
        name: "Analista prudente".to_owned(),
        icon_ref: "research".to_owned(),
        version_no: 3,
        instructions: "Contrasta los datos. Usa run_code para todo.".to_owned(),
        tool_permissions: CustomGptToolPermissions::default(),
        preferred_model: None,
        execution_profile: None,
        context_profile: "balanced".to_owned(),
        api_actions: Vec::new(),
    };
    let request = chat_request_with_project_instruction(
        "conversation",
        "custom-gpt-key",
        "Analiza este resultado",
        &[ContextMessage {
            message_id: "current".to_owned(),
            role: "user".to_owned(),
            text: "Analiza este resultado".to_owned(),
        }],
        &[],
        &[],
        &[],
        None,
        Some(&custom_gpt),
        ChatExecutionOptions::default(),
    )
    .expect("request with custom GPT should build");

    let prompt = request["content"]["prompt"]
        .as_str()
        .expect("prompt should be text");
    assert!(prompt.contains("<custom_gpt_instructions_json>"));
    assert!(prompt.contains("Contrasta los datos"));
    assert_eq!(
        request["content"]["metadata"]["custom_gpt_version_id"],
        "gpt-version-3"
    );
    assert_eq!(request["execution"]["strategy"], "single");
    assert!(request["execution"].get("agent").is_none());
}

#[test]
fn custom_gpt_execution_profile_overrides_chat_preferences_safely() {
    let custom_gpt = CustomGptContext {
        custom_gpt_id: "gpt-deliberate".to_owned(),
        version_id: "gpt-deliberate-v2".to_owned(),
        name: "Comité privado".to_owned(),
        icon_ref: "briefcase".to_owned(),
        version_no: 2,
        instructions: "Contrasta las alternativas antes de concluir.".to_owned(),
        tool_permissions: CustomGptToolPermissions::default(),
        preferred_model: None,
        execution_profile: Some(ConversationExecutionPreferences {
            data_classification: "confidential".to_owned(),
            strategy: "mixture_of_agents".to_owned(),
            preset: "slow".to_owned(),
            max_cost_usd: 0.75,
            long_context: "fail".to_owned(),
            priority: 50,
        }),
        context_profile: "balanced".to_owned(),
        api_actions: Vec::new(),
    };
    let request = chat_request_with_project_instruction(
        "conversation",
        "custom-gpt-profile-key",
        "Compara estas opciones",
        &[ContextMessage {
            message_id: "current".to_owned(),
            role: "user".to_owned(),
            text: "Compara estas opciones".to_owned(),
        }],
        &[],
        &[],
        &[],
        None,
        Some(&custom_gpt),
        ChatExecutionOptions::default(),
    )
    .expect("profiled request should build");

    assert_eq!(request["execution"]["strategy"], "mixture_of_agents");
    assert_eq!(request["execution"]["preset"], "slow");
    assert_eq!(request["execution"]["scheduling"], "adaptive");
    assert_eq!(request["risk"]["data_classification"], "confidential");
    assert_eq!(request["model_requirements"]["max_cost_usd"], 0.75);
    assert_eq!(request["priority"], 50);
}

#[test]
fn authorized_folder_tools_require_gpt_permission_and_force_local_routing() {
    let custom_gpt = CustomGptContext {
        custom_gpt_id: "gpt-folders".to_owned(),
        version_id: "gpt-folders-v1".to_owned(),
        name: "Archivista".to_owned(),
        icon_ref: "research".to_owned(),
        version_no: 1,
        instructions: "Ayuda a localizar información autorizada.".to_owned(),
        tool_permissions: CustomGptToolPermissions {
            run_code: "deny".to_owned(),
            rename_conversation: "deny".to_owned(),
            read_authorized_folders: "confirm".to_owned(),
            modify_authorized_files: "deny".to_owned(),
            create_scheduled_tasks: "deny".to_owned(),
            call_external_apis: "deny".to_owned(),
        },
        preferred_model: None,
        execution_profile: None,
        context_profile: "balanced".to_owned(),
        api_actions: Vec::new(),
    };
    let request = chat_request_with_project_instruction(
        "conversation",
        "folder-key",
        "Lista los archivos de la carpeta autorizada",
        &[],
        &[],
        &[],
        &[],
        None,
        Some(&custom_gpt),
        ChatExecutionOptions {
            tools_enabled: true,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("la petición con permiso debe construirse");

    let names = request["execution"]["agent"]["client_tools"]
        .as_array()
        .expect("debe ofrecer herramientas")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        vec!["list_authorized_folders", "read_authorized_file"]
    );
    assert_eq!(request["risk"]["data_classification"], "local_only");
}

#[test]
fn scheduled_task_tool_requires_explicit_intent_and_gpt_permission() {
    let custom_gpt = CustomGptContext {
        custom_gpt_id: "gpt-scheduler".to_owned(),
        version_id: "gpt-scheduler-v1".to_owned(),
        name: "Organizador".to_owned(),
        icon_ref: "briefcase".to_owned(),
        version_no: 1,
        instructions: "Ayuda a organizar el trabajo.".to_owned(),
        tool_permissions: CustomGptToolPermissions {
            create_scheduled_tasks: "confirm".to_owned(),
            ..CustomGptToolPermissions::default()
        },
        preferred_model: None,
        execution_profile: None,
        context_profile: "balanced".to_owned(),
        api_actions: Vec::new(),
    };
    let request = chat_request_with_project_instruction(
        "conversation",
        "schedule-key",
        "Programa un recordatorio mañana a las 10",
        &[],
        &[],
        &[],
        &[],
        None,
        Some(&custom_gpt),
        ChatExecutionOptions {
            tools_enabled: true,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("la petición de programación debe construirse");

    let names = request["execution"]["agent"]["client_tools"]
        .as_array()
        .expect("debe ofrecer la herramienta")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["create_scheduled_task"]);
    assert_eq!(request["execution"]["strategy"], "agent");
    assert_eq!(
        request["execution"]["agent"]["skills"],
        json!(["current_datetime"])
    );
}

#[test]
fn external_api_tool_requires_explicit_https_url_and_permission() {
    let custom_gpt = CustomGptContext {
        custom_gpt_id: "gpt-api".to_owned(),
        version_id: "gpt-api-v1".to_owned(),
        name: "Datos públicos".to_owned(),
        icon_ref: "data".to_owned(),
        version_no: 1,
        instructions: "Consulta datos públicos cuando te lo pidan.".to_owned(),
        tool_permissions: CustomGptToolPermissions {
            call_external_apis: "confirm".to_owned(),
            ..CustomGptToolPermissions::default()
        },
        preferred_model: None,
        execution_profile: None,
        context_profile: "balanced".to_owned(),
        api_actions: Vec::new(),
    };
    let request = chat_request_with_project_instruction(
        "conversation",
        "api-key",
        "Consulta la API https://api.example.org/v1/weather?q=Arrecife",
        &[],
        &[],
        &[],
        &[],
        None,
        Some(&custom_gpt),
        ChatExecutionOptions {
            tools_enabled: true,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("la petición debe ofrecer la herramienta");
    assert_eq!(
        request["execution"]["agent"]["client_tools"][0]["name"],
        "call_external_api"
    );
    assert_eq!(request["execution"]["preset"], "fast");

    let without_url = chat_request_with_project_instruction(
        "conversation",
        "api-key-2",
        "Explícame qué es una API",
        &[],
        &[],
        &[],
        &[],
        None,
        Some(&custom_gpt),
        ChatExecutionOptions {
            tools_enabled: true,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("una explicación normal debe seguir funcionando");
    assert!(without_url["execution"].get("agent").is_none());
}

#[test]
fn configured_api_action_has_a_fixed_destination_and_versioned_parameters() {
    let custom_gpt = CustomGptContext {
        custom_gpt_id: "gpt-weather".to_owned(),
        version_id: "gpt-weather-v1".to_owned(),
        name: "Tiempo".to_owned(),
        icon_ref: "data".to_owned(),
        version_no: 1,
        instructions: "Consulta el tiempo cuando sea útil.".to_owned(),
        tool_permissions: CustomGptToolPermissions {
            call_external_apis: "confirm".to_owned(),
            ..CustomGptToolPermissions::default()
        },
        preferred_model: None,
        execution_profile: None,
        context_profile: "balanced".to_owned(),
        api_actions: vec![crate::db::CustomGptApiAction {
            name: "consultar_tiempo".to_owned(),
            description: "Consulta el tiempo de una ciudad".to_owned(),
            url: "https://api.example.org/weather/{city}".to_owned(),
            query_parameters: Vec::new(),
            credential_ref: Some("weather_service".to_owned()),
            auth_mode: "bearer".to_owned(),
            parameters: vec![
                crate::db::CustomGptApiParameter {
                    name: "city".to_owned(),
                    value_type: "string".to_owned(),
                    required: true,
                    location: "path".to_owned(),
                    description: Some("Ciudad que se quiere consultar".to_owned()),
                },
                crate::db::CustomGptApiParameter {
                    name: "metric".to_owned(),
                    value_type: "boolean".to_owned(),
                    required: false,
                    location: "query".to_owned(),
                    description: Some("Usar unidades métricas".to_owned()),
                },
            ],
        }],
    };
    let request = chat_request_with_project_instruction(
        "conversation",
        "weather-key",
        "¿Qué tiempo hace en Arrecife?",
        &[],
        &[],
        &[],
        &[],
        None,
        Some(&custom_gpt),
        ChatExecutionOptions {
            tools_enabled: true,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("la acción configurada debe ofrecerse");
    let tool = &request["execution"]["agent"]["client_tools"][0];
    assert_eq!(tool["name"], "api_action_consultar_tiempo");
    assert_eq!(
        tool["parameters"]["properties"]["url"]["const"],
        "https://api.example.org/weather/{city}"
    );
    assert_eq!(
        request["content"]["metadata"]["custom_gpt_api_actions"][0]["parameters"][0]["name"],
        "city"
    );
    assert_eq!(tool["parameters"]["properties"]["city"]["type"], "string");
    assert_eq!(
        tool["parameters"]["properties"]["metric"]["type"],
        "boolean"
    );
    assert_eq!(
        tool["parameters"]["required"],
        json!(["city", "url", "credential_ref", "auth_mode"])
    );
    assert_eq!(
        tool["parameters"]["properties"]["credential_ref"]["const"],
        "weather_service"
    );
    assert_eq!(
        tool["parameters"]["properties"]["auth_mode"]["const"],
        "bearer"
    );
    assert!(
        !request.to_string().contains("weather-secret-value"),
        "la petición al Broker no debe contener el secreto"
    );
    let action = &request["content"]["metadata"]["custom_gpt_api_actions"][0];
    let valid = configured_api_url(
        action,
        &json!({
            "url": "https://api.example.org/weather/{city}",
            "credential_ref": "weather_service",
            "auth_mode": "bearer",
            "city": "Arrecife",
            "metric": true
        }),
    )
    .expect("los valores tipados deben formar una URL segura");
    assert!(valid.contains("/weather/Arrecife"));
    assert!(valid.contains("metric=true"));
    assert!(
        configured_api_url(
            action,
            &json!({
                "url": "https://evil.example/steal",
                "credential_ref": "weather_service",
                "auth_mode": "bearer",
                "city": "Arrecife"
            })
        )
        .is_err(),
        "el modelo no puede sustituir el destino fijo"
    );
}

#[test]
fn custom_gpt_context_profiles_have_bounded_distinct_budgets() {
    let context = |profile: &str| CustomGptContext {
        custom_gpt_id: "gpt-context".to_owned(),
        version_id: "gpt-context-v1".to_owned(),
        name: "Contextual".to_owned(),
        icon_ref: "research".to_owned(),
        version_no: 1,
        instructions: "Usa solo el contexto seleccionado.".to_owned(),
        tool_permissions: CustomGptToolPermissions::default(),
        preferred_model: None,
        execution_profile: None,
        context_profile: profile.to_owned(),
        api_actions: Vec::new(),
    };
    let focused_context = context("focused");
    let balanced_context = context("balanced");
    let broad_context = context("broad");
    let focused = custom_gpt_context_budget(Some(&focused_context));
    let balanced = custom_gpt_context_budget(Some(&balanced_context));
    let broad = custom_gpt_context_budget(Some(&broad_context));

    assert!(focused.recent_messages < balanced.recent_messages);
    assert!(balanced.recent_messages < broad.recent_messages);
    assert!(focused.document_characters < balanced.document_characters);
    assert!(balanced.document_characters < broad.document_characters);
    assert_eq!(custom_gpt_context_budget(None), balanced);
}

#[test]
fn authorized_file_replacement_is_atomic_and_rejects_stale_content() {
    let root = std::env::temp_dir().join(format!("chatygpt-edit-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("la carpeta temporal debe existir");
    let file = root.join("notes.txt");
    fs::write(&file, "versión uno").expect("el archivo debe crearse");
    let original_hash = format!("{:x}", Sha256::digest("versión uno".as_bytes()));

    let after_hash =
        replace_bounded_authorized_text(&root, "notes.txt", &original_hash, "versión dos")
            .expect("el reemplazo vigente debe funcionar");
    assert_eq!(fs::read_to_string(&file).unwrap(), "versión dos");
    assert_eq!(
        after_hash,
        format!("{:x}", Sha256::digest("versión dos".as_bytes()))
    );

    let stale = replace_bounded_authorized_text(
        &root,
        "notes.txt",
        &original_hash,
        "contenido que no debe escribirse",
    );
    assert!(matches!(stale, Err(AppError::Conflict(_))));
    assert_eq!(fs::read_to_string(&file).unwrap(), "versión dos");
    let _ = fs::remove_dir_all(root);
}

#[test]
fn the_preview_block_is_literally_the_one_sent_to_the_broker() {
    let custom_gpt = CustomGptContext {
        custom_gpt_id: "gpt-preview".to_owned(),
        version_id: "gpt-version-7".to_owned(),
        name: "Corrector".to_owned(),
        icon_ref: "writing".to_owned(),
        version_no: 7,
        instructions: "Corrige sin cambiar el sentido.".to_owned(),
        tool_permissions: CustomGptToolPermissions {
            run_code: "deny".to_owned(),
            rename_conversation: "confirm".to_owned(),
            read_authorized_folders: "deny".to_owned(),
            modify_authorized_files: "deny".to_owned(),
            create_scheduled_tasks: "deny".to_owned(),
            call_external_apis: "deny".to_owned(),
        },
        preferred_model: Some("qwen2.5:14b".to_owned()),
        execution_profile: None,
        context_profile: "balanced".to_owned(),
        api_actions: Vec::new(),
    };
    let block = custom_gpt_prompt_block(&custom_gpt).expect("el bloque debe construirse");
    let request = chat_request_with_project_instruction(
        "conversation",
        "preview-key",
        "Corrige este párrafo",
        &[ContextMessage {
            message_id: "current".to_owned(),
            role: "user".to_owned(),
            text: "Corrige este párrafo".to_owned(),
        }],
        &[],
        &[],
        &[],
        None,
        Some(&custom_gpt),
        ChatExecutionOptions::default(),
    )
    .expect("la petición debe construirse");
    let prompt = request["content"]["prompt"]
        .as_str()
        .expect("el prompt debe ser texto");

    // La vista previa muestra este bloque; si dejara de aparecer literalmente
    // en la petición, la vista previa estaría mintiendo.
    assert!(
        prompt.contains(&block),
        "el bloque de la vista previa debe aparecer tal cual en la petición"
    );
    assert!(block.contains("Corrige sin cambiar el sentido."));
    assert!(block.contains("\"version\":7"));
    // Los permisos se serializan en camelCase, tal como los recibe el modelo.
    assert!(
        block.contains("\"renameConversation\":\"confirm\""),
        "los permisos vigentes forman parte de lo que ve la persona: {block}"
    );
    // El modelo preferido viaja aparte, en model_requirements, no en el prompt.
    assert!(!block.contains("qwen2.5:14b"));
    assert_eq!(
        request["model_requirements"]["preferred_model"],
        "qwen2.5:14b"
    );
}

#[test]
fn custom_gpt_permission_matrix_gates_rename_tool_without_skipping_confirmation() {
    let mut custom_gpt = CustomGptContext {
        custom_gpt_id: "gpt-tools".to_owned(),
        version_id: "gpt-tools-version".to_owned(),
        name: "Organizador".to_owned(),
        icon_ref: "spark".to_owned(),
        version_no: 1,
        instructions: "Ayuda a organizar el chat.".to_owned(),
        tool_permissions: CustomGptToolPermissions::default(),
        preferred_model: None,
        execution_profile: None,
        context_profile: "balanced".to_owned(),
        api_actions: Vec::new(),
    };
    let context = [ContextMessage {
        message_id: "current".to_owned(),
        role: "user".to_owned(),
        text: "Renombra el chat como Plan semanal".to_owned(),
    }];
    let denied = chat_request_with_project_instruction(
        "conversation",
        "denied-key",
        "Renombra el chat como Plan semanal",
        &context,
        &[],
        &[],
        &[],
        None,
        Some(&custom_gpt),
        ChatExecutionOptions {
            tools_enabled: true,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("denied request should still build without the tool");
    assert_eq!(denied["execution"]["strategy"], "single");

    custom_gpt.tool_permissions.rename_conversation = "confirm".to_owned();
    let confirmable = chat_request_with_project_instruction(
        "conversation",
        "confirm-key",
        "Renombra el chat como Plan semanal",
        &context,
        &[],
        &[],
        &[],
        None,
        Some(&custom_gpt),
        ChatExecutionOptions {
            tools_enabled: true,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("confirmable request should expose the client tool");
    assert_eq!(confirmable["execution"]["strategy"], "agent");
    assert_eq!(
        confirmable["execution"]["agent"]["client_tools"][0]["name"],
        "rename_conversation"
    );
}

#[test]
fn frozen_custom_gpt_permission_is_rechecked_before_tool_execution() {
    let denied = json!({
        "content": {
            "metadata": {
                "custom_gpt_id": "gpt",
                "custom_gpt_tool_permissions": {
                    "runCode": "deny",
                    "renameConversation": "deny"
                }
            }
        }
    });
    assert!(!persisted_custom_gpt_allows_tool(
        &denied,
        "rename_conversation"
    ));
    let confirmable = json!({
        "content": {
            "metadata": {
                "custom_gpt_id": "gpt",
                "custom_gpt_tool_permissions": {
                    "runCode": "confirm",
                    "renameConversation": "confirm"
                }
            }
        }
    });
    assert!(persisted_custom_gpt_allows_tool(
        &confirmable,
        "rename_conversation"
    ));
    assert!(persisted_custom_gpt_allows_tool(
        &json!({"content": {"metadata": {}}}),
        "rename_conversation"
    ));
}

#[test]
fn project_instructions_are_explicit_reusable_context_in_the_broker_prompt() {
    let instruction = ProjectInstructionContext {
        project_id: "project-research".to_owned(),
        project_name: "Investigación".to_owned(),
        instructions: "Distingue hechos de hipótesis y cita las fuentes.".to_owned(),
    };
    let request = chat_request_with_project_instruction(
        "conversation",
        "project-instruction-key",
        "Analiza el resultado",
        &[ContextMessage {
            message_id: "current".to_owned(),
            role: "user".to_owned(),
            text: "Analiza el resultado".to_owned(),
        }],
        &[],
        &[],
        &[],
        Some(&instruction),
        None,
        ChatExecutionOptions::default(),
    )
    .expect("request with project instructions should build");

    let prompt = request["content"]["prompt"]
        .as_str()
        .expect("prompt should be text");
    assert!(prompt.contains("<project_instructions_json>"));
    assert!(prompt.contains("Distingue hechos de hipótesis"));
    assert_eq!(
        request["content"]["metadata"]["project_instruction_configured"],
        true
    );
}

#[test]
fn jitter_is_bounded_and_stable() {
    let first = deterministic_jitter("task", 1);
    assert_eq!(first, deterministic_jitter("task", 1));
    assert!((-1_500..=1_500).contains(&first));
}

#[test]
fn tools_mode_uses_agent_passthrough_only_when_enabled() {
    let context = vec![ContextMessage {
        message_id: "message-1".to_owned(),
        role: "user".to_owned(),
        text: "Renombra el chat".to_owned(),
    }];
    let agent = chat_request(
        "conversation",
        "key-agent",
        "Renombra el chat",
        &context,
        &[],
        &[],
        &[],
        ChatExecutionOptions {
            tools_enabled: true,
            sandbox_enabled: false,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("agent request should build");
    assert_eq!(agent["execution"]["strategy"], "agent");
    assert_eq!(
        agent["execution"]["agent"]["client_tools"][0]["name"],
        "rename_conversation"
    );

    let single = chat_request(
        "conversation",
        "key-single",
        "Hola",
        &context,
        &[],
        &[],
        &[],
        ChatExecutionOptions {
            tools_enabled: false,
            sandbox_enabled: false,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("single request should build");
    assert_eq!(single["execution"]["strategy"], "single");
    assert!(single["execution"].get("agent").is_none());
}

#[test]
fn tools_mode_does_not_offer_rename_for_an_unrelated_request() {
    let context = vec![ContextMessage {
        message_id: "message-weights".to_owned(),
        role: "user".to_owned(),
        text: "Dime lo que sepas sobre los pesos de los LLM".to_owned(),
    }];
    let request = chat_request(
        "conversation",
        "key-unrelated-tool",
        "Dime lo que sepas sobre los pesos de los LLM",
        &context,
        &[],
        &[],
        &[],
        ChatExecutionOptions {
            tools_enabled: true,
            sandbox_enabled: false,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("request should build");

    assert_eq!(request["execution"]["strategy"], "single");
    assert!(request["execution"].get("agent").is_none());
}

#[test]
fn sandbox_is_explicit_and_requires_broker_capability() {
    let context = vec![ContextMessage {
        message_id: "message-code".to_owned(),
        role: "user".to_owned(),
        text: "Calcula con Python".to_owned(),
    }];
    let request = chat_request(
        "conversation",
        "key-code",
        "Calcula",
        &context,
        &[],
        &[],
        &[],
        ChatExecutionOptions {
            tools_enabled: false,
            sandbox_enabled: true,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("sandbox request should build");
    assert_eq!(request["execution"]["strategy"], "agent");
    assert_eq!(request["execution"]["agent"]["skills"][0], "run_code");
    assert_eq!(
        request["execution"]["agent"]["client_tools"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );

    let unavailable = BrokerCapabilities {
        contract_version: "2.6".to_owned(),
        derived_data_boundary: true,
        work_lanes: vec!["inference".to_owned(), "ingestion".to_owned()],
        strategies: vec!["agent".to_owned()],
        presets: serde_json::Value::Null,
        scheduling_by_preset: serde_json::Value::Null,
        agent_skills: Vec::new(),
        agent_skills_egress: Vec::new(),
        task_dependencies: false,
        sandbox_run_code: false,
        file_ingestion: true,
        ingestion_formats: std::collections::HashMap::new(),
        long_context_map_reduce: true,
        max_active_workflows: Some(1),
        client_tool_passthrough: Some(true),
        // Un Broker 2.6 no publica nada de 2.9.
        exclude_from_model_learning: false,
        invocation_telemetry: false,
        execution_fingerprint: false,
    };
    assert!(validate_sandbox_capability(&unavailable).is_err());
    let available = BrokerCapabilities {
        sandbox_run_code: true,
        agent_skills: vec!["run_code".to_owned()],
        ..unavailable
    };
    assert!(validate_sandbox_capability(&available).is_ok());
}

#[test]
fn deep_research_is_an_explicit_multi_source_agent_workflow() {
    let request = chat_request(
        "conversation",
        "research-key",
        "Compara la regulación europea y estadounidense de IA",
        &[ContextMessage {
            message_id: "current".to_owned(),
            role: "user".to_owned(),
            text: "Compara la regulación europea y estadounidense de IA".to_owned(),
        }],
        &[],
        &[],
        &[],
        ChatExecutionOptions::default(),
    )
    .expect("base request should build");
    let capabilities = BrokerCapabilities {
        contract_version: "2.7".to_owned(),
        strategies: vec!["single".to_owned(), "agent".to_owned()],
        agent_skills: vec![
            "web_search".to_owned(),
            "fetch_url".to_owned(),
            "calculator".to_owned(),
            "current_datetime".to_owned(),
        ],
        client_tool_passthrough: Some(true),
        ..BrokerCapabilities::default()
    };
    let plan = deep_research_plan(&capabilities).expect("research plan should be decided");
    let research =
        apply_deep_research_plan(request, &plan).expect("research workflow should build");
    assert_eq!(
        research["content"]["metadata"]["workflow_kind"],
        "deep_research"
    );
    assert_eq!(research["execution"]["strategy"], "agent");
    assert_eq!(
        research["execution"]["preset"], "fast",
        "Broker contract: agent strategy only supports preset fast"
    );
    assert_eq!(research["execution"]["agent"]["max_iterations"], 12);
    // Diseño híbrido: buscar lo hace el Broker, abrir enlaces lo hace
    // ChatyGPT para que cada fuente sea una subtarea visible.
    assert_eq!(
        research["execution"]["agent"]["skills"],
        json!(["web_search", "calculator", "current_datetime"])
    );
    let client_tools = research["execution"]["agent"]["client_tools"]
        .as_array()
        .expect("las herramientas de cliente deben ser una lista");
    assert_eq!(client_tools.len(), 1);
    assert_eq!(client_tools[0]["name"], "fetch_url");
    assert_eq!(client_tools[0]["parameters"]["required"], json!(["url"]));
    // Ningún nombre puede estar en las dos listas a la vez.
    assert!(!research["execution"]["agent"]["skills"]
        .as_array()
        .expect("las habilidades deben ser una lista")
        .iter()
        .any(|skill| skill == "fetch_url"));
    let prompt = research["content"]["prompt"]
        .as_str()
        .expect("research prompt should be text");
    assert!(prompt.contains("No la trates como una sola búsqueda"));
    assert!(prompt.contains("contrasta"));
    // La estrategia `agent` rechaza el formato JSON con 422, y el campo del
    // contrato es `output.format`: sanearlo en `generation` no haría nada.
    assert_eq!(research["output"]["format"], "markdown");
    assert!(research["generation"].get("output_format").is_none());

    // Sin `web_search` la investigación se quedaría en abrir enlaces que el
    // modelo recuerde, que es justo lo que el prompt prohíbe.
    let missing_search = BrokerCapabilities {
        agent_skills: vec!["calculator".to_owned()],
        ..capabilities
    };
    assert!(deep_research_plan(&missing_search).is_err());
    // Sin passthrough no hay subtareas visibles: el Broker no podría
    // pausar la tarea para pedir `fetch_url`.
    let no_passthrough = BrokerCapabilities {
        client_tool_passthrough: Some(false),
        ..missing_search.clone()
    };
    assert!(deep_research_plan(&no_passthrough).is_err());
    let missing_agent = BrokerCapabilities {
        strategies: vec!["single".to_owned()],
        ..missing_search
    };
    assert!(deep_research_plan(&missing_agent).is_err());
}

#[test]
fn a_research_turn_never_asks_the_agent_for_json() {
    let capabilities = BrokerCapabilities {
        strategies: vec!["agent".to_owned()],
        agent_skills: vec!["web_search".to_owned()],
        client_tool_passthrough: Some(true),
        ..BrokerCapabilities::default()
    };
    let plan = deep_research_plan(&capabilities).expect("plan should be decided");
    let mut request = chat_request(
        "conversation",
        "json-key",
        "Investiga esto",
        &[ContextMessage {
            message_id: "current".to_owned(),
            role: "user".to_owned(),
            text: "Investiga esto".to_owned(),
        }],
        &[],
        &[],
        &[],
        ChatExecutionOptions::default(),
    )
    .expect("base request should build");
    // Aunque el turno base pidiera JSON, la investigación sale en Markdown.
    request["output"]["format"] = json!("json");
    let research = apply_deep_research_plan(request, &plan).expect("should build");
    assert_eq!(research["output"]["format"], "markdown");
}

#[test]
fn contract_2_8_blocks_research_egress_for_local_data_before_persisting() {
    let capabilities = BrokerCapabilities {
        contract_version: "2.8".to_owned(),
        strategies: vec!["agent".to_owned()],
        agent_skills: vec!["web_search".to_owned()],
        agent_skills_egress: vec!["web_search".to_owned(), "fetch_url".to_owned()],
        client_tool_passthrough: Some(true),
        ..BrokerCapabilities::default()
    };
    let plan = deep_research_plan(&capabilities).expect("plan should be decided");
    let request = chat_request(
        "conversation",
        "local-research-key",
        "Investiga sin sacar datos del equipo",
        &[ContextMessage {
            message_id: "current".to_owned(),
            role: "user".to_owned(),
            text: "Investiga sin sacar datos del equipo".to_owned(),
        }],
        &[],
        &[],
        &[],
        ChatExecutionOptions {
            execution_preferences: ConversationExecutionPreferences {
                data_classification: "local_only".to_owned(),
                ..ConversationExecutionPreferences::default()
            },
            ..ChatExecutionOptions::default()
        },
    )
    .expect("base request should build");
    let error = apply_deep_research_plan(request, &plan)
        .expect_err("egress must be rejected before the Broker returns 422");
    assert!(error.to_string().contains("web_search"));
    assert!(error.to_string().contains("Solo en este equipo"));
}

/// El plan viaja con el flujo semántico y no vuelve a negociarse.
///
/// Entre decidirlo y aplicarlo media una tarea de embeddings y, quizá, un
/// reinicio: si el Broker retira una herramienta mientras tanto, la
/// investigación ya autorizada debe ejecutarse tal y como se aprobó.
#[test]
fn research_plan_is_frozen_and_survives_the_semantic_round_trip() {
    let capabilities = BrokerCapabilities {
        contract_version: "2.7".to_owned(),
        strategies: vec!["single".to_owned(), "agent".to_owned()],
        agent_skills: vec![
            "web_search".to_owned(),
            "fetch_url".to_owned(),
            "calculator".to_owned(),
        ],
        client_tool_passthrough: Some(true),
        ..BrokerCapabilities::default()
    };
    let plan = deep_research_plan(&capabilities).expect("research plan should be decided");
    assert_eq!(plan.skills, ["web_search", "calculator"]);
    // Solo se incluyen las habilidades realmente anunciadas.
    assert!(!plan.skills.iter().any(|skill| skill == "current_datetime"));
    // `fetch_url` no es una habilidad del Broker sino una herramienta
    // nuestra: viaja en la otra lista aunque el Broker la anuncie.
    assert!(!plan.skills.iter().any(|skill| skill == "fetch_url"));
    assert_eq!(plan.client_tools, ["fetch_url"]);
    // El tope del contrato acota la profundidad total de la investigación.
    assert!(plan.max_iterations <= 20);

    // Ida y vuelta por SQLite: se persiste como JSON y se recupera igual.
    let persisted = serde_json::to_value(&plan).expect("plan should serialize");
    let restored: ResearchPlan =
        serde_json::from_value(persisted).expect("plan should deserialize");
    assert_eq!(restored, plan);

    // La segunda etapa aplica el plan sin consultar capacidades y conserva
    // el contexto ya recuperado por similitud.
    let request = chat_request(
        "conversation",
        "semantic-research-key",
        "Contrasta lo que dice el informe adjunto con fuentes públicas",
        &[ContextMessage {
            message_id: "current".to_owned(),
            role: "user".to_owned(),
            text: "Contrasta lo que dice el informe adjunto con fuentes públicas".to_owned(),
        }],
        &[],
        &[],
        &[],
        ChatExecutionOptions::default(),
    )
    .expect("base request should build");
    let research = apply_deep_research_plan(request, &restored)
        .expect("research workflow should build from the frozen plan");
    assert_eq!(
        research["content"]["metadata"]["workflow_kind"],
        "deep_research"
    );
    assert_eq!(
        research["execution"]["agent"]["skills"],
        json!(["web_search", "calculator"])
    );
    assert_eq!(
        research["execution"]["agent"]["client_tools"][0]["name"],
        "fetch_url"
    );
    assert!(research["content"]["prompt"]
        .as_str()
        .expect("research prompt should be text")
        .contains("Contrasta lo que dice el informe adjunto"));
}

#[test]
fn contract_2_7_uses_priority_and_gives_run_code_to_collaborative_proposers() {
    let request = chat_request(
        "conversation",
        "key-collaborative-code",
        "Analiza los datos y comprueba el resultado",
        &[ContextMessage {
            message_id: "message-code".to_owned(),
            role: "user".to_owned(),
            text: "Analiza los datos y comprueba el resultado".to_owned(),
        }],
        &[],
        &[],
        &[],
        ChatExecutionOptions {
            sandbox_enabled: true,
            execution_preferences: ConversationExecutionPreferences {
                strategy: "mixture_of_agents".to_owned(),
                priority: 25,
                ..ConversationExecutionPreferences::default()
            },
            ..ChatExecutionOptions::default()
        },
    )
    .expect("collaborative sandbox request should build");

    assert_eq!(request["priority"], 25);
    assert_eq!(request["execution"]["strategy"], "mixture_of_agents");
    assert_eq!(request["execution"]["proposer_skills"][0], "run_code");
    assert!(request["execution"].get("agent").is_none());
}

#[test]
fn contract_2_7_keeps_tabular_files_as_broker_attachments() {
    let attachment = AttachmentRecord {
        id: "table".to_owned(),
        local_path: "prices.csv".to_owned(),
        display_name: "prices.csv".to_owned(),
        media_type: Some("text/csv".to_owned()),
        size_bytes: 128,
        sha256: "hash".to_owned(),
        broker_file_id: Some("file-table".to_owned()),
        ingestion_status: "ready".to_owned(),
        describe_images: None,
    };
    assert!(is_tabular_attachment(&attachment));
    let request = chat_request(
        "conversation",
        "key-table",
        "Calcula la media",
        &[ContextMessage {
            message_id: "message-table".to_owned(),
            role: "user".to_owned(),
            text: "Calcula la media".to_owned(),
        }],
        std::slice::from_ref(&attachment),
        &[SelectedAttachmentChunk {
            id: "chunk-table".to_owned(),
            attachment_id: attachment.id.clone(),
            attachment_name: attachment.display_name.clone(),
            ordinal: 0,
            text: "price\n10\n20".to_owned(),
            score: 1.0,
            reason: "Coincidencia con la pregunta".to_owned(),
        }],
        &[],
        ChatExecutionOptions {
            sandbox_enabled: true,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("tabular request should build");

    assert_eq!(
        request["content"]["attachments"][0]["metadata"]["file_id"],
        "file-table"
    );
    assert_eq!(request["execution"]["agent"]["skills"][0], "run_code");
}

#[test]
fn approved_memory_is_visible_in_request_and_absent_without_items() {
    let context = vec![ContextMessage {
        message_id: "message-memory".to_owned(),
        role: "user".to_owned(),
        text: "¿Cómo prefiero las respuestas?".to_owned(),
    }];
    let memory = MemoryItemView {
        id: "memory-visible".to_owned(),
        project_id: None,
        project_name: None,
        custom_gpt_id: None,
        custom_gpt_name: None,
        category: "preference".to_owned(),
        content: "Prefiero respuestas breves".to_owned(),
        sensitivity: "normal".to_owned(),
        enabled: true,
        embedding_status: "ready".to_owned(),
        embedding_model: Some("ollama/local/embed".to_owned()),
        embedding_error: None,
        created_at: "2026-07-22 00:00:00".to_owned(),
        updated_at: "2026-07-22 00:00:00".to_owned(),
    };
    let with_memory = chat_request(
        "conversation",
        "key-memory",
        "Responde",
        &context,
        &[],
        &[],
        &[memory],
        ChatExecutionOptions {
            tools_enabled: false,
            sandbox_enabled: false,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("request with memory should build");
    let prompt = with_memory["content"]["prompt"]
        .as_str()
        .expect("prompt should be text");
    assert!(prompt.contains("Prefiero respuestas breves"));
    assert_eq!(
        with_memory["content"]["metadata"]["approved_memory_count"],
        1
    );

    let without_memory = chat_request(
        "conversation",
        "key-no-memory",
        "Responde",
        &context,
        &[],
        &[],
        &[],
        ChatExecutionOptions {
            tools_enabled: false,
            sandbox_enabled: false,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("request without memory should build");
    assert!(!without_memory["content"]["prompt"]
        .as_str()
        .expect("prompt should be text")
        .contains("user_approved_memory_json"));
}

#[test]
fn selected_document_fragments_replace_the_full_broker_attachment() {
    let context = vec![ContextMessage {
        message_id: "message-document".to_owned(),
        role: "user".to_owned(),
        text: "Calcula la mediana del cierre".to_owned(),
    }];
    let attachment = AttachmentRecord {
        id: "attachment-prices".to_owned(),
        local_path: "managed/report.pdf".to_owned(),
        display_name: "report.pdf".to_owned(),
        media_type: Some("application/pdf".to_owned()),
        size_bytes: 9_000_000,
        sha256: "prices-hash".to_owned(),
        broker_file_id: Some("broker-prices".to_owned()),
        ingestion_status: "ready".to_owned(),
        describe_images: None,
    };
    let chunk = SelectedAttachmentChunk {
        id: "chunk-prices-1".to_owned(),
        attachment_id: attachment.id.clone(),
        attachment_name: attachment.display_name.clone(),
        ordinal: 1,
        text: "OHLC: el cierre medio es 102,4".to_owned(),
        score: 0.8,
        reason: "Coincidencia con la pregunta".to_owned(),
    };
    let request = chat_request(
        "conversation",
        "key-document",
        "Calcula la mediana del cierre",
        &context,
        &[attachment],
        &[chunk],
        &[],
        ChatExecutionOptions {
            tools_enabled: false,
            sandbox_enabled: false,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("request with selected document fragment should build");

    assert!(request["content"]["attachments"]
        .as_array()
        .expect("attachments should be an array")
        .is_empty());
    assert!(request["content"]["prompt"]
        .as_str()
        .expect("prompt should be text")
        .contains("OHLC: el cierre medio es 102,4"));
    assert_eq!(
        request["content"]["metadata"]["selected_document_fragment_count"],
        1
    );
}

#[test]
fn global_document_view_is_explicit_and_cannot_be_denied_by_the_prompt() {
    let attachment = AttachmentRecord {
        id: "attachment-book".to_owned(),
        local_path: "managed/book.pdf".to_owned(),
        display_name: "book.pdf".to_owned(),
        media_type: Some("application/pdf".to_owned()),
        size_bytes: 42_000,
        sha256: "book-hash".to_owned(),
        broker_file_id: Some("broker-book".to_owned()),
        ingestion_status: "ready".to_owned(),
        describe_images: None,
    };
    let chunk = SelectedAttachmentChunk {
        id: "chunk-preface".to_owned(),
        attachment_id: attachment.id.clone(),
        attachment_name: attachment.display_name.clone(),
        ordinal: 2,
        text: "Preface. This book explains pattern recognition.".to_owned(),
        score: 0.96,
        reason: "Vista global del documento · prefacio".to_owned(),
    };
    let context = vec![ContextMessage {
        message_id: "message-book".to_owned(),
        role: "user".to_owned(),
        text: "Dime de qué va el libro".to_owned(),
    }];
    let request = chat_request(
        "conversation",
        "key-global-document",
        "Dime de qué va el libro",
        &context,
        &[attachment],
        &[chunk],
        &[],
        ChatExecutionOptions::default(),
    )
    .expect("global document request should build");

    let prompt = request["content"]["prompt"]
        .as_str()
        .expect("prompt should be text");
    assert!(prompt.contains("deliberate global document view"));
    assert!(prompt.contains("Do not claim that the document or its content was not provided"));
    assert_eq!(
        request["content"]["metadata"]["document_context_mode"],
        "global_document_view"
    );
}

#[test]
fn current_attachment_scope_overrides_removed_books_mentioned_in_history() {
    let context = vec![
        ContextMessage {
            message_id: "message-old-book".to_owned(),
            role: "assistant".to_owned(),
            text: "El libro de Mark Minervini tiene varios temas.".to_owned(),
        },
        ContextMessage {
            message_id: "message-current".to_owned(),
            role: "user".to_owned(),
            text: "¿Cuántos temas tiene?".to_owned(),
        },
    ];
    let current_attachment = AttachmentRecord {
        id: "attachment-math".to_owned(),
        local_path: "managed/math-deep.pdf".to_owned(),
        display_name: "math-deep.pdf".to_owned(),
        media_type: Some("application/pdf".to_owned()),
        size_bytes: 1_000_000,
        sha256: "math-hash".to_owned(),
        broker_file_id: Some("broker-math".to_owned()),
        ingestion_status: "ready".to_owned(),
        describe_images: None,
    };
    let request = chat_request(
        "conversation",
        "key-current-attachment-scope",
        "¿Cuántos temas tiene?",
        &context,
        &[current_attachment],
        &[],
        &[],
        ChatExecutionOptions::default(),
    )
    .expect("request with one current attachment should build");
    let prompt = request["content"]["prompt"]
        .as_str()
        .expect("prompt should be text");

    assert!(prompt.contains(
        "<active_attachment_scope_json>[\"math-deep.pdf\"]</active_attachment_scope_json>"
    ));
    assert!(prompt.contains("removed files"));
}

#[test]
fn chat_routing_delegates_provider_selection_for_internal_context() {
    let context = vec![ContextMessage {
        message_id: "message-routing".to_owned(),
        role: "user".to_owned(),
        text: "Responde usando un modelo local".to_owned(),
    }];
    let request = chat_request(
        "conversation",
        "key-routing",
        "Responde",
        &context,
        &[],
        &[],
        &[],
        ChatExecutionOptions {
            tools_enabled: false,
            sandbox_enabled: false,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("chat request should build");
    assert!(request["model_requirements"]
        .get("allowed_providers")
        .is_none());
    assert!(request["model_requirements"].get("cloud_allowed").is_none());
    assert_eq!(request["model_requirements"]["max_cost_usd"], 0.1);
    assert_eq!(request["risk"]["data_classification"], "internal");
}

#[test]
fn conversation_preferences_enable_auto_routing_budget_and_long_documents() {
    let context = vec![ContextMessage {
        message_id: "message-options".to_owned(),
        role: "user".to_owned(),
        text: "Analiza el informe completo".to_owned(),
    }];
    let attachment = AttachmentRecord {
        id: "attachment-report".to_owned(),
        local_path: "managed/report.pdf".to_owned(),
        display_name: "report.pdf".to_owned(),
        media_type: Some("application/pdf".to_owned()),
        size_bytes: 12_000_000,
        sha256: "report-hash".to_owned(),
        broker_file_id: Some("broker-report".to_owned()),
        ingestion_status: "ready".to_owned(),
        describe_images: None,
    };
    let request = chat_request(
        "conversation",
        "key-options",
        "Analiza el informe completo",
        &context,
        &[attachment],
        &[],
        &[],
        ChatExecutionOptions {
            execution_preferences: ConversationExecutionPreferences {
                data_classification: "public".to_owned(),
                strategy: "auto".to_owned(),
                preset: "fast".to_owned(),
                max_cost_usd: 0.5,
                long_context: "map_reduce".to_owned(),
                priority: 100,
            },
            ..ChatExecutionOptions::default()
        },
    )
    .expect("2.6 execution options should build");

    assert_eq!(request["execution"]["strategy"], "auto");
    assert_eq!(request["execution"]["long_context"], "map_reduce");
    assert!(request["execution"].get("preset").is_none());
    assert_eq!(request["risk"]["data_classification"], "public");
    assert_eq!(request["model_requirements"]["max_cost_usd"], 0.5);
}

#[test]
fn contract_2_8_adds_the_document_group_only_after_the_batch_is_ready() {
    let request = json!({
        "idempotency_key": "question-key",
        "content": {"prompt": "Pregunta"}
    });
    let without_dependency = apply_document_index_dependency(request.clone(), None);
    assert!(without_dependency.get("depends_on_group").is_none());

    let group = super::DocumentIndexDependency::Group("chatygpt-index-documento".to_owned());
    let dependent = apply_document_index_dependency(request.clone(), Some(&group));
    assert_eq!(dependent["depends_on_group"], "chatygpt-index-documento");

    let tasks =
        super::DocumentIndexDependency::Tasks(vec!["task-a".to_owned(), "task-b".to_owned()]);
    let dependent = apply_document_index_dependency(request, Some(&tasks));
    assert_eq!(dependent["depends_on"], json!(["task-a", "task-b"]));
}

#[test]
fn collaborative_analysis_uses_the_selected_depth_without_invalid_map_reduce() {
    let context = vec![ContextMessage {
        message_id: "message-collaboration".to_owned(),
        role: "user".to_owned(),
        text: "Contrasta las alternativas".to_owned(),
    }];
    let request = chat_request(
        "conversation",
        "key-collaboration",
        "Contrasta las alternativas",
        &context,
        &[],
        &[],
        &[],
        ChatExecutionOptions {
            execution_preferences: ConversationExecutionPreferences {
                strategy: "mixture_of_agents".to_owned(),
                preset: "slow".to_owned(),
                long_context: "map_reduce".to_owned(),
                ..ConversationExecutionPreferences::default()
            },
            ..ChatExecutionOptions::default()
        },
    )
    .expect("collaborative request should build");

    assert_eq!(request["execution"]["strategy"], "mixture_of_agents");
    assert_eq!(request["execution"]["preset"], "slow");
    assert_eq!(request["execution"]["selection"]["proposer_count"], 3);
    assert_eq!(request["execution"]["long_context"], "fail");
}

#[test]
fn chat_routing_keeps_sensitive_memory_local_only() {
    let context = vec![ContextMessage {
        message_id: "message-sensitive-routing".to_owned(),
        role: "user".to_owned(),
        text: "Usa el contexto sensible".to_owned(),
    }];
    let memories = vec![MemoryItemView {
        id: "memory-sensitive".to_owned(),
        project_id: None,
        project_name: None,
        custom_gpt_id: None,
        custom_gpt_name: None,
        category: "personal".to_owned(),
        content: "Dato privado".to_owned(),
        sensitivity: "sensitive".to_owned(),
        enabled: true,
        embedding_status: "ready".to_owned(),
        embedding_model: Some("ollama/local/embed".to_owned()),
        embedding_error: None,
        created_at: "2026-07-22 00:00:00".to_owned(),
        updated_at: "2026-07-22 00:00:00".to_owned(),
    }];
    let request = chat_request(
        "conversation",
        "key-sensitive-routing",
        "Responde",
        &context,
        &[],
        &[],
        &memories,
        ChatExecutionOptions {
            tools_enabled: false,
            sandbox_enabled: false,
            ..ChatExecutionOptions::default()
        },
    )
    .expect("sensitive chat request should build");

    assert!(request["model_requirements"]
        .get("allowed_providers")
        .is_none());
    assert!(request["model_requirements"].get("cloud_allowed").is_none());
    assert_eq!(request["risk"]["data_classification"], "local_only");
}

#[test]
fn memory_embedding_request_is_local_only_and_traceable() {
    let request = memory_embedding_request(
        "embedding-key",
        "memory-1",
        "Texto para indexar",
        "content-hash",
    );
    assert_eq!(request["inference_kind"], "embedding");
    assert_eq!(request["execution"]["strategy"], "single");
    assert!(request["model_requirements"].get("cloud_allowed").is_none());
    assert!(request["model_requirements"]
        .get("selection_mode")
        .is_none());
    assert!(request["model_requirements"]
        .get("allowed_providers")
        .is_none());
    assert_eq!(request["content"]["metadata"]["source_id"], "memory-1");
    assert_eq!(
        request["content"]["metadata"]["content_sha256"],
        "content-hash"
    );
}

#[test]
fn document_chunk_embedding_request_is_local_only_and_traceable() {
    let request = embedding_request(
        "chunk-key",
        "attachment_chunk",
        "chunk-attachment-3",
        "Texto del fragmento",
        "chunk-content-hash",
    );

    assert_eq!(request["inference_kind"], "embedding");
    assert_eq!(
        request["content"]["metadata"]["source_type"],
        "attachment_chunk"
    );
    assert_eq!(
        request["content"]["metadata"]["source_id"],
        "chunk-attachment-3"
    );
    assert_eq!(
        request["content"]["metadata"]["content_sha256"],
        "chunk-content-hash"
    );
    assert_eq!(request["model_requirements"]["max_cost_usd"], 0);
    assert_eq!(request["risk"]["data_classification"], "local_only");
}
