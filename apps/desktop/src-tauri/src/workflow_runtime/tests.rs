//! Pruebas de `workflow_runtime`.
//!
//! Viven aparte desde que el fichero paso de mil lineas: separarlas deja
//! la logica a la vista sin cambiar una sola linea de codigo.

use super::{cancel, decide_approval, start, validate_definition};
use crate::broker::simulated::{accepted_task, task_state, ScriptedResponse, SimulatedBroker};
use crate::broker::BrokerClient;
use crate::db::{
    ConversationExecutionPreferences, CustomGptToolPermissions, Database, WorkflowDefinition,
    WorkflowEdge, WorkflowNode,
};
use std::time::{Duration, Instant};
use uuid::Uuid;

#[test]
fn remote_cancellation_failures_are_reported_without_reopening_the_run() {
    let simulated = SimulatedBroker::start();
    simulated.always("DELETE /api/v1/tasks/{id}", ScriptedResponse::transient());
    let path = std::env::temp_dir().join(format!(
        "chatygpt-workflow-cancel-{}.sqlite",
        Uuid::new_v4().simple()
    ));
    let database = Database::open(&path).expect("database should open");
    let client = BrokerClient::for_base_url(simulated.base_url()).expect("client should open");
    let workflow = database
        .create_workflow("Flujo cancelable", None)
        .expect("workflow should be created");
    database
        .publish_workflow(&workflow.summary.id)
        .expect("workflow should publish");
    let record = database
        .create_workflow_run(&workflow.summary.id, "Entrada")
        .expect("workflow run should be created");
    let node_id = record.definition.nodes[0].id.clone();
    database
        .update_workflow_run_status(&record.run_id, "running", None, None)
        .expect("run should start");
    database
        .update_workflow_node_run(
            &record.run_id,
            &node_id,
            "running",
            Some("Entrada"),
            None,
            Some("broker-cancel"),
            None,
        )
        .expect("node should start");

    let error = tauri::async_runtime::block_on(cancel(database.clone(), client, &record.run_id))
        .expect_err("the remote failure must be visible to the caller");
    assert!(
        error.to_string().contains("cancelación remota"),
        "message: {error}"
    );
    assert_eq!(
        database
            .workflow_run(&record.run_id)
            .expect("run should load")
            .status,
        "cancelled"
    );
    assert_eq!(
        simulated
            .requests_to("DELETE", "/api/v1/tasks/broker-cancel")
            .len(),
        1
    );
    drop(database);
    let _ = std::fs::remove_file(path);
}

fn node(id: &str, kind: &str) -> WorkflowNode {
    WorkflowNode {
        id: id.to_owned(),
        kind: kind.to_owned(),
        label: id.to_owned(),
        x: 0.0,
        y: 0.0,
        custom_gpt_id: (kind == "custom_gpt").then(|| "gpt-1".to_owned()),
        custom_gpt_version_id: None,
        custom_gpt_name: None,
        custom_gpt_icon_ref: None,
        custom_gpt_instructions: None,
        preferred_model: None,
        execution_profile: None,
        context_profile: "balanced".to_owned(),
        custom_gpt_memory_ids: Vec::new(),
        custom_gpt_attachment_ids: Vec::new(),
        instruction: (kind == "prompt").then(|| "Resume".to_owned()),
        attachment_ids: Vec::new(),
    }
}

#[test]
fn valid_branching_dag_is_accepted() {
    let definition = WorkflowDefinition {
        nodes: vec![
            node("input", "input"),
            node("a", "prompt"),
            node("b", "prompt"),
            node("out", "result"),
        ],
        edges: vec![
            WorkflowEdge {
                id: "e1".to_owned(),
                source: "input".to_owned(),
                target: "a".to_owned(),
            },
            WorkflowEdge {
                id: "e2".to_owned(),
                source: "input".to_owned(),
                target: "b".to_owned(),
            },
            WorkflowEdge {
                id: "e3".to_owned(),
                source: "a".to_owned(),
                target: "out".to_owned(),
            },
            WorkflowEdge {
                id: "e4".to_owned(),
                source: "b".to_owned(),
                target: "out".to_owned(),
            },
        ],
        project_context: None,
    };
    validate_definition(&definition).expect("branching DAG should be valid");
}

#[test]
fn cycles_are_rejected_before_any_broker_task_exists() {
    let definition = WorkflowDefinition {
        nodes: vec![
            node("input", "input"),
            node("a", "prompt"),
            node("out", "result"),
        ],
        edges: vec![
            WorkflowEdge {
                id: "e1".to_owned(),
                source: "input".to_owned(),
                target: "a".to_owned(),
            },
            WorkflowEdge {
                id: "e2".to_owned(),
                source: "a".to_owned(),
                target: "out".to_owned(),
            },
            WorkflowEdge {
                id: "e3".to_owned(),
                source: "out".to_owned(),
                target: "a".to_owned(),
            },
        ],
        project_context: None,
    };
    assert!(validate_definition(&definition).is_err());
}

#[test]
fn duplicate_result_labels_are_rejected_before_publication() {
    let mut first = node("out-a", "result");
    first.label = "Resultado".to_owned();
    let mut second = node("out-b", "result");
    second.label = " resultado ".to_owned();
    let definition = WorkflowDefinition {
        nodes: vec![node("input", "input"), first, second],
        edges: vec![
            WorkflowEdge {
                id: "e1".to_owned(),
                source: "input".to_owned(),
                target: "out-a".to_owned(),
            },
            WorkflowEdge {
                id: "e2".to_owned(),
                source: "input".to_owned(),
                target: "out-b".to_owned(),
            },
        ],
        project_context: None,
    };

    let error = validate_definition(&definition)
        .expect_err("two visible result labels must not collapse into one output key");
    assert!(error.to_string().contains("nombre distinto"));
}

#[test]
fn a_transient_poll_failure_is_retried_before_materializing_the_result() {
    let simulated = SimulatedBroker::start();
    simulated.always(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("workflow-task")),
    );
    simulated.script("GET /api/v1/tasks/{id}", ScriptedResponse::transient());
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state(
            "workflow-task",
            "completed",
            Some(serde_json::json!({"assistant_content": "Respuesta encadenada"})),
        )),
    );
    let path = std::env::temp_dir().join(format!(
        "chatygpt-workflow-runtime-{}.sqlite",
        Uuid::new_v4().simple()
    ));
    let database = Database::open(&path).expect("database should open");
    let client = BrokerClient::for_base_url(simulated.base_url()).expect("client should open");
    let mut workflow = database
        .create_workflow("Flujo ejecutable", None)
        .expect("workflow should be created");
    let input_id = workflow.definition.nodes[0].id.clone();
    let result_id = workflow.definition.nodes[1].id.clone();
    workflow.definition.nodes.push(node("prompt", "prompt"));
    workflow.definition.edges = vec![
        WorkflowEdge {
            id: "e1".to_owned(),
            source: input_id,
            target: "prompt".to_owned(),
        },
        WorkflowEdge {
            id: "e2".to_owned(),
            source: "prompt".to_owned(),
            target: result_id,
        },
    ];
    database
        .update_workflow(
            &workflow.summary.id,
            &workflow.summary.name,
            None,
            None,
            &workflow.definition,
        )
        .expect("workflow should save");
    database
        .publish_workflow(&workflow.summary.id)
        .expect("workflow should publish");
    let run = start(
        database.clone(),
        client,
        &workflow.summary.id,
        "Entrada original",
    )
    .expect("workflow should start");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let current = database.workflow_run(&run.id).expect("run should load");
        if current.status == "completed" {
            assert_eq!(
                current.outputs["Resultado"],
                serde_json::Value::String("### Salida de prompt\nRespuesta encadenada".to_owned())
            );
            break;
        }
        assert!(
            Instant::now() < deadline,
            "workflow should finish before timeout"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(
        simulated
            .requests_to("GET", "/api/v1/tasks/workflow-task")
            .len(),
        2
    );
    assert!(simulated
        .requests_to("DELETE", "/api/v1/tasks/workflow-task")
        .is_empty());
    drop(database);
    let _ = std::fs::remove_file(path);
}

#[test]
fn exhausted_poll_retries_cancel_the_remote_work_before_failing() {
    let simulated = SimulatedBroker::start();
    simulated.always(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("unreachable-task")),
    );
    simulated.always("GET /api/v1/tasks/{id}", ScriptedResponse::transient());
    simulated.always(
        "DELETE /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state("unreachable-task", "cancelled", None)),
    );
    let path = std::env::temp_dir().join(format!(
        "chatygpt-workflow-poll-exhausted-{}.sqlite",
        Uuid::new_v4().simple()
    ));
    let database = Database::open(&path).expect("database should open");
    let client = BrokerClient::for_base_url(simulated.base_url()).expect("client should open");
    let mut workflow = database
        .create_workflow("Flujo sin sondeo", None)
        .expect("workflow should be created");
    let input_id = workflow.definition.nodes[0].id.clone();
    let result_id = workflow.definition.nodes[1].id.clone();
    workflow.definition.nodes.push(node("prompt", "prompt"));
    workflow.definition.edges = vec![
        WorkflowEdge {
            id: "e1".to_owned(),
            source: input_id,
            target: "prompt".to_owned(),
        },
        WorkflowEdge {
            id: "e2".to_owned(),
            source: "prompt".to_owned(),
            target: result_id,
        },
    ];
    database
        .update_workflow(
            &workflow.summary.id,
            &workflow.summary.name,
            None,
            None,
            &workflow.definition,
        )
        .expect("workflow should save");
    database
        .publish_workflow(&workflow.summary.id)
        .expect("workflow should publish");

    let run = start(database.clone(), client, &workflow.summary.id, "Entrada")
        .expect("workflow should start");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let current = database.workflow_run(&run.id).expect("run should load");
        if current.status == "failed" {
            break;
        }
        assert!(Instant::now() < deadline, "workflow should settle");
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(
        simulated
            .requests_to("GET", "/api/v1/tasks/unreachable-task")
            .len(),
        6
    );
    assert_eq!(
        simulated
            .requests_to("DELETE", "/api/v1/tasks/unreachable-task")
            .len(),
        1
    );
    drop(database);
    let _ = std::fs::remove_file(path);
}

#[test]
fn a_failed_node_promotes_its_error_to_the_workflow_run() {
    let simulated = SimulatedBroker::start();
    simulated.always(
        "POST /api/v1/tasks",
        ScriptedResponse {
            status: 403,
            body: serde_json::json!({
                "code": "ADMIN_AUTH_REQUIRED",
                "message": "forbidden"
            })
            .to_string()
            .into_bytes(),
            content_type: "application/json",
        },
    );
    let path = std::env::temp_dir().join(format!(
        "chatygpt-workflow-visible-failure-{}.sqlite",
        Uuid::new_v4().simple()
    ));
    let database = Database::open(&path).expect("database should open");
    let client = BrokerClient::for_base_url(simulated.base_url()).expect("client should open");
    let mut workflow = database
        .create_workflow("Flujo con fallo visible", None)
        .expect("workflow should be created");
    let input_id = workflow.definition.nodes[0].id.clone();
    let result_id = workflow.definition.nodes[1].id.clone();
    workflow.definition.nodes.push(node("prompt", "prompt"));
    workflow.definition.edges = vec![
        WorkflowEdge {
            id: "e1".to_owned(),
            source: input_id,
            target: "prompt".to_owned(),
        },
        WorkflowEdge {
            id: "e2".to_owned(),
            source: "prompt".to_owned(),
            target: result_id,
        },
    ];
    database
        .update_workflow(
            &workflow.summary.id,
            &workflow.summary.name,
            None,
            None,
            &workflow.definition,
        )
        .expect("workflow should save");
    database
        .publish_workflow(&workflow.summary.id)
        .expect("workflow should publish");

    let run = start(database.clone(), client, &workflow.summary.id, "Entrada")
        .expect("workflow should start");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let current = database.workflow_run(&run.id).expect("run should load");
        if current.status == "failed" {
            let error = current
                .error
                .expect("run should expose its primary failure");
            assert_eq!(error["node_id"], "prompt");
            assert_eq!(error["node_label"], "prompt");
            assert!(error["message"]
                .as_str()
                .is_some_and(|message| message.contains("ADMIN_AUTH_REQUIRED")));
            assert_eq!(error["failures"].as_array().map(Vec::len), Some(1));
            break;
        }
        assert!(
            Instant::now() < deadline,
            "workflow should fail before timeout"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    drop(database);
    let _ = std::fs::remove_file(path);
}

#[test]
fn a_published_custom_gpt_profile_reaches_the_broker_request() {
    let simulated = SimulatedBroker::start();
    simulated.always(
        "POST /api/v1/tasks",
        ScriptedResponse::accepted(accepted_task("profile-task")),
    );
    simulated.always(
        "GET /api/v1/tasks/{id}",
        ScriptedResponse::ok(task_state(
            "profile-task",
            "completed",
            Some(serde_json::json!({"assistant_content": "Perfil aplicado"})),
        )),
    );
    let path = std::env::temp_dir().join(format!(
        "chatygpt-workflow-profile-{}.sqlite",
        Uuid::new_v4().simple()
    ));
    let database = Database::open(&path).expect("database should open");
    let client = BrokerClient::for_base_url(simulated.base_url()).expect("client should open");
    let project = database
        .create_project("Proyecto de análisis", None)
        .expect("project should be created");
    database
        .update_project_instructions(
            &project.id,
            Some("Presenta por separado los hechos y las inferencias."),
        )
        .expect("project instructions should persist");
    database
        .set_memory_enabled(true)
        .expect("memory should be enabled");
    database
        .create_memory_item(
            "La audiencia del proyecto es no técnica.",
            "preference",
            "normal",
            Some(&project.id),
        )
        .expect("project memory should be created");
    let profile = ConversationExecutionPreferences {
        data_classification: "confidential".to_owned(),
        strategy: "mixture_of_agents".to_owned(),
        preset: "slow".to_owned(),
        max_cost_usd: 0.75,
        long_context: "fail".to_owned(),
        priority: 50,
    };
    let gpt = database
        .create_custom_gpt_with_starters(
            "Analista profundo",
            None,
            "Contrasta las perspectivas antes de responder.",
            &[],
            &CustomGptToolPermissions::default(),
            None,
            None,
            Some(&profile),
        )
        .expect("custom GPT should be created");
    database
        .create_custom_gpt_memory_item(&gpt.id, "Dato privado del flujo", "fact", "sensitive")
        .expect("private knowledge should be created");
    let gpt_file = database
        .register_custom_gpt_attachment(
            &gpt.id,
            "C:/managed/private-guide.pdf",
            "private-guide.pdf",
            Some("application/pdf"),
            128,
            "workflow-profile-file",
        )
        .expect("private file should register");
    database
        .update_attachment_ingestion(
            &gpt_file.id,
            "ready",
            Some("broker-profile-file"),
            Some("document"),
            Some("test"),
            Some(&serde_json::json!({})),
            None,
        )
        .expect("private file should become ready");
    let mut workflow = database
        .create_workflow("Flujo con perfil propio", Some(&project.id))
        .expect("workflow should be created");
    let input_id = workflow.definition.nodes[0].id.clone();
    let result_id = workflow.definition.nodes[1].id.clone();
    let mut gpt_node = node("analyst", "custom_gpt");
    gpt_node.custom_gpt_id = Some(gpt.id.clone());
    workflow.definition.nodes.push(gpt_node);
    workflow.definition.edges = vec![
        WorkflowEdge {
            id: "e1".to_owned(),
            source: input_id,
            target: "analyst".to_owned(),
        },
        WorkflowEdge {
            id: "e2".to_owned(),
            source: "analyst".to_owned(),
            target: result_id,
        },
    ];
    database
        .update_workflow(
            &workflow.summary.id,
            &workflow.summary.name,
            None,
            Some(&project.id),
            &workflow.definition,
        )
        .expect("workflow should save");
    database
        .publish_workflow(&workflow.summary.id)
        .expect("workflow should publish");

    let run = start(
        database.clone(),
        client,
        &workflow.summary.id,
        "Analiza esta entrada",
    )
    .expect("workflow should start");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let current = database.workflow_run(&run.id).expect("run should load");
        if current.status == "completed" {
            break;
        }
        assert!(Instant::now() < deadline, "workflow should complete");
        std::thread::sleep(Duration::from_millis(25));
    }

    let requests = simulated.requests_to("POST", "/api/v1/tasks");
    assert_eq!(requests.len(), 1);
    let request = &requests[0].body;
    assert_eq!(request["execution"]["strategy"], "mixture_of_agents");
    assert_eq!(request["execution"]["preset"], "slow");
    assert_eq!(request["execution"]["scheduling"], "adaptive");
    assert_eq!(request["risk"]["data_classification"], "local_only");
    assert_eq!(request["model_requirements"]["max_cost_usd"], 0.75);
    assert_eq!(request["priority"], 50);
    assert!(request["content"]["prompt"]
        .as_str()
        .expect("prompt should be text")
        .contains("Dato privado del flujo"));
    assert!(request["content"]["prompt"]
        .as_str()
        .expect("prompt should be text")
        .contains("Presenta por separado los hechos y las inferencias."));
    assert!(request["content"]["prompt"]
        .as_str()
        .expect("prompt should be text")
        .contains("La audiencia del proyecto es no técnica."));
    assert_eq!(
        request["content"]["attachments"][0]["metadata"]["file_id"],
        "broker-profile-file"
    );
    assert_eq!(
        request["content"]["metadata"]["custom_gpt_knowledge_count"],
        1
    );
    assert_eq!(request["content"]["metadata"]["custom_gpt_file_count"], 1);
    assert_eq!(request["content"]["metadata"]["active_file_count"], 1);
    assert_eq!(request["content"]["metadata"]["project_id"], project.id);
    assert_eq!(
        request["content"]["metadata"]["project_instruction_applied"],
        true
    );
    assert_eq!(request["content"]["metadata"]["project_memory_count"], 1);
    assert_eq!(
        request["content"]["metadata"]["custom_gpt_execution_profile"]["strategy"],
        "mixture_of_agents"
    );
    drop(database);
    let _ = std::fs::remove_file(path);
}

#[test]
fn approval_pauses_durably_and_rejection_does_not_stop_an_independent_branch() {
    let simulated = SimulatedBroker::start();
    let path = std::env::temp_dir().join(format!(
        "chatygpt-workflow-approval-{}.sqlite",
        Uuid::new_v4().simple()
    ));
    let database = Database::open(&path).expect("database should open");
    let client = BrokerClient::for_base_url(simulated.base_url()).expect("client should open");
    let mut workflow = database
        .create_workflow("Flujo con aprobación", None)
        .expect("workflow should be created");
    let input_id = workflow.definition.nodes[0].id.clone();
    let result_id = workflow.definition.nodes[1].id.clone();
    workflow.definition.nodes.push(node("approval", "approval"));
    workflow
        .definition
        .nodes
        .push(node("independent", "result"));
    workflow.definition.edges = vec![
        WorkflowEdge {
            id: "e1".to_owned(),
            source: input_id.clone(),
            target: "approval".to_owned(),
        },
        WorkflowEdge {
            id: "e2".to_owned(),
            source: "approval".to_owned(),
            target: result_id,
        },
        WorkflowEdge {
            id: "e3".to_owned(),
            source: input_id,
            target: "independent".to_owned(),
        },
    ];
    database
        .update_workflow(
            &workflow.summary.id,
            &workflow.summary.name,
            None,
            None,
            &workflow.definition,
        )
        .expect("workflow should save");
    database
        .publish_workflow(&workflow.summary.id)
        .expect("workflow should publish");

    let run = start(
        database.clone(),
        client.clone(),
        &workflow.summary.id,
        "Contenido que requiere revisión",
    )
    .expect("workflow should start");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let current = database.workflow_run(&run.id).expect("run should load");
        if current.status == "waiting_approval" {
            assert_eq!(
                current
                    .node_runs
                    .iter()
                    .find(|item| item.node_id == "approval")
                    .map(|item| item.status.as_str()),
                Some("waiting_approval")
            );
            assert!(current.outputs.get("independent").is_some());
            break;
        }
        assert!(Instant::now() < deadline, "workflow should pause");
        std::thread::sleep(Duration::from_millis(25));
    }

    drop(database);
    let database = Database::open(&path).expect("database should reopen after the pause");
    assert_eq!(
        database
            .workflow_run(&run.id)
            .expect("paused run should survive restart")
            .status,
        "waiting_approval"
    );
    decide_approval(database.clone(), client.clone(), &run.id, "approval", false)
        .expect("rejection should resume the run");
    loop {
        let current = database.workflow_run(&run.id).expect("run should load");
        if current.status == "partial_failed" {
            assert!(current.outputs.get("independent").is_some());
            assert_eq!(
                current
                    .node_runs
                    .iter()
                    .find(|item| item.node_id == "approval")
                    .map(|item| item.status.as_str()),
                Some("failed")
            );
            break;
        }
        assert!(Instant::now() < deadline, "rejected run should settle");
        std::thread::sleep(Duration::from_millis(25));
    }

    let approved_run = start(
        database.clone(),
        client.clone(),
        &workflow.summary.id,
        "Contenido aprobado",
    )
    .expect("second workflow should start");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let current = database
            .workflow_run(&approved_run.id)
            .expect("run should load");
        if current.status == "waiting_approval" {
            break;
        }
        assert!(Instant::now() < deadline, "second workflow should pause");
        std::thread::sleep(Duration::from_millis(25));
    }
    decide_approval(database.clone(), client, &approved_run.id, "approval", true)
        .expect("approval should resume the run");
    loop {
        let current = database
            .workflow_run(&approved_run.id)
            .expect("run should load");
        if current.status == "completed" {
            assert!(current.outputs.get("Resultado").is_some());
            assert!(current.outputs.get("independent").is_some());
            break;
        }
        assert!(Instant::now() < deadline, "approved run should complete");
        std::thread::sleep(Duration::from_millis(25));
    }
    drop(database);
    let _ = std::fs::remove_file(path);
}
