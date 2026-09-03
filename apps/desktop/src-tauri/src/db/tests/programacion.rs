//! Tareas y workflows programados: reclamo, reintento y cancelacion.

use super::comunes::{cleanup, test_database};
use crate::error::AppError;
use rusqlite::params;

#[test]
fn scheduled_task_templates_are_durable_reusable_and_audited() {
    let database = test_database();
    let template = database
        .create_scheduled_task_template(
            "Resumen semanal",
            "Resume los avances y bloqueos.",
            "weekly",
        )
        .expect("template should be created");
    assert_eq!(template.name, "Resumen semanal");
    assert_eq!(template.schedule_expression, "weekly");

    let listed = database
        .list_scheduled_task_templates()
        .expect("templates should be listed");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].prompt, "Resume los avances y bloqueos.");
    assert!(matches!(
        database.delete_scheduled_task_template(&template.id, false),
        Err(AppError::Validation(_))
    ));
    database
        .delete_scheduled_task_template(&template.id, true)
        .expect("confirmed deletion should succeed");
    assert!(database
        .list_scheduled_task_templates()
        .expect("templates should be listed")
        .is_empty());

    let events: Vec<String> = database
        .connect()
        .expect("database should connect")
        .prepare(
            "SELECT event_type FROM audit_events
             WHERE event_type LIKE 'scheduled_template.%'
             ORDER BY id",
        )
        .expect("audit query should prepare")
        .query_map([], |row| row.get(0))
        .expect("audit query should run")
        .collect::<Result<_, _>>()
        .expect("audit events should collect");
    assert_eq!(
        events,
        vec!["scheduled_template.created", "scheduled_template.deleted"]
    );
    cleanup(&database);
}

#[test]
fn manual_scheduled_run_preserves_the_future_schedule_and_blocks_overlap() {
    let database = test_database();
    let conversation = database
        .create_conversation("Seguimiento manual", None)
        .expect("conversation should be created");
    let scheduled = database
        .create_scheduled_task(
            "Informe diario",
            &conversation.id,
            "Resume las novedades.",
            "2099-01-01T10:00:00.000Z",
            "Atlantic/Canary",
            "daily",
            true,
        )
        .expect("schedule should be created");
    assert!(matches!(
        database.claim_scheduled_task_now(&scheduled.id, false),
        Err(AppError::Validation(_))
    ));

    let manual = database
        .claim_scheduled_task_now(&scheduled.id, true)
        .expect("manual run should be claimed");
    assert_eq!(manual.scheduled_task_id, scheduled.id);
    assert_eq!(manual.conversation_id, Some(conversation.id));
    assert!(matches!(
        database.claim_scheduled_task_now(&scheduled.id, true),
        Err(AppError::Conflict(_))
    ));

    let listed = database
        .list_scheduled_tasks()
        .expect("schedule should remain visible");
    assert!(listed[0].enabled);
    assert_eq!(listed[0].next_run_at, scheduled.next_run_at);
    assert_eq!(listed[0].runs[0].id, manual.run_id);
    assert_eq!(listed[0].runs[0].status, "claimed");
    let audited: bool = database
        .connect()
        .expect("database should connect")
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM audit_events
                WHERE event_type = 'scheduled_run.manual_requested'
            )",
            [],
            |row| row.get(0),
        )
        .expect("audit event should be queryable");
    assert!(audited);
    cleanup(&database);
}

#[test]
fn tool_scheduled_task_is_idempotent_for_the_same_tool_call() {
    let database = test_database();
    let conversation = database
        .create_conversation("Agenda del GPT", None)
        .expect("conversation should be created");
    let first = database
        .create_scheduled_task_from_tool(
            "tool-call-42",
            "Revisión",
            &conversation.id,
            "Resume el estado del proyecto.",
            "2099-01-01T10:00:00.000Z",
            "Atlantic/Canary",
        )
        .expect("the tool schedule should be created");
    let repeated = database
        .create_scheduled_task_from_tool(
            "tool-call-42",
            "Revisión",
            &conversation.id,
            "Resume el estado del proyecto.",
            "2099-01-01T10:00:00.000Z",
            "Atlantic/Canary",
        )
        .expect("replaying the confirmation should be safe");

    assert_eq!(first.id, repeated.id);
    assert_eq!(database.list_scheduled_tasks().unwrap().len(), 1);
    cleanup(&database);
}

#[test]
fn published_workflow_can_be_scheduled_claimed_and_reconciled() {
    let database = test_database();
    let workflow = database
        .create_workflow("Informe encadenado", None)
        .expect("workflow should be created");
    database
        .publish_workflow(&workflow.summary.id)
        .expect("workflow should publish");
    assert!(matches!(
        database.create_scheduled_workflow(
            "Informe nocturno",
            &workflow.summary.id,
            "Resume la actividad de hoy.",
            "2099-01-01T22:00:00.000Z",
            "Atlantic/Canary",
            "daily",
            false,
        ),
        Err(AppError::Validation(_))
    ));
    let scheduled = database
        .create_scheduled_workflow(
            "Informe nocturno",
            &workflow.summary.id,
            "Resume la actividad de hoy.",
            "2099-01-01T22:00:00.000Z",
            "Atlantic/Canary",
            "daily",
            true,
        )
        .expect("published workflow should be scheduled");
    assert_eq!(scheduled.target_kind, "workflow");
    assert_eq!(
        scheduled.workflow_id.as_deref(),
        Some(workflow.summary.id.as_str())
    );
    assert_eq!(
        scheduled.workflow_name.as_deref(),
        Some("Informe encadenado")
    );
    assert_eq!(scheduled.workflow_version_no, Some(1));
    assert!(scheduled.conversation_id.is_none());
    database
        .publish_workflow(&workflow.summary.id)
        .expect("a later workflow version should publish");

    database
        .connect()
        .expect("database should connect")
        .execute(
            "UPDATE scheduled_tasks
             SET next_run_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 minute')
             WHERE id = ?1",
            params![scheduled.id],
        )
        .expect("workflow schedule should become due");
    let claim = database
        .claim_due_scheduled_task()
        .expect("claim should succeed")
        .expect("workflow schedule should be due");
    assert_eq!(claim.target_kind, "workflow");
    assert_eq!(
        claim.workflow_id.as_deref(),
        Some(workflow.summary.id.as_str())
    );
    assert!(claim.workflow_version_id.is_some());
    assert_eq!(claim.prompt, "Resume la actividad de hoy.");

    let workflow_run = database
        .create_workflow_run_from_version(
            &workflow.summary.id,
            claim
                .workflow_version_id
                .as_deref()
                .expect("version should be frozen"),
            &claim.prompt,
        )
        .expect("workflow run should be durable");
    assert_eq!(
        database
            .workflow_run(&workflow_run.run_id)
            .expect("workflow run should load")
            .version_no,
        1,
        "the schedule must keep the version confirmed by the user"
    );
    database
        .start_scheduled_workflow_run(&claim.run_id, &workflow_run.run_id)
        .expect("scheduled run should link to workflow run");
    database
        .update_workflow_run_status(
            &workflow_run.run_id,
            "completed",
            Some(&serde_json::json!({"Resultado": "Informe listo"})),
            None,
        )
        .expect("workflow should complete");
    assert_eq!(
        database
            .reconcile_scheduled_runs()
            .expect("scheduler should reconcile"),
        1
    );
    let reloaded = database
        .list_scheduled_tasks()
        .expect("schedule should reload");
    assert_eq!(reloaded[0].runs[0].status, "completed");
    assert_eq!(
        reloaded[0].runs[0].workflow_run_id.as_deref(),
        Some(workflow_run.run_id.as_str())
    );
    assert_eq!(
        reloaded[0].runs[0]
            .result
            .as_ref()
            .and_then(|value| value.pointer("/outputs/Resultado"))
            .and_then(serde_json::Value::as_str),
        Some("Informe listo")
    );
    cleanup(&database);
}

#[test]
fn scheduled_history_is_filtered_sorted_and_paginated_in_sqlite() {
    let database = test_database();
    let conversation = database
        .create_conversation("Historial extenso", None)
        .expect("conversation should be created");
    let scheduled = database
        .create_scheduled_task(
            "Informe histórico",
            &conversation.id,
            "Resume el periodo.",
            "2099-01-01T10:00:00.000Z",
            "Atlantic/Canary",
            "weekly",
            true,
        )
        .expect("schedule should be created");
    let connection = database.connect().expect("database should connect");
    for index in 0..23 {
        let status = if index % 2 == 0 {
            "completed"
        } else {
            "failed"
        };
        let timestamp = format!("2026-07-{:02}T10:00:00.000Z", index + 1);
        connection
            .execute(
                "INSERT INTO scheduled_runs(
                    id, scheduled_task_id, due_at, claim_key, status, attempt,
                    result_json, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, 1, '{}', ?3, ?3)",
                params![
                    format!("history-run-{index:02}"),
                    scheduled.id,
                    timestamp,
                    format!("history-claim-{index:02}"),
                    status
                ],
            )
            .expect("history row should persist");
    }
    drop(connection);

    let newest_page = database
        .scheduled_run_page(&scheduled.id, "all", "all", "newest", 2, 10)
        .expect("second page should load");
    assert_eq!(newest_page.total, 23);
    assert_eq!(newest_page.page, 2);
    assert_eq!(newest_page.items.len(), 10);
    assert_eq!(newest_page.items[0].id, "history-run-12");
    assert_eq!(newest_page.items[9].id, "history-run-03");

    let oldest_last_page = database
        .scheduled_run_page(&scheduled.id, "all", "all", "oldest", 3, 10)
        .expect("last page should load");
    assert_eq!(oldest_last_page.items.len(), 3);
    assert_eq!(oldest_last_page.items[0].id, "history-run-20");
    assert_eq!(oldest_last_page.items[2].id, "history-run-22");

    let failed = database
        .scheduled_run_page(&scheduled.id, "failed", "all", "newest", 1, 10)
        .expect("failed history should load");
    assert_eq!(failed.total, 11);
    assert!(failed.items.iter().all(|run| run.status == "failed"));
    assert!(matches!(
        database.scheduled_run_page(&scheduled.id, "all", "all", "newest", 1, 11),
        Err(AppError::Validation(_))
    ));
    cleanup(&database);
}

#[test]
fn scheduled_task_is_confirmed_pauseable_and_claimed_exactly_once() {
    let database = test_database();
    let conversation = database
        .create_conversation("Informe programado", None)
        .expect("conversation should be created");
    let scheduled = database
        .create_scheduled_task(
            "Preparar informe",
            &conversation.id,
            "Resume la actividad pendiente.",
            "2099-01-01T10:00:00.000Z",
            "Atlantic/Canary",
            "once",
            true,
        )
        .expect("schedule should be created");
    assert!(scheduled.enabled);
    assert_eq!(scheduled.conversation_id, Some(conversation.id.clone()));
    let updated = database
        .update_scheduled_task(
            &scheduled.id,
            "Preparar informe revisado",
            &conversation.id,
            "Resume la actividad pendiente con tres puntos.",
            "2099-01-02T11:00:00.000Z",
            "Atlantic/Canary",
            "once",
            true,
        )
        .expect("schedule should be editable before running");
    assert_eq!(updated.name, "Preparar informe revisado");
    assert_eq!(
        updated.prompt,
        "Resume la actividad pendiente con tres puntos."
    );

    let paused = database
        .set_scheduled_task_enabled(&scheduled.id, false, false)
        .expect("schedule should pause without confirmation");
    assert!(!paused.enabled);
    let active = database
        .set_scheduled_task_enabled(&scheduled.id, true, true)
        .expect("schedule should reactivate with confirmation");
    assert!(active.enabled);

    database
        .connect()
        .expect("database should connect")
        .execute(
            "UPDATE scheduled_tasks SET next_run_at = '2000-01-01T00:00:00.000Z'
             WHERE id = ?1",
            params![scheduled.id],
        )
        .expect("schedule should become due");
    let claim = database
        .claim_due_scheduled_task()
        .expect("claim should succeed")
        .expect("due schedule should be claimed");
    assert_eq!(claim.conversation_id, Some(conversation.id));
    assert_eq!(
        claim.prompt,
        "Resume la actividad pendiente con tres puntos."
    );
    assert!(database
        .claim_due_scheduled_task()
        .expect("second claim should be safe")
        .is_none());

    let listed = database
        .list_scheduled_tasks()
        .expect("schedule should remain visible");
    assert_eq!(listed[0].runs.len(), 1);
    assert_eq!(listed[0].runs[0].status, "claimed");
    assert!(!listed[0].enabled);
    cleanup(&database);
}

#[test]
fn recurring_schedule_advances_before_it_can_be_claimed_again() {
    let database = test_database();
    let conversation = database
        .create_conversation("Seguimiento diario", None)
        .expect("conversation should be created");
    let scheduled = database
        .create_scheduled_task(
            "Seguimiento",
            &conversation.id,
            "Resume las novedades.",
            "2099-01-01T10:00:00.000Z",
            "Atlantic/Canary",
            "daily",
            true,
        )
        .expect("recurring schedule should be created");
    database
        .connect()
        .expect("database should connect")
        .execute(
            "UPDATE scheduled_tasks
             SET next_run_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 hour')
             WHERE id = ?1",
            params![scheduled.id],
        )
        .expect("schedule should become due");
    database
        .claim_due_scheduled_task()
        .expect("claim should succeed")
        .expect("recurring schedule should be claimed");
    assert!(database
        .claim_due_scheduled_task()
        .expect("second immediate claim should be safe")
        .is_none());
    let listed = database
        .list_scheduled_tasks()
        .expect("schedule should remain visible");
    assert!(listed[0].enabled);
    assert_eq!(listed[0].schedule_expression, "daily");
    assert_eq!(listed[0].runs.len(), 1);
    let next_run_at = listed[0]
        .next_run_at
        .as_deref()
        .expect("recurring schedule should have a next run");
    let is_future: bool = database
        .connect()
        .expect("database should connect")
        .query_row(
            "SELECT datetime(?1) > datetime('now')",
            params![next_run_at],
            |row| row.get(0),
        )
        .expect("next run should be comparable");
    assert!(is_future);
    cleanup(&database);
}

#[test]
fn failed_scheduled_run_can_be_retried_without_losing_history() {
    let database = test_database();
    let conversation = database
        .create_conversation("Informe recuperable", None)
        .expect("conversation should be created");
    let scheduled = database
        .create_scheduled_task(
            "Informe",
            &conversation.id,
            "Prepara el informe.",
            "2099-01-01T10:00:00.000Z",
            "Atlantic/Canary",
            "once",
            true,
        )
        .expect("schedule should be created");
    database
        .connect()
        .expect("database should connect")
        .execute(
            "UPDATE scheduled_tasks SET next_run_at = '2000-01-01T00:00:00.000Z'
             WHERE id = ?1",
            params![scheduled.id],
        )
        .expect("schedule should become due");
    let first = database
        .claim_due_scheduled_task()
        .expect("claim should succeed")
        .expect("due schedule should be claimed");
    database
        .fail_scheduled_run(&first.run_id, "Broker temporalmente no disponible")
        .expect("run should fail");
    assert!(matches!(
        database.retry_failed_scheduled_run(&first.run_id, false),
        Err(AppError::Validation(_))
    ));

    let retry = database
        .retry_failed_scheduled_run(&first.run_id, true)
        .expect("failed run should be retried");
    assert_eq!(retry.scheduled_task_id, scheduled.id);
    assert_ne!(retry.run_id, first.run_id);
    assert!(matches!(
        database.retry_failed_scheduled_run(&first.run_id, true),
        Err(AppError::Conflict(_))
    ));

    let listed = database
        .list_scheduled_tasks()
        .expect("schedule should remain visible");
    assert_eq!(listed[0].runs.len(), 2);
    assert_eq!(listed[0].runs[0].status, "claimed");
    assert_eq!(listed[0].runs[0].attempt, 2);
    assert_eq!(listed[0].runs[1].status, "failed");
    assert_eq!(listed[0].runs[1].attempt, 1);
    cleanup(&database);
}

#[test]
fn running_scheduled_run_can_be_cancelled_without_pausing_recurrence() {
    let database = test_database();
    let conversation = database
        .create_conversation("Seguimiento cancelable", None)
        .expect("conversation should be created");
    let scheduled = database
        .create_scheduled_task(
            "Seguimiento",
            &conversation.id,
            "Resume las novedades.",
            "2099-01-01T10:00:00.000Z",
            "Atlantic/Canary",
            "daily",
            true,
        )
        .expect("recurring schedule should be created");
    let connection = database.connect().expect("database should connect");
    connection
        .execute(
            "UPDATE scheduled_tasks
             SET next_run_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 hour')
             WHERE id = ?1",
            params![scheduled.id],
        )
        .expect("schedule should become due");
    connection
        .execute(
            "INSERT INTO broker_tasks(
                id, idempotency_key, request_json, remote_status, local_state
             ) VALUES ('cancel-local-task', 'cancel-key', '{}', 'generating', 'polling')",
            [],
        )
        .expect("broker task should be stored");
    drop(connection);
    let claim = database
        .claim_due_scheduled_task()
        .expect("claim should succeed")
        .expect("schedule should be claimed");
    database
        .start_scheduled_run(&claim.run_id, "cancel-local-task")
        .expect("scheduled run should start");

    assert!(matches!(
        database.scheduled_cancellation_target(&claim.run_id, false),
        Err(AppError::Validation(_))
    ));
    let target = database
        .scheduled_cancellation_target(&claim.run_id, true)
        .expect("running run should expose its local task");
    assert_eq!(target.broker_task_id.as_deref(), Some("cancel-local-task"));
    database
        .finish_scheduled_cancellation(
            &claim.run_id,
            target
                .broker_task_id
                .as_deref()
                .expect("broker task should exist"),
        )
        .expect("cancellation should be persisted");

    let listed = database
        .list_scheduled_tasks()
        .expect("schedule should remain visible");
    assert!(listed[0].enabled);
    assert_eq!(listed[0].runs[0].status, "cancelled");
    assert!(matches!(
        database.scheduled_cancellation_target(&claim.run_id, true),
        Err(AppError::Conflict(_))
    ));
    let audited: i64 = database
        .connect()
        .expect("database should connect")
        .query_row(
            "SELECT COUNT(*) FROM audit_events
             WHERE event_type = 'scheduled_run.cancelled'",
            [],
            |row| row.get(0),
        )
        .expect("audit should be readable");
    assert_eq!(audited, 1);
    cleanup(&database);
}
