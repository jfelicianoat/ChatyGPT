//! Auditoria de decisiones, permisos de Athena y presentacion segura.

use super::comunes::{cleanup, test_database};
use crate::db::{confirmation_blueprint, Database};
use serde_json::Value;

fn tipos_de_auditoria(database: &Database) -> Vec<String> {
    let connection = database.connect().expect("conexión");
    let mut statement = connection
        .prepare("SELECT event_type FROM audit_events ORDER BY id")
        .expect("consulta");
    let tipos = statement
        .query_map([], |row| row.get::<_, String>(0))
        .expect("filas")
        .collect::<Result<Vec<_>, _>>()
        .expect("tipos");
    tipos
}

#[test]
fn cada_decision_sobre_un_permiso_de_athena_queda_registrada() {
    // Conceder y denegar dejan rastros distintos: una auditoría que no
    // distingue las dos cosas no sirve para responder «quién autorizó esto».
    let database = test_database();

    database
        .record_athena_permission_decision(
            "run-1",
            "req-1",
            "write_file",
            "escribir",
            true,
            "aplicada",
        )
        .expect("concesión registrada");
    database
        .record_athena_permission_decision(
            "run-1",
            "req-2",
            "run_command",
            "ejecutar",
            false,
            "aplicada",
        )
        .expect("denegación registrada");

    assert_eq!(
        tipos_de_auditoria(&database),
        vec![
            "athena.permission_granted".to_owned(),
            "athena.permission_denied".to_owned(),
        ]
    );
    cleanup(&database);
}

#[test]
fn una_decision_que_el_servicio_rechaza_tambien_se_audita() {
    // Lo que se registra es que alguien decidió, no que la decisión llegara
    // a tiempo. Perder ese rastro dejaría huecos justo en los casos raros.
    let database = test_database();

    database
        .record_athena_permission_decision(
            "run-1",
            "req-1",
            "write_file",
            "escribir",
            true,
            "caducada",
        )
        .expect("registrada");

    assert_eq!(
        tipos_de_auditoria(&database),
        vec!["athena.permission_rejected_by_service".to_owned()]
    );
    let connection = database.connect().expect("conexión");
    let carga: String = connection
        .query_row("SELECT payload_json FROM audit_events", [], |row| {
            row.get(0)
        })
        .expect("carga");
    let carga: Value = serde_json::from_str(&carga).expect("json");
    assert_eq!(carga["outcome"], "caducada");
    assert_eq!(carga["granted"], true);
    cleanup(&database);
}

#[test]
fn un_run_abierto_sobrevive_al_reinicio_y_se_cierra_una_sola_vez() {
    // ChatyGPT no guarda el estado del agente, solo cómo volver a
    // preguntarle a Athena por él. Y el cierre es idempotente porque la
    // interfaz sondea: si no, cada sondeo dejaría un evento de auditoría.
    let database = test_database();
    database
        .record_athena_run_started("run-1", "Arreglar calc.add", "D:/repo")
        .expect("run anotado");

    let abiertos = database.list_open_athena_runs().expect("lista");
    assert_eq!(abiertos.len(), 1);
    assert_eq!(abiertos[0].run_id, "run-1");
    assert_eq!(abiertos[0].workspace, "D:/repo");
    assert_eq!(abiertos[0].ultima_fase, None);

    database
        .record_athena_run_phase("run-1", "verifying")
        .expect("fase anotada");
    assert_eq!(
        database.list_open_athena_runs().expect("lista")[0].ultima_fase,
        Some("verifying".to_owned())
    );

    database
        .close_athena_run("run-1", "completed")
        .expect("cerrado");
    database
        .close_athena_run("run-1", "completed")
        .expect("cerrado de nuevo");

    assert!(database.list_open_athena_runs().expect("lista").is_empty());
    let cierres = tipos_de_auditoria(&database)
        .into_iter()
        .filter(|tipo| tipo == "athena.run_closed")
        .count();
    assert_eq!(cierres, 1, "el segundo cierre no deja rastro");
    cleanup(&database);
}

#[test]
fn api_confirmation_reveals_the_alias_but_never_treats_it_as_sent_data() {
    let (_, _, disclosure, consequences) = confirmation_blueprint(
        "api_action_private_status",
        &serde_json::json!({
            "url": "https://api.example.org/status",
            "credential_ref": "private_service",
            "auth_mode": "bearer",
            "detail": "summary"
        }),
        Some("conversation"),
    );
    assert_eq!(disclosure["credential_label"], "private_service");
    assert_eq!(disclosure["data_sent"].as_array().unwrap().len(), 1);
    assert_eq!(disclosure["data_sent"][0]["label"], "detail");
    assert!(consequences.contains("private_service"));
    assert!(!disclosure.to_string().contains("bearer"));
}

#[test]
fn audit_inspector_exposes_only_safe_presentation_fields() {
    let database = test_database();
    let conversation = database
        .create_conversation("Auditoría segura", None)
        .expect("conversation should be created");
    let secret_path = r"C:\Users\private\Documents\conversation.md";
    let internal_hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    database
        .record_export(
            &conversation.id,
            "conversation:audit:markdown:v1",
            secret_path,
            internal_hash,
            None,
            Some(internal_hash),
            "completed",
            None,
        )
        .expect("export audit should be recorded");

    let events = database
        .list_audit_events(50)
        .expect("safe audit view should load");
    let serialized = serde_json::to_string(&events).expect("audit view should serialize");
    assert!(!serialized.contains(secret_path));
    assert!(!serialized.contains(internal_hash));
    assert!(events
        .iter()
        .any(|event| event.summary == "Exportación completada"));
    cleanup(&database);
}
