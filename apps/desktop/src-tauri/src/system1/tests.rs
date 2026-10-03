use super::*;
use crate::broker::simulated::{ScriptedResponse, SimulatedBroker};

fn memory(id: &str, category: &str) -> MemoryItemView {
    MemoryItemView {
        id: id.to_owned(), category: category.to_owned(), content: format!("{id} {}", "contexto ".repeat(120)),
        project_id: None, project_name: None, custom_gpt_id: None, custom_gpt_name: None,
        sensitivity: "normal".to_owned(), enabled: true, embedding_status: "missing".to_owned(),
        embedding_model: None, embedding_error: None, created_at: String::new(), updated_at: String::new(),
    }
}

fn response(score: Value, confidence: f64) -> Value {
    json!({"use_case":"chatygpt_context_relevance", "accepted":true, "decision":score,
        "confidence":confidence, "confidence_is_calibrated":false, "provider":"laya_mcp",
        "latency_ms":2, "fallback_used":true, "reason_code":"PROVIDER_UNAVAILABLE"})
}

fn client(server: &SimulatedBroker) -> BrokerClient {
    server.always("GET /api/v1/capabilities", ScriptedResponse::ok(json!({"system1_judgments":true})));
    let mut broker = BrokerClient::for_base_url(server.base_url()).unwrap();
    broker.context_filter = ContextFilterConfig {
        enabled: true, shadow_mode: false, max_context_tokens: 100, ..Default::default()
    };
    broker
}

fn evaluate(broker: &BrokerClient, query: &str, memories: &mut Vec<MemoryItemView>) -> ContextFilterTrace {
    tauri::async_runtime::block_on(filter_context(broker, query, &[],
        &json!({"content":{"prompt":"texto ".repeat(1000)}}), memories, &mut vec![])).unwrap()
}

#[test]
fn system1_keeps_athena_reference_and_removes_unrelated_project_with_zero_score() {
    let server = SimulatedBroker::start();
    let broker = client(&server);
    broker.replace_admin_token(Some("test-only-system1-token")).unwrap();
    server.script("POST /api/v1/system1/judge", ScriptedResponse::ok(response(json!(9), 0.99)));
    server.script("POST /api/v1/system1/judge", ScriptedResponse::ok(response(json!(0), 0.99)));
    let mut memories = vec![memory("Athena", "fact"), memory("Trading", "fact")];
    let trace = evaluate(&broker, "Haz lo mismo con Athena", &mut memories);
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].id, "Athena");
    assert_eq!(trace.excluded, 1);
    assert!(trace.implicit_reference_signal);
    assert!(!trace.scores[0].confidence_is_calibrated);
    assert!(trace.scores[0].provider_fallback);
    let requests = server.requests_to("POST", "/api/v1/system1/judge");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].headers["x-admin-token"], "test-only-system1-token");
    assert_eq!(requests[0].body["cloud_allowed"], false);
    assert_eq!(requests[0].body["decision_type"], "score");
    assert_eq!(requests[0].body["rubric"].as_array().unwrap().len(), 11);
    for forbidden in ["risk", "task_id", "timeout", "model", "threshold_profile", "idempotency_key"] {
        assert!(requests[0].body.get(forbidden).is_none());
    }
}

#[test]
fn system1_second_option_keeps_uncertain_referent_and_history_never_changes() {
    let server = SimulatedBroker::start();
    let broker = client(&server);
    server.always("POST /api/v1/system1/judge", ScriptedResponse::ok(response(json!(6), 0.99)));
    let history = vec![ContextMessage {message_id:"prior".to_owned(), role:"assistant".to_owned(),
        text:"1. LAYA; 2. Ollama; 3. Otro modelo. No publicar todavía.".to_owned()}];
    let mut memories = vec![memory("Ollama", "fact")];
    let trace = tauri::async_runtime::block_on(filter_context(&broker, "Usa la segunda opción", &history,
        &json!({"content":{"prompt":"x".repeat(2000)}}), &mut memories, &mut vec![])).unwrap();
    assert_eq!(trace.excluded, 0);
    assert_eq!(memories.len(), 1);
    let requests = server.requests_to("POST", "/api/v1/system1/judge");
    assert_eq!(requests[0].body["input"]["recent_history"][0]["text"], history[0].text);
}

#[test]
fn system1_short_history_and_feature_off_do_not_call_broker() {
    let server = SimulatedBroker::start();
    let mut broker = client(&server);
    let mut memories = vec![memory("Athena", "fact")];
    let trace = tauri::async_runtime::block_on(filter_context(&broker, "Athena", &[],
        &json!({"content":{"prompt":"Corto"}}), &mut memories, &mut vec![])).unwrap();
    assert_eq!(trace.reason, "fits_budget");
    broker.context_filter.enabled = false;
    assert!(tauri::async_runtime::block_on(filter_context(&broker, "Athena", &[],
        &json!({"content":{"prompt":"x".repeat(2000)}}), &mut memories, &mut vec![])).is_none());
    assert!(server.requests().is_empty());
}

#[test]
fn system1_errors_revert_the_whole_selection_without_retries() {
    for failure in [ScriptedResponse::transient(), ScriptedResponse::malformed(),
        ScriptedResponse::ok(response(json!(5), 0.50)),
        ScriptedResponse::ok(response(json!(11), 0.99)),
        ScriptedResponse::ok(response(json!(0.5), 0.99)),
        ScriptedResponse::ok(response(json!("0"), 0.99)),
        ScriptedResponse::ok(json!({"use_case":"chatygpt_context_relevance", "accepted":false,
            "decision":null,"confidence":null,"confidence_is_calibrated":false,"provider":null,
            "latency_ms":1,"fallback_used":true,"reason_code":"LOW_CONFIDENCE"})),
        ScriptedResponse::ok(json!({"accepted":true,"decision":0})),
    ] {
        let server = SimulatedBroker::start();
        let broker = client(&server);
        server.script("POST /api/v1/system1/judge", ScriptedResponse::ok(response(json!(0), 0.99)));
        server.script("POST /api/v1/system1/judge", failure);
        let mut memories = vec![memory("primero", "fact"), memory("segundo", "fact")];
        let before = json!(memories);
        let trace = evaluate(&broker, "Evalúa Athena", &mut memories);
        assert!(trace.fallback);
        assert_eq!(trace.excluded, 0);
        assert_eq!(json!(memories), before);
        assert_eq!(server.requests_to("POST", "/api/v1/system1/judge").len(), 2);
    }
}

#[test]
fn system1_old_broker_and_unreachable_broker_fall_back() {
    let server = SimulatedBroker::start();
    let broker = client(&server);
    server.always("GET /api/v1/capabilities", ScriptedResponse::ok(json!({"contract_version":"2.10"})));
    let mut memories = vec![memory("Athena", "fact")];
    assert_eq!(evaluate(&broker, "Athena", &mut memories).reason, "not_supported");
    assert!(server.requests_to("POST", "/api/v1/system1/judge").is_empty());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let mut broker = BrokerClient::for_base_url(&url).unwrap();
    broker.context_filter.enabled = true;
    broker.context_filter.max_context_tokens = 100;
    assert!(evaluate(&broker, "Athena", &mut memories).fallback);
    assert_eq!(memories.len(), 1);
}

#[test]
fn system1_global_timeout_preserves_context() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut broker = BrokerClient::for_base_url(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
    broker.context_filter = ContextFilterConfig {enabled:true, max_context_tokens:100, timeout_ms:20, ..Default::default()};
    let mut memories = vec![memory("Athena", "fact")];
    let started = Instant::now();
    let trace = evaluate(&broker, "Athena", &mut memories);
    assert!(trace.fallback);
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(memories.len(), 1);
}

#[test]
fn system1_shadow_and_candidate_limit_keep_unscored_sources() {
    let server = SimulatedBroker::start();
    let mut broker = client(&server);
    server.always("POST /api/v1/system1/judge", ScriptedResponse::ok(response(json!(0), 0.99)));
    broker.context_filter.shadow_mode = true;
    broker.context_filter.max_candidates = 1;
    let mut memories = vec![memory("A", "fact"), memory("B", "fact")];
    let trace = evaluate(&broker, "Athena", &mut memories);
    assert_eq!(trace.mode, "shadow");
    assert_eq!(trace.proposed_excluded, 1);
    assert_eq!(trace.excluded, 0);
    assert_eq!(memories.len(), 2);
    broker.context_filter.shadow_mode = false;
    evaluate(&broker, "Athena", &mut memories);
    assert_eq!(memories.len(), 1);
    assert_eq!(memories[0].id, "B");
}

#[test]
fn system1_protects_active_constraints_global_documents_and_each_file_anchor() {
    let server = SimulatedBroker::start();
    let broker = client(&server);
    server.always("POST /api/v1/system1/judge", ScriptedResponse::ok(response(json!(0), 0.99)));
    let mut memories = vec![memory("restricción", "instruction"), memory("preferencia", "preference"), memory("dato", "fact")];
    let chunk = |id: &str, file: &str, reason: &str| SelectedAttachmentChunk {
        id:id.to_owned(),attachment_id:file.to_owned(),attachment_name:file.to_owned(),ordinal:0,
        text:"fragmento".repeat(100),score:0.9,reason:reason.to_owned()
    };
    let mut chunks = vec![chunk("A1","A","relevancia"),chunk("A2","A","relevancia"),
        chunk("B1","B","relevancia"),chunk("B2","B","Vista global del documento: muestra")];
    let trace = tauri::async_runtime::block_on(filter_context(&broker, "Athena", &[],
        &json!({"content":{"prompt":"x".repeat(5000)}}), &mut memories, &mut chunks)).unwrap();
    assert_eq!(trace.excluded, 2);
    assert_eq!(memories.len(), 2);
    assert_eq!(chunks.iter().map(|item| item.id.as_str()).collect::<Vec<_>>(), vec!["A1","B1","B2"]);
}

#[test]
fn system1_configuration_is_configurable_and_nan_disables_filter() {
    let config = ContextFilterConfig::from_lookup(|key| match key {
        "CHATYGPT_SYSTEM1_CONTEXT_ENABLED" => Some("true".to_owned()),
        "CHATYGPT_SYSTEM1_CONTEXT_INCLUDE_THRESHOLD" => Some("NaN".to_owned()),
        _ => None,
    });
    assert!(!config.enabled);
    let config = ContextFilterConfig::from_lookup(|key| match key {
        "CHATYGPT_SYSTEM1_CONTEXT_ENABLED" => Some("1".to_owned()),
        "CHATYGPT_SYSTEM1_CONTEXT_SHADOW" => Some("false".to_owned()),
        "CHATYGPT_SYSTEM1_CONTEXT_TIMEOUT_MS" => Some("90000".to_owned()),
        "CHATYGPT_SYSTEM1_CONTEXT_MAX_CANDIDATES" => Some("5".to_owned()),
        _ => None,
    });
    assert!(config.enabled);
    assert!(!config.shadow_mode);
    assert_eq!(config.timeout_ms, 90000);
    assert_eq!(config.max_candidates, 5);
}
