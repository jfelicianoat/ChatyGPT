//! Pruebas de los comandos Tauri declarados en `lib.rs`.
//!
//! Viven aparte desde que el fichero paso de mil lineas: separarlas deja
//! la logica a la vista sin cambiar una sola linea de codigo.

use super::{
    attachment_filter_patterns, preview_custom_gpt_api_action, test_custom_gpt_api_action_impl,
    validated_managed_source_path,
};
use crate::db::{CustomGptApiAction, CustomGptApiParameter};
use crate::error::AppError;
use std::fs;
use uuid::Uuid;

#[test]
fn context_source_path_must_exist_inside_managed_storage() {
    let base = std::env::temp_dir().join(format!(
        "chatygpt-source-path-test-{}",
        Uuid::new_v4().simple()
    ));
    let managed = base.join("attachments");
    let source = managed.join("hash").join("documento.pdf");
    let outside = base.join("fuera.pdf");
    fs::create_dir_all(source.parent().expect("source parent should exist"))
        .expect("managed directory should exist");
    fs::write(&source, b"document").expect("managed source should exist");
    fs::write(&outside, b"outside").expect("outside source should exist");

    assert_eq!(
        validated_managed_source_path(&managed, &source).expect("managed source should validate"),
        source.canonicalize().expect("source should canonicalize")
    );
    assert!(matches!(
        validated_managed_source_path(&managed, &outside),
        Err(AppError::Validation(_))
    ));
    assert!(matches!(
        validated_managed_source_path(&managed, &managed.join("missing.pdf")),
        Err(AppError::NotFound(_))
    ));
    fs::remove_dir_all(base).expect("test directory should be removed");
}

#[test]
fn attachment_filter_uses_only_safe_extensions_from_capabilities() {
    assert_eq!(
        attachment_filter_patterns(&[
            ".PDF".to_owned(),
            "csv".to_owned(),
            "csv".to_owned(),
            "x';Remove-Item".to_owned(),
        ]),
        "*.csv;*.pdf"
    );
}

#[test]
fn api_action_preview_builds_the_final_url_without_network_access() {
    let preview = preview_custom_gpt_api_action(
        CustomGptApiAction {
            name: "buscar_pais".to_owned(),
            description: "Busca datos públicos de un país".to_owned(),
            url: "https://restcountries.com/v3.1/name/{name}".to_owned(),
            query_parameters: Vec::new(),
            credential_ref: None,
            auth_mode: "none".to_owned(),
            parameters: vec![CustomGptApiParameter {
                name: "name".to_owned(),
                value_type: "string".to_owned(),
                required: true,
                location: "path".to_owned(),
                description: None,
            }],
        },
        serde_json::json!({"name": "Costa Rica"}),
    )
    .expect("la vista previa debe componerse localmente");
    assert_eq!(preview.destination, "restcountries.com");
    assert_eq!(preview.method, "GET");
    assert!(
        preview.final_url.ends_with("/Costa%20Rica"),
        "URL generada: {}",
        preview.final_url
    );
    assert_eq!(preview.data_sent.len(), 1);
}

#[test]
fn api_action_test_cannot_connect_without_backend_confirmation() {
    let result = tauri::async_runtime::block_on(test_custom_gpt_api_action_impl(
        CustomGptApiAction {
            name: "buscar_pais".to_owned(),
            description: "Busca datos públicos de un país".to_owned(),
            url: "https://restcountries.com/v3.1/name/{name}".to_owned(),
            query_parameters: Vec::new(),
            credential_ref: None,
            auth_mode: "none".to_owned(),
            parameters: vec![CustomGptApiParameter {
                name: "name".to_owned(),
                value_type: "string".to_owned(),
                required: true,
                location: "path".to_owned(),
                description: None,
            }],
        },
        serde_json::json!({"name": "Costa Rica"}),
        false,
        &std::env::temp_dir(),
    ));
    assert!(matches!(result, Err(AppError::Validation(_))));
}

#[test]
fn authenticated_api_action_stops_before_network_when_credential_is_missing() {
    let directory = std::env::temp_dir().join(format!(
        "chatygpt-missing-api-secret-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let result = tauri::async_runtime::block_on(test_custom_gpt_api_action_impl(
        CustomGptApiAction {
            name: "private_status".to_owned(),
            description: "Consulta el estado autenticado del servicio".to_owned(),
            url: "https://api.example.org/status".to_owned(),
            query_parameters: Vec::new(),
            credential_ref: Some("missing_service".to_owned()),
            auth_mode: "bearer".to_owned(),
            parameters: Vec::new(),
        },
        serde_json::json!({}),
        true,
        &directory,
    ));
    assert!(
        matches!(result, Err(AppError::Validation(message)) if message.contains("no está disponible"))
    );
    assert!(
        !directory.exists(),
        "leer una credencial ausente no debe crear archivos"
    );
}
