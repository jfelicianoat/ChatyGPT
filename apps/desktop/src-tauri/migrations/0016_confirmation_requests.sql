-- Fase 0 (endurecimiento): activa el expediente durable de confirmaciones.
--
-- La tabla existía desde la migración inicial pero ningún código la escribía:
-- las confirmaciones vivían solo en la interfaz, de modo que después no era
-- posible demostrar qué autorizó la persona ni cuándo. Estas columnas la
-- vinculan con la llamada de herramienta y la conversación afectadas.

ALTER TABLE confirmation_requests
    ADD COLUMN tool_call_id TEXT REFERENCES tool_calls(id) ON DELETE CASCADE;

ALTER TABLE confirmation_requests
    ADD COLUMN conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL;

-- Una llamada de herramienta no puede acumular dos expedientes de confirmación.
CREATE UNIQUE INDEX IF NOT EXISTS idx_confirmation_requests_tool_call
    ON confirmation_requests(tool_call_id)
    WHERE tool_call_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_confirmation_requests_timeline
    ON confirmation_requests(status, requested_at DESC);
