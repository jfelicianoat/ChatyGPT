import type { AuditEventView, Loadable } from "../domain";

/** Actividad reciente registrada en local (ajustes). Extraída de App.tsx (CH-03): recibe el estado y los manejadores tal cual. */
export type TarjetaActividadProps = {
  auditEvents: Loadable<AuditEventView[]>;
  refreshAuditEvents: () => Promise<void> | void;
};

export function TarjetaActividad({ auditEvents, refreshAuditEvents }: TarjetaActividadProps) {
  return (
    <section className="activity-card">
      <div className="panel-heading">
        <div>
          <span className="kicker">Trazabilidad local</span>
          <h3>Actividad reciente</h3>
        </div>
        <button
          className="secondary"
          onClick={refreshAuditEvents}
          disabled={auditEvents.state === "loading"}
        >
          {auditEvents.state === "loading" ? "Actualizando…" : "Actualizar"}
        </button>
      </div>
      <p className="muted">
        Resumen seguro de las acciones guardadas. No muestra prompts, tokens, rutas ni datos técnicos internos.
      </p>
      {auditEvents.state === "ready" && auditEvents.value.length === 0 && (
        <p className="activity-empty">Todavía no hay actividad registrada.</p>
      )}
      {auditEvents.state === "ready" && auditEvents.value.length > 0 && (
        <ol className="activity-list">
          {auditEvents.value.map((event) => (
            <li key={event.id} className={`activity-item ${event.severity}`}>
              <span className="activity-marker" aria-hidden="true" />
              <div>
                <strong>{event.summary}</strong>
                <small>
                  {event.conversationTitle ?? (event.actor === "user" ? "Acción del usuario" : "Sistema")}
                  {" · "}
                  {new Date(`${event.occurredAt.replace(" ", "T")}Z`).toLocaleString("es-ES")}
                </small>
              </div>
            </li>
          ))}
        </ol>
      )}
      {auditEvents.state === "error" && <p className="error">{auditEvents.message}</p>}
    </section>
  );
}
