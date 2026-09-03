//! Que fuentes entraron en un turno y de donde salio cada una.

use super::*;

impl Database {
    pub fn task_context(&self, task_id: &str) -> Result<ContextSnapshotView, AppError> {
        let connection = self.connect()?;
        let (strategy_version, estimated_tokens): (String, i64) = connection
            .query_row(
                "SELECT strategy_version, COALESCE(estimated_tokens, 0)
                 FROM context_snapshots
                 WHERE broker_task_id = ?1",
                params![task_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound("contexto de la respuesta".to_owned()))?;
        let mut strategy = match strategy_version.as_str() {
            "window-memory-v1" => "Ventana reciente + memoria",
            "window-summary-v1" => "Resumen aprobado + ventana reciente",
            "window-summary-memory-v1" => "Resumen aprobado + ventana reciente + memoria",
            "window-summary-semantic-memory-v1" => {
                "Resumen aprobado + ventana reciente + memoria semántica"
            }
            "window-semantic-memory-v1" => "Ventana reciente + memoria semántica",
            "window-summary-semantic-memory-document-v1" => {
                "Resumen aprobado + ventana reciente + memoria semántica + documentos"
            }
            "window-semantic-memory-document-v1" => {
                "Ventana reciente + memoria semántica + documentos"
            }
            "window-summary-document-v1" => "Resumen aprobado + ventana reciente + documentos",
            "window-summary-memory-document-v1" => {
                "Resumen aprobado + ventana reciente + memoria + documentos"
            }
            "window-summary-project-v1" => {
                "Resumen aprobado + ventana reciente + instrucciones del proyecto"
            }
            "window-summary-project-memory-v1" => {
                "Resumen aprobado + ventana reciente + instrucciones del proyecto + memoria"
            }
            "window-project-v1" => "Ventana reciente + instrucciones del proyecto",
            "window-project-memory-v1" => {
                "Ventana reciente + instrucciones del proyecto + memoria"
            }
            "window-summary-project-document-v1" => {
                "Resumen aprobado + ventana reciente + instrucciones del proyecto + documentos"
            }
            "window-summary-project-memory-document-v1" => {
                "Resumen aprobado + ventana reciente + instrucciones del proyecto + memoria + documentos"
            }
            "window-project-document-v1" => {
                "Ventana reciente + instrucciones del proyecto + documentos"
            }
            "window-project-memory-document-v1" => {
                "Ventana reciente + instrucciones del proyecto + memoria + documentos"
            }
            "window-summary-project-semantic-memory-v1" => {
                "Resumen aprobado + ventana reciente + instrucciones del proyecto + memoria semántica"
            }
            "window-summary-project-semantic-memory-document-v1" => {
                "Resumen aprobado + ventana reciente + instrucciones del proyecto + memoria semántica + documentos"
            }
            "window-project-semantic-memory-v1" => {
                "Ventana reciente + instrucciones del proyecto + memoria semántica"
            }
            "window-project-semantic-memory-document-v1" => {
                "Ventana reciente + instrucciones del proyecto + memoria semántica + documentos"
            }
            "window-document-v1" => "Ventana reciente + documentos",
            "window-memory-document-v1" => "Ventana reciente + memoria + documentos",
            "window-v1" => "Ventana reciente",
            other => other,
        }
        .to_owned();
        let mut statement = connection.prepare(
            "SELECT source.source_type, source.reason, source.score,
                    COALESCE(source.estimated_tokens, 0),
                    COALESCE(source.excerpt, ''), memory.category,
                    attachment.display_name, chunk.ordinal,
                    source.id, attachment.local_path, memory.custom_gpt_id,
                    custom_gpt.name
             FROM context_sources source
             LEFT JOIN memory_items memory
               ON source.source_type = 'memory' AND memory.id = source.source_id
             LEFT JOIN custom_gpts custom_gpt ON custom_gpt.id = memory.custom_gpt_id
             LEFT JOIN attachment_chunks chunk
               ON source.source_type = 'attachment_chunk' AND chunk.id = source.source_id
             LEFT JOIN attachments attachment ON attachment.id = chunk.attachment_id
             JOIN context_snapshots snapshot ON snapshot.id = source.snapshot_id
             WHERE snapshot.broker_task_id = ?1
             ORDER BY source.ordinal",
        )?;
        let sources = statement
            .query_map(params![task_id], |row| {
                let kind: String = row.get(0)?;
                let stored_reason: String = row.get(1)?;
                let category: Option<String> = row.get(5)?;
                let attachment_name: Option<String> = row.get(6)?;
                let chunk_ordinal: Option<i64> = row.get(7)?;
                let source_id: String = row.get(8)?;
                let attachment_path: Option<String> = row.get(9)?;
                let custom_gpt_id: Option<String> = row.get(10)?;
                let custom_gpt_name: Option<String> = row.get(11)?;
                let label = match (
                    kind.as_str(),
                    stored_reason.as_str(),
                    category.as_deref(),
                    custom_gpt_id.as_deref(),
                ) {
                    ("message", "current_user_turn", _, _) => "Mensaje actual".to_owned(),
                    ("message", _, _, _) => "Mensaje reciente".to_owned(),
                    ("summary", _, _, _) => "Resumen aprobado".to_owned(),
                    ("project_instruction", _, _, _) => "Instrucciones del proyecto".to_owned(),
                    ("custom_gpt", _, _, _) => "GPT personal".to_owned(),
                    ("memory", _, Some("preference"), Some(_)) => format!(
                        "Conocimiento GPT · Preferencia · {}",
                        custom_gpt_name.as_deref().unwrap_or("GPT personal")
                    ),
                    ("memory", _, Some("instruction"), Some(_)) => format!(
                        "Conocimiento GPT · Instrucción · {}",
                        custom_gpt_name.as_deref().unwrap_or("GPT personal")
                    ),
                    ("memory", _, Some("fact"), Some(_)) => format!(
                        "Conocimiento GPT · Hecho · {}",
                        custom_gpt_name.as_deref().unwrap_or("GPT personal")
                    ),
                    ("memory", _, Some("preference"), _) => "Recuerdo · Preferencia".to_owned(),
                    ("memory", _, Some("instruction"), _) => "Recuerdo · Instrucción".to_owned(),
                    ("memory", _, Some("fact"), _) => "Recuerdo · Hecho".to_owned(),
                    ("memory", _, _, _) => "Recuerdo".to_owned(),
                    ("attachment_chunk", _, _, _) => format!(
                        "{} · fragmento {}",
                        attachment_name.as_deref().unwrap_or("Documento"),
                        chunk_ordinal.unwrap_or(0) + 1
                    ),
                    _ => "Fuente de contexto".to_owned(),
                };
                let reason = match stored_reason.as_str() {
                    "current_user_turn" => "Petición que acabas de enviar".to_owned(),
                    "recent_conversation_window" => {
                        "Mensaje reciente de la conversación".to_owned()
                    }
                    "approved_conversation_summary" => {
                        "Resumen revisado y aprobado por ti".to_owned()
                    }
                    "Instrucciones configuradas para el proyecto" => {
                        "Configuración reutilizable del proyecto".to_owned()
                    }
                    "Versión del GPT personal seleccionada al enviar" => {
                        "Versión exacta congelada al enviar".to_owned()
                    }
                    _ => stored_reason,
                };
                let excerpt: String = row.get(4)?;
                let source_reference = (kind == "attachment_chunk").then_some(source_id);
                Ok(ContextSourceView {
                    kind,
                    label,
                    reason,
                    score: row.get(2)?,
                    estimated_tokens: row.get(3)?,
                    excerpt: excerpt.chars().take(600).collect(),
                    source_reference,
                    source_available: attachment_path
                        .as_deref()
                        .is_some_and(|path| Path::new(path).is_file()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if sources.iter().any(|source| source.kind == "custom_gpt") {
            strategy.push_str(" + GPT personal");
        }
        if sources.iter().any(|source| {
            source.kind == "attachment_chunk"
                && source.reason.contains("Vista global del documento")
        }) {
            strategy.push_str(" · Vista global del documento");
        }
        Ok(ContextSnapshotView {
            strategy,
            estimated_tokens,
            sources,
        })
    }

    pub fn context_source_file(
        &self,
        task_id: &str,
        source_reference: &str,
    ) -> Result<ContextSourceFile, AppError> {
        self.connect()?
            .query_row(
                "SELECT attachment.local_path, attachment.display_name
                 FROM context_sources source
                 JOIN context_snapshots snapshot ON snapshot.id = source.snapshot_id
                 JOIN attachment_chunks chunk
                   ON source.source_type = 'attachment_chunk' AND chunk.id = source.source_id
                 JOIN attachments attachment ON attachment.id = chunk.attachment_id
                 WHERE snapshot.broker_task_id = ?1 AND source.id = ?2",
                params![task_id, source_reference],
                |row| {
                    Ok(ContextSourceFile {
                        local_path: row.get(0)?,
                        display_name: row.get(1)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound("fuente documental de la respuesta".to_owned()))
    }
}
