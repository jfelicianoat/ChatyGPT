//! Ajustes de la aplicación: `app_settings`, clave → valor JSON.

use super::*;

impl Database {
    /// Un valor ilegible cuenta como ausente: quien lee aplica su valor por defecto.
    pub fn setting(&self, key: &str) -> Result<Option<Value>, AppError> {
        let raw: Option<String> = self
            .connect()?
            .query_row(
                "SELECT value_json FROM app_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(raw.and_then(|raw| serde_json::from_str(&raw).ok()))
    }

    pub fn set_setting(&self, key: &str, value: &Value) -> Result<(), AppError> {
        self.connect()?.execute(
            "INSERT INTO app_settings(key, value_json) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE
             SET value_json = excluded.value_json, updated_at = datetime('now')",
            params![key, value.to_string()],
        )?;
        Ok(())
    }
}
