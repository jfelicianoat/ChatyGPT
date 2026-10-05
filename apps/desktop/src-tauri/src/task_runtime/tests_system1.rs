use super::*;
use crate::broker::simulated::{ScriptedResponse, SimulatedBroker};
use crate::db::ContextMessage;

fn completed_embedding(id: &str) -> crate::broker::TaskState {
    serde_json::from_value(json!({
        "task_id":id,"status":"completed","created_at":"2026-10-03T00:00:00Z",
        "updated_at":"2026-10-03T00:00:01Z","progress":{},
        "result":{"inference_kind":"embedding","embedding":[1.0,0.0],
            "model_used":{"provider":"ollama","deployment":"local","model":"nomic"}},"error":null
    }))
    .unwrap()
}

fn completed_server(server: &SimulatedBroker, remote_id: &str) {
    server.always(
        "GET /api/v1/capabilities",
        ScriptedResponse::ok(json!({"system1_judgments":true})),
    );
    server.always(
        "POST /api/v1/system1/judge",
        ScriptedResponse::ok(score(0, 0.99)),
    );
    server.always(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(crate::broker::simulated::accepted_task(remote_id)),
    );
    server.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(crate::broker::simulated::task_state(
            remote_id,
            "completed",
            Some(crate::broker::simulated::completed_chat_result(
                "Continuidad preservada: Athena.",
            )),
        )),
    );
}

#[test]
fn system1_normal_turn_filters_before_persisting_and_materializes_response() {
    let server = SimulatedBroker::start();
    completed_server(&server, "normal-system1");
    let mut broker = BrokerClient::for_base_url(server.base_url()).unwrap();
    broker.context_filter = crate::system1::ContextFilterConfig {
        enabled: true,
        shadow_mode: false,
        max_context_tokens: 100,
        ..Default::default()
    };
    let path =
        std::env::temp_dir().join(format!("chatygpt-system1-normal-{}.sqlite", Uuid::new_v4()));
    let database = Database::open(path.clone()).unwrap();
    let project = database.create_project("TFM", None).unwrap();
    database
        .update_project_instructions(
            &project.id,
            Some("Preserva las instrucciones obligatorias."),
        )
        .unwrap();
    let conversation = database
        .create_conversation("System1", Some(&project.id))
        .unwrap();
    database.set_memory_enabled(true).unwrap();
    database
        .create_memory_item(&"Datos de trading. ".repeat(40), "fact", "normal", None)
        .unwrap();
    database
        .create_memory_item("Sin publicación automática.", "instruction", "normal", None)
        .unwrap();
    let snapshot = tauri::async_runtime::block_on(start_chat_turn(
        database.clone(),
        broker,
        &conversation.id,
        "Configura Athena",
        &[],
        false,
        false,
        false,
        false,
    ))
    .unwrap();
    assert!(SimulatedBroker::wait_until(Duration::from_secs(10), || {
        database
            .task_snapshot(&snapshot.id)
            .is_ok_and(|task| task.remote_status == "completed")
    }));
    let saved = database.task_context(&snapshot.id).unwrap();
    assert_eq!(saved.system1.unwrap()["excluded"], 1);
    assert_eq!(
        saved
            .sources
            .iter()
            .filter(|source| source.kind == "memory")
            .count(),
        1
    );
    assert!(saved
        .sources
        .iter()
        .any(|source| source.kind == "project_instruction"));
    let requests = server.requests_to("POST", "/api/v1/tasks");
    assert_eq!(requests.len(), 1);
    let prompt = requests[0].body["content"]["prompt"].as_str().unwrap();
    assert!(!prompt.contains("Datos de trading"));
    assert!(prompt.contains("Sin publicación automática"));
    assert!(prompt.contains("Configura Athena"));
    assert_eq!(
        database
            .conversation_view(&conversation.id)
            .unwrap()
            .messages[1]
            .text
            .as_deref(),
        Some("Continuidad preservada: Athena.")
    );
    drop(database);
    let _ = std::fs::remove_file(path);
}

#[test]
fn system1_shadow_runs_after_the_turn_and_never_changes_what_is_sent() {
    let server = SimulatedBroker::start();
    completed_server(&server, "shadow-system1");
    let mut broker = BrokerClient::for_base_url(server.base_url()).unwrap();
    broker.context_filter = crate::system1::ContextFilterConfig {
        enabled: true,
        shadow_mode: true,
        max_context_tokens: 100,
        ..Default::default()
    };
    let path =
        std::env::temp_dir().join(format!("chatygpt-system1-shadow-{}.sqlite", Uuid::new_v4()));
    let database = Database::open(path.clone()).unwrap();
    let conversation = database.create_conversation("Sombra", None).unwrap();
    database.set_memory_enabled(true).unwrap();
    database
        .create_memory_item(&"Datos de trading. ".repeat(40), "fact", "normal", None)
        .unwrap();
    let snapshot = tauri::async_runtime::block_on(start_chat_turn(
        database.clone(),
        broker,
        &conversation.id,
        "Configura Athena",
        &[],
        false,
        false,
        false,
        false,
    ))
    .unwrap();
    // La petición persistida es la del recorrido normal, sin traza dentro.
    let persisted = database.task_record(&snapshot.id).unwrap();
    assert!(persisted.request["content"]["metadata"]
        .get("system1_context")
        .is_none());
    assert!(persisted.request["content"]["prompt"]
        .as_str()
        .unwrap()
        .contains("Datos de trading"));
    // La propuesta llega después y queda aparte, visible en el panel.
    assert!(SimulatedBroker::wait_until(Duration::from_secs(10), || {
        database
            .task_context(&snapshot.id)
            .is_ok_and(|context| context.system1.is_some())
    }));
    let trace = database
        .task_context(&snapshot.id)
        .unwrap()
        .system1
        .unwrap();
    assert_eq!(trace["mode"], "shadow");
    assert_eq!(trace["proposedExcluded"], 1);
    assert_eq!(trace["excluded"], 0);
    assert!(SimulatedBroker::wait_until(Duration::from_secs(10), || {
        database
            .task_snapshot(&snapshot.id)
            .is_ok_and(|task| task.remote_status == "completed")
    }));
    let sent = server.requests_to("POST", "/api/v1/tasks");
    assert!(sent[0].body["content"]["prompt"]
        .as_str()
        .unwrap()
        .contains("Datos de trading"));
    assert_eq!(
        database.task_record(&snapshot.id).unwrap().request,
        persisted.request
    );
    drop(database);
    let _ = std::fs::remove_file(path);
}

#[test]
fn system1_semantic_recovery_filters_only_after_retrieval_and_snapshot_matches() {
    let server = SimulatedBroker::start();
    completed_server(&server, "semantic-system1");
    let mut broker = BrokerClient::for_base_url(server.base_url()).unwrap();
    broker.context_filter = crate::system1::ContextFilterConfig {
        enabled: true,
        shadow_mode: false,
        max_context_tokens: 100,
        ..Default::default()
    };
    let path = std::env::temp_dir().join(format!(
        "chatygpt-system1-semantic-{}.sqlite",
        Uuid::new_v4()
    ));
    let database = Database::open(path.clone()).unwrap();
    let conversation = database.create_conversation("Semántica", None).unwrap();
    database.set_memory_enabled(true).unwrap();
    let content = "Trading de otro proyecto. ".repeat(30);
    let (memory_id, _) = database
        .create_memory_item(&content, "fact", "normal", None)
        .unwrap();
    let request = embedding_request(
        "mem-key",
        "memory",
        &memory_id,
        &content,
        &format!("{:x}", Sha256::digest(content.as_bytes())),
    );
    database
        .prepare_broker_task("mem-task", "mem-key", &request)
        .unwrap();
    database
        .record_remote_state("mem-task", &completed_embedding("mem-task"))
        .unwrap();
    let query = "Haz lo mismo con Athena";
    let context = vec![ContextMessage {
        message_id: "user".to_owned(),
        role: "user".to_owned(),
        text: query.to_owned(),
    }];
    let request = embedding_request(
        "query-key",
        "chat_memory_search",
        "workflow",
        query,
        "query-hash",
    );
    database
        .prepare_semantic_chat_turn(
            "workflow",
            &conversation.id,
            "user",
            "assistant",
            "query-task",
            "query-key",
            query,
            &request,
            &context,
            &[],
            false,
            false,
            &ConversationExecutionPreferences::default(),
            None,
        )
        .unwrap();
    database
        .record_remote_state("query-task", &completed_embedding("query-task"))
        .unwrap();
    assert!(server.requests().is_empty());
    assert_eq!(
        database.semantic_memory_matches("workflow").unwrap().len(),
        1
    );
    // Reanuda el mismo recorrido que se usa al completar embeddings o recuperar el arranque.
    advance_semantic_chat(database.clone(), broker, "query-task");
    assert!(SimulatedBroker::wait_until(Duration::from_secs(10), || {
        database
            .semantic_chat_workflow_for_task("query-task")
            .ok()
            .flatten()
            .and_then(|workflow| workflow.chat_task_id)
            .is_some_and(|id| {
                database
                    .task_snapshot(&id)
                    .is_ok_and(|task| task.remote_status == "completed")
            })
    }));
    let workflow = database
        .semantic_chat_workflow_for_task("query-task")
        .unwrap()
        .unwrap();
    let saved = database
        .task_context(&workflow.chat_task_id.unwrap())
        .unwrap();
    assert_eq!(saved.system1.unwrap()["excluded"], 1);
    assert_eq!(
        saved
            .sources
            .iter()
            .filter(|source| source.kind == "memory")
            .count(),
        0
    );
    assert_eq!(server.requests_to("POST", "/api/v1/system1/judge").len(), 1);
    assert!(
        !server.requests_to("POST", "/api/v1/tasks")[0].body["content"]["prompt"]
            .as_str()
            .unwrap()
            .contains("Trading de otro proyecto")
    );
    assert_eq!(
        database
            .conversation_view(&conversation.id)
            .unwrap()
            .messages[1]
            .text
            .as_deref(),
        Some("Continuidad preservada: Athena.")
    );
    drop(database);
    let _ = std::fs::remove_file(path);
}

fn memory(id: &str, text: &str, category: &str) -> MemoryItemView {
    MemoryItemView {
        id: id.to_owned(),
        content: text.to_owned(),
        category: category.to_owned(),
        project_id: None,
        project_name: None,
        custom_gpt_id: None,
        custom_gpt_name: None,
        sensitivity: "normal".to_owned(),
        enabled: true,
        embedding_status: "missing".to_owned(),
        embedding_model: None,
        embedding_error: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

fn score(value: i32, confidence: f64) -> serde_json::Value {
    json!({"use_case":"chatygpt_context_relevance", "accepted":true,"decision":value,
        "confidence":confidence,"confidence_is_calibrated":false,"provider":"ollama_system1",
        "latency_ms":1,"fallback_used":false,"reason_code":null})
}

#[test]
fn system1_five_conversations_compare_the_actual_assembler_with_feature_off() {
    let scenarios = [
        (
            "Athena implícita",
            "Haz lo mismo con Athena",
            "Diseño común: conserva MCP, timeout y fallback. Aplica el diseño a cada aplicación.",
            "Athena",
            false,
            false,
        ),
        (
            "Segunda opción",
            "Usa la segunda opción",
            "1. LAYA remoto; 2. Ollama local. La segunda opción conserva los datos localmente.",
            "Ollama local",
            false,
            false,
        ),
        (
            "Athena explícita",
            "Configura Athena siguiendo la propuesta anterior",
            "Propuesta aprobada: Broker como único acceso a modelos.",
            "Athena",
            false,
            false,
        ),
        (
            "Historial corto",
            "Explica Athena",
            "Athena ejecuta objetivos.",
            "Athena",
            true,
            false,
        ),
        (
            "Confianza baja",
            "Aplica eso al Broker",
            "No publiques. Conserva el diseño aprobado y la restricción de privacidad.",
            "Broker",
            false,
            true,
        ),
    ];
    let mut evidence = Vec::new();
    for (name, query, previous, relevant, short, low_confidence) in scenarios {
        let server = SimulatedBroker::start();
        server.always(
            "GET /api/v1/capabilities",
            ScriptedResponse::ok(json!({"system1_judgments":true})),
        );
        server.always(
            "POST /api/v1/system1/judge",
            ScriptedResponse::ok(if low_confidence {
                // Lo que Nimble devuelve de verdad a un referente implícito.
                json!({"use_case":"chatygpt_context_relevance","accepted":false,
                    "decision":null,"confidence":null,"confidence_is_calibrated":false,
                    "provider":"ollama_system1","latency_ms":1,"fallback_used":true,
                    "reason_code":"LOW_CONFIDENCE"})
            } else {
                score(2, 0.99)
            }),
        );
        server.respond_when(
            "POST /api/v1/system1/judge",
            "Otro proyecto",
            ScriptedResponse::ok(score(0, 0.99)),
        );
        let mut broker = BrokerClient::for_base_url(server.base_url()).unwrap();
        let memories = vec![
            memory(
                "relevante",
                &format!(
                    "{relevant}: {}",
                    "Diseño acordado y pasos verificables. ".repeat(25)
                ),
                "fact",
            ),
            memory(
                "trading",
                &format!(
                    "Otro proyecto: trading. {}",
                    "Estrategia de inversión sin relación con esta tarea. ".repeat(25)
                ),
                "fact",
            ),
            memory(
                "restricción",
                "Mantén la privacidad y pide permiso antes de publicar.",
                "instruction",
            ),
        ];
        let context = vec![
            ContextMessage {
                message_id: "prior".to_owned(),
                role: "assistant".to_owned(),
                text: previous.to_owned(),
            },
            ContextMessage {
                message_id: "current".to_owned(),
                role: "user".to_owned(),
                text: query.to_owned(),
            },
        ];
        let project = ProjectInstructionContext {
            project_id: "project".to_owned(),
            project_name: "TFM".to_owned(),
            instructions: "Conserva las instrucciones del proyecto y sus criterios de aceptación."
                .to_owned(),
        };
        let baseline = tauri::async_runtime::block_on(chat_request_with_system1(
            &broker,
            "chat",
            "key",
            query,
            &context,
            &[],
            &[],
            &memories,
            Some(&project),
            None,
            None,
            ChatExecutionOptions::default(),
        ))
        .unwrap()
        .0;
        assert!(server.requests().is_empty());
        broker.context_filter = crate::system1::ContextFilterConfig {
            enabled: true,
            shadow_mode: false,
            max_context_tokens: if short { 10_000 } else { 200 },
            ..Default::default()
        };
        let (request, selected, _) = tauri::async_runtime::block_on(chat_request_with_system1(
            &broker,
            "chat",
            "key",
            query,
            &context,
            &[],
            &[],
            &memories,
            Some(&project),
            None,
            None,
            ChatExecutionOptions::default(),
        ))
        .unwrap();
        let before = crate::system1::prompt_tokens(&baseline);
        let after = crate::system1::prompt_tokens(&request);
        let prompt = request["content"]["prompt"].as_str().unwrap();
        for required in [
            previous,
            query,
            project.instructions.as_str(),
            memories[2].content.as_str(),
            relevant,
        ] {
            assert!(prompt.contains(required));
        }
        // Con baja confianza el referente dudoso se conserva por sí mismo y el
        // descarte claro de trading se aplica igualmente.
        if short {
            assert_eq!(before, after);
            assert_eq!(request["content"]["prompt"], baseline["content"]["prompt"]);
            assert_eq!(selected.len(), 3);
        } else {
            assert!(after < before);
            assert!(!prompt.contains("Otro proyecto: trading"));
            assert_eq!(selected.len(), 2);
        }
        let response_text = format!(
            "Aplicaré {relevant} con el diseño acordado, conservando la privacidad y sin publicar."
        );
        // Respuesta guionizada, misma tarea generativa en ambos modos. No
        // representa una evaluación de calidad de un modelo real.
        let response = crate::broker::simulated::completed_chat_result(&response_text);
        evidence.push(json!({
            "scenario":name,"currentRequest":query,"mandatoryHistory":previous,
            "candidates":[{"id":"relevante","excerpt":relevant},{"id":"trading","excerpt":"Otro proyecto: trading"}],
            "judgments":request["content"]["metadata"]["system1_context"]["scores"],
            "finalContextSources":selected.iter().map(|item| item.id.clone()).collect::<Vec<_>>(),
            "responseSimulated":response,"responseFeatureOffSimulated":response,
            "tokensBeforeEstimated":before,"tokensAfterEstimated":after,
            "mode":request["content"]["metadata"]["system1_context"]["mode"],
            "reason":request["content"]["metadata"]["system1_context"]["reason"],
            "judgeCalls":server.requests_to("POST","/api/v1/system1/judge").len()
        }));
    }
    if let Some(path) = std::env::var_os("CHATYGPT_SYSTEM1_TEST_EVIDENCE") {
        std::fs::write(path, serde_json::to_vec_pretty(&json!({"simulated":true,"tokenEstimator":"ceil(prompt Unicode characters / 4)","conversations":evidence})).unwrap()).unwrap();
    }
}

#[test]
fn system1_applied_snapshot_matches_request_and_sensitive_exclusion_keeps_privacy() {
    let server = SimulatedBroker::start();
    server.always(
        "GET /api/v1/capabilities",
        ScriptedResponse::ok(json!({"system1_judgments":true})),
    );
    server.always(
        "POST /api/v1/system1/judge",
        ScriptedResponse::ok(score(0, 0.99)),
    );
    let mut broker = BrokerClient::for_base_url(server.base_url()).unwrap();
    broker.context_filter = crate::system1::ContextFilterConfig {
        enabled: true,
        shadow_mode: false,
        max_context_tokens: 100,
        ..Default::default()
    };
    let path = std::env::temp_dir().join(format!("chatygpt-system1-{}.sqlite", Uuid::new_v4()));
    let database = Database::open(path.clone()).unwrap();
    let conversation = database.create_conversation("Prueba", None).unwrap();
    let mut sensitive = memory(
        "secreto",
        &"Texto privado sin relación. ".repeat(40),
        "fact",
    );
    sensitive.sensitivity = "sensitive".to_owned();
    let context = vec![ContextMessage {
        message_id: "user".to_owned(),
        role: "user".to_owned(),
        text: "Habla de Athena".to_owned(),
    }];
    let (request, memories, chunks) = tauri::async_runtime::block_on(chat_request_with_system1(
        &broker,
        &conversation.id,
        "key",
        "Habla de Athena",
        &context,
        &[],
        &[],
        &[sensitive],
        None,
        None,
        None,
        ChatExecutionOptions::default(),
    ))
    .unwrap();
    assert!(memories.is_empty());
    // Los jueces System 1 son locales: un turno local_only puede consultarlos,
    // pero nunca autoriza un proveedor que saque el contenido de la máquina.
    let judged = server.requests_to("POST", "/api/v1/system1/judge");
    assert_eq!(judged.len(), 1);
    assert_eq!(judged[0].body["cloud_allowed"], false);
    assert_eq!(request["risk"]["data_classification"], "local_only");
    assert_eq!(request["auxiliary_invocations"], false);
    database
        .prepare_chat_turn(
            &conversation.id,
            "user",
            "assistant",
            "task",
            "key",
            "Habla de Athena",
            &request,
            &context,
            &memories,
            &chunks,
            &[],
        )
        .unwrap();
    let snapshot = database.task_context("task").unwrap();
    assert_eq!(snapshot.sources.len(), 1);
    assert_eq!(snapshot.system1.unwrap()["excluded"], 1);
    let persisted = database.task_record("task").unwrap();
    assert_eq!(persisted.request, request);
    drop(database);
    let _ = std::fs::remove_file(path);
}
