/** Vista previa de un GPT personalizado antes de usarlo. */
import type { RefObject } from "react";

import { customGptIconGlyph, type CustomGptPreview, type Loadable } from "../domain";

export function VistaPreviaGpt({
  customGptPreview,
  activeModalRef,
  setCustomGptPreview,
}: {
  customGptPreview: Loadable<CustomGptPreview> | null;
  activeModalRef: RefObject<HTMLElement | null>;
  setCustomGptPreview: (preview: Loadable<CustomGptPreview> | null) => void;
}) {
  if (!customGptPreview) return null;
  return (
    <div className="modal-backdrop" role="presentation">
      <section
        ref={activeModalRef}
        className="modal custom-gpt-preview-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="custom-gpt-preview-title"
        aria-describedby="custom-gpt-preview-description"
        tabIndex={-1}
      >
        <span className="kicker">Vista previa</span>
        <h2 id="custom-gpt-preview-title">
          {customGptPreview.state === "ready"
            ? `${customGptIconGlyph(customGptPreview.value.iconRef)} ${customGptPreview.value.name} · versión ${customGptPreview.value.versionNo}`
            : "GPT personal"}
        </h2>
        <p id="custom-gpt-preview-description">
          Esto es exactamente lo que recibiría el modelo. No se ha enviado nada a
          Broker AI ni se ha generado ningún coste.
        </p>
        {customGptPreview.state === "loading" && <small>Preparando la vista previa…</small>}
        {customGptPreview.state === "error" && (
          <p className="error" role="alert">{customGptPreview.message}</p>
        )}
        {customGptPreview.state === "ready" && (
          <div className="custom-gpt-preview-body">
            {customGptPreview.value.warnings.length > 0 && (
              <ul className="custom-gpt-preview-warnings">
                {customGptPreview.value.warnings.map((warning) => (
                  <li key={warning}>{warning}</li>
                ))}
              </ul>
            )}
            <dl className="custom-gpt-preview-facts">
              <div>
                <dt>Modelo preferido</dt>
                <dd>{customGptPreview.value.preferredModel ?? "Lo elige el Broker"}</dd>
              </div>
              <div>
                <dt>Perfil de ejecución</dt>
                <dd>{customGptPreview.value.executionProfile
                  ? `${customGptPreview.value.executionProfile.strategy} · hasta ${customGptPreview.value.executionProfile.maxCostUsd.toFixed(2)} USD`
                  : "Hereda los ajustes del chat"}</dd>
              </div>
              <div>
                <dt>Proyecto predeterminado</dt>
                <dd>{customGptPreview.value.defaultProjectName ?? "Ninguno"}</dd>
              </div>
              <div>
                <dt>Código aislado</dt>
                <dd>
                  {customGptPreview.value.toolPermissions.runCode === "confirm"
                    ? "Puede solicitarlo, con tu confirmación"
                    : "Denegado"}
                </dd>
              </div>
              <div>
                <dt>Renombrar conversación</dt>
                <dd>
                  {customGptPreview.value.toolPermissions.renameConversation === "confirm"
                    ? "Puede proponerlo, con tu confirmación"
                    : "Denegado"}
                </dd>
              </div>
              <div>
                <dt>Cantidad de contexto</dt>
                <dd>
                  {customGptPreview.value.contextProfile === "focused"
                    ? "Enfocado · hasta 6 mensajes, 5 recuerdos y 4 fragmentos"
                    : customGptPreview.value.contextProfile === "broad"
                      ? "Amplio · hasta 20 mensajes, 30 recuerdos y 12 fragmentos"
                      : "Equilibrado · hasta 12 mensajes, 20 recuerdos y 8 fragmentos"}
                </dd>
              </div>
              <div>
                <dt>Tareas programadas</dt>
                <dd>
                  {customGptPreview.value.toolPermissions.createScheduledTasks === "confirm"
                    ? "Puede proponerlas, con tu confirmación"
                    : "Denegado"}
                </dd>
              </div>
              <div>
                <dt>APIs externas</dt>
                <dd>
                  {customGptPreview.value.toolPermissions.callExternalApis === "confirm"
                    ? "HTTPS GET, con tu confirmación para cada URL"
                    : "Denegado"}
                </dd>
              </div>
              <div>
                <dt>Conocimiento</dt>
                <dd>
                  {customGptPreview.value.activeKnowledgeCount} activo(s),{" "}
                  {customGptPreview.value.disabledKnowledgeCount} desactivado(s),{" "}
                  {customGptPreview.value.sensitiveKnowledgeCount} sensible(s)
                </dd>
              </div>
              <div>
                <dt>Archivos</dt>
                <dd>
                  {customGptPreview.value.readyFileCount} preparado(s),{" "}
                  {customGptPreview.value.pendingFileCount} pendiente(s)
                </dd>
              </div>
            </dl>
            <h3>Bloque exacto que se antepone al mensaje</h3>
            <pre>{customGptPreview.value.promptBlock}</pre>
            {customGptPreview.value.conversationStarters.length > 0 && (
              <>
                <h3>Iniciadores visibles en un chat vacío</h3>
                <ul className="custom-gpt-preview-starters">
                  {customGptPreview.value.conversationStarters.map((starter) => (
                    <li key={starter}>{starter}</li>
                  ))}
                </ul>
              </>
            )}
          </div>
        )}
        <div className="modal-actions">
          <button className="primary" autoFocus onClick={() => setCustomGptPreview(null)}>
            Cerrar
          </button>
        </div>
      </section>
    </div>
  );
}
