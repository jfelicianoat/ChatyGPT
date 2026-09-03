//! Carpetas autorizadas, permisos de Athena y registro de decisiones.
//!
//! Autorizar es explicito y revocable, y cada decision queda auditada: sin
//! eso, «la aplicacion puede leer mis carpetas» no seria una frase con
//! respuesta.

use super::*;

impl Database {
    /// Autoriza una carpeta para escritura tras una elección humana explícita.
    ///
    /// Reautorizar una carpeta previamente revocada la reactiva y actualiza su
    /// motivo, sin duplicar la fila.
    pub fn authorize_folder(
        &self,
        folder: &Path,
        display_name: &str,
        purpose: &str,
    ) -> Result<(), AppError> {
        let key = folder_key(folder);
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO authorized_folders(
                id, canonical_path, display_name, permissions_json
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(canonical_path) DO UPDATE SET
                display_name = excluded.display_name,
                permissions_json = excluded.permissions_json,
                granted_at = datetime('now'),
                revoked_at = NULL",
            params![
                format!("folder_{}", Uuid::new_v4().simple()),
                key,
                display_name,
                serde_json::json!({"write": true, "purpose": purpose}).to_string()
            ],
        )?;
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('authorized_folder.granted', 'user', ?1)",
            params![serde_json::json!({"purpose": purpose}).to_string()],
        )?;
        Ok(())
    }

    /// Autoriza una carpeta para que un GPT personal pueda solicitar lecturas.
    /// La concesión es independiente de la escritura y conserva permisos previos.
    pub fn authorize_folder_for_read(
        &self,
        folder: &Path,
        display_name: &str,
    ) -> Result<(), AppError> {
        let key = folder_key(folder);
        let connection = self.connect()?;
        let existing: Option<String> = connection
            .query_row(
                "SELECT permissions_json FROM authorized_folders WHERE canonical_path = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        let mut permissions = existing
            .as_deref()
            .and_then(|value| serde_json::from_str::<Value>(value).ok())
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        permissions.insert("read".to_owned(), Value::Bool(true));
        permissions.insert("purpose".to_owned(), Value::String("gpt_read".to_owned()));
        connection.execute(
            "INSERT INTO authorized_folders(
                id, canonical_path, display_name, permissions_json
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(canonical_path) DO UPDATE SET
                display_name = excluded.display_name,
                permissions_json = excluded.permissions_json,
                granted_at = datetime('now'),
                revoked_at = NULL",
            params![
                format!("folder_{}", Uuid::new_v4().simple()),
                key,
                display_name,
                Value::Object(permissions).to_string()
            ],
        )?;
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('authorized_folder.read_granted', 'user', '{}')",
            [],
        )?;
        Ok(())
    }

    /// Autoriza modificaciones confirmadas dentro de una carpeta. Modificar
    /// implica poder leer primero la versión y su huella para evitar pérdidas.
    pub fn authorize_folder_for_modify(
        &self,
        folder: &Path,
        display_name: &str,
    ) -> Result<(), AppError> {
        let key = folder_key(folder);
        let connection = self.connect()?;
        let existing: Option<String> = connection
            .query_row(
                "SELECT permissions_json FROM authorized_folders WHERE canonical_path = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        let mut permissions = existing
            .as_deref()
            .and_then(|value| serde_json::from_str::<Value>(value).ok())
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        permissions.insert("read".to_owned(), Value::Bool(true));
        permissions.insert("modify".to_owned(), Value::Bool(true));
        permissions.insert("purpose".to_owned(), Value::String("gpt_modify".to_owned()));
        connection.execute(
            "INSERT INTO authorized_folders(
                id, canonical_path, display_name, permissions_json
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(canonical_path) DO UPDATE SET
                display_name = excluded.display_name,
                permissions_json = excluded.permissions_json,
                granted_at = datetime('now'),
                revoked_at = NULL",
            params![
                format!("folder_{}", Uuid::new_v4().simple()),
                key,
                display_name,
                Value::Object(permissions).to_string()
            ],
        )?;
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('authorized_folder.modify_granted', 'user', '{}')",
            [],
        )?;
        Ok(())
    }

    /// Autoriza una carpeta como límite de trabajo de Athena sin retirar los
    /// permisos de lectura o modificación que ya tuviera para otros usos.
    pub fn authorize_folder_for_athena(
        &self,
        folder: &Path,
        display_name: &str,
    ) -> Result<(), AppError> {
        let key = folder_key(folder);
        let connection = self.connect()?;
        let existing: Option<String> = connection
            .query_row(
                "SELECT permissions_json FROM authorized_folders WHERE canonical_path = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        let mut permissions = existing
            .as_deref()
            .and_then(|value| serde_json::from_str::<Value>(value).ok())
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        permissions.insert("athena".to_owned(), Value::Bool(true));
        permissions.insert(
            "purpose".to_owned(),
            Value::String("athena_workspace".to_owned()),
        );
        connection.execute(
            "INSERT INTO authorized_folders(
                id, canonical_path, display_name, permissions_json
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(canonical_path) DO UPDATE SET
                display_name = excluded.display_name,
                permissions_json = excluded.permissions_json,
                granted_at = datetime('now'),
                revoked_at = NULL",
            params![
                format!("folder_{}", Uuid::new_v4().simple()),
                key,
                display_name,
                Value::Object(permissions).to_string()
            ],
        )?;
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('authorized_folder.athena_granted', 'user', '{}')",
            [],
        )?;
        Ok(())
    }

    /// Resuelve una concesión de lectura por su identificador opaco.
    pub fn authorized_folder_for_read(
        &self,
        folder_id: &str,
    ) -> Result<(PathBuf, String), AppError> {
        let row = self
            .connect()?
            .query_row(
                "SELECT canonical_path, display_name, permissions_json
                 FROM authorized_folders
                 WHERE id = ?1 AND revoked_at IS NULL",
                params![folder_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound("carpeta autorizada no disponible".to_owned()))?;
        let readable = serde_json::from_str::<Value>(&row.2)
            .ok()
            .and_then(|value| value.get("read").and_then(Value::as_bool))
            .unwrap_or(false);
        if !readable {
            return Err(AppError::Conflict(
                "la carpeta no tiene permiso de lectura para GPTs".to_owned(),
            ));
        }
        Ok((PathBuf::from(row.0), row.1))
    }

    pub fn authorized_folder_for_modify(
        &self,
        folder_id: &str,
    ) -> Result<(PathBuf, String), AppError> {
        let folder = self
            .list_authorized_folders()?
            .into_iter()
            .find(|folder| folder.id == folder_id && folder.revoked_at.is_none())
            .ok_or_else(|| AppError::NotFound("carpeta autorizada no disponible".to_owned()))?;
        if folder.permissions.get("modify").and_then(Value::as_bool) != Some(true) {
            return Err(AppError::Conflict(
                "la carpeta no tiene permiso para modificar archivos".to_owned(),
            ));
        }
        Ok((PathBuf::from(folder.path), folder.display_name))
    }

    pub fn record_authorized_file_modified(
        &self,
        folder_id: &str,
        before_sha256: &str,
        after_sha256: &str,
    ) -> Result<(), AppError> {
        self.connect()?.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('authorized_file.modified', 'tool', ?1)",
            params![serde_json::json!({
                "folder_id": folder_id,
                "before_sha256": before_sha256,
                "after_sha256": after_sha256
            })
            .to_string()],
        )?;
        Ok(())
    }

    /// Deja constancia de cada decisión sobre un permiso de Athena.
    ///
    /// Se escribe siempre, incluso cuando el servicio rechaza la respuesta por
    /// tardía o repetida: lo que se audita es que alguien decidió, no que la
    /// decisión llegase a aplicarse. `resultado` distingue ambos casos.
    pub fn record_athena_permission_decision(
        &self,
        run_id: &str,
        request_id: &str,
        herramienta: &str,
        accion: &str,
        conceder: bool,
        resultado: &str,
    ) -> Result<(), AppError> {
        let tipo = if resultado != "aplicada" {
            "athena.permission_rejected_by_service"
        } else if conceder {
            "athena.permission_granted"
        } else {
            "athena.permission_denied"
        };
        self.connect()?.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES (?1, 'user', ?2)",
            params![
                tipo,
                serde_json::json!({
                    "run_id": run_id,
                    "request_id": request_id,
                    "tool": herramienta,
                    "action": accion,
                    "granted": conceder,
                    "outcome": resultado
                })
                .to_string()
            ],
        )?;
        Ok(())
    }

    /// Anota el run abierto para poder re-engancharse tras reiniciar ChatyGPT.
    pub fn record_athena_run_started(
        &self,
        run_id: &str,
        objetivo: &str,
        workspace: &str,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO athena_runs(run_id, objective, workspace)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(run_id) DO UPDATE SET
                 objective = excluded.objective,
                 workspace = excluded.workspace,
                 closed_at = NULL,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            params![run_id, objetivo, workspace],
        )?;
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('athena.run_started', 'user', ?1)",
            params![serde_json::json!({ "run_id": run_id, "workspace": workspace }).to_string()],
        )?;
        Ok(())
    }

    /// Guarda la última fase vista, que es lo único que se refleja de Athena.
    pub fn record_athena_run_phase(&self, run_id: &str, fase: &str) -> Result<(), AppError> {
        self.connect()?.execute(
            "UPDATE athena_runs
                SET last_phase = ?2,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
              WHERE run_id = ?1",
            params![run_id, fase],
        )?;
        Ok(())
    }

    /// Cierra el apunte del run. Es idempotente a propósito: la interfaz sondea,
    /// y un run terminado se consultará muchas veces más; solo el primer cierre
    /// deja rastro en la auditoría.
    pub fn close_athena_run(&self, run_id: &str, fase: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let cerrados = connection.execute(
            "UPDATE athena_runs
                SET last_phase = ?2,
                    closed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
              WHERE run_id = ?1 AND closed_at IS NULL",
            params![run_id, fase],
        )?;
        if cerrados == 0 {
            return Ok(());
        }
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('athena.run_closed', 'system', ?1)",
            params![serde_json::json!({ "run_id": run_id, "phase": fase }).to_string()],
        )?;
        Ok(())
    }

    /// Runs que quedaron abiertos. Athena dirá si siguen vivos; aquí solo se
    /// recuerda a quién preguntar.
    pub fn list_open_athena_runs(&self) -> Result<Vec<AthenaRunRecordado>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT run_id, objective, workspace, last_phase, started_at
               FROM athena_runs
              WHERE closed_at IS NULL
              ORDER BY updated_at DESC
              LIMIT 20",
        )?;
        let runs = statement
            .query_map([], |row| {
                Ok(AthenaRunRecordado {
                    run_id: row.get(0)?,
                    objetivo: row.get(1)?,
                    workspace: row.get(2)?,
                    ultima_fase: row.get(3)?,
                    iniciado_en: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(runs)
    }

    pub fn list_authorized_folders(&self) -> Result<Vec<AuthorizedFolderView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, canonical_path, display_name, permissions_json, granted_at, revoked_at
             FROM authorized_folders
             ORDER BY revoked_at IS NOT NULL, granted_at DESC",
        )?;
        let folders = statement
            .query_map([], |row| {
                let permissions_json: String = row.get(3)?;
                Ok(AuthorizedFolderView {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    display_name: row.get(2)?,
                    permissions: serde_json::from_str(&permissions_json).unwrap_or(Value::Null),
                    granted_at: row.get(4)?,
                    revoked_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(folders)
    }

    pub fn list_read_authorized_folders(&self) -> Result<Vec<AuthorizedFolderView>, AppError> {
        Ok(self
            .list_authorized_folders()?
            .into_iter()
            .filter(|folder| {
                folder.revoked_at.is_none()
                    && folder.permissions.get("read").and_then(Value::as_bool) == Some(true)
            })
            .collect())
    }

    /// Revoca una carpeta: las exportaciones posteriores exigirán volver a
    /// elegirla en el selector nativo.
    pub fn revoke_authorized_folder(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let affected = connection.execute(
            "UPDATE authorized_folders SET revoked_at = datetime('now')
             WHERE id = ?1 AND revoked_at IS NULL",
            params![id],
        )?;
        if affected == 0 {
            return Err(AppError::NotFound(
                "la carpeta autorizada no existe o ya estaba revocada".to_owned(),
            ));
        }
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('authorized_folder.revoked', 'user', '{}')",
            [],
        )?;
        Ok(())
    }

    /// Indica si un destino cae dentro de una carpeta autorizada y vigente.
    ///
    /// Acepta descendientes —la exportación a Obsidian escribe en subcarpetas de
    /// la bóveda— pero nunca una carpeta hermana con nombre parecido.
    pub fn write_is_authorized(&self, destination: &Path) -> Result<bool, AppError> {
        let target = folder_key(destination);
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT canonical_path FROM authorized_folders WHERE revoked_at IS NULL")?;
        let authorized = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(authorized.iter().any(|folder| {
            target == *folder || target.starts_with(&format!("{folder}{MAIN_SEPARATOR}"))
        }))
    }

    pub fn record_scheduled_history_export(
        &self,
        destination_path: &str,
        destination_hash: &str,
        run_count: usize,
        status_filter: &str,
        period_filter: &str,
    ) -> Result<(), AppError> {
        self.connect()?.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('scheduled_history.exported', 'user', ?1)",
            params![serde_json::json!({
                "destination_path": destination_path,
                "destination_hash": destination_hash,
                "run_count": run_count,
                "status_filter": status_filter,
                "period_filter": period_filter
            })
            .to_string()],
        )?;
        Ok(())
    }

    pub fn record_scheduled_calendar_export(
        &self,
        destination_path: &str,
        destination_hash: &str,
        event_count: usize,
        range_days: u8,
    ) -> Result<(), AppError> {
        self.connect()?.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('scheduled_calendar.exported', 'user', ?1)",
            params![serde_json::json!({
                "destination_path": destination_path,
                "destination_hash": destination_hash,
                "event_count": event_count,
                "range_days": range_days
            })
            .to_string()],
        )?;
        Ok(())
    }

    pub fn record_windows_startup_changed(
        &self,
        enabled: bool,
        credential_protected: bool,
    ) -> Result<(), AppError> {
        self.connect()?.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('windows_startup.changed', 'user', ?1)",
            params![serde_json::json!({
                "enabled": enabled,
                "credential_protected": credential_protected,
                "scope": "current_user"
            })
            .to_string()],
        )?;
        Ok(())
    }

    /// Deja constancia de que la credencial cambió, nunca de su valor.
    pub fn record_broker_credential_changed(&self, stored: bool) -> Result<(), AppError> {
        self.connect()?.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('broker_credential.changed', 'user', ?1)",
            params![serde_json::json!({
                "stored": stored,
                "protection": "dpapi_current_user"
            })
            .to_string()],
        )?;
        Ok(())
    }
}
