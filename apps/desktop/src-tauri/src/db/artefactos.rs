//! Copias locales de los ficheros que generan las tareas del Broker (H23).
//!
//! El Broker poda sus artefactos pasado un tiempo. Lo que la persona decidió
//! conservar tiene que seguir accesible desde el mensaje aunque el remoto ya no
//! exista, así que la asociación entre artefacto y copia vive aquí.

use super::*;

/// Copia local registrada de un artefacto remoto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalArtifactCopy {
    pub artifact_id: String,
    pub filename: String,
    pub media_type: Option<String>,
    pub size_bytes: u64,
    pub sha256: String,
    pub local_path: String,
    pub saved_at: String,
}

impl Database {
    #[allow(clippy::too_many_arguments)]
    pub fn record_task_artifact_copy(
        &self,
        remote_task_id: &str,
        artifact_id: &str,
        filename: &str,
        media_type: Option<&str>,
        size_bytes: u64,
        sha256: &str,
        local_path: &str,
    ) -> Result<LocalArtifactCopy, AppError> {
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO task_artifact_copies(
                id, remote_task_id, artifact_id, filename, media_type, size_bytes,
                sha256, local_path
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(remote_task_id, artifact_id) DO UPDATE SET
                filename = excluded.filename,
                media_type = excluded.media_type,
                size_bytes = excluded.size_bytes,
                sha256 = excluded.sha256,
                local_path = excluded.local_path,
                saved_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            params![
                format!("artifact_copy_{}", Uuid::new_v4().simple()),
                remote_task_id,
                artifact_id,
                filename,
                media_type,
                size_bytes as i64,
                sha256,
                local_path
            ],
        )?;
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             SELECT 'task_artifact.kept', 'user', task.conversation_id, ?3
             FROM (SELECT ?1 AS remote_task_id) requested
             LEFT JOIN broker_tasks task ON task.remote_task_id = requested.remote_task_id",
            params![
                remote_task_id,
                artifact_id,
                serde_json::json!({
                    "remote_task_id": remote_task_id,
                    "artifact_id": artifact_id,
                    "sha256": sha256,
                    "size_bytes": size_bytes
                })
                .to_string()
            ],
        )?;
        self.task_artifact_copy(remote_task_id, artifact_id)?
            .ok_or_else(|| AppError::NotFound("copia local del artefacto".to_owned()))
    }

    pub fn task_artifact_copies(
        &self,
        remote_task_id: &str,
    ) -> Result<Vec<LocalArtifactCopy>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT artifact_id, filename, media_type, size_bytes, sha256, local_path, saved_at
             FROM task_artifact_copies
             WHERE remote_task_id = ?1
             ORDER BY saved_at",
        )?;
        let copies = statement
            .query_map(params![remote_task_id], local_copy_from_row)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(copies)
    }

    pub fn task_artifact_copy(
        &self,
        remote_task_id: &str,
        artifact_id: &str,
    ) -> Result<Option<LocalArtifactCopy>, AppError> {
        Ok(self
            .connect()?
            .query_row(
                "SELECT artifact_id, filename, media_type, size_bytes, sha256, local_path, saved_at
                 FROM task_artifact_copies
                 WHERE remote_task_id = ?1 AND artifact_id = ?2",
                params![remote_task_id, artifact_id],
                local_copy_from_row,
            )
            .optional()?)
    }
}

fn local_copy_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<LocalArtifactCopy> {
    Ok(LocalArtifactCopy {
        artifact_id: row.get(0)?,
        filename: row.get(1)?,
        media_type: row.get(2)?,
        size_bytes: row.get::<_, i64>(3)?.max(0) as u64,
        sha256: row.get(4)?,
        local_path: row.get(5)?,
        saved_at: row.get(6)?,
    })
}
