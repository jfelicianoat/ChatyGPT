//! GPTs personalizados: versiones, iconos, portabilidad y conocimiento.

use super::comunes::{cleanup, test_database};
use crate::db::{
    validated_preferred_model, ContextMessage, ConversationExecutionPreferences,
    CustomGptToolPermissions,
};
use crate::error::AppError;
use rusqlite::params;
use serde_json::Value;

#[test]
fn custom_gpt_edits_create_immutable_versions_without_tool_permissions() {
    let database = test_database();
    assert!(matches!(
        database.create_custom_gpt("", None, "Responde con claridad."),
        Err(AppError::Validation(_))
    ));
    assert!(matches!(
        database.create_custom_gpt("Ayudante", None, "   "),
        Err(AppError::Validation(_))
    ));

    let created = database
        .create_custom_gpt(
            "Ayudante de estudio",
            Some("Explica conceptos técnicos"),
            "Responde paso a paso y define cada término.",
        )
        .expect("custom GPT should be created");
    assert_eq!(created.version_no, 1);
    assert_eq!(
        created.instructions,
        "Responde paso a paso y define cada término."
    );

    let updated = database
        .update_custom_gpt(
            &created.id,
            "Tutor de estudio",
            Some("Explica y comprueba la comprensión"),
            "Primero explica; después formula una pregunta de comprobación.",
        )
        .expect("custom GPT should create a new version");
    assert_eq!(updated.version_no, 2);
    assert_eq!(updated.name, "Tutor de estudio");
    assert_eq!(
        updated.instructions,
        "Primero explica; después formula una pregunta de comprobación."
    );
    let listed = database
        .list_custom_gpts()
        .expect("custom GPTs should list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].version_no, 2);

    let connection = database.connect().expect("connection should open");
    let versions: Vec<(i64, String)> = {
        let mut statement = connection
            .prepare(
                "SELECT version_no, configuration_json
                 FROM gpt_versions
                 WHERE custom_gpt_id = ?1
                 ORDER BY version_no",
            )
            .expect("version query should prepare");
        statement
            .query_map(params![created.id], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("versions should query")
            .collect::<Result<Vec<_>, _>>()
            .expect("versions should collect")
    };
    assert_eq!(versions.len(), 2);
    let first_configuration: Value =
        serde_json::from_str(&versions[0].1).expect("first configuration should be JSON");
    assert_eq!(
        first_configuration["instructions"],
        "Responde paso a paso y define cada término."
    );
    assert_eq!(first_configuration["toolsEnabled"], false);
    let (permission_count, non_denied_count): (i64, i64) = connection
        .query_row(
            "SELECT COUNT(*), SUM(effect != 'deny') FROM gpt_tool_permissions",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("permission matrix should load");
    assert_eq!(permission_count, 12);
    assert_eq!(non_denied_count, 0);
    let feature_enabled: bool = connection
        .query_row(
            "SELECT enabled FROM feature_flags WHERE key = 'custom_gpts'",
            [],
            |row| row.get(0),
        )
        .expect("feature flag should load");
    assert!(feature_enabled);
    let audited: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM audit_events
             WHERE event_type IN ('custom_gpt.created', 'custom_gpt.version_created')",
            [],
            |row| row.get(0),
        )
        .expect("audit count should load");
    assert_eq!(audited, 2);
    drop(connection);
    cleanup(&database);
}

#[test]
fn custom_gpt_execution_profile_is_optional_versioned_and_restorable() {
    let database = test_database();
    let profile = ConversationExecutionPreferences {
        data_classification: "confidential".to_owned(),
        strategy: "mixture_of_agents".to_owned(),
        preset: "slow".to_owned(),
        max_cost_usd: 0.75,
        long_context: "fail".to_owned(),
        priority: 50,
    };
    let created = database
        .create_custom_gpt_with_starters(
            "Analista versionado",
            None,
            "Contrasta varias perspectivas.",
            &[],
            &CustomGptToolPermissions::default(),
            None,
            None,
            Some(&profile),
        )
        .expect("profile should be accepted");
    let created_profile = created
        .execution_profile
        .as_ref()
        .expect("the active version should expose its profile");
    assert_eq!(created_profile.strategy, "mixture_of_agents");
    assert_eq!(created_profile.preset, "slow");
    assert_eq!(created_profile.data_classification, "confidential");

    let inherited = database
        .update_custom_gpt_with_starters(
            &created.id,
            "Analista versionado",
            None,
            "Ahora hereda los ajustes del chat.",
            &[],
            &CustomGptToolPermissions::default(),
            None,
            None,
            None,
        )
        .expect("profile can be disabled in a new version");
    assert!(inherited.execution_profile.is_none());

    let history = database
        .list_custom_gpt_versions(&created.id)
        .expect("history should preserve both profiles");
    assert!(history[0].execution_profile.is_none());
    assert_eq!(
        history[1]
            .execution_profile
            .as_ref()
            .map(|value| value.max_cost_usd),
        Some(0.75)
    );
    let restored = database
        .restore_custom_gpt_version(&created.id, &history[1].id, true)
        .expect("old profile should restore as a new version");
    assert_eq!(
        restored
            .execution_profile
            .as_ref()
            .map(|value| value.priority),
        Some(50)
    );

    let invalid = ConversationExecutionPreferences {
        priority: 1001,
        ..ConversationExecutionPreferences::default()
    };
    assert!(matches!(
        database.create_custom_gpt_with_starters(
            "Perfil inválido",
            None,
            "No debe guardarse.",
            &[],
            &CustomGptToolPermissions::default(),
            None,
            None,
            Some(&invalid),
        ),
        Err(AppError::Validation(_))
    ));
    cleanup(&database);
}

#[test]
fn custom_gpt_starters_and_portable_json_round_trip_safely() {
    let database = test_database();
    let permissions = CustomGptToolPermissions {
        run_code: "confirm".to_owned(),
        rename_conversation: "deny".to_owned(),
        read_authorized_folders: "deny".to_owned(),
        modify_authorized_files: "deny".to_owned(),
        create_scheduled_tasks: "deny".to_owned(),
        call_external_apis: "deny".to_owned(),
    };
    let created = database
        .create_custom_gpt_with_starters(
            "Tutor portable",
            Some("Ayuda a estudiar"),
            "Explica con ejemplos.",
            &[
                " Explícame el tema paso a paso ".to_owned(),
                "Hazme cinco preguntas".to_owned(),
                "hazme cinco preguntas".to_owned(),
            ],
            &permissions,
            Some("qwen2.5:14b"),
            None,
            None,
        )
        .expect("custom GPT with starters should be created");
    assert_eq!(
        created.conversation_starters,
        vec![
            "Explícame el tema paso a paso".to_owned(),
            "Hazme cinco preguntas".to_owned()
        ]
    );
    assert_eq!(created.tool_permissions.run_code, "confirm");
    assert_eq!(created.tool_permissions.rename_conversation, "deny");

    let exported = database
        .export_custom_gpt_json(&created.id)
        .expect("custom GPT should export");
    let portable: Value = serde_json::from_str(&exported).expect("export should be JSON");
    assert_eq!(portable["schemaVersion"], 1);
    assert_eq!(
        portable["conversationStarters"].as_array().unwrap().len(),
        2
    );
    assert!(portable.get("id").is_none());
    assert!(portable.get("toolsEnabled").is_none());
    assert!(portable.get("toolPermissions").is_none());

    let imported = database
        .import_custom_gpt_json(&exported)
        .expect("portable GPT should import");
    assert_ne!(imported.id, created.id);
    assert_eq!(imported.name, created.name);
    assert_eq!(
        imported.conversation_starters,
        created.conversation_starters
    );
    assert_eq!(imported.tool_permissions.run_code, "deny");
    assert_eq!(imported.tool_permissions.rename_conversation, "deny");
    assert!(matches!(
        database.import_custom_gpt_json(
            r#"{"schemaVersion":1,"name":"X","instructions":"Y","unexpected":true}"#
        ),
        Err(AppError::Validation(_))
    ));
    assert!(matches!(
        database.create_custom_gpt_with_starters(
            "Demasiados",
            None,
            "Instrucciones",
            &vec!["Inicio".to_owned(); 7],
            &CustomGptToolPermissions::default(),
            None,
            None,
            None
        ),
        Err(AppError::Validation(_))
    ));
    cleanup(&database);
}

#[test]
fn custom_gpt_history_restores_a_previous_version_without_losing_any() {
    let database = test_database();
    let created = database
        .create_custom_gpt_with_starters(
            "Revisor",
            Some("Revisa textos"),
            "Versión uno de las instrucciones.",
            &["Revisa este texto".to_owned()],
            &CustomGptToolPermissions {
                run_code: "deny".to_owned(),
                rename_conversation: "confirm".to_owned(),
                read_authorized_folders: "deny".to_owned(),
                modify_authorized_files: "deny".to_owned(),
                create_scheduled_tasks: "deny".to_owned(),
                call_external_apis: "deny".to_owned(),
            },
            Some("qwen2.5:14b"),
            None,
            None,
        )
        .expect("el GPT debe crearse");
    assert_eq!(created.preferred_model.as_deref(), Some("qwen2.5:14b"));

    database
        .update_custom_gpt_with_starters(
            &created.id,
            "Revisor",
            Some("Revisa textos"),
            "Versión dos de las instrucciones.",
            &[],
            &CustomGptToolPermissions::default(),
            None,
            None,
            None,
        )
        .expect("la edición debe crear otra versión");

    let history = database
        .list_custom_gpt_versions(&created.id)
        .expect("el historial debe cargarse");
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].version_no, 2);
    assert!(history[0].active, "la más reciente es la activa");
    assert!(!history[1].active);
    assert_eq!(history[1].instructions, "Versión uno de las instrucciones.");
    assert_eq!(history[1].preferred_model.as_deref(), Some("qwen2.5:14b"));
    assert_eq!(history[1].tool_permissions.rename_conversation, "confirm");

    // Restaurar exige confirmación explícita.
    assert!(matches!(
        database.restore_custom_gpt_version(&created.id, &history[1].id, false),
        Err(AppError::Validation(_))
    ));
    // Y no tiene sentido restaurar la que ya está activa.
    assert!(matches!(
        database.restore_custom_gpt_version(&created.id, &history[0].id, true),
        Err(AppError::Conflict(_))
    ));

    let restored = database
        .restore_custom_gpt_version(&created.id, &history[1].id, true)
        .expect("la restauración debe funcionar");
    assert_eq!(restored.version_no, 3, "restaurar crea una versión nueva");
    assert_eq!(restored.instructions, "Versión uno de las instrucciones.");
    assert_eq!(restored.preferred_model.as_deref(), Some("qwen2.5:14b"));
    assert_eq!(
        restored.tool_permissions.rename_conversation, "confirm",
        "los permisos de la versión restaurada la acompañan"
    );

    let history = database
        .list_custom_gpt_versions(&created.id)
        .expect("el historial debe recargarse");
    assert_eq!(history.len(), 3, "no se borra ninguna revisión");
    assert_eq!(
        history.iter().filter(|version| version.active).count(),
        1,
        "solo puede haber una versión activa"
    );
    cleanup(&database);
}

#[test]
fn duplicating_a_custom_gpt_never_carries_permissions_or_knowledge() {
    let database = test_database();
    let source = database
        .create_custom_gpt_with_starters(
            "Asistente con permisos",
            None,
            "Instrucciones originales.",
            &["Empieza aquí".to_owned()],
            &CustomGptToolPermissions {
                run_code: "confirm".to_owned(),
                rename_conversation: "confirm".to_owned(),
                read_authorized_folders: "deny".to_owned(),
                modify_authorized_files: "deny".to_owned(),
                create_scheduled_tasks: "deny".to_owned(),
                call_external_apis: "deny".to_owned(),
            },
            Some("qwen2.5:14b"),
            None,
            None,
        )
        .expect("el GPT origen debe crearse");
    database
        .create_custom_gpt_memory_item(
            &source.id,
            "Dato reservado del asistente",
            "fact",
            "sensitive",
        )
        .expect("el conocimiento debe guardarse");

    let copy = database
        .duplicate_custom_gpt(&source.id, None)
        .expect("la duplicación debe funcionar");

    assert_ne!(copy.id, source.id);
    assert_eq!(copy.name, "Asistente con permisos (copia)");
    assert_eq!(copy.instructions, source.instructions);
    assert_eq!(copy.conversation_starters, source.conversation_starters);
    assert_eq!(copy.preferred_model.as_deref(), Some("qwen2.5:14b"));
    assert_eq!(copy.version_no, 1, "la copia empieza su propio historial");
    assert_eq!(
        copy.tool_permissions.run_code, "deny",
        "un duplicado nunca hereda permisos concedidos"
    );
    assert_eq!(copy.tool_permissions.rename_conversation, "deny");
    assert!(
        database
            .custom_gpt_knowledge(&copy.id)
            .expect("el conocimiento de la copia debe consultarse")
            .is_empty(),
        "el conocimiento no se copia con el asistente"
    );
    // El original permanece intacto.
    let originals = database
        .custom_gpt_knowledge(&source.id)
        .expect("el conocimiento original debe seguir ahí");
    assert_eq!(originals.len(), 1);
    cleanup(&database);
}

#[test]
fn custom_gpt_icon_is_validated_versioned_portable_and_duplicated() {
    let database = test_database();
    let created = database
        .create_custom_gpt_with_icon(
            "Research helper",
            None,
            Some("research"),
            "Investigate carefully.",
            &[],
            &CustomGptToolPermissions::default(),
            None,
            None,
            None,
            None,
        )
        .expect("a catalog icon should be accepted");
    assert_eq!(created.icon_ref, "research");

    let updated = database
        .update_custom_gpt_with_icon(
            &created.id,
            &created.name,
            None,
            Some("code"),
            "Build and verify the solution.",
            &[],
            &CustomGptToolPermissions::default(),
            None,
            None,
            None,
            None,
        )
        .expect("changing the icon should create a version");
    assert_eq!(updated.icon_ref, "code");
    assert_eq!(updated.version_no, 2);

    let history = database
        .list_custom_gpt_versions(&created.id)
        .expect("icon history should load");
    assert_eq!(history[0].icon_ref, "code");
    assert_eq!(history[1].icon_ref, "research");

    let exported = database
        .export_custom_gpt_json(&created.id)
        .expect("icon should export");
    let portable: Value = serde_json::from_str(&exported).expect("export should be JSON");
    assert_eq!(portable["iconRef"], "code");
    let imported = database
        .import_custom_gpt_json(&exported)
        .expect("icon should import");
    assert_eq!(imported.icon_ref, "code");

    let duplicate = database
        .duplicate_custom_gpt(&created.id, Some("Research helper copy"))
        .expect("duplicate should retain presentation");
    assert_eq!(duplicate.icon_ref, "code");

    let restored = database
        .restore_custom_gpt_version(&created.id, &history[1].id, true)
        .expect("restoring should retain the historical icon");
    assert_eq!(restored.icon_ref, "research");

    assert!(matches!(
        database.create_custom_gpt_with_icon(
            "Invalid icon",
            None,
            Some("../../icon.png"),
            "Do not save this.",
            &[],
            &CustomGptToolPermissions::default(),
            None,
            None,
            None,
            None,
        ),
        Err(AppError::Validation(_))
    ));
    assert!(matches!(
        database.import_custom_gpt_json(
            r#"{"schemaVersion":1,"name":"Invalid icon","iconRef":"../../icon.png","instructions":"Do not save this."}"#
        ),
        Err(AppError::Validation(_))
    ));
    cleanup(&database);
}

#[test]
fn custom_gpt_context_profile_is_versioned_portable_and_validated() {
    let database = test_database();
    let created = database
        .create_custom_gpt_with_icon(
            "Documental",
            None,
            Some("research"),
            "Responde desde el contexto.",
            &[],
            &CustomGptToolPermissions::default(),
            None,
            None,
            None,
            Some("broad"),
        )
        .expect("a broad profile should be accepted");
    assert_eq!(created.context_profile, "broad");

    let updated = database
        .update_custom_gpt_with_icon(
            &created.id,
            &created.name,
            None,
            Some("research"),
            &created.instructions,
            &[],
            &CustomGptToolPermissions::default(),
            None,
            None,
            None,
            Some("focused"),
        )
        .expect("the context profile should create a new version");
    assert_eq!(updated.context_profile, "focused");
    let history = database.list_custom_gpt_versions(&created.id).unwrap();
    assert_eq!(history[0].context_profile, "focused");
    assert_eq!(history[1].context_profile, "broad");

    let exported = database.export_custom_gpt_json(&created.id).unwrap();
    let imported = database.import_custom_gpt_json(&exported).unwrap();
    assert_eq!(imported.context_profile, "focused");
    assert!(matches!(
        database.create_custom_gpt_with_icon(
            "Inválido",
            None,
            None,
            "No importa.",
            &[],
            &CustomGptToolPermissions::default(),
            None,
            None,
            None,
            Some("unlimited"),
        ),
        Err(AppError::Validation(_))
    ));
    cleanup(&database);
}

#[test]
fn preferred_model_is_validated_against_the_broker_limit() {
    assert_eq!(
        validated_preferred_model(Some("  qwen2.5:14b  ")).expect("debe normalizarse"),
        Some("qwen2.5:14b".to_owned())
    );
    assert_eq!(
        validated_preferred_model(Some("   ")).expect("vacío es ninguno"),
        None
    );
    assert_eq!(validated_preferred_model(None).expect("sin valor"), None);
    assert!(matches!(
        validated_preferred_model(Some(&"a".repeat(129))),
        Err(AppError::Validation(_))
    ));
    assert!(matches!(
        validated_preferred_model(Some("modelo con espacios")),
        Err(AppError::Validation(_))
    ));
}

#[test]
fn custom_gpt_portable_knowledge_is_explicit_filtered_and_quarantined() {
    let database = test_database();
    let created = database
        .create_custom_gpt(
            "Analista portable",
            Some("Conocimiento transferible"),
            "Responde solo con datos revisados.",
        )
        .expect("custom GPT should be created");
    let (portable_id, _) = database
        .create_custom_gpt_memory_item(&created.id, "La versión estable es la 3.", "fact", "normal")
        .expect("portable knowledge should be created");
    database
        .create_custom_gpt_memory_item(
            &created.id,
            "Clave que nunca debe viajar.",
            "instruction",
            "sensitive",
        )
        .expect("sensitive knowledge should be created");
    let (disabled_id, _) = database
        .create_custom_gpt_memory_item(
            &created.id,
            "Borrador todavía sin revisar.",
            "preference",
            "normal",
        )
        .expect("draft knowledge should be created");
    database
        .set_custom_gpt_memory_item_enabled(&created.id, &disabled_id, false)
        .expect("draft knowledge should be disabled");
    database
        .register_custom_gpt_attachment(
            &created.id,
            "C:\\managed\\manual.pdf",
            "manual.pdf",
            Some("application/pdf"),
            42,
            "portable-knowledge-file-hash",
        )
        .expect("private file should be linked");

    let configuration_only = database
        .export_custom_gpt_portable(&created.id, false)
        .expect("configuration should export");
    assert_eq!(configuration_only.included_knowledge, 0);
    let configuration_json: Value =
        serde_json::from_str(&configuration_only.json).expect("export should be JSON");
    assert_eq!(configuration_json["schemaVersion"], 1);
    assert!(configuration_json.get("knowledge").is_none());

    let package = database
        .export_custom_gpt_portable(&created.id, true)
        .expect("knowledge package should export");
    assert_eq!(package.included_knowledge, 1);
    assert_eq!(package.excluded_sensitive, 1);
    assert_eq!(package.excluded_disabled, 1);
    assert_eq!(package.excluded_files, 1);
    assert!(!package.json.contains("Clave que nunca debe viajar."));
    assert!(!package.json.contains("Borrador todavía sin revisar."));
    assert!(!package.json.contains("manual.pdf"));
    assert!(!package.json.contains(&portable_id));
    let package_json: Value = serde_json::from_str(&package.json).expect("package should be JSON");
    assert_eq!(package_json["schemaVersion"], 2);
    assert_eq!(package_json["knowledge"].as_array().unwrap().len(), 1);
    assert!(package_json.get("toolPermissions").is_none());

    let imported = database
        .import_custom_gpt_package_json(&package.json)
        .expect("knowledge package should import");
    assert_eq!(imported.imported_knowledge, 1);
    assert!(imported.knowledge_requires_review);
    assert_eq!(imported.custom_gpt.tool_permissions.run_code, "deny");
    assert_eq!(
        imported.custom_gpt.tool_permissions.rename_conversation,
        "deny"
    );
    let imported_knowledge = database
        .custom_gpt_knowledge(&imported.custom_gpt.id)
        .expect("imported knowledge should load");
    assert_eq!(imported_knowledge.len(), 1);
    assert_eq!(imported_knowledge[0].content, "La versión estable es la 3.");
    assert!(!imported_knowledge[0].enabled);
    assert_eq!(imported_knowledge[0].sensitivity, "normal");
    assert!(database
        .list_custom_gpt_files(&imported.custom_gpt.id)
        .expect("imported files should load")
        .is_empty());
    assert!(matches!(
        database.import_custom_gpt_package_json(
            r#"{"schemaVersion":1,"name":"X","instructions":"Y","knowledge":[{"category":"fact","content":"No permitido"}]}"#
        ),
        Err(AppError::Validation(_))
    ));
    cleanup(&database);
}

#[test]
fn custom_gpt_knowledge_is_private_and_independent_from_global_memory() {
    let database = test_database();
    let gpt_a = database
        .create_custom_gpt("GPT Alfa", None, "Usa conocimiento Alfa.")
        .expect("first custom GPT should exist");
    let gpt_b = database
        .create_custom_gpt("GPT Beta", None, "Usa conocimiento Beta.")
        .expect("second custom GPT should exist");
    let conversation_a = database
        .create_conversation("Chat Alfa", None)
        .expect("first conversation should exist");
    let conversation_b = database
        .create_conversation("Chat Beta", None)
        .expect("second conversation should exist");
    database
        .set_conversation_custom_gpt(&conversation_a.id, Some(&gpt_a.id))
        .expect("first GPT should be selected");
    database
        .set_conversation_custom_gpt(&conversation_b.id, Some(&gpt_b.id))
        .expect("second GPT should be selected");
    let (memory_a_id, _) = database
        .create_custom_gpt_memory_item(&gpt_a.id, "Dato exclusivo de Alfa", "fact", "normal")
        .expect("first GPT knowledge should be created");
    database
        .create_custom_gpt_memory_item(&gpt_b.id, "Dato exclusivo de Beta", "fact", "normal")
        .expect("second GPT knowledge should be created");

    assert!(database
        .memory_overview()
        .expect("global memory should load")
        .items
        .is_empty());
    let memories_a = database
        .active_memories_for_conversation(&conversation_a.id)
        .expect("first GPT knowledge should load while global memory is off");
    assert_eq!(memories_a.len(), 1);
    assert_eq!(memories_a[0].content, "Dato exclusivo de Alfa");
    assert_eq!(
        memories_a[0].custom_gpt_id.as_deref(),
        Some(gpt_a.id.as_str())
    );
    let memories_b = database
        .active_memories_for_conversation(&conversation_b.id)
        .expect("second GPT knowledge should load");
    assert_eq!(memories_b.len(), 1);
    assert_eq!(memories_b[0].content, "Dato exclusivo de Beta");

    database
        .set_custom_gpt_memory_item_enabled(&gpt_a.id, &memory_a_id, false)
        .expect("first GPT knowledge should disable");
    assert!(database
        .active_memories_for_conversation(&conversation_a.id)
        .expect("disabled GPT knowledge should be excluded")
        .is_empty());
    assert_eq!(
        database
            .active_memories_for_conversation(&conversation_b.id)
            .expect("second GPT should remain unaffected")
            .len(),
        1
    );

    cleanup(&database);
}

#[test]
fn custom_gpt_files_follow_the_selected_gpt_without_sticky_chat_links() {
    let database = test_database();
    let gpt_a = database
        .create_custom_gpt("GPT con archivo", None, "Consulta su archivo.")
        .expect("first GPT should exist");
    let gpt_b = database
        .create_custom_gpt("GPT sin archivo", None, "No comparte archivos.")
        .expect("second GPT should exist");
    let conversation = database
        .create_conversation("Chat con archivo de GPT", None)
        .expect("conversation should exist");
    database
        .set_conversation_custom_gpt(&conversation.id, Some(&gpt_a.id))
        .expect("first GPT should be selected");
    let file = database
        .register_custom_gpt_attachment(
            &gpt_a.id,
            "C:/managed/private-gpt.pdf",
            "private-gpt.pdf",
            Some("application/pdf"),
            512,
            "custom-gpt-file-sha",
        )
        .expect("GPT file should be registered");
    database
        .update_attachment_ingestion(
            &file.id,
            "ready",
            Some("broker-custom-gpt-file"),
            Some("document"),
            Some("docling"),
            None,
            None,
        )
        .expect("GPT file should become ready");

    let active = database
        .ready_custom_gpt_file_ids_for_conversation(&conversation.id)
        .expect("selected GPT files should resolve");
    assert_eq!(active, vec![file.id.clone()]);
    assert_eq!(
        database
            .ready_attachments_for_turn(&conversation.id, &active)
            .expect("GPT file should be authorized for the turn")
            .len(),
        1
    );
    database
        .replace_attachment_chunks(
            &file.id,
            &["El archivo privado del GPT contiene el dato Delta.".to_owned()],
        )
        .expect("GPT file chunks should be stored");
    let trace_conversation = database
        .create_conversation("Traza del archivo de GPT", None)
        .expect("trace conversation should exist");
    database
        .set_conversation_custom_gpt(&trace_conversation.id, Some(&gpt_a.id))
        .expect("GPT should be selected for trace");
    let trace_files = database
        .ready_custom_gpt_file_ids_for_conversation(&trace_conversation.id)
        .expect("trace GPT files should resolve");
    let chunks = database
        .select_attachment_chunks(&trace_conversation.id, &trace_files, "dato Delta", 4, 8_000)
        .expect("GPT file chunks should be selectable");
    let frozen_gpt = database
        .custom_gpt_for_conversation(&trace_conversation.id)
        .expect("selected GPT should resolve")
        .expect("selected GPT should exist");
    let context = vec![ContextMessage {
        message_id: "custom-gpt-file-user".to_owned(),
        role: "user".to_owned(),
        text: "¿Cuál es el dato?".to_owned(),
    }];
    database
        .prepare_chat_turn_with_project_instruction(
            &trace_conversation.id,
            "custom-gpt-file-user",
            "custom-gpt-file-assistant",
            "custom-gpt-file-task",
            "custom-gpt-file-key",
            "¿Cuál es el dato?",
            &serde_json::json!({}),
            &context,
            None,
            Some(&frozen_gpt),
            &[],
            &chunks,
            &trace_files,
        )
        .expect("GPT file turn should persist");
    let trace = database
        .task_context("custom-gpt-file-task")
        .expect("GPT file context should be inspectable");
    assert!(trace.sources.iter().any(|source| {
        source.kind == "attachment_chunk"
            && source
                .reason
                .contains("Archivo de conocimiento del GPT personal seleccionado")
    }));
    let connection = database.connect().expect("connection should open");
    let sticky_links: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM conversation_attachments
             WHERE conversation_id = ?1 AND attachment_id = ?2",
            params![conversation.id, file.id],
            |row| row.get(0),
        )
        .expect("chat links should be counted");
    assert_eq!(sticky_links, 0);
    drop(connection);

    database
        .set_conversation_custom_gpt(&conversation.id, Some(&gpt_b.id))
        .expect("second GPT should be selected");
    assert!(database
        .ready_custom_gpt_file_ids_for_conversation(&conversation.id)
        .expect("second GPT files should resolve")
        .is_empty());
    assert!(matches!(
        database.ready_attachments_for_turn(&conversation.id, std::slice::from_ref(&file.id)),
        Err(AppError::Validation(_))
    ));

    database
        .set_conversation_custom_gpt(&conversation.id, Some(&gpt_a.id))
        .expect("first GPT should be selected again");
    assert!(database
        .remove_custom_gpt_file(&gpt_a.id, &file.id)
        .expect("GPT file should be removed")
        .is_empty());
    assert!(database
        .ready_custom_gpt_file_ids_for_conversation(&conversation.id)
        .expect("removed GPT files should resolve")
        .is_empty());
    cleanup(&database);
}
