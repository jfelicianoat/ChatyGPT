//! Workflows: publicacion que congela la version del GPT y sus ejecuciones.

use super::comunes::{cleanup, test_database};
use crate::db::{WorkflowEdge, WorkflowNode};

#[test]
fn workflow_publication_freezes_gpt_version_and_creates_durable_node_runs() {
    let database = test_database();
    let project = database
        .create_project("Proyecto de revisión", None)
        .expect("project should be created");
    database
        .update_project_instructions(
            &project.id,
            Some("Distingue siempre los hechos de las hipótesis."),
        )
        .expect("project instructions should persist");
    database
        .set_memory_enabled(true)
        .expect("memory should be enabled");
    let (project_memory_id, _) = database
        .create_memory_item(
            "La revisión se entrega en español.",
            "instruction",
            "normal",
            Some(&project.id),
        )
        .expect("project memory should be created");
    let gpt = database
        .create_custom_gpt("Revisor", None, "Revisa el texto con rigor.")
        .expect("custom GPT should be created");
    let (memory_id, _) = database
        .create_custom_gpt_memory_item(
            &gpt.id,
            "Solo responde con evidencia verificable.",
            "instruction",
            "sensitive",
        )
        .expect("custom GPT knowledge should be created");
    let gpt_file = database
        .register_custom_gpt_attachment(
            &gpt.id,
            "C:/managed/guide.pdf",
            "guide.pdf",
            Some("application/pdf"),
            42,
            "workflow-gpt-file",
        )
        .expect("custom GPT file should register");
    database
        .update_attachment_ingestion(
            &gpt_file.id,
            "ready",
            Some("broker-gpt-file"),
            Some("document"),
            Some("test"),
            Some(&serde_json::json!({})),
            None,
        )
        .expect("custom GPT file should become ready");
    let mut workflow = database
        .create_workflow("Revisión en cadena", Some(&project.id))
        .expect("workflow should be created");
    let input_id = workflow.definition.nodes[0].id.clone();
    let result_id = workflow.definition.nodes[1].id.clone();
    let gpt_node_id = "node-reviewer".to_owned();
    workflow.definition.nodes.push(WorkflowNode {
        id: gpt_node_id.clone(),
        kind: "custom_gpt".to_owned(),
        label: "Revisor".to_owned(),
        x: 350.0,
        y: 170.0,
        custom_gpt_id: Some(gpt.id.clone()),
        custom_gpt_version_id: None,
        custom_gpt_name: None,
        custom_gpt_icon_ref: None,
        custom_gpt_instructions: None,
        preferred_model: None,
        execution_profile: None,
        context_profile: "balanced".to_owned(),
        custom_gpt_memory_ids: Vec::new(),
        custom_gpt_attachment_ids: Vec::new(),
        instruction: None,
        attachment_ids: Vec::new(),
    });
    workflow.definition.edges = vec![
        WorkflowEdge {
            id: "edge-in".to_owned(),
            source: input_id.clone(),
            target: gpt_node_id.clone(),
        },
        WorkflowEdge {
            id: "edge-out".to_owned(),
            source: gpt_node_id.clone(),
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
        .expect("draft should save");
    let published = database
        .publish_workflow(&workflow.summary.id)
        .expect("workflow should publish");
    assert_eq!(published.summary.published_version_no, Some(1));

    let record = database
        .create_workflow_run(&workflow.summary.id, "Texto para revisar")
        .expect("durable run should be created");
    let frozen_gpt = record
        .definition
        .nodes
        .iter()
        .find(|node| node.kind == "custom_gpt")
        .expect("published GPT node should exist");
    let frozen_project = record
        .definition
        .project_context
        .as_ref()
        .expect("published project context should exist");
    assert_eq!(frozen_project.project_id, project.id);
    assert_eq!(
        frozen_project.instructions.as_deref(),
        Some("Distingue siempre los hechos de las hipótesis.")
    );
    assert_eq!(frozen_project.memory_ids, vec![project_memory_id.clone()]);
    assert!(database
        .project_instruction_for_workflow(frozen_project)
        .expect("project instruction should resolve")
        .is_some());
    assert_eq!(
        database
            .project_memories_for_workflow(frozen_project)
            .expect("project memories should resolve")
            .len(),
        1
    );
    assert!(frozen_gpt.custom_gpt_version_id.is_some());
    assert_eq!(frozen_gpt.custom_gpt_icon_ref.as_deref(), Some("spark"));
    assert_eq!(
        frozen_gpt.custom_gpt_instructions.as_deref(),
        Some("Revisa el texto con rigor.")
    );
    assert_eq!(frozen_gpt.custom_gpt_memory_ids, vec![memory_id.clone()]);
    assert_eq!(
        frozen_gpt.custom_gpt_attachment_ids,
        vec![gpt_file.id.clone()]
    );
    assert_eq!(
        database
            .custom_gpt_memories_for_workflow(&gpt.id, &frozen_gpt.custom_gpt_memory_ids)
            .expect("published knowledge should resolve")
            .len(),
        1
    );
    assert_eq!(
        database
            .ready_custom_gpt_attachments_for_workflow(
                &gpt.id,
                &frozen_gpt.custom_gpt_attachment_ids,
            )
            .expect("published files should resolve")
            .len(),
        1
    );
    let run = database
        .workflow_run(&record.run_id)
        .expect("run should load");
    assert_eq!(run.node_runs.len(), 3);
    assert!(run.node_runs.iter().all(|node| node.status == "pending"));

    database
        .update_workflow_node_run(
            &record.run_id,
            &input_id,
            "completed",
            Some("Texto para revisar"),
            Some("Texto para revisar"),
            None,
            None,
        )
        .expect("input should complete");
    database
        .update_workflow_node_run(
            &record.run_id,
            &gpt_node_id,
            "failed",
            Some("Texto para revisar"),
            None,
            Some("broker-failed"),
            Some(&serde_json::json!({"message": "fallo"})),
        )
        .expect("GPT node should fail");
    database
        .update_workflow_run_status(
            &record.run_id,
            "failed",
            None,
            Some(&serde_json::json!({"message": "fallo"})),
        )
        .expect("run should fail");
    let retry = database
        .retry_workflow_run(&record.run_id)
        .expect("failed run should be retried");
    let retry_view = database
        .workflow_run(&retry.run_id)
        .expect("retry should load");
    assert_eq!(
        retry_view
            .node_runs
            .iter()
            .find(|node| node.node_id == input_id)
            .expect("input should exist")
            .status,
        "completed",
        "successful upstream work is reused"
    );
    assert_eq!(
        retry_view
            .node_runs
            .iter()
            .find(|node| node.node_id == gpt_node_id)
            .expect("GPT should exist")
            .status,
        "pending",
        "the failed node is executed again"
    );
    database
        .set_custom_gpt_memory_item_enabled(&gpt.id, &memory_id, false)
        .expect("knowledge should be disabled");
    database
        .remove_custom_gpt_file(&gpt.id, &gpt_file.id)
        .expect("file should be removed from the GPT");
    assert!(database
        .custom_gpt_memories_for_workflow(&gpt.id, &frozen_gpt.custom_gpt_memory_ids)
        .expect("revoked knowledge should be ignored")
        .is_empty());
    assert!(database
        .ready_custom_gpt_attachments_for_workflow(&gpt.id, &frozen_gpt.custom_gpt_attachment_ids,)
        .expect("revoked files should be ignored")
        .is_empty());
    database
        .update_project_instructions(&project.id, Some("Nueva instrucción"))
        .expect("project instructions should change");
    database
        .set_memory_item_enabled(&project_memory_id, false)
        .expect("project memory should be disabled");
    assert!(database
        .project_instruction_for_workflow(frozen_project)
        .expect("changed instructions should be revoked")
        .is_none());
    assert!(database
        .project_memories_for_workflow(frozen_project)
        .expect("disabled project memories should be ignored")
        .is_empty());
    cleanup(&database);
}
