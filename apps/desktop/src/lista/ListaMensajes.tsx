/**
 * Lista de mensajes de la conversación: cabecera de paginación (H17), los
 * mensajes con su progreso real, artefactos, contexto utilizado y fuentes.
 *
 * Recibe el hook entero (`useListaMensajes`) más el estado del turno y del
 * panel de contexto, que son de la conversación y no de la lista. Extraída
 * de App.tsx (CH-03) tal cual, sin cambiar el DOM.
 */
import {
  canRevealContextSource,
  formatResponseDuration,
  formatResponseUsage,
  shouldFollowConversationScroll,
  taskFailureSummary,
  taskProgressSummary,
  type ContextSnapshotView,
  type ConversationView,
  type CustomGptView,
  type Loadable,
  type LocalTaskSnapshot
} from "../domain";
import type { RefObject } from "react";
import { MarkdownContent } from "../MarkdownContent";
import { MessageArtifacts } from "../MessageArtifacts";
import { EARLIER_MESSAGE_PAGE_SIZE, type useListaMensajes } from "./useListaMensajes";

type Props = {
  lista: ReturnType<typeof useListaMensajes>;
  /** Conversación abierta: el chat solo se pinta con ella «ready». */
  conversation: ConversationView;
  currentTurn: Loadable<LocalTaskSnapshot> | null;
  currentProgress: ReturnType<typeof taskProgressSummary> | null;
  /** GPT personal asociado a la conversación, si lo hay (iniciadores). */
  selectedCustomGpt?: CustomGptView;
  currentTurnBlocks: boolean;
  setDraft: (text: string) => void;
  /** Decide si el scroll sigue al último mensaje; lo comparte toda la app. */
  followConversationScrollRef: RefObject<boolean>;
  contextPanel: {
    taskId: string;
    data: Loadable<ContextSnapshotView>;
  } | null;
  toggleTaskContext: (taskId: string) => Promise<void>;
  contextSourceAction: {
    taskId: string;
    reference: string;
    state: "loading" | "success" | "error";
    message?: string;
  } | null;
  revealContextSource: (taskId: string, sourceReference: string) => Promise<void>;
};

export function ListaMensajes({
  lista,
  conversation,
  currentTurn,
  currentProgress,
  selectedCustomGpt,
  currentTurnBlocks,
  setDraft,
  followConversationScrollRef,
  contextPanel,
  toggleTaskContext,
  contextSourceAction,
  revealContextSource
}: Props) {
  const {
    messageListRef,
    earlierMessagesBusy,
    progressiveMessages,
    hasEarlierMessages,
    revealEarlierMessages
  } = lista;
  return (
    <div
                className="message-list"
                aria-live="polite"
                ref={messageListRef}
                onScroll={(event) => {
                  followConversationScrollRef.current = shouldFollowConversationScroll(
                    event.currentTarget
                  );
                }}
              >
                {conversation.messages.length === 0 && (
                  <div className="chat-empty">
                    <span className="pill">
                      {conversation.projectId ? "Conversación de proyecto" : "Nueva conversación"}
                    </span>
                    <h2>¿En qué quieres trabajar?</h2>
                    <p>El mensaje y su contexto se guardarán antes de contactar con Broker AI.</p>
                    {selectedCustomGpt && selectedCustomGpt.conversationStarters.length > 0 && (
                      <div
                        className="conversation-starters"
                        aria-label={`Iniciadores de ${selectedCustomGpt.name}`}
                      >
                        <strong>Empieza con {selectedCustomGpt.name}</strong>
                        {selectedCustomGpt.conversationStarters.map((starter) => (
                          <button
                            key={starter}
                            onClick={() => setDraft(starter)}
                            disabled={Boolean(currentTurnBlocks)}
                          >
                            {starter}
                          </button>
                        ))}
                      </div>
                    )}
                  </div>
                )}
                {hasEarlierMessages && (
                  <div className="earlier-messages">
                    <button
                      className="secondary"
                      onClick={() => void revealEarlierMessages()}
                      disabled={earlierMessagesBusy}
                    >
                      {earlierMessagesBusy
                        ? "Cargando mensajes anteriores…"
                        : `Mostrar ${EARLIER_MESSAGE_PAGE_SIZE} mensajes anteriores`}
                    </button>
                    {conversation.totalMessageCount !== undefined && (
                      <small>
                        {conversation.totalMessageCount - conversation.messages.length}{" "}
                        mensajes anteriores sin cargar
                      </small>
                    )}
                  </div>
                )}
                {progressiveMessages.visibleItems.map((message) => (
                  <article
                    key={message.id}
                    id={`mensaje-${message.id}`}
                    className={`message ${message.role}`}
                  >
                    <span className="message-role">
                      {message.role === "user" ? "Tú" : "ChatyGPT"}
                    </span>
                    {message.status === "pending" ? (
                      <div className="real-progress">
                        <span /> {
                          currentTurn?.state === "ready"
                            ? currentProgress?.total
                              ? `${currentProgress.label} · ${currentProgress.completed}/${currentProgress.total}`
                              : currentProgress?.label ?? "Esperando resultado"
                            : `Esperando resultado · ${message.taskRemoteStatus ?? "recuperando"}`
                        }
                        {currentProgress?.total && (
                          <progress
                            max={currentProgress.total}
                            value={currentProgress.completed ?? 0}
                            aria-label={currentProgress.label}
                          />
                        )}
                        {currentTurn?.state === "ready" && currentTurn.value.cancelRequested && (
                          <small role="status">
                            Cancelación solicitada: se confirmará en cuanto Broker AI responda. La
                            pregunta no se volverá a enviar.
                          </small>
                        )}
                        {currentTurn?.state === "ready" &&
                          (currentTurn.value.unverifiedCapabilities?.length ?? 0) > 0 && (
                            <small role="status">
                              No se pudo comprobar antes de enviar que Broker AI ofrezca{" "}
                              {currentTurn.value.unverifiedCapabilities!
                                .map((capability) =>
                                  capability === "sandbox"
                                    ? "Código aislado"
                                    : capability === "research"
                                      ? "las herramientas de investigación"
                                      : "las dependencias documentales"
                                )
                                .join(" y ")}
                              . El Broker lo validará y puede rechazar la petición.
                            </small>
                          )}
                        {currentTurn?.state === "ready" &&
                          (currentTurn.value.progress.phase === "waiting_for_memory" ||
                            currentTurn.value.remoteStatus === "waiting_for_memory") && (
                            <small>
                              No requiere ninguna acción: conserva su turno y continuará sola
                              cuando haya memoria disponible.
                            </small>
                          )}
                        {currentTurn?.state === "ready" &&
                          (currentTurn.value.progress.phase === "waiting_for_dependencies" ||
                            currentTurn.value.remoteStatus === "waiting_for_dependencies") && (
                            <small>
                              La pregunta ya está encolada y empezará sola cuando termine el lote
                              de indexación documental existente.
                            </small>
                          )}
                      </div>
                    ) : message.text ? (
                      message.role === "assistant" ? (
                        <MarkdownContent text={message.text} />
                      ) : (
                        <div className="message-text">{message.text}</div>
                      )
                    ) : message.status === "cancelled" ? (
                      // Cancelar es una decisión de la persona, no un fallo del
                      // Broker: se dice así, sin culpar a nadie ni invitar a revisar.
                      <div className="task-cancelled" role="status">
                        <strong>Respuesta cancelada</strong>
                        <span>
                          {message.error?.code === "CANCELLED_LOCALLY"
                            ? "La pregunta se canceló antes de enviarse a Broker AI; no se ejecutará."
                            : "La cancelaste antes de que terminara. Puedes volver a preguntar cuando quieras."}
                        </span>
                      </div>
                    ) : message.error ? (
                      (() => {
                        const failure = taskFailureSummary(message.error);
                        if (!failure) return null;
                        return (
                          <div className="task-failure" role="alert">
                            <strong>{failure.title}</strong>
                            <span>{failure.detail}</span>
                            {failure.guidance && <span>{failure.guidance}</span>}
                            <small>
                              {failure.retryable
                                ? "El Broker indica que puede tener sentido volver a intentarlo."
                                : "Revisa las opciones o el contenido antes de repetir la petición."}
                            </small>
                          </div>
                        );
                      })()
                    ) : null}
                    {message.role === "assistant" &&
                      message.brokerTaskId &&
                      conversation.researchRuns
                        .filter((run) => run.brokerTaskId === message.brokerTaskId)
                        .map((run) => (
                          <section
                            className={`research-run research-run-${run.status}`}
                            key={run.id}
                            aria-label="Progreso de Investigación profunda"
                          >
                            <div className="research-run-heading">
                              <span>
                                <strong>Investigación profunda</strong>
                                <small>{run.objective}</small>
                              </span>
                              <span className="research-status">
                                {run.status === "planning"
                                  ? "Planificando"
                                  : run.status === "researching"
                                    ? "Investigando"
                                    : run.status === "synthesizing"
                                      ? "Sintetizando"
                                      : run.status === "completed"
                                        ? "Completada"
                                        : run.status === "cancelled"
                                          ? "Cancelada"
                                          : "Fallida"}
                              </span>
                            </div>
                            <ol className="research-steps">
                              {run.steps.map((step) => (
                                <li className={`research-step research-step-${step.status}`} key={step.id}>
                                  <span aria-hidden="true" />
                                  <span>
                                    <strong>{step.title}</strong>
                                    <small>
                                      {step.status === "running"
                                        ? "En curso"
                                        : step.status === "completed"
                                          ? "Completada"
                                          : step.status === "failed"
                                            ? "Fallida"
                                            : step.status === "cancelled"
                                              ? "Cancelada"
                                              : "Pendiente"}
                                    </small>
                                  </span>
                                </li>
                              ))}
                            </ol>
                            {run.status === "completed" && (
                              <small className="research-source-count">
                                {run.sourceCount} fuente(s) trazable(s) asociada(s) al informe.
                              </small>
                            )}
                          </section>
                        ))}
                    {message.role === "assistant" &&
                      message.consensusSynthesized === false && (
                        <div className="consensus-warning" role="status">
                          <strong>Respuesta sin consenso completo</strong>
                          <span>
                            Los revisores no pudieron sintetizar una respuesta. El Broker entregó
                            la mejor propuesta disponible
                            {(message.arbiterFailureCount ?? 0) > 0
                              ? ` después de ${message.arbiterFailureCount} fallo(s) del revisor.`
                              : "."}
                          </span>
                          {(message.consensusWarnings?.length ?? 0) > 0 && (
                            <small>{message.consensusWarnings?.join(" · ")}</small>
                          )}
                        </div>
                      )}
                    {message.role === "assistant" &&
                      (message.executionWarnings?.length ?? 0) > 0 && (
                        <div className="consensus-warning" role="status">
                          <strong>La respuesta se generó con advertencias</strong>
                          <span>
                            El Broker continuó, pero alguna dependencia no terminó como se esperaba.
                          </span>
                          <small>{message.executionWarnings?.join(" · ")}</small>
                        </div>
                      )}
                    {message.role === "assistant" &&
                      (message.unsupportedCitationUrls?.length ?? 0) > 0 && (
                        <div className="consensus-warning" role="status">
                          <strong>Hay enlaces que el agente no llegó a consultar</strong>
                          <span>
                            Comprueba estas referencias antes de confiar en ellas:
                          </span>
                          <small>{message.unsupportedCitationUrls?.join(" · ")}</small>
                        </div>
                      )}
                    {message.role === "assistant" &&
                      (message.modelUsed ||
                        message.responseDurationMs !== undefined ||
                        message.usage ||
                        message.fallbackUsed ||
                        message.longContext) && (
                        <div className="message-meta">
                          {message.modelUsed && (
                            <small>
                              Modelo: {message.modelUsed.provider} · {message.modelUsed.model}
                            </small>
                          )}
                          {formatResponseDuration(message.responseDurationMs) && (
                            <small>
                              Tiempo de respuesta:{" "}
                              {formatResponseDuration(message.responseDurationMs)}
                            </small>
                          )}
                          {formatResponseUsage(message.usage) && (
                            <small>Uso: {formatResponseUsage(message.usage)}</small>
                          )}
                          {message.fallbackUsed && <small>El Broker utilizó un modelo alternativo</small>}
                          {message.longContext && <small>Contexto largo procesado por el Broker</small>}
                        </div>
                      )}
                    {message.role === "assistant" &&
                      message.brokerTaskId &&
                      message.status !== "pending" && (
                        <>
                          {message.status === "complete" && (
                            <MessageArtifacts localTaskId={message.brokerTaskId} />
                          )}
                          <button
                            className="context-toggle"
                            onClick={() => void toggleTaskContext(message.brokerTaskId!)}
                            aria-expanded={contextPanel?.taskId === message.brokerTaskId}
                          >
                            {contextPanel?.taskId === message.brokerTaskId
                              ? "Ocultar contexto"
                              : "Ver contexto utilizado"}
                          </button>
                          {contextPanel?.taskId === message.brokerTaskId && (
                            <section className="context-panel" aria-label="Contexto utilizado">
                              {contextPanel.data.state === "loading" && (
                                <p>Recuperando el contexto guardado…</p>
                              )}
                              {contextPanel.data.state === "error" && (
                                <p className="error">{contextPanel.data.message}</p>
                              )}
                              {contextPanel.data.state === "ready" && (
                                <>
                                  <header>
                                    <div>
                                      <strong>Contexto utilizado</strong>
                                      <small>{contextPanel.data.value.strategy}</small>
                                    </div>
                                    <span>
                                      ~{contextPanel.data.value.estimatedTokens.toLocaleString("es-ES")} tokens
                                    </span>
                                  </header>
                                  <div className="context-source-list">
                                    {contextPanel.data.value.system1 && (
                                      <p>
                                        {contextPanel.data.value.system1.mode === "applied"
                                          ? `Contexto priorizado: ${contextPanel.data.value.system1.excluded} fuentes opcionales retiradas.`
                                          : contextPanel.data.value.system1.mode === "shadow"
                                            ? `Evaluación en modo sombra: se conservaron todas las fuentes (${contextPanel.data.value.system1.proposedExcluded} descartes propuestos).`
                                            : contextPanel.data.value.system1.fallback
                                              ? "Se conservó el contexto habitual porque la evaluación no fue concluyente."
                                              : "Se conservó el contexto habitual: no necesitaba priorización."}
                                        {" "}Estimación del texto enviado: {contextPanel.data.value.system1.tokensBefore.toLocaleString("es-ES")}
                                        {" → "}{contextPanel.data.value.system1.tokensAfter.toLocaleString("es-ES")} tokens.
                                      </p>
                                    )}
                                    {contextPanel.data.value.sources.map((source, index) => (
                                      <article key={`${source.kind}-${index}`}>
                                        <div className="context-source-heading">
                                          <strong>{source.label}</strong>
                                          <span>
                                            {source.score !== undefined &&
                                              `${Math.round(source.score * 100)}% · `}
                                            ~{source.estimatedTokens.toLocaleString("es-ES")} tokens
                                          </span>
                                        </div>
                                        <small>{source.reason}</small>
                                        <p>{source.excerpt}</p>
                                        {source.kind === "attachment_chunk" && source.sourceReference && (
                                          <div className="context-source-actions">
                                            <button
                                              className="secondary"
                                              onClick={() => void revealContextSource(
                                                contextPanel.taskId,
                                                source.sourceReference!
                                              )}
                                              disabled={
                                                !canRevealContextSource(source) ||
                                                (contextSourceAction?.taskId === contextPanel.taskId &&
                                                  contextSourceAction.state === "loading")
                                              }
                                              title={
                                                source.sourceAvailable
                                                  ? "Selecciona la copia local administrada por ChatyGPT en el Explorador"
                                                  : "La copia local ya no está disponible"
                                              }
                                            >
                                              {contextSourceAction?.taskId === contextPanel.taskId &&
                                              contextSourceAction.reference === source.sourceReference &&
                                              contextSourceAction.state === "loading"
                                                ? "Mostrando…"
                                                : "Mostrar archivo"}
                                            </button>
                                            {!source.sourceAvailable && (
                                              <small>La copia local de esta fuente ya no está disponible.</small>
                                            )}
                                            {contextSourceAction?.taskId === contextPanel.taskId &&
                                              contextSourceAction.reference === source.sourceReference &&
                                              contextSourceAction.state !== "loading" && (
                                                <small
                                                  className={contextSourceAction.state}
                                                  role={contextSourceAction.state === "error" ? "alert" : "status"}
                                                  aria-live="polite"
                                                >
                                                  {contextSourceAction.message}
                                                </small>
                                              )}
                                          </div>
                                        )}
                                      </article>
                                    ))}
                                  </div>
                                </>
                              )}
                            </section>
                          )}
                        </>
                      )}
                    {message.sources.length > 0 && (
                      <section className="message-sources" aria-label="Fuentes usadas">
                        <h4>Fuentes usadas</h4>
                        <div className="source-list">
                          {message.sources.map((source, index) => (
                            <article key={source.id} className="source-card">
                              <span>{index + 1}</span>
                              <div>
                                <strong>{source.title}</strong>
                                <small>
                                  {source.url ? "Fuente web" : source.mediaType ?? "Archivo adjunto"}
                                  {source.sizeBytes !== undefined &&
                                    ` · ${(source.sizeBytes / 1024).toFixed(1)} KB`}
                                </small>
                                {source.url && (
                                  <span className="source-url" title={source.url}>
                                    {source.url}
                                  </span>
                                )}
                                {source.quoteText && <p>{source.quoteText}</p>}
                              </div>
                            </article>
                          ))}
                        </div>
                        <p className="source-disclaimer">
                          Fuentes asociadas de forma durable a esta respuesta. Los enlaces web
                          proceden del informe generado; no implican por sí solos una cita por frase.
                        </p>
                      </section>
                    )}
                  </article>
                ))}
              </div>
  );
}
