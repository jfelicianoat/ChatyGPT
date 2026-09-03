//! Pruebas de `export`.
//!
//! Viven aparte desde que el fichero paso de mil lineas: separarlas deja
//! la logica a la vista sin cambiar una sola linea de codigo.

use super::{
    atomic_write, export_conversation, export_conversation_to_obsidian, export_scheduled_calendar,
    export_scheduled_history, hash_file, ScheduledCalendarExportEntry,
};
use crate::db::{ContextMessage, Database};
use crate::error::AppError;
use rusqlite::params;
use uuid::Uuid;

/// Concede la carpeta de destino igual que haría el selector nativo.
fn authorize(database: &Database, folder: &std::path::Path) {
    database
        .authorize_folder(folder, &folder.to_string_lossy(), "test")
        .expect("la carpeta de prueba debe autorizarse");
}

#[test]
fn atomic_write_replaces_file_and_hashes_final_bytes() {
    let path = std::env::temp_dir().join(format!("chatygpt-export-{}.md", Uuid::new_v4()));
    std::fs::write(&path, b"old").expect("old export should exist");
    atomic_write(&path, b"new export").expect("atomic replacement should work");
    assert_eq!(
        std::fs::read(&path).expect("export should read"),
        b"new export"
    );
    assert_eq!(hash_file(&path).expect("hash should work").len(), 64);
    let _ = std::fs::remove_file(path);
}

#[test]
fn writing_outside_an_authorized_folder_is_refused_until_it_is_granted() {
    let root = std::env::temp_dir().join(format!("chatygpt-unauthorized-{}", Uuid::new_v4()));
    let outside = root.join("carpeta-ajena");
    std::fs::create_dir_all(&outside).expect("test directory should exist");
    let database = Database::open(root.join("chatygpt.sqlite")).expect("database should open");
    let conversation = database
        .create_conversation("Conversación exportable", None)
        .expect("conversation should be created");
    let destination = outside.join("conversation.md");

    // Sin concesión previa no se escribe nada, ni siquiera un archivo vacío.
    let refused = export_conversation(
        database.clone(),
        &conversation.id,
        &destination.to_string_lossy(),
        false,
    );
    assert!(
        matches!(refused, Err(AppError::Conflict(_))),
        "una carpeta sin autorizar no puede recibir la exportación: {refused:?}"
    );
    assert!(
        !destination.exists(),
        "no debe crearse el archivo de destino"
    );

    // Autorizar la carpeta equivale a haberla elegido en el selector nativo.
    authorize(&database, &outside);
    export_conversation(
        database.clone(),
        &conversation.id,
        &destination.to_string_lossy(),
        false,
    )
    .expect("con la carpeta autorizada la exportación funciona");
    assert!(destination.exists());

    // Revocarla vuelve a cerrar la puerta sin borrar lo ya exportado.
    let granted = database
        .list_authorized_folders()
        .expect("las carpetas deben listarse");
    let folder = granted
        .iter()
        .find(|folder| folder.revoked_at.is_none())
        .expect("debe haber una carpeta vigente");
    database
        .revoke_authorized_folder(&folder.id)
        .expect("la revocación debe funcionar");
    let after_revocation = export_conversation(
        database,
        &conversation.id,
        &destination.to_string_lossy(),
        true,
    );
    assert!(
        matches!(after_revocation, Err(AppError::Conflict(_))),
        "tras revocar, la carpeta deja de admitir escrituras: {after_revocation:?}"
    );
    assert!(destination.exists(), "lo ya exportado no se toca");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn scheduled_history_export_is_filtered_readable_and_verified() {
    let root = std::env::temp_dir().join(format!("chatygpt-scheduler-export-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("test directory should exist");
    let database = Database::open(root.join("chatygpt.sqlite")).expect("database should open");
    authorize(&database, &root);
    let conversation = database
        .create_conversation("Informe semanal", None)
        .expect("conversation should be created");
    let scheduled = database
        .create_scheduled_task(
            "Resumen",
            &conversation.id,
            "Resume las novedades.",
            "2099-01-01T10:00:00.000Z",
            "Atlantic/Canary",
            "once",
            true,
        )
        .expect("schedule should be created");
    rusqlite::Connection::open(database.path())
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
        .expect("schedule should be claimed");
    database
        .fail_scheduled_run(&claim.run_id, "Broker temporalmente no disponible")
        .expect("run should fail");

    let destination = root.join("historial.txt");
    let report = export_scheduled_history(
        database.clone(),
        &destination.to_string_lossy(),
        "failed",
        "all",
        false,
    )
    .expect("history should export");
    assert_eq!(report.run_count, 1);
    assert_eq!(report.destination_hash.len(), 64);
    let text = std::fs::read_to_string(&destination).expect("history should read");
    assert!(text.contains("HISTORIAL DE AUTOMATIZACIONES"));
    assert!(text.contains("Broker temporalmente no disponible"));
    assert!(text.contains("Estado: Fallida"));
    assert!(matches!(
        export_scheduled_history(
            database,
            &destination.to_string_lossy(),
            "failed",
            "all",
            false
        ),
        Err(AppError::Conflict(_))
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn scheduled_calendar_export_is_private_folded_and_verified() {
    let root = std::env::temp_dir().join(format!("chatygpt-calendar-export-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("test directory should exist");
    let database = Database::open(root.join("chatygpt.sqlite")).expect("database should open");
    authorize(&database, &root);
    let destination = root.join("automatizaciones.ics");
    let entries = vec![
        ScheduledCalendarExportEntry {
            occurrence_id: "schedule-1:2026-08-01T10:00:00.000Z".to_owned(),
            task_name: "Informe, diario; equipo".to_owned(),
            conversation_title: "Seguimiento".to_owned(),
            starts_at: "2026-08-01T10:00:00.000Z".to_owned(),
            projected: false,
            overdue: false,
        },
        ScheduledCalendarExportEntry {
            occurrence_id: "schedule-1:2026-08-02T10:00:00.000Z".to_owned(),
            task_name: "Informe diario con un nombre suficientemente largo para plegar la línea del calendario".to_owned(),
            conversation_title: "Seguimiento".to_owned(),
            starts_at: "2026-08-02T10:00:00.000Z".to_owned(),
            projected: true,
            overdue: false,
        },
    ];

    let report = export_scheduled_calendar(
        database.clone(),
        &destination.to_string_lossy(),
        &entries,
        14,
        false,
    )
    .expect("calendar should export");
    assert_eq!(report.event_count, 2);
    assert_eq!(report.destination_hash.len(), 64);
    let bytes = std::fs::read(&destination).expect("calendar should read");
    let text = String::from_utf8(bytes).expect("calendar should be UTF-8");
    assert!(text.starts_with("BEGIN:VCALENDAR\r\n"));
    assert!(text.ends_with("END:VCALENDAR\r\n"));
    assert_eq!(text.matches("BEGIN:VEVENT").count(), 2);
    assert!(text.contains("SUMMARY:Informe\\, diario\\; equipo"));
    assert!(text.contains("X-CHATYGPT-DATE-KIND:DURABLE"));
    assert!(text.contains("X-CHATYGPT-DATE-KIND:PROJECTED"));
    assert!(text.contains("\r\n "));
    assert!(!text.contains("prompt"));
    assert!(!text.contains("schedule-1"));
    assert!(matches!(
        export_scheduled_calendar(
            database,
            &destination.to_string_lossy(),
            &entries,
            14,
            false
        ),
        Err(AppError::Conflict(_))
    ));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn export_detects_external_changes_and_requires_overwrite_confirmation() {
    let root = std::env::temp_dir().join(format!("chatygpt-export-test-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("test directory should exist");
    let database_path = root.join("chatygpt.sqlite");
    let database = Database::open(&database_path).expect("database should open");
    authorize(&database, &root);
    let conversation = database
        .create_conversation("Conversación exportable", None)
        .expect("conversation should be created");
    let context = vec![ContextMessage {
        message_id: "export-user-message".to_owned(),
        role: "user".to_owned(),
        text: "Contenido para exportar".to_owned(),
    }];
    database
        .prepare_chat_turn(
            &conversation.id,
            "export-user-message",
            "export-assistant-message",
            "export-local-task",
            "export-idempotency",
            "Contenido para exportar",
            &serde_json::json!({}),
            &context,
            &[],
            &[],
            &[],
        )
        .expect("turn should be prepared");
    let destination = root.join("conversation.md");
    let report = export_conversation(
        database.clone(),
        &conversation.id,
        &destination.to_string_lossy(),
        false,
    )
    .expect("new destination should export");
    assert!(!report.overwritten);
    let markdown = std::fs::read_to_string(&destination).expect("export should read");
    assert!(markdown.contains("# Contenido para exportar"));
    assert!(markdown.contains("Contenido para exportar"));

    std::fs::write(&destination, "cambio externo").expect("external edit should work");
    assert!(matches!(
        export_conversation(
            database.clone(),
            &conversation.id,
            &destination.to_string_lossy(),
            false,
        ),
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        std::fs::read_to_string(&destination).expect("external edit should survive"),
        "cambio externo"
    );
    let forced = export_conversation(
        database,
        &conversation.id,
        &destination.to_string_lossy(),
        true,
    )
    .expect("confirmed overwrite should work");
    assert!(forced.overwritten);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn obsidian_export_links_project_and_reuses_verified_attachments() {
    let root = std::env::temp_dir().join(format!("chatygpt-obsidian-test-{}", Uuid::new_v4()));
    let vault = root.join("vault");
    std::fs::create_dir_all(&vault).expect("vault should exist");
    let database = Database::open(root.join("chatygpt.sqlite")).expect("database should open");
    authorize(&database, &root);
    let project = database
        .create_project("Análisis de precios", None)
        .expect("project should be created");
    database
        .set_memory_enabled(true)
        .expect("memory should be enabled");
    let conversation = database
        .create_conversation("Informe OHLC", Some(&project.id))
        .expect("conversation should be created");
    database
        .create_memory_item(
            "Usar siempre precios ajustados",
            "project_context",
            "normal",
            Some(&project.id),
        )
        .expect("approved project memory should be created");
    database
        .create_memory_item(
            "Clave privada que no debe exportarse",
            "project_context",
            "sensitive",
            Some(&project.id),
        )
        .expect("sensitive project memory should be created");
    let source = root.join("precios.csv");
    std::fs::write(&source, b"open,high,low,close\n1,2,0,1.5\n").expect("attachment should exist");
    let attachment_id = format!("attachment_{}", Uuid::new_v4().simple());
    let attachment_hash = hash_file(&source).expect("attachment should hash");
    let connection = rusqlite::Connection::open(database.path()).expect("database should connect");
    connection
        .execute(
            "INSERT INTO attachments(
                id, conversation_id, local_path, display_name, media_type,
                size_bytes, sha256, ingestion_status
             ) VALUES (?1, ?2, ?3, 'precios.csv', 'text/csv', ?4, ?5, 'ready')",
            params![
                attachment_id,
                conversation.id,
                source.to_string_lossy(),
                32_i64,
                attachment_hash
            ],
        )
        .expect("attachment should be recorded");
    connection
        .execute(
            "INSERT INTO conversation_attachments(conversation_id, attachment_id)
             VALUES (?1, ?2)",
            params![conversation.id, attachment_id],
        )
        .expect("attachment should be linked");

    let first = export_conversation_to_obsidian(
        database.clone(),
        &conversation.id,
        &vault.to_string_lossy(),
        false,
    )
    .expect("obsidian export should work");
    assert_eq!(first.format, "obsidian");
    assert_eq!(first.attachment_count, 1);
    assert_eq!(first.reused_attachment_count, 0);
    assert!(first.project_index_updated);
    assert_eq!(first.approved_memory_count, 1);
    let note = std::fs::read_to_string(&first.destination_path).expect("note should read");
    assert!(note.contains("type: conversation"));
    assert!(note.contains(&format!("chatygpt_id: \"{}\"", conversation.id)));
    assert!(note.contains("Proyecto: [[../Proyectos/"));
    assert!(note.contains("[[../Adjuntos/"));
    let project_index = std::fs::read_to_string(
        vault
            .join("ChatyGPT")
            .join("Indices")
            .join("Proyectos")
            .join(format!("{}.md", project.id)),
    )
    .expect("project index should read");
    assert!(project_index.contains("Informe OHLC"));
    assert!(project_index.contains("Usar siempre precios ajustados"));
    assert!(!project_index.contains("Clave privada"));
    let memory_index =
        std::fs::read_to_string(vault.join("ChatyGPT").join("Memoria").join("Aprobada.md"))
            .expect("approved memory index should read");
    assert!(memory_index.contains("Usar siempre precios ajustados"));
    assert!(!memory_index.contains("Clave privada"));

    let second = export_conversation_to_obsidian(
        database.clone(),
        &conversation.id,
        &vault.to_string_lossy(),
        false,
    )
    .expect("unchanged export should be idempotent");
    assert_eq!(second.reused_attachment_count, 1);

    let project_index_path = vault
        .join("ChatyGPT")
        .join("Indices")
        .join("Proyectos")
        .join(format!("{}.md", project.id));
    std::fs::write(&project_index_path, "cambio externo en el indice")
        .expect("external project index edit should work");
    assert!(matches!(
        export_conversation_to_obsidian(
            database.clone(),
            &conversation.id,
            &vault.to_string_lossy(),
            false
        ),
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        std::fs::read_to_string(&project_index_path)
            .expect("external project index edit should survive"),
        "cambio externo en el indice"
    );
    export_conversation_to_obsidian(
        database.clone(),
        &conversation.id,
        &vault.to_string_lossy(),
        true,
    )
    .expect("confirmed project index replacement should work");

    let exported_attachment = std::fs::read_dir(vault.join("ChatyGPT").join("Adjuntos"))
        .expect("attachments directory should read")
        .next()
        .expect("one attachment should exist")
        .expect("attachment entry should read")
        .path();
    std::fs::write(&exported_attachment, b"cambio externo").expect("external edit should work");
    assert!(matches!(
        export_conversation_to_obsidian(
            database,
            &conversation.id,
            &vault.to_string_lossy(),
            false
        ),
        Err(AppError::Conflict(_))
    ));
    assert_eq!(
        std::fs::read(&exported_attachment).expect("external edit should survive"),
        b"cambio externo"
    );
    let _ = std::fs::remove_dir_all(root);
}
