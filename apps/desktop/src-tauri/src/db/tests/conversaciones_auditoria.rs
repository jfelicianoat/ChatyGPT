//! Regresiones de la auditoría del 28-sep-2026: búsqueda (H22) y archivo (H14).

use super::comunes::{cleanup, test_database};
use crate::db::fold_for_search;
use crate::error::AppError;
use serde_json::json;

fn add_message(database: &crate::db::Database, conversation_id: &str, key: &str, text: &str) {
    database
        .prepare_chat_turn(
            conversation_id,
            &format!("user-{key}"),
            &format!("assistant-{key}"),
            &format!("task-{key}"),
            &format!("key-{key}"),
            text,
            &json!({"risk": {"data_classification": "internal"}}),
            &[],
            &[],
            &[],
            &[],
        )
        .expect("mensaje");
}

#[test]
fn search_ignores_case_and_spanish_accents_and_shows_where_it_matched() {
    let database = test_database();
    let tree = database
        .create_conversation("Árbol genealógico", None)
        .expect("chat");
    let other = database
        .create_conversation("Presupuesto", None)
        .expect("chat");
    add_message(
        &database,
        &other.id,
        "a",
        "Hablamos de la PEÑA del pueblo y de su cumpleaños en la montaña.",
    );

    for query in ["Árbol", "árbol", "arbol", "ARBOL"] {
        let hits = database
            .search_conversation_hits(query, 10, false)
            .expect("búsqueda");
        assert_eq!(hits.len(), 1, "consulta {query}");
        assert_eq!(hits[0].id, tree.id);
    }
    for query in ["peña", "PEÑA", "pena", "cumpleanos"] {
        let hits = database
            .search_conversation_hits(query, 10, false)
            .expect("búsqueda");
        assert_eq!(hits.len(), 1, "consulta {query}");
        assert_eq!(hits[0].id, other.id);
        assert!(hits[0].matched_message_id.is_some());
        let snippet = hits[0].snippet.as_deref().unwrap_or_default();
        assert!(snippet.contains("PEÑA"), "{snippet}");
    }
    // Los comodines de LIKE son texto literal.
    assert!(database
        .search_conversation_hits("%", 10, false)
        .expect("búsqueda")
        .is_empty());
    assert_eq!(fold_for_search("ÁÉÍÓÚÜÑ"), "aeiouun");
    cleanup(&database);
}

#[test]
fn archived_conversations_are_listed_searchable_and_restorable() {
    let database = test_database();
    let conversation = database
        .create_conversation("Viaje a Tenerife", None)
        .expect("chat");
    database
        .archive_conversation(&conversation.id)
        .expect("archivar");
    assert!(database
        .search_conversation_hits("tenerife", 10, false)
        .expect("búsqueda")
        .is_empty());
    let with_archived = database
        .search_conversation_hits("tenerife", 10, true)
        .expect("búsqueda");
    assert_eq!(with_archived.len(), 1);
    assert!(with_archived[0].archived);

    let overview = database.archive_overview().expect("archivo");
    assert_eq!(overview.conversations.len(), 1);
    assert!(overview.trash.is_empty());

    database
        .restore_conversation(&conversation.id)
        .expect("restaurar");
    assert!(database
        .list_conversations()
        .expect("lista")
        .iter()
        .any(|item| item.id == conversation.id));
    cleanup(&database);
}

#[test]
fn restoring_a_project_brings_back_its_conversations() {
    let database = test_database();
    let project = database.create_project("Reforma", None).expect("proyecto");
    let kept = database
        .create_conversation("Presupuestos", Some(&project.id))
        .expect("chat");
    let moved = database
        .create_conversation("Permisos", Some(&project.id))
        .expect("chat");
    database.archive_project(&project.id).expect("archivar");
    // Mientras está archivado, la persona recoloca una de las conversaciones.
    let other = database.create_project("Otro", None).expect("proyecto");
    database
        .move_conversation(&moved.id, Some(&other.id))
        .expect("mover");

    let overview = database.archive_overview().expect("archivo");
    assert_eq!(overview.projects.len(), 1);
    assert_eq!(overview.projects[0].conversation_count, 2);

    database.restore_project(&project.id).expect("restaurar");
    let conversations = database.list_conversations().expect("lista");
    let project_of = |id: &str| {
        conversations
            .iter()
            .find(|item| item.id == id)
            .and_then(|item| item.project_id.clone())
    };
    assert_eq!(project_of(&kept.id).as_deref(), Some(project.id.as_str()));
    // Restaurar no deshace lo que la persona decidió después.
    assert_eq!(project_of(&moved.id).as_deref(), Some(other.id.as_str()));
    cleanup(&database);
}

#[test]
fn only_the_trash_can_be_purged_and_it_needs_confirmation() {
    let database = test_database();
    let conversation = database
        .create_conversation("Borrador", None)
        .expect("chat");
    add_message(&database, &conversation.id, "p", "Texto a purgar");
    database
        .record_remote_state(
            "task-p",
            &serde_json::from_value(json!({
                "task_id": "r-p", "status": "completed",
                "created_at": "2026-09-28T10:00:00Z", "updated_at": "2026-09-28T10:00:01Z",
                "progress": {}, "result": {"result_markdown": "Respuesta"}, "error": null
            }))
            .expect("estado"),
        )
        .expect("la tarea termina");
    assert!(matches!(
        database.purge_conversation(&conversation.id, true),
        Err(AppError::Conflict(_))
    ));
    database
        .delete_conversation(&conversation.id)
        .expect("a la papelera");
    assert_eq!(database.archive_overview().expect("archivo").trash.len(), 1);
    assert!(matches!(
        database.purge_conversation(&conversation.id, false),
        Err(AppError::Validation(_))
    ));
    database
        .purge_conversation(&conversation.id, true)
        .expect("purga");
    assert!(database
        .archive_overview()
        .expect("archivo")
        .trash
        .is_empty());
    assert!(database.conversation_view(&conversation.id).is_err());
    cleanup(&database);
}

/// H17: abrir una conversación larga devuelve un lote acotado; las páginas
/// anteriores se piden aparte y enlazan sin huecos ni duplicados.
#[test]
fn a_long_conversation_is_paged_in_rust_not_only_in_the_screen() {
    let database = test_database();
    let conversation = database.create_conversation("Larga", None).expect("chat");
    for index in 0..30 {
        add_message(
            &database,
            &conversation.id,
            &format!("m{index}"),
            &format!("Pregunta {index}"),
        );
        database
            .record_remote_state(
                &format!("task-m{index}"),
                &serde_json::from_value(json!({
                    "task_id": format!("r-{index}"), "status": "completed",
                    "created_at": "2026-09-28T10:00:00Z", "updated_at": "2026-09-28T10:00:01Z",
                    "progress": {}, "result": {"result_markdown": format!("Respuesta {index}")},
                    "error": null
                }))
                .expect("estado"),
            )
            .expect("respuesta");
    }
    let page = database
        .conversation_page(&conversation.id, Some(10))
        .expect("primera página");
    assert_eq!(page.messages.len(), 10);
    assert_eq!(page.total_message_count, 60);
    assert!(page.has_earlier_messages);
    assert_eq!(
        page.messages.last().and_then(|m| m.text.clone()).as_deref(),
        Some("Respuesta 29")
    );

    let mut loaded = page.messages.clone();
    let mut has_earlier = page.has_earlier_messages;
    while has_earlier {
        let before = loaded.first().expect("hay mensajes").sequence_no;
        let earlier = database
            .conversation_messages_before(&conversation.id, before, 25)
            .expect("página anterior");
        assert!(earlier.messages.len() <= 25);
        has_earlier = earlier.has_earlier_messages;
        let mut combined = earlier.messages;
        combined.extend(loaded);
        loaded = combined;
    }
    assert_eq!(loaded.len(), 60);
    let sequences = loaded.iter().map(|m| m.sequence_no).collect::<Vec<_>>();
    let mut sorted = sequences.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sequences, sorted, "orden cronológico sin duplicados");
    cleanup(&database);
}

/// H23: la copia conservada de un fichero generado queda asociada a su tarea y
/// sigue consultable sin preguntar al Broker.
#[test]
fn a_kept_artifact_copy_stays_associated_with_its_task() {
    let database = test_database();
    let copy = database
        .record_task_artifact_copy(
            "remote-art",
            "artifact-1",
            "grafico.png",
            Some("image/png"),
            2_048,
            &"a".repeat(64),
            "C:/datos/adjuntos/aaa/grafico.png",
        )
        .expect("registro");
    assert_eq!(copy.filename, "grafico.png");
    // Guardarlo otra vez actualiza la misma asociación, no duplica.
    database
        .record_task_artifact_copy(
            "remote-art",
            "artifact-1",
            "grafico.png",
            Some("image/png"),
            2_048,
            &"a".repeat(64),
            "C:/datos/adjuntos/aaa/grafico.png",
        )
        .expect("registro repetido");
    let copies = database.task_artifact_copies("remote-art").expect("copias");
    assert_eq!(copies.len(), 1);
    assert!(database
        .task_artifact_copy("remote-art", "otro")
        .expect("consulta")
        .is_none());
    cleanup(&database);
}

/// Hallazgo adicional (28-sep-2026): exportar a una carpeta que un GPT podía
/// leer le quitaba la lectura, y reautorizar una carpeta revocada resucitaba
/// permisos ya retirados. Además, leer no autoriza a escribir.
#[test]
fn folder_permissions_combine_while_active_and_never_resurrect_after_revocation() {
    let database = test_database();
    let folder = std::env::temp_dir().join(format!(
        "chatygpt-carpeta-{}",
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&folder).expect("carpeta");
    let permissions = |database: &crate::db::Database| {
        database
            .list_authorized_folders()
            .expect("carpetas")
            .into_iter()
            .next()
            .expect("una carpeta")
    };

    database
        .authorize_folder_for_modify(&folder, "Documentos")
        .expect("modificar");
    assert!(database
        .write_is_authorized(&folder.join("nota.md"))
        .expect("comprobación"));
    database
        .authorize_folder(&folder, "Documentos", "conversation_markdown")
        .expect("exportar");
    let granted = permissions(&database);
    assert_eq!(
        granted.permissions["read"], true,
        "exportar no quita la lectura"
    );
    assert_eq!(granted.permissions["modify"], true, "ni la modificación");
    assert_eq!(granted.permissions["write"], true);

    database
        .revoke_authorized_folder(&granted.id)
        .expect("revocar");
    database
        .authorize_folder_for_read(&folder, "Documentos")
        .expect("volver a autorizar lectura");
    let renewed = permissions(&database);
    assert_eq!(renewed.permissions["read"], true);
    assert!(
        renewed.permissions.get("modify").is_none(),
        "lo revocado no vuelve"
    );
    assert!(renewed.permissions.get("write").is_none());
    assert!(
        !database
            .write_is_authorized(&folder.join("nota.md"))
            .expect("comprobación"),
        "una carpeta de solo lectura no admite escrituras"
    );
    let _ = std::fs::remove_dir_all(folder);
    cleanup(&database);
}
