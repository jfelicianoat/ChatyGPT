/**
 * Tarjeta de la memoria personal.
 *
 * Recibe el hook entero (`useMemoria`): el estado y las acciones que lo mueven
 * ya viajan juntos, y asi la tarjeta no necesita treinta props sueltas.
 */
import { canStartMemoryEdit, type ProjectSummary } from "../domain";
import type { MemoryEditDraft, useMemoria } from "./useMemoria";

type Props = {
  memoria: ReturnType<typeof useMemoria>;
  projects: ProjectSummary[];
};

export function TarjetaMemoria({ memoria, projects }: Props) {
  const {
    memory,
    memoryDraft,
    setMemoryDraft,
    memoryCategory,
    setMemoryCategory,
    memoryProjectId,
    setMemoryProjectId,
    memorySensitive,
    setMemorySensitive,
    memoryBusy,
    memoryEditingId,
    memoryEditDraft,
    setMemoryEditDraft,
    memoryEditError,
    memoryNotice,
    memorySearchQuery,
    setMemorySearchQuery,
    memorySearchProjectId,
    setMemorySearchProjectId,
    memorySearch,
    toggleMemory,
    createMemory,
    toggleMemoryItem,
    beginMemoryEdit,
    cancelMemoryEdit,
    saveMemoryEdit,
    removeMemoryItem,
    reindexMemoryItem,
    runMemorySearch
  } = memoria;

  return (
  <section className="memory-card">
    <div className="panel-heading">
      <div>
        <span className="kicker">Fase 2 · Contexto personal</span>
        <h3>Memoria</h3>
      </div>
      {memory.state === "ready" && (
        <button
          className={memory.value.enabled ? "secondary" : "primary"}
          onClick={toggleMemory}
          disabled={memoryBusy}
        >
          {memory.value.enabled ? "Desactivar memoria" : "Activar memoria"}
        </button>
      )}
    </div>
    <p className="muted">
      Solo se reutiliza lo que añadas aquí. ChatyGPT no crea recuerdos automáticamente.
    </p>
    {memory.state === "ready" && (
      <>
        <div className={`memory-status ${memory.value.enabled ? "enabled" : ""}`}>
          {memory.value.enabled
            ? "Memoria activa: los recuerdos habilitados se añadirán al contexto de los próximos mensajes."
            : "Memoria desactivada: ningún recuerdo se enviará al Broker."}
        </div>
        {memoryNotice && (
          <p className="memory-update-notice" role="status" aria-live="polite">
            {memoryNotice}
          </p>
        )}
        <div className="memory-form">
          <textarea
            value={memoryDraft}
            onChange={(event) => setMemoryDraft(event.target.value)}
            placeholder="Ejemplo: Prefiero respuestas breves y en español."
            rows={2}
            maxLength={2000}
            disabled={memoryBusy}
          />
          <div className="memory-form-controls">
            <select
              value={memoryCategory}
              onChange={(event) => setMemoryCategory(event.target.value as "preference" | "instruction" | "fact")}
              disabled={memoryBusy}
              aria-label="Categoría del recuerdo"
            >
              <option value="preference">Preferencia</option>
              <option value="instruction">Instrucción</option>
              <option value="fact">Dato</option>
            </select>
            <select
              value={memoryProjectId}
              onChange={(event) => setMemoryProjectId(event.target.value)}
              disabled={memoryBusy}
              aria-label="Ámbito del recuerdo"
            >
              <option value="global">Todos los chats</option>
              {projects.map((project) => (
                <option key={project.id} value={project.id}>{project.name}</option>
              ))}
            </select>
            <label className="memory-sensitive">
              <input
                type="checkbox"
                checked={memorySensitive}
                onChange={(event) => setMemorySensitive(event.target.checked)}
                disabled={memoryBusy}
              />
              Marcar como sensible
            </label>
            <button
              className="primary"
              onClick={createMemory}
              disabled={memoryBusy || !memoryDraft.trim()}
            >
              Guardar recuerdo
            </button>
          </div>
        </div>
        <div className="memory-search-box">
          <div>
            <span className="kicker">Prueba semántica</span>
            <h4>Buscar recuerdos por significado</h4>
            <p className="muted">
              Compara una frase con los recuerdos habilitados que ya tienen el índice preparado.
            </p>
          </div>
          <div className="memory-search-controls">
            <input
              value={memorySearchQuery}
              onChange={(event) => setMemorySearchQuery(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && memorySearchQuery.trim()) {
                  event.preventDefault();
                  void runMemorySearch();
                }
              }}
              placeholder="Ejemplo: ¿Cómo prefiere el usuario las respuestas?"
              maxLength={500}
              disabled={memorySearch?.state === "loading" || (memorySearch?.state === "ready" && memorySearch.value.status === "searching")}
            />
            <select
              value={memorySearchProjectId}
              onChange={(event) => setMemorySearchProjectId(event.target.value)}
              aria-label="Ámbito de la búsqueda semántica"
              disabled={memorySearch?.state === "loading" || (memorySearch?.state === "ready" && memorySearch.value.status === "searching")}
            >
              <option value="global">Todos los chats</option>
              {projects.map((project) => (
                <option key={project.id} value={project.id}>{project.name}</option>
              ))}
            </select>
            <button
              className="secondary"
              onClick={runMemorySearch}
              disabled={!memorySearchQuery.trim() || memorySearch?.state === "loading" || (memorySearch?.state === "ready" && memorySearch.value.status === "searching")}
            >
              {memorySearch?.state === "loading" || (memorySearch?.state === "ready" && memorySearch.value.status === "searching")
                ? "Buscando…"
                : "Buscar"}
            </button>
          </div>
          {memorySearch?.state === "error" && <p className="error">{memorySearch.message}</p>}
          {memorySearch?.state === "ready" && memorySearch.value.status === "failed" && (
            <p className="error">No se pudo completar la búsqueda: {memorySearch.value.error ?? "error desconocido"}</p>
          )}
          {memorySearch?.state === "ready" && memorySearch.value.status === "completed" && (
            <div className="memory-search-results">
              <small>Modelo local: {memorySearch.value.model ?? "no identificado"}</small>
              {memorySearch.value.results.length === 0 ? (
                <p className="activity-empty">No hay coincidencias suficientes en este ámbito.</p>
              ) : memorySearch.value.results.map((result) => (
                <article key={result.memoryId}>
                  <div className="memory-search-result-heading">
                    <strong>{Math.round(result.score * 100)}% · {result.reason}</strong>
                    <span>{result.projectName ?? "Todos los chats"}</span>
                  </div>
                  <p>{result.content}</p>
                </article>
              ))}
            </div>
          )}
        </div>
        {memory.value.items.length === 0 ? (
          <p className="activity-empty">Todavía no has guardado recuerdos.</p>
        ) : (
          <div className="memory-list">
            {memory.value.items.map((item) => (
              <article
                className={`memory-item ${item.enabled ? "" : "disabled"} ${memoryEditingId === item.id ? "editing" : ""}`}
                data-memory-id={item.id}
                key={item.id}
              >
                {memoryEditingId === item.id && memoryEditDraft ? (
                  <div className="memory-editor">
                    <label>
                      <span>Contenido del recuerdo</span>
                      <textarea
                        value={memoryEditDraft.content}
                        onChange={(event) => setMemoryEditDraft({
                          ...memoryEditDraft,
                          content: event.target.value
                        })}
                        rows={3}
                        maxLength={2000}
                        disabled={memoryBusy}
                        autoFocus
                      />
                    </label>
                    <div className="memory-editor-controls">
                      <label>
                        <span>Categoría</span>
                        <select
                          value={memoryEditDraft.category}
                          onChange={(event) => setMemoryEditDraft({
                            ...memoryEditDraft,
                            category: event.target.value as MemoryEditDraft["category"]
                          })}
                          disabled={memoryBusy}
                        >
                          <option value="preference">Preferencia</option>
                          <option value="instruction">Instrucción</option>
                          <option value="fact">Dato</option>
                        </select>
                      </label>
                      <label>
                        <span>Ámbito</span>
                        <select
                          value={memoryEditDraft.projectId}
                          onChange={(event) => setMemoryEditDraft({
                            ...memoryEditDraft,
                            projectId: event.target.value
                          })}
                          disabled={memoryBusy}
                        >
                          <option value="global">Todos los chats</option>
                          {projects.map((project) => (
                            <option key={project.id} value={project.id}>{project.name}</option>
                          ))}
                        </select>
                      </label>
                      <label className="memory-sensitive">
                        <input
                          type="checkbox"
                          checked={memoryEditDraft.sensitive}
                          onChange={(event) => setMemoryEditDraft({
                            ...memoryEditDraft,
                            sensitive: event.target.checked
                          })}
                          disabled={memoryBusy}
                        />
                        Sensible
                      </label>
                    </div>
                    <small>
                      Si cambias el texto, ChatyGPT descartará cualquier índice
                      anterior o en curso y preparará uno nuevo automáticamente.
                    </small>
                    {memoryEditError && <p className="error" role="alert">{memoryEditError}</p>}
                    <div className="memory-editor-actions">
                      <button
                        className="secondary"
                        onClick={cancelMemoryEdit}
                        disabled={memoryBusy}
                      >
                        Cancelar
                      </button>
                      <button
                        className="primary"
                        onClick={saveMemoryEdit}
                        disabled={memoryBusy || !memoryEditDraft.content.trim()}
                      >
                        {memoryBusy ? "Guardando…" : "Guardar cambios"}
                      </button>
                    </div>
                  </div>
                ) : (
                  <>
                    <div className="memory-item-copy">
                      <div className="memory-badges">
                        <span>{item.category === "preference" ? "Preferencia" : item.category === "instruction" ? "Instrucción" : "Dato"}</span>
                        <span>{item.projectName ?? "Todos los chats"}</span>
                        {item.sensitivity === "sensitive" && <span className="sensitive">Sensible</span>}
                        <span
                          className={`embedding ${item.embeddingStatus}`}
                          title={item.embeddingModel ?? "Índice semántico local"}
                        >
                          {item.embeddingStatus === "ready"
                            ? "Índice preparado"
                            : item.embeddingStatus === "indexing"
                              ? "Indexando…"
                              : item.embeddingStatus === "failed"
                                ? "Error de índice"
                                : "Sin índice"}
                        </span>
                      </div>
                      <p>{item.content}</p>
                      {item.embeddingStatus === "failed" && item.embeddingError && (
                        <small className="memory-index-error">
                          No se pudo indexar: {item.embeddingError}
                        </small>
                      )}
                    </div>
                    <div className="memory-actions">
                      <button
                        className="secondary memory-edit-button"
                        onClick={() => beginMemoryEdit(item)}
                        disabled={memoryBusy || !canStartMemoryEdit(memoryEditingId)}
                      >
                        Editar
                      </button>
                      <label>
                        <input
                          type="checkbox"
                          checked={item.enabled}
                          onChange={(event) => toggleMemoryItem(item.id, event.target.checked)}
                          disabled={memoryBusy}
                        />
                        Usar
                      </label>
                      {item.embeddingStatus !== "ready" && item.embeddingStatus !== "indexing" && (
                        <button
                          className="secondary"
                          onClick={() => reindexMemoryItem(item.id)}
                          disabled={memoryBusy}
                        >
                          Indexar
                        </button>
                      )}
                      <button
                        className="danger-text"
                        onClick={() => removeMemoryItem(item.id)}
                        disabled={memoryBusy}
                      >
                        Eliminar
                      </button>
                    </div>
                  </>
                )}
              </article>
            ))}
          </div>
        )}
      </>
    )}
    {memory.state === "loading" && <p className="muted">Cargando memoria…</p>}
    {memory.state === "error" && <p className="error">{memory.message}</p>}
  </section>
  );
}
