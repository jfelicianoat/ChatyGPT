/** Resumen de la conversacion: se genera, se revisa y se aprueba.
 *
 * Aprobar compacta el contexto pero no borra mensajes: lo resumido sigue
 * estando para quien quiera mirarlo. */
import type { RefObject } from "react";

import type { ConversationSummaryOverview, Loadable } from "../domain";

export function ResumenConversacion({
  summaryPanel,
  summaryDraft,
  summaryBusy,
  activeModalRef,
  setSummaryPanel,
  setSummaryDraft,
  generateSummary,
  saveSummaryDraft,
  approveSummaryDraft,
}: {
  summaryPanel: Loadable<ConversationSummaryOverview> | null;
  summaryDraft: string;
  summaryBusy: boolean;
  activeModalRef: RefObject<HTMLElement | null>;
  setSummaryPanel: (panel: Loadable<ConversationSummaryOverview> | null) => void;
  setSummaryDraft: (draft: string) => void;
  generateSummary: () => void | Promise<void>;
  saveSummaryDraft: () => void | Promise<void>;
  approveSummaryDraft: () => void | Promise<void>;
}) {
  if (!summaryPanel) return null;
  return (
    <div className="modal-backdrop" role="presentation">
      <section
        ref={activeModalRef}
        className="modal summary-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="summary-title"
        tabIndex={-1}
      >
        <span className="kicker">Contexto controlado</span>
        <h2 id="summary-title">Resumen de la conversación</h2>
        <p>
          El historial original siempre se conserva. Solo un resumen que edites y apruebes
          se utilizará para representar los mensajes anteriores que cubre.
        </p>
        {summaryPanel.state === "loading" && <p className="muted">Cargando resumen…</p>}
        {summaryPanel.state === "error" && <p className="error">{summaryPanel.message}</p>}
        {summaryPanel.state !== "ready" && (
          <div className="modal-actions">
            <button className="secondary" onClick={() => setSummaryPanel(null)}>
              Cerrar
            </button>
          </div>
        )}
        {summaryPanel.state === "ready" && (
          <>
            {summaryPanel.value.active && (
              <div className="summary-active">
                <strong>Resumen activo</strong>
                <p>{summaryPanel.value.active.approvedText}</p>
                <small>
                  Cubre {summaryPanel.value.activeCoveredMessageCount} de{" "}
                  {summaryPanel.value.totalMessageCount} mensajes · quedan{" "}
                  {summaryPanel.value.remainingMessageCount}
                </small>
              </div>
            )}
            {summaryPanel.value.candidate?.status === "generating" && (
              <div className="summary-progress">
                <span className="spinner" aria-hidden="true" />
                <div>
                  <strong>Preparando borrador…</strong>
                  <p>Puedes cerrar esta ventana; la tarea continuará y se recuperará al reiniciar.</p>
                  {summaryPanel.value.candidateCoveredMessageCount !== undefined && (
                    <small>
                      Este lote avanzará hasta{" "}
                      {summaryPanel.value.candidateCoveredMessageCount} de{" "}
                      {summaryPanel.value.totalMessageCount} mensajes.
                    </small>
                  )}
                </div>
              </div>
            )}
            {summaryPanel.value.candidate?.status === "draft" && (
              <label className="summary-editor">
                <span>Borrador pendiente de aprobación</span>
                <textarea
                  autoFocus
                  value={summaryDraft}
                  onChange={(event) => setSummaryDraft(event.target.value)}
                  maxLength={10_000}
                />
                <small>{summaryDraft.length.toLocaleString("es-ES")} / 10.000 caracteres</small>
                {summaryPanel.value.candidateCoveredMessageCount !== undefined && (
                  <small className="summary-coverage">
                    Al aprobarlo cubrirá{" "}
                    {summaryPanel.value.candidateCoveredMessageCount} de{" "}
                    {summaryPanel.value.totalMessageCount} mensajes y conservará{" "}
                    {summaryPanel.value.totalMessageCount -
                      summaryPanel.value.candidateCoveredMessageCount} recientes.
                  </small>
                )}
              </label>
            )}
            {!summaryPanel.value.candidate && (
              <p className="muted">
                {summaryPanel.value.totalMessageCount === 0
                  ? "Todavía no hay mensajes que resumir."
                  : summaryPanel.value.active && summaryPanel.value.remainingMessageCount === 0
                    ? "El resumen está al día y ya cubre todos los mensajes disponibles."
                    : summaryPanel.value.active
                  ? "Puedes generar un nuevo borrador sin desactivar el resumen actual."
                  : "Todavía no hay ningún resumen. La generación crea un borrador, nunca uno activo."}
              </p>
            )}
            <div className="modal-actions">
              <button
                className="secondary"
                onClick={() => setSummaryPanel(null)}
                disabled={summaryBusy}
              >
                Cerrar
              </button>
              {!summaryPanel.value.candidate &&
                summaryPanel.value.totalMessageCount > 0 &&
                summaryPanel.value.remainingMessageCount > 0 && (
                <button className="primary" onClick={generateSummary} disabled={summaryBusy}>
                  {summaryBusy
                    ? "Preparando…"
                    : summaryPanel.value.active
                      ? "Actualizar borrador"
                      : "Generar borrador"}
                </button>
              )}
              {summaryPanel.value.candidate?.status === "draft" && (
                <>
                  <button
                    className="secondary"
                    onClick={saveSummaryDraft}
                    disabled={summaryBusy || !summaryDraft.trim()}
                  >
                    Guardar borrador
                  </button>
                  <button
                    className="primary"
                    onClick={approveSummaryDraft}
                    disabled={summaryBusy || !summaryDraft.trim()}
                  >
                    {summaryBusy ? "Guardando…" : "Guardar y aprobar"}
                  </button>
                </>
              )}
            </div>
          </>
        )}
      </section>
    </div>
  );
}
