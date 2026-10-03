use super::*;
use crate::broker::simulated::{ScriptedResponse, SimulatedBroker};
use crate::db::ContextMessage;

fn memory(id: &str, text: &str, category: &str) -> MemoryItemView {
    MemoryItemView {
        id:id.to_owned(), content:text.to_owned(), category:category.to_owned(),
        project_id:None, project_name:None, custom_gpt_id:None, custom_gpt_name:None,
        sensitivity:"normal".to_owned(), enabled:true, embedding_status:"missing".to_owned(),
        embedding_model:None, embedding_error:None, created_at:String::new(), updated_at:String::new(),
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
        ("Athena implícita", "Haz lo mismo con Athena", "Diseño común: conserva MCP, timeout y fallback. Aplica el diseño a cada aplicación.", "Athena", false, false),
        ("Segunda opción", "Usa la segunda opción", "1. LAYA remoto; 2. Ollama local. La segunda opción conserva los datos localmente.", "Ollama local", false, false),
        ("Athena explícita", "Configura Athena siguiendo la propuesta anterior", "Propuesta aprobada: Broker como único acceso a modelos.", "Athena", false, false),
        ("Historial corto", "Explica Athena", "Athena ejecuta objetivos.", "Athena", true, false),
        ("Confianza baja", "Aplica eso al Broker", "No publiques. Conserva el diseño aprobado y la restricción de privacidad.", "Broker", false, true),
    ];
    let mut evidence = Vec::new();
    for (name, query, previous, relevant, short, low_confidence) in scenarios {
        let server = SimulatedBroker::start();
        server.always("GET /api/v1/capabilities", ScriptedResponse::ok(json!({"system1_judgments":true})));
        server.script("POST /api/v1/system1/judge", ScriptedResponse::ok(score(9, if low_confidence {0.50} else {0.99})));
        server.script("POST /api/v1/system1/judge", ScriptedResponse::ok(score(0, 0.99)));
        let mut broker = BrokerClient::for_base_url(server.base_url()).unwrap();
        let memories = vec![
            memory("relevante", &format!("{relevant}: {}", "Diseño acordado y pasos verificables. ".repeat(25)), "fact"),
            memory("trading", &format!("Otro proyecto: trading. {}", "Estrategia de inversión sin relación con esta tarea. ".repeat(25)), "fact"),
            memory("restricción", "Mantén la privacidad y pide permiso antes de publicar.", "instruction"),
        ];
        let context = vec![
            ContextMessage {message_id:"prior".to_owned(),role:"assistant".to_owned(),text:previous.to_owned()},
            ContextMessage {message_id:"current".to_owned(),role:"user".to_owned(),text:query.to_owned()},
        ];
        let project = ProjectInstructionContext {project_id:"project".to_owned(),project_name:"TFM".to_owned(),
            instructions:"Conserva las instrucciones del proyecto y sus criterios de aceptación.".to_owned()};
        let baseline = tauri::async_runtime::block_on(chat_request_with_system1(
            &broker, "chat", "key", query, &context, &[], &[], &memories, Some(&project), None,
            None, ChatExecutionOptions::default(),
        )).unwrap().0;
        assert!(server.requests().is_empty());
        broker.context_filter = crate::system1::ContextFilterConfig {
            enabled:true, shadow_mode:false, max_context_tokens:if short {10_000} else {200}, ..Default::default()
        };
        let (request, selected, _) = tauri::async_runtime::block_on(chat_request_with_system1(
            &broker, "chat", "key", query, &context, &[], &[], &memories, Some(&project), None,
            None, ChatExecutionOptions::default(),
        )).unwrap();
        let before = crate::system1::prompt_tokens(&baseline);
        let after = crate::system1::prompt_tokens(&request);
        let prompt = request["content"]["prompt"].as_str().unwrap();
        for required in [previous, query, project.instructions.as_str(), memories[2].content.as_str(), relevant] {
            assert!(prompt.contains(required));
        }
        if short || low_confidence {
            assert_eq!(before, after);
            assert_eq!(request["content"]["prompt"], baseline["content"]["prompt"]);
            assert_eq!(selected.len(), 3);
        } else {
            assert!(after < before);
            assert!(!prompt.contains("Otro proyecto: trading"));
            assert_eq!(selected.len(), 2);
        }
        let response_text = format!("Aplicaré {relevant} con el diseño acordado, conservando la privacidad y sin publicar.");
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
    server.always("GET /api/v1/capabilities", ScriptedResponse::ok(json!({"system1_judgments":true})));
    server.always("POST /api/v1/system1/judge", ScriptedResponse::ok(score(0,0.99)));
    let mut broker = BrokerClient::for_base_url(server.base_url()).unwrap();
    broker.context_filter = crate::system1::ContextFilterConfig {enabled:true,shadow_mode:false,max_context_tokens:100,..Default::default()};
    let path = std::env::temp_dir().join(format!("chatygpt-system1-{}.sqlite",Uuid::new_v4()));
    let database = Database::open(path.clone()).unwrap();
    let conversation = database.create_conversation("Prueba",None).unwrap();
    let mut sensitive = memory("secreto", &"Texto privado sin relación. ".repeat(40), "fact");
    sensitive.sensitivity = "sensitive".to_owned();
    let context = vec![ContextMessage {message_id:"user".to_owned(),role:"user".to_owned(),text:"Habla de Athena".to_owned()}];
    let (request, memories, chunks) = tauri::async_runtime::block_on(chat_request_with_system1(
        &broker,&conversation.id,"key","Habla de Athena",&context,&[],&[],&[sensitive],None,None,None,ChatExecutionOptions::default(),
    )).unwrap();
    assert!(memories.is_empty());
    assert_eq!(request["risk"]["data_classification"],"local_only");
    assert_eq!(request["auxiliary_invocations"],false);
    database.prepare_chat_turn(&conversation.id,"user","assistant","task","key","Habla de Athena",&request,&context,&memories,&chunks,&[]).unwrap();
    let snapshot = database.task_context("task").unwrap();
    assert_eq!(snapshot.sources.len(),1);
    assert_eq!(snapshot.system1.unwrap()["excluded"],1);
    let persisted = database.task_record("task").unwrap();
    assert_eq!(persisted.request, request);
    drop(database);
    let _ = std::fs::remove_file(path);
}
