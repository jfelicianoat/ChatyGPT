//! Regresiones de la auditoría del 28-sep-2026 sobre lo programado (H05, H06, H15).

use super::comunes::{cleanup, test_database};
use crate::db::Database;
use crate::error::AppError;
use rusqlite::params;

fn schedule(database: &Database, expression: &str) -> (String, String) {
    let conversation = database
        .create_conversation("Programación", None)
        .expect("conversación");
    let scheduled = database
        .create_scheduled_task(
            "Informe",
            &conversation.id,
            "Instrucción original",
            "2099-01-01T10:00:00.000Z",
            "Atlantic/Canary",
            expression,
            true,
        )
        .expect("programación");
    (conversation.id, scheduled.id)
}

fn make_due(database: &Database, scheduled_task_id: &str) {
    database
        .connect()
        .expect("conexión")
        .execute(
            "UPDATE scheduled_tasks
             SET next_run_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 minute')
             WHERE id = ?1",
            params![scheduled_task_id],
        )
        .expect("vencida");
}

fn active_runs(database: &Database, scheduled_task_id: &str) -> i64 {
    database
        .connect()
        .expect("conexión")
        .query_row(
            "SELECT COUNT(*) FROM scheduled_runs
             WHERE scheduled_task_id = ?1 AND status IN ('claimed', 'running')",
            params![scheduled_task_id],
            |row| row.get(0),
        )
        .expect("recuento")
}

/// H06: con una ejecución manual en curso, la fecha recurrente que vence no
/// lanza otra; se omite y el historial explica por qué.
#[test]
fn a_recurring_date_that_falls_during_an_active_run_is_skipped_and_explained() {
    let database = test_database();
    let (_, scheduled_task_id) = schedule(&database, "daily");
    database
        .claim_scheduled_task_now(&scheduled_task_id, true)
        .expect("ejecución manual");
    make_due(&database, &scheduled_task_id);

    assert!(database
        .claim_due_scheduled_task()
        .expect("el reclamo no falla")
        .is_none());
    assert_eq!(active_runs(&database, &scheduled_task_id), 1);

    let task = database
        .list_scheduled_tasks()
        .expect("listado")
        .into_iter()
        .find(|task| task.id == scheduled_task_id)
        .expect("programación visible");
    let skipped = task
        .runs
        .iter()
        .find(|run| run.status == "skipped")
        .expect("la fecha omitida queda en el historial");
    let message = skipped
        .result
        .as_ref()
        .and_then(|value| value.get("message"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    assert!(message.contains("seguía en curso"), "{message}");
    // La recurrencia avanza: no se queda atascada en la fecha omitida.
    let next_is_future: bool = database
        .connect()
        .expect("conexión")
        .query_row(
            "SELECT datetime(next_run_at) > datetime('now') FROM scheduled_tasks WHERE id = ?1",
            params![scheduled_task_id],
            |row| row.get(0),
        )
        .expect("próxima fecha");
    assert!(next_is_future);
    cleanup(&database);
}

/// H06: una ejecución única nunca se pierde; espera a que termine la otra.
#[test]
fn a_one_off_date_waits_for_the_active_run_instead_of_overlapping() {
    let database = test_database();
    let (_, scheduled_task_id) = schedule(&database, "once");
    let manual = database
        .claim_scheduled_task_now(&scheduled_task_id, true)
        .expect("ejecución manual");
    make_due(&database, &scheduled_task_id);

    assert!(database
        .claim_due_scheduled_task()
        .expect("reclamo")
        .is_none());
    assert_eq!(active_runs(&database, &scheduled_task_id), 1);

    database
        .fail_scheduled_run(&manual.run_id, "terminó")
        .expect("la manual termina");
    let claimed = database
        .claim_due_scheduled_task()
        .expect("reclamo")
        .expect("ahora sí se ejecuta la fecha única");
    assert_eq!(claimed.scheduled_task_id, scheduled_task_id);
    cleanup(&database);
}

/// H05: la ejecución recuperada usa lo que se reclamó, no la programación
/// editada después.
#[test]
fn a_recovered_claim_runs_the_instruction_that_was_claimed() {
    let database = test_database();
    let (_, scheduled_task_id) = schedule(&database, "daily");
    make_due(&database, &scheduled_task_id);
    let claimed = database
        .claim_due_scheduled_task()
        .expect("reclamo")
        .expect("reclamada");
    database
        .connect()
        .expect("conexión")
        .execute(
            "UPDATE scheduled_tasks
             SET payload_json = json_set(payload_json, '$.prompt', 'Instrucción editada')
             WHERE id = ?1",
            params![scheduled_task_id],
        )
        .expect("edición posterior");

    let recovered = Database::open(database.path())
        .expect("reapertura")
        .recover_claimed_scheduled_runs()
        .expect("recuperación");
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered[0].run_id, claimed.run_id);
    assert_eq!(recovered[0].prompt, "Instrucción original");
    cleanup(&database);
}

/// H15: retirar una programación detiene futuras ejecuciones y conserva su
/// historial; purgarlo es otra decisión, solo posible tras retirarla.
#[test]
fn retiring_a_schedule_keeps_its_history_and_purging_is_a_separate_decision() {
    let database = test_database();
    let (_, scheduled_task_id) = schedule(&database, "daily");
    let manual = database
        .claim_scheduled_task_now(&scheduled_task_id, true)
        .expect("ejecución");
    database
        .connect()
        .expect("conexión")
        .execute(
            "UPDATE scheduled_runs SET status = 'completed' WHERE id = ?1",
            params![manual.run_id],
        )
        .expect("completada");

    assert!(matches!(
        database.purge_scheduled_task_history(&scheduled_task_id, true),
        Err(AppError::Conflict(_))
    ));
    database
        .delete_scheduled_task(&scheduled_task_id, true)
        .expect("retirada");

    let task = database
        .list_scheduled_tasks()
        .expect("listado")
        .into_iter()
        .find(|task| task.id == scheduled_task_id)
        .expect("la programación retirada sigue consultable");
    assert!(task.retired_at.is_some());
    assert!(!task.enabled);
    assert!(task.next_run_at.is_none());
    assert_eq!(task.runs.len(), 1);
    assert_eq!(task.runs[0].status, "completed");

    make_due(&database, &scheduled_task_id);
    assert!(database
        .claim_due_scheduled_task()
        .expect("reclamo")
        .is_none());
    assert!(database
        .claim_scheduled_task_now(&scheduled_task_id, true)
        .is_err());

    assert!(matches!(
        database.purge_scheduled_task_history(&scheduled_task_id, false),
        Err(AppError::Validation(_))
    ));
    database
        .purge_scheduled_task_history(&scheduled_task_id, true)
        .expect("purga explícita");
    let remaining: i64 = database
        .connect()
        .expect("conexión")
        .query_row("SELECT COUNT(*) FROM scheduled_runs", [], |row| row.get(0))
        .expect("recuento");
    assert_eq!(remaining, 0);
    cleanup(&database);
}
