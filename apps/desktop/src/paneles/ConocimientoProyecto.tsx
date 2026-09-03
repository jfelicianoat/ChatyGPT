/** El conocimiento de un proyecto: ficheros y recuerdos, con su filtro. */
import type { RefObject } from "react";

import type { DialogState } from "../dialogs";
import {
  attachmentImagePolicyLabel,
  attachmentStatusLabel,
  type FilteredProjectKnowledge,
  type Loadable,
  type ProjectKnowledgeFilter,
  type ProjectKnowledgeOverview,
} from "../domain";

export function ConocimientoProyecto({
  projectKnowledge,
  filteredProjectKnowledge,
  projectKnowledgeQuery,
  projectKnowledgeFilter,
  projectKnowledgeBusyId,
  projectKnowledgeActionError,
  activeModalRef,
  setProjectKnowledge,
  setProjectKnowledgeQuery,
  setProjectKnowledgeFilter,
  openDialog,
  openConversationFromProjectKnowledge,
  removeFileFromProjectKnowledge,
  toggleProjectMemoryFromKnowledge,
}: {
  projectKnowledge: Loadable<ProjectKnowledgeOverview> | null;
  filteredProjectKnowledge: FilteredProjectKnowledge | null;
  projectKnowledgeQuery: string;
  projectKnowledgeFilter: ProjectKnowledgeFilter;
  projectKnowledgeBusyId: string | null;
  projectKnowledgeActionError: string | null;
  activeModalRef: RefObject<HTMLElement | null>;
  setProjectKnowledge: (overview: Loadable<ProjectKnowledgeOverview> | null) => void;
  setProjectKnowledgeQuery: (query: string) => void;
  setProjectKnowledgeFilter: (filter: ProjectKnowledgeFilter) => void;
  openDialog: (dialog: DialogState) => void;
  openConversationFromProjectKnowledge: (conversationId: string) => void | Promise<void>;
  removeFileFromProjectKnowledge: (
    projectId: string,
    attachmentId: string,
    displayName: string
  ) => void | Promise<void>;
  toggleProjectMemoryFromKnowledge: (
    projectId: string,
    memoryId: string,
    enabled: boolean
  ) => void | Promise<void>;
}) {
  if (!projectKnowledge) return null;
  return (
    <div className="modal-backdrop" role="presentation">
      <section
        ref={activeModalRef}
        className="modal project-knowledge-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="project-knowledge-title"
        tabIndex={-1}
      >
        {projectKnowledge.state === "loading" && (
          <>
            <span className="kicker">Conocimiento del proyecto</span>
            <h2 id="project-knowledge-title">Reuniendo fuentes…</h2>
            <p>Consultando instrucciones, archivos y recuerdos guardados.</p>
          </>
        )}
        {projectKnowledge.state === "error" && (
          <>
            <span className="kicker">Conocimiento del proyecto</span>
            <h2 id="project-knowledge-title">No se pudo abrir la vista</h2>
            <p className="error">{projectKnowledge.message}</p>
          </>
        )}
        {projectKnowledge.state === "ready" && (
          <>
            <span className="kicker">Conocimiento del proyecto</span>
            <h2 id="project-knowledge-title">{projectKnowledge.value.project.name}</h2>
            <div className="project-knowledge-stats">
              <span>
                <strong>{projectKnowledge.value.project.conversationCount}</strong>
                chats
              </span>
              <span>
                <strong>{projectKnowledge.value.files.length}</strong>
                archivos
              </span>
              <span>
                <strong>{projectKnowledge.value.memories.length}</strong>
                recuerdos
              </span>
            </div>

            <div className="project-knowledge-search">
              <label htmlFor="project-knowledge-query">
                Buscar archivos y recuerdos
              </label>
              <div>
                <input
                  id="project-knowledge-query"
                  type="search"
                  value={projectKnowledgeQuery}
                  onChange={(event) => setProjectKnowledgeQuery(event.target.value)}
                  placeholder="Nombre del archivo o contenido del recuerdo"
                  autoComplete="off"
                />
                {projectKnowledgeQuery && (
                  <button
                    type="button"
                    onClick={() => setProjectKnowledgeQuery("")}
                    aria-label="Limpiar búsqueda"
                  >
                    Limpiar
                  </button>
                )}
              </div>
              <div
                className="project-knowledge-filters"
                role="group"
                aria-label="Tipo de conocimiento"
              >
                {([
                  ["all", "Todo"],
                  ["files", "Archivos"],
                  ["memories", "Recuerdos"]
                ] as const).map(([value, label]) => (
                  <button
                    type="button"
                    key={value}
                    className={projectKnowledgeFilter === value ? "active" : ""}
                    aria-pressed={projectKnowledgeFilter === value}
                    onClick={() => setProjectKnowledgeFilter(value)}
                  >
                    {label}
                  </button>
                ))}
                <span aria-live="polite">
                  {filteredProjectKnowledge?.total ?? 0} resultado(s)
                </span>
              </div>
            </div>

            <div className="project-knowledge-sections">
              {projectKnowledgeFilter === "all" && (
              <section>
                <header>
                  <strong>Instrucciones</strong>
                  <span>
                    {projectKnowledge.value.project.instructions
                      ? "Configuradas"
                      : "Sin configurar"}
                  </span>
                </header>
                <p>
                  {projectKnowledge.value.project.instructions
                    ?? "Este proyecto todavía no tiene instrucciones reutilizables."}
                </p>
              </section>
              )}

              {projectKnowledgeFilter !== "memories" && (
              <section>
                <header>
                  <strong>Archivos reutilizables</strong>
                  <span>
                    {filteredProjectKnowledge?.files.length ?? 0}
                    {" de "}
                    {projectKnowledge.value.files.length}
                  </span>
                </header>
                {filteredProjectKnowledge?.files.length === 0 ? (
                  <p>
                    {projectKnowledgeQuery
                      ? "Ningún archivo coincide con la búsqueda."
                      : "No hay archivos guardados en este proyecto."}
                  </p>
                ) : (
                  <div className="project-knowledge-list">
                    {filteredProjectKnowledge?.files.map((file) => {
                      const conversations =
                        projectKnowledge.value.fileUsages.find(
                          (usage) => usage.attachmentId === file.id
                        )?.conversations ?? [];
                      return (
                        <article className="project-knowledge-item" key={file.id}>
                          <div>
                            <strong>{file.displayName}</strong>
                            <span>
                              {attachmentStatusLabel(file.ingestionStatus)}
                              {attachmentImagePolicyLabel(file) &&
                                ` · ${attachmentImagePolicyLabel(file)}`}
                              {" · "}
                              {file.chunkCount} fragmentos
                            </span>
                            <div className="project-knowledge-uses">
                              <span>
                                {conversations.length === 0
                                  ? "Todavía no se usa en ningún chat activo"
                                  : conversations.length === 1
                                    ? "Usado en 1 chat"
                                    : `Usado en ${conversations.length} chats`}
                              </span>
                              {conversations.length > 0 && (
                                <div className="project-knowledge-chat-links">
                                  {conversations.map((usedBy) => (
                                    <button
                                      key={usedBy.id}
                                      onClick={() => void openConversationFromProjectKnowledge(
                                        usedBy.id
                                      )}
                                      title={`Abrir ${usedBy.title}`}
                                    >
                                      {usedBy.title}
                                    </button>
                                  ))}
                                </div>
                              )}
                            </div>
                          </div>
                          <button
                            className="danger-text"
                            onClick={() => void removeFileFromProjectKnowledge(
                              projectKnowledge.value.project.id,
                              file.id,
                              file.displayName
                            )}
                            disabled={projectKnowledgeBusyId === file.id}
                          >
                            {projectKnowledgeBusyId === file.id
                              ? "Retirando…"
                              : "Retirar del proyecto"}
                          </button>
                        </article>
                      );
                    })}
                  </div>
                )}
              </section>
              )}

              {projectKnowledgeFilter !== "files" && (
              <section>
                <header>
                  <strong>Recuerdos del proyecto</strong>
                  <span>
                    {projectKnowledge.value.memoryEnabled
                      ? "Memoria activada"
                      : "Memoria desactivada"}
                  </span>
                </header>
                {filteredProjectKnowledge?.memories.length === 0 ? (
                  <p>
                    {projectKnowledgeQuery
                      ? "Ningún recuerdo coincide con la búsqueda."
                      : "No hay recuerdos limitados a este proyecto."}
                  </p>
                ) : (
                  <div className="project-knowledge-list">
                    {filteredProjectKnowledge?.memories.map((item) => (
                      <article className="project-knowledge-item" key={item.id}>
                        <div>
                          <strong>{item.content}</strong>
                          <span>
                            {item.category === "preference"
                              ? "Preferencia"
                              : item.category === "instruction"
                                ? "Instrucción"
                                : "Dato"}
                            {" · "}
                            {item.enabled ? "Activo" : "Desactivado"}
                            {item.sensitivity === "sensitive" ? " · Sensible" : ""}
                          </span>
                        </div>
                        <button
                          onClick={() => void toggleProjectMemoryFromKnowledge(
                            projectKnowledge.value.project.id,
                            item.id,
                            !item.enabled
                          )}
                          disabled={projectKnowledgeBusyId === item.id}
                        >
                          {projectKnowledgeBusyId === item.id
                            ? "Guardando…"
                            : item.enabled
                              ? "Desactivar"
                              : "Activar"}
                        </button>
                      </article>
                    ))}
                  </div>
                )}
              </section>
              )}
            </div>
            {projectKnowledgeActionError && (
              <p className="project-knowledge-error" role="alert">
                {projectKnowledgeActionError}
              </p>
            )}
          </>
        )}
        <div className="modal-actions">
          {projectKnowledge.state === "ready" && (
            <button
              className="secondary"
              onClick={() => {
                const project = projectKnowledge.value.project;
                setProjectKnowledge(null);
                openDialog({ kind: "project-instructions", project });
              }}
            >
              Editar instrucciones
            </button>
          )}
          <button className="primary" onClick={() => setProjectKnowledge(null)}>
            Cerrar
          </button>
        </div>
      </section>
    </div>
  );
}
