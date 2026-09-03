//! Funciones sueltas de la capa de persistencia: validacion, vectores y
//! seleccion de fragmentos de documento.
//!
//! Ninguna toca la base: reciben datos y devuelven datos, que es lo que las
//! hace probables sin abrir un SQLite.

use super::*;

pub(crate) fn validate_execution_preferences(
    preferences: &ConversationExecutionPreferences,
) -> Result<(), AppError> {
    if !matches!(
        preferences.data_classification.as_str(),
        "public" | "internal" | "confidential" | "local_only"
    ) {
        return Err(AppError::Validation(
            "la clasificación de datos no es válida".to_owned(),
        ));
    }
    if !matches!(
        preferences.strategy.as_str(),
        "single" | "auto" | "mixture_of_agents"
    ) {
        return Err(AppError::Validation(
            "la estrategia de ejecución no es válida".to_owned(),
        ));
    }
    if !matches!(preferences.preset.as_str(), "fast" | "slow") {
        return Err(AppError::Validation(
            "la profundidad de análisis no es válida".to_owned(),
        ));
    }
    if !preferences.max_cost_usd.is_finite() || !(0.0..=10.0).contains(&preferences.max_cost_usd) {
        return Err(AppError::Validation(
            "el límite de coste debe estar entre 0 y 10 USD".to_owned(),
        ));
    }
    if !matches!(preferences.long_context.as_str(), "fail" | "map_reduce") {
        return Err(AppError::Validation(
            "el tratamiento de documentos largos no es válido".to_owned(),
        ));
    }
    if preferences.priority > 1000 {
        return Err(AppError::Validation(
            "la prioridad debe estar entre 0 y 1000".to_owned(),
        ));
    }
    Ok(())
}

pub(crate) fn decode_embedding(blob: &[u8], dimensions: i64) -> Result<Vec<f64>, AppError> {
    let expected = usize::try_from(dimensions)
        .ok()
        .and_then(|value| value.checked_mul(8))
        .ok_or_else(|| AppError::BrokerContract("dimensiones de embedding inválidas".to_owned()))?;
    if blob.len() != expected {
        return Err(AppError::BrokerContract(
            "el vector almacenado no coincide con sus dimensiones".to_owned(),
        ));
    }
    Ok(blob
        .chunks_exact(8)
        .map(|chunk| f64::from_le_bytes(chunk.try_into().expect("chunk de ocho bytes")))
        .collect())
}

pub(crate) fn cosine_similarity(left: &[f64], right: &[f64]) -> f64 {
    if left.len() != right.len() || left.is_empty() {
        return f64::NAN;
    }
    let dot = left.iter().zip(right).map(|(a, b)| a * b).sum::<f64>();
    let left_norm = left.iter().map(|value| value * value).sum::<f64>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f64>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        f64::NAN
    } else {
        (dot / (left_norm * right_norm)).clamp(-1.0, 1.0)
    }
}

/// Abre el expediente durable de una investigación cuando la petición lo es.
///
/// La decisión se toma leyendo la petición ya construida, no un parámetro
/// aparte: así una investigación abre exactamente el mismo expediente tanto si
/// llega por el camino directo como si llega tras una recuperación semántica, y
/// no puede existir una petición `deep_research` sin sus etapas asociadas.
pub(crate) fn insert_research_run_if_needed(
    transaction: &rusqlite::Transaction<'_>,
    request: &Value,
    conversation_id: &str,
    local_task_id: &str,
    user_text: &str,
) -> Result<(), AppError> {
    if request
        .get("content")
        .and_then(|content| content.get("metadata"))
        .and_then(|metadata| metadata.get("workflow_kind"))
        .and_then(Value::as_str)
        != Some("deep_research")
    {
        return Ok(());
    }
    let research_run_id = format!("research_{}", Uuid::new_v4().simple());
    transaction.execute(
        "INSERT INTO research_runs(
            id, conversation_id, broker_task_id, objective, status
         ) VALUES (?1, ?2, ?3, ?4, 'planning')",
        params![research_run_id, conversation_id, local_task_id, user_text],
    )?;
    // Sin etapas fijas. Antes se insertaban tres —plan, búsqueda, síntesis— que
    // no describían nada: eran una plantilla dibujada antes de que ocurriera
    // nada. Los pasos reales los escribe `record_research_tool_step` conforme
    // el modelo pide herramientas, cada uno con su parámetro y su resultado.
    transaction.execute(
        "INSERT INTO audit_events(
            event_type, actor, conversation_id, payload_json
         ) VALUES ('research.started', 'user', ?1, ?2)",
        params![
            conversation_id,
            serde_json::json!({
                "research_run_id": research_run_id,
                "broker_task_id": local_task_id
            })
            .to_string()
        ],
    )?;
    Ok(())
}

pub(crate) fn lexical_terms(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| term.chars().count() >= 3)
        .map(str::to_owned)
        .collect()
}

pub(crate) fn normalized_document_query(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|character| match character {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            other if other.is_alphanumeric() => other,
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn is_global_document_request(query: &str) -> bool {
    let query = normalized_document_query(query);
    let explicit_global_request = [
        "de que va",
        "de que trata",
        "resumen del libro",
        "resumen del documento",
        "resume el libro",
        "resume este libro",
        "resume el documento",
        "resume este documento",
        "hazme un resumen",
        "vision general",
        "idea principal",
        "ideas principales",
        "estructura del libro",
        "estructura del documento",
        "cuantos capitulos",
        "cuantos temas",
        "what is the book about",
        "what is this book about",
        "what is the document about",
        "summarize the book",
        "summarize this book",
        "summarize the document",
        "document overview",
    ]
    .iter()
    .any(|phrase| query.contains(phrase));
    if explicit_global_request {
        return true;
    }

    let asks_for_summary = query.contains("resumen") || query.contains("summary");
    let narrows_to_part = [
        "capitulo",
        "seccion",
        "apartado",
        "pagina",
        "fragmento",
        "chapter",
        "section",
        "page",
    ]
    .iter()
    .any(|term| query.contains(term));
    asks_for_summary && !narrows_to_part
}

pub(crate) fn global_chunk_role(chunk: &SelectedAttachmentChunk) -> (&'static str, i32) {
    let text = normalized_document_query(&chunk.text);
    if text.contains("table of contents")
        || text.contains("indice general")
        || text.contains("indice de contenidos")
        || text.contains("contenido") && text.contains("capitulo")
    {
        ("Vista global del documento · índice", 1_000)
    } else if text.contains("abstract") || text.contains("sinopsis") || text.contains("resumen") {
        ("Vista global del documento · resumen editorial", 980)
    } else if text.contains("preface")
        || text.contains("foreword")
        || text.contains("prefacio")
        || text.contains("prologo")
    {
        ("Vista global del documento · prefacio", 960)
    } else if text.contains("introduction") || text.contains("introduccion") {
        ("Vista global del documento · introducción", 940)
    } else if text.contains("conclusion")
        || text.contains("conclusiones")
        || text.contains("epilogue")
        || text.contains("epilogo")
    {
        ("Vista global del documento · conclusiones", 920)
    } else if chunk.ordinal == 0 {
        ("Vista global del documento · cabecera", 900)
    } else if chunk.ordinal <= 2 {
        (
            "Vista global del documento · apertura",
            850 - chunk.ordinal as i32,
        )
    } else {
        ("Vista global del documento · muestra representativa", 100)
    }
}

pub(crate) fn select_global_document_chunks(
    candidates: Vec<SelectedAttachmentChunk>,
    maximum_chunks: usize,
    character_budget: usize,
) -> Result<Vec<SelectedAttachmentChunk>, AppError> {
    let mut attachment_order = Vec::new();
    let mut grouped: HashMap<String, Vec<SelectedAttachmentChunk>> = HashMap::new();
    for candidate in candidates {
        if !grouped.contains_key(&candidate.attachment_id) {
            attachment_order.push(candidate.attachment_id.clone());
        }
        grouped
            .entry(candidate.attachment_id.clone())
            .or_default()
            .push(candidate);
    }

    let mut ranked_groups = Vec::new();
    for attachment_id in attachment_order {
        let group = grouped.remove(&attachment_id).unwrap_or_default();
        let group_len = group.len();
        let mut structural = group
            .iter()
            .filter(|chunk| global_chunk_role(chunk).1 > 100)
            .cloned()
            .collect::<Vec<_>>();
        structural.sort_by(|left, right| {
            let (_, left_priority) = global_chunk_role(left);
            let (_, right_priority) = global_chunk_role(right);
            right_priority
                .cmp(&left_priority)
                .then_with(|| left.ordinal.cmp(&right.ordinal))
        });

        // If the converter did not preserve recognizable headings, add samples
        // from the beginning, middle and end instead of pretending that cosine
        // similarity can answer a question about the whole document.
        let mut ranked = structural;
        let mut included = ranked
            .iter()
            .map(|chunk| chunk.id.clone())
            .collect::<HashSet<_>>();
        for ordinal in [
            0,
            group_len / 3,
            group_len.saturating_mul(2) / 3,
            group_len.saturating_sub(1),
        ] {
            if let Some(sample) = group.iter().find(|chunk| chunk.ordinal == ordinal as i64) {
                if included.insert(sample.id.clone()) {
                    ranked.push(sample.clone());
                }
            }
        }
        let mut remaining = group
            .into_iter()
            .filter(|chunk| included.insert(chunk.id.clone()))
            .collect::<Vec<_>>();
        remaining.sort_by_key(|chunk| chunk.ordinal);
        ranked.extend(remaining);
        ranked_groups.push(ranked);
    }

    let mut selected = Vec::new();
    let mut used_characters = 0_usize;
    let mut next_indexes = vec![0_usize; ranked_groups.len()];
    while selected.len() < maximum_chunks {
        let mut progressed = false;
        for (group_index, group) in ranked_groups.iter().enumerate() {
            while let Some(candidate) = group.get(next_indexes[group_index]) {
                next_indexes[group_index] += 1;
                let candidate_characters = candidate.text.chars().count();
                if used_characters.saturating_add(candidate_characters) > character_budget {
                    continue;
                }
                let mut candidate = candidate.clone();
                let (reason, priority) = global_chunk_role(&candidate);
                candidate.reason = reason.to_owned();
                candidate.score = f64::from(priority) / 1_000.0;
                used_characters += candidate_characters;
                selected.push(candidate);
                progressed = true;
                break;
            }
            if selected.len() == maximum_chunks {
                break;
            }
        }
        if !progressed {
            break;
        }
    }
    Ok(selected)
}
