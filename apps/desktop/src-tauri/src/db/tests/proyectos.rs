//! Proyectos: busqueda, ciclo de vida, instrucciones y ficheros.

use super::comunes::{cleanup, test_database};
use crate::db::ContextMessage;
use crate::error::AppError;
use rusqlite::params;

#[test]
fn projects_search_and_lifecycle_are_audited() {
    let database = test_database();
    let project = database
        .create_project("TFM", Some("Trabajo final"))
        .expect("project should be created");
    let conversation = database
        .create_conversation("Normativa", Some(&project.id))
        .expect("conversation should be created");

    let connection = database.connect().expect("connection should open");
    connection
        .execute(
            "INSERT INTO messages(
                id, conversation_id, role, status, sequence_no
             ) VALUES ('message-search', ?1, 'user', 'complete', 1)",
            params![conversation.id],
        )
        .expect("message should be inserted");
    connection
        .execute(
            "INSERT INTO message_parts(
                id, message_id, ordinal, kind, content_text
             ) VALUES (
                'part-search', 'message-search', 0, 'text',
                'consulta sobre contratación pública'
             )",
            [],
        )
        .expect("message part should be inserted");
    drop(connection);

    let results = database
        .search_conversations("contratación", 10)
        .expect("search should succeed");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, conversation.id);
    assert!(database
        .search_conversations("%", 10)
        .expect("wildcard should be treated literally")
        .is_empty());

    database
        .rename_conversation(&conversation.id, "Normativa española")
        .expect("rename should succeed");
    database
        .archive_project(&project.id)
        .expect("archive should succeed");

    let conversation_after = database
        .conversation_summary(&conversation.id)
        .expect("conversation should remain");
    assert!(conversation_after.project_id.is_none());
    assert!(database
        .list_projects()
        .expect("projects should list")
        .is_empty());

    let connection = database.connect().expect("connection should open");
    let audited: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM audit_events
             WHERE event_type IN (
                'project.created', 'conversation.created',
                'conversation.renamed', 'project.archived'
             )",
            [],
            |row| row.get(0),
        )
        .expect("audit count should load");
    assert_eq!(audited, 4);
    drop(connection);
    cleanup(&database);
}

#[test]
fn project_instructions_are_scoped_and_visible_in_the_exact_task_context() {
    let database = test_database();
    let project = database
        .create_project("Investigación", None)
        .expect("project should be created");
    let other_project = database
        .create_project("Otro", None)
        .expect("other project should be created");
    let conversation = database
        .create_conversation("Chat del proyecto", Some(&project.id))
        .expect("conversation should be created");
    let other_conversation = database
        .create_conversation("Chat aislado", Some(&other_project.id))
        .expect("other conversation should be created");

    let updated = database
        .update_project_instructions(
            &project.id,
            Some("Distingue hechos de hipótesis y cita las fuentes."),
        )
        .expect("instructions should persist");
    assert_eq!(
        updated.instructions.as_deref(),
        Some("Distingue hechos de hipótesis y cita las fuentes.")
    );
    let instruction = database
        .project_instruction_for_conversation(&conversation.id)
        .expect("instruction lookup should succeed")
        .expect("project instruction should be available");
    assert!(database
        .project_instruction_for_conversation(&other_conversation.id)
        .expect("isolated lookup should succeed")
        .is_none());

    let context = vec![ContextMessage {
        message_id: "project-instruction-user".to_owned(),
        role: "user".to_owned(),
        text: "Analiza el resultado".to_owned(),
    }];
    database
        .prepare_chat_turn_with_project_instruction(
            &conversation.id,
            "project-instruction-user",
            "project-instruction-assistant",
            "project-instruction-task",
            "project-instruction-key",
            "Analiza el resultado",
            &serde_json::json!({"inference_kind": "chat"}),
            &context,
            Some(&instruction),
            None,
            &[],
            &[],
            &[],
        )
        .expect("turn should retain the project instruction");
    let visible = database
        .task_context("project-instruction-task")
        .expect("task context should load");
    assert!(visible.strategy.contains("instrucciones del proyecto"));
    assert!(visible.sources.iter().any(|source| {
        source.kind == "project_instruction"
            && source.label == "Instrucciones del proyecto"
            && source.excerpt.contains("Distingue hechos")
    }));

    database
        .update_project_instructions(&project.id, None)
        .expect("instructions should be removable");
    assert!(database
        .project_instruction_for_conversation(&conversation.id)
        .expect("cleared lookup should succeed")
        .is_none());
    cleanup(&database);
}

#[test]
fn project_file_can_be_reused_without_leaking_into_another_project() {
    let database = test_database();
    let project = database
        .create_project("Proyecto compartido", None)
        .expect("project should be created");
    let other_project = database
        .create_project("Proyecto aislado", None)
        .expect("other project should be created");
    let source_conversation = database
        .create_conversation("Origen", Some(&project.id))
        .expect("source conversation should be created");
    let target_conversation = database
        .create_conversation("Destino", Some(&project.id))
        .expect("target conversation should be created");
    let isolated_conversation = database
        .create_conversation("Aislada", Some(&other_project.id))
        .expect("isolated conversation should be created");
    let attachment = database
        .register_attachment(
            &source_conversation.id,
            "C:/managed/project.pdf",
            "project.pdf",
            Some("application/pdf"),
            42,
            "project-file-sha",
        )
        .expect("attachment should be registered");

    database
        .set_project_file(&source_conversation.id, &attachment.id, true)
        .expect("attachment should be saved to the project");
    assert_eq!(
        database
            .list_project_files(&target_conversation.id)
            .expect("project files should list")[0]
            .id,
        attachment.id
    );
    database
        .use_project_file(&target_conversation.id, &attachment.id)
        .expect("project file should link to the target conversation");
    assert_eq!(
        database
            .list_attachments(&target_conversation.id)
            .expect("target attachments should list")
            .len(),
        1
    );
    assert!(matches!(
        database.use_project_file(&isolated_conversation.id, &attachment.id),
        Err(AppError::NotFound(_))
    ));

    database
        .set_project_file(&source_conversation.id, &attachment.id, false)
        .expect("project association should be removable");
    assert!(database
        .list_project_files(&target_conversation.id)
        .expect("project files should list")
        .is_empty());
    assert_eq!(
        database
            .list_attachments(&target_conversation.id)
            .expect("existing conversation link should remain")
            .len(),
        1
    );
    cleanup(&database);
}

#[test]
fn project_knowledge_overview_composes_only_the_selected_project_sources() {
    let database = test_database();
    let project = database
        .create_project("Proyecto visible", None)
        .expect("project should be created");
    let other_project = database
        .create_project("Proyecto ajeno", None)
        .expect("other project should be created");
    let conversation = database
        .create_conversation("Chat visible", Some(&project.id))
        .expect("conversation should be created");
    let attachment = database
        .register_attachment(
            &conversation.id,
            "C:/managed/visible.pdf",
            "visible.pdf",
            Some("application/pdf"),
            99,
            "visible-project-file",
        )
        .expect("attachment should be registered");
    database
        .set_project_file(&conversation.id, &attachment.id, true)
        .expect("file should be saved to the project");
    let second_conversation = database
        .create_conversation("Segundo chat visible", Some(&project.id))
        .expect("second conversation should be created");
    database
        .use_project_file(&second_conversation.id, &attachment.id)
        .expect("project file should be used by the second conversation");
    database
        .update_project_instructions(&project.id, Some("Cita siempre las fuentes."))
        .expect("instructions should persist");
    let (memory_id, _) = database
        .create_memory_item(
            "La fecha de corte es mensual.",
            "fact",
            "normal",
            Some(&project.id),
        )
        .expect("project memory should be created");
    let (other_memory_id, _) = database
        .create_memory_item(
            "Este recuerdo pertenece a otro proyecto.",
            "fact",
            "normal",
            Some(&other_project.id),
        )
        .expect("other memory should be created");

    let overview = database
        .project_knowledge_overview(&project.id)
        .expect("overview should load");
    assert_eq!(overview.project.id, project.id);
    assert_eq!(
        overview.project.instructions.as_deref(),
        Some("Cita siempre las fuentes.")
    );
    assert_eq!(overview.files.len(), 1);
    assert_eq!(overview.files[0].display_name, "visible.pdf");
    assert_eq!(overview.file_usages.len(), 1);
    assert_eq!(overview.file_usages[0].attachment_id, attachment.id);
    assert_eq!(overview.file_usages[0].conversations.len(), 2);
    assert!(overview.file_usages[0]
        .conversations
        .iter()
        .all(|item| item.project_id.as_deref() == Some(project.id.as_str())));
    assert!(overview.file_usages[0]
        .conversations
        .iter()
        .any(|item| item.id == conversation.id && item.title == "Chat visible"));
    assert!(overview.file_usages[0]
        .conversations
        .iter()
        .any(|item| item.id == second_conversation.id && item.title == "Segundo chat visible"));
    assert_eq!(overview.memories.len(), 1);
    assert_eq!(
        overview.memories[0].content,
        "La fecha de corte es mensual."
    );

    let toggled = database
        .set_project_memory_item_enabled(&project.id, &memory_id, false)
        .expect("project memory should be disabled from the overview");
    assert!(!toggled.memories[0].enabled);
    assert!(matches!(
        database.set_project_memory_item_enabled(&project.id, &other_memory_id, false),
        Err(AppError::NotFound(_))
    ));

    let without_file = database
        .remove_project_file(&project.id, &attachment.id)
        .expect("project file should be removable from the overview");
    assert!(without_file.files.is_empty());
    assert!(without_file.file_usages.is_empty());
    assert_eq!(
        database
            .list_attachments(&conversation.id)
            .expect("conversation attachment should remain")
            .len(),
        1
    );
    assert_eq!(
        database
            .list_attachments(&second_conversation.id)
            .expect("second conversation attachment should remain")
            .len(),
        1
    );
    cleanup(&database);
}
