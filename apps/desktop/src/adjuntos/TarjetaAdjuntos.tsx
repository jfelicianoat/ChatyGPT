/**
 * Zona de adjuntos del compositor: cámara, captura de pantalla, fila de
 * botones, biblioteca del proyecto y chips de adjuntos.
 *
 * Recibe el hook entero (`useAdjuntos`): el estado y las acciones que lo
 * mueven ya viajan juntos, y así la tarjeta no necesita cuarenta props.
 * Se extraída de App.tsx (CH-03) tal cual, sin cambiar el DOM.
 */
import {
  attachmentContextSummary,
  attachmentFailureGuidance,
  attachmentImagePolicyLabel,
  attachmentStatusLabel,
  type ConversationView
} from "../domain";
import type { useAdjuntos } from "./useAdjuntos";

type Props = {
  adjuntos: ReturnType<typeof useAdjuntos>;
  /** Conversación abierta: el compositor solo se pinta con ella «ready». */
  conversation: ConversationView;
  /** Un turno del Broker en curso bloquea editar los adjuntos. */
  currentTurnBlocks: boolean;
};

export function TarjetaAdjuntos({ adjuntos, conversation, currentTurnBlocks }: Props) {
  const {
    attachments,
    projectFiles,
    draftAttachmentIds,
    setDraftAttachmentIds,
    attachmentBusy,
    projectFileBusyId,
    attachmentContextRetryId,
    attachmentSemanticRetryId,
    availableProjectFiles,
    screenCaptureBusy,
    screenCapturePreview,
    cropMode,
    setCropMode,
    cropSelection,
    setCropSelection,
    cropStartRef,
    cameraOpen,
    cameraReady,
    setCameraReady,
    cameraBusy,
    cameraError,
    setCameraError,
    cameraConversationId,
    cameraVideoRef,
    chooseAttachments,
    discardScreenCapture,
    beginCropSelection,
    updateCropSelection,
    finishCropSelection,
    applyScreenCaptureCrop,
    takeScreenCapture,
    attachScreenCapture,
    openCamera,
    takeCameraPhoto,
    stopCamera,
    removeAttachment,
    setAttachmentProjectSharing,
    addProjectFileToConversation,
    retryAttachment,
    retryAttachmentContext,
    retryAttachmentSemanticIndex
  } = adjuntos;
  return (
    <>
                {cameraOpen && conversation.id === cameraConversationId && (
                  <section className="camera-preview" aria-label="Vista previa de cámara">
                    <div className="camera-live-heading">
                      <span><i aria-hidden="true" /> Cámara activa</span>
                      <small>No se graba vídeo ni audio.</small>
                    </div>
                    <video
                      ref={cameraVideoRef}
                      autoPlay
                      muted
                      playsInline
                      onLoadedData={() => setCameraReady(true)}
                    />
                    <div className="camera-actions">
                      <button
                        onClick={() => {
                          stopCamera();
                          setCameraError(null);
                        }}
                        disabled={cameraBusy}
                      >
                        Cancelar
                      </button>
                      <button
                        className="primary"
                        onClick={takeCameraPhoto}
                        disabled={!cameraReady || cameraBusy}
                      >
                        {cameraBusy ? "Preparando foto…" : "Tomar foto"}
                      </button>
                    </div>
                  </section>
                )}
                {cameraError && (
                  <div className="camera-error" role="alert">
                    {cameraError}
                  </div>
                )}
                {screenCapturePreview &&
                  conversation.id === screenCapturePreview.conversationId && (
                    <section className="screen-capture-preview" aria-label="Vista previa de captura">
                      <div
                        className={`capture-crop-surface${cropMode ? " active" : ""}`}
                        onPointerDown={beginCropSelection}
                        onPointerMove={updateCropSelection}
                        onPointerUp={finishCropSelection}
                        onPointerCancel={finishCropSelection}
                      >
                        <img
                          alt={
                            screenCapturePreview.source === "camera"
                              ? "Vista previa de la fotografía"
                              : "Vista previa de la pantalla seleccionada"
                          }
                          src={screenCapturePreview.previewUrl}
                          draggable={false}
                        />
                        {cropMode && (
                          <span className="capture-crop-shade" aria-hidden="true" />
                        )}
                        {cropMode && cropSelection && (
                          <span
                            className="capture-crop-selection"
                            aria-hidden="true"
                            style={{
                              left: `${cropSelection.x * 100}%`,
                              top: `${cropSelection.y * 100}%`,
                              width: `${cropSelection.width * 100}%`,
                              height: `${cropSelection.height * 100}%`
                            }}
                          />
                        )}
                      </div>
                      <div>
                        <strong>
                          {screenCapturePreview.source === "camera"
                            ? "Foto lista para adjuntar"
                            : "Captura lista para adjuntar"}
                        </strong>
                        <small>
                          {screenCapturePreview.width} × {screenCapturePreview.height} píxeles ·{" "}
                          {(screenCapturePreview.blob.size / 1024).toLocaleString("es-ES", {
                            maximumFractionDigits: 0
                          })}{" "}
                          KB
                        </small>
                        <p>
                          {cropMode
                            ? "Arrastra sobre la imagen para marcar la zona que quieres conservar."
                            : "Revisa o recorta la imagen antes de incorporarla. Solo se enviará a Broker AI cuando la adjuntes y utilices en un mensaje."}
                        </p>
                        <div>
                          <button onClick={discardScreenCapture} disabled={attachmentBusy}>
                            Descartar
                          </button>
                          {cropMode ? (
                            <>
                              <button
                                onClick={() => {
                                  cropStartRef.current = null;
                                  setCropMode(false);
                                  setCropSelection(null);
                                }}
                                disabled={screenCaptureBusy}
                              >
                                Cancelar recorte
                              </button>
                              <button
                                className="primary"
                                onClick={applyScreenCaptureCrop}
                                disabled={!cropSelection || screenCaptureBusy}
                              >
                                {screenCaptureBusy ? "Recortando…" : "Aplicar recorte"}
                              </button>
                            </>
                          ) : (
                            <button
                              onClick={() => {
                                setCropSelection(null);
                                setCropMode(true);
                              }}
                              disabled={attachmentBusy}
                            >
                              Recortar
                            </button>
                          )}
                          <button
                            className="primary"
                            onClick={attachScreenCapture}
                            disabled={attachmentBusy || cropMode}
                          >
                            {attachmentBusy
                              ? "Adjuntando…"
                              : screenCapturePreview.source === "camera"
                                ? "Adjuntar foto"
                                : "Adjuntar captura"}
                          </button>
                        </div>
                      </div>
                    </section>
                  )}
                <div className="attachment-row">
                  <button
                    className="attachment-picker"
                    onClick={chooseAttachments}
                    disabled={
                      Boolean(currentTurnBlocks) ||
                      attachmentBusy ||
                      screenCaptureBusy ||
                      cameraBusy ||
                      cameraOpen
                    }
                  >
                    {attachmentBusy ? "Importando…" : "+ Adjuntar archivos"}
                  </button>
                  <button
                    className="screen-capture-button"
                    onClick={takeScreenCapture}
                    disabled={
                      Boolean(currentTurnBlocks) ||
                      attachmentBusy ||
                      screenCaptureBusy ||
                      cameraBusy ||
                      cameraOpen
                    }
                  >
                    {screenCaptureBusy
                      ? "Abriendo selector…"
                      : screenCapturePreview
                        ? "Repetir captura"
                        : "Capturar pantalla"}
                  </button>
                  <button
                    className="camera-button"
                    onClick={openCamera}
                    disabled={
                      Boolean(currentTurnBlocks) ||
                      attachmentBusy ||
                      screenCaptureBusy ||
                      cameraBusy ||
                      cameraOpen
                    }
                  >
                    {cameraBusy ? "Abriendo cámara…" : "Usar cámara"}
                  </button>
                  <span>o arrástralos a esta ventana</span>
                </div>
                {availableProjectFiles.length > 0 && (
                  // Plegada por defecto: con varios archivos, la lista empujaba la
                  // conversación hasta dejarla en unas pocas líneas visibles.
                  <details className="project-file-library" aria-label="Archivos del proyecto">
                    <summary>
                      <strong>Archivos del proyecto ({availableProjectFiles.length})</strong>
                      <small>Reutiliza contexto sin volver a subir el archivo.</small>
                    </summary>
                    <div className="project-file-list">
                      {availableProjectFiles.map((file) => (
                        <article className="project-file-item" key={file.id}>
                          <span>
                            <strong>{file.displayName}</strong>
                            <small>
                              {attachmentStatusLabel(file.ingestionStatus)}
                              {attachmentImagePolicyLabel(file) &&
                                ` · ${attachmentImagePolicyLabel(file)}`}
                            </small>
                          </span>
                          <button
                            onClick={() => addProjectFileToConversation(file.id)}
                            disabled={Boolean(currentTurnBlocks) || projectFileBusyId === file.id}
                          >
                            {projectFileBusyId === file.id ? "Añadiendo…" : "Usar en este chat"}
                          </button>
                        </article>
                      ))}
                    </div>
                  </details>
                )}
                {attachments.length > 0 && (
                  <div className="attachment-list" aria-label="Archivos de la conversación">
                    {attachments.map((attachment) => {
                      const selected = draftAttachmentIds.includes(attachment.id);
                      const failureGuidance = attachmentFailureGuidance(attachment);
                      const contextSummary = attachmentContextSummary(attachment);
                      const retryingContext = attachmentContextRetryId === attachment.id;
                      const retryingSemantic = attachmentSemanticRetryId === attachment.id;
                      return (
                        <div
                          key={attachment.id}
                          className={`attachment-chip ${selected ? "selected" : ""}`}
                        >
                          <button
                            className="attachment-select"
                            onClick={() => setDraftAttachmentIds((ids) =>
                              selected
                                ? ids.filter((id) => id !== attachment.id)
                                : [...ids, attachment.id]
                            )}
                            disabled={Boolean(currentTurnBlocks)}
                            title={
                              selected
                                ? "Desactivar para los próximos mensajes"
                                : "Activar para los próximos mensajes"
                            }
                          >
                            <strong>{attachment.displayName}</strong>
                            <small>
                              {(attachment.sizeBytes / 1024).toFixed(1)} KB ·{" "}
                              {attachmentStatusLabel(attachment.ingestionStatus)}
                              {attachmentImagePolicyLabel(attachment) &&
                                ` · ${attachmentImagePolicyLabel(attachment)}`}
                            </small>
                          </button>
                          {conversation.projectId && (
                            <button
                              className="attachment-project-action"
                              onClick={() => setAttachmentProjectSharing(
                                attachment.id,
                                !projectFiles.some((file) => file.id === attachment.id)
                              )}
                              disabled={
                                Boolean(currentTurnBlocks)
                                || projectFileBusyId === attachment.id
                              }
                            >
                              {projectFileBusyId === attachment.id
                                ? "Guardando…"
                                : projectFiles.some((file) => file.id === attachment.id)
                                  ? "Quitar del proyecto"
                                  : "Guardar en proyecto"}
                            </button>
                          )}
                          {attachment.ingestionStatus === "failed" && (
                            <button
                              className="attachment-retry"
                              onClick={() => retryAttachment(attachment.id)}
                            >
                              {failureGuidance?.retryLabel ?? "Reintentar"}
                            </button>
                          )}
                          <button
                            className="attachment-remove"
                            onClick={() => removeAttachment(attachment.id)}
                            aria-label={`Quitar ${attachment.displayName}`}
                          >
                            ×
                          </button>
                          {failureGuidance && (
                            <section className="attachment-guidance" aria-live="polite">
                              <strong>{failureGuidance.title}</strong>
                              <p>{failureGuidance.detail}</p>
                              <small>{failureGuidance.action}</small>
                            </section>
                          )}
                          {contextSummary && (
                            <section
                              className={`attachment-context attachment-context-${contextSummary.tone}`}
                              aria-live="polite"
                            >
                              <div>
                                <strong>{contextSummary.label}</strong>
                                <small>{contextSummary.detail}</small>
                              </div>
                              {contextSummary.retryable && (
                                <button
                                  className="attachment-context-retry"
                                  onClick={() => void (
                                    contextSummary.retryTarget === "semantic"
                                      ? retryAttachmentSemanticIndex(attachment.id)
                                      : retryAttachmentContext(attachment.id)
                                  )}
                                  disabled={retryingContext || retryingSemantic}
                                >
                                  {retryingContext || retryingSemantic
                                    ? "Reintentando…"
                                    : contextSummary.retryLabel ?? "Reintentar contexto"}
                                </button>
                              )}
                            </section>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}
    </>
  );
}
