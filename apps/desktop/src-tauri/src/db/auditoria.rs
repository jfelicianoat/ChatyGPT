//! Lectura del registro de auditoria para la interfaz.

use super::*;

impl Database {
    pub fn list_audit_events(&self, limit: u32) -> Result<Vec<AuditEventView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT ae.id, ae.event_type, ae.actor, c.title, ae.occurred_at
             FROM audit_events ae
             LEFT JOIN conversations c ON c.id = ae.conversation_id
             ORDER BY ae.occurred_at DESC, ae.id DESC
             LIMIT ?1",
        )?;
        let events = statement
            .query_map(params![i64::from(limit.clamp(1, 100))], |row| {
                let event_type: String = row.get(1)?;
                let (category, summary, severity) = audit_presentation(&event_type);
                Ok(AuditEventView {
                    id: row.get(0)?,
                    category: category.to_owned(),
                    summary: summary.to_owned(),
                    severity: severity.to_owned(),
                    actor: row.get(2)?,
                    conversation_title: row.get(3)?,
                    occurred_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(events)
    }
}
