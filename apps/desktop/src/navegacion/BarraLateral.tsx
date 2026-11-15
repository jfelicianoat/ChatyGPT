/**
 * Barra lateral: marca, nueva conversación, búsqueda, espacios, proyectos,
 * conversaciones (recientes y resultados de búsqueda), acciones del proyecto,
 * atajos de teclado y pie de estado.
 *
 * No tiene estado propio: la búsqueda y la navegación viven en `App.tsx` y
 * entran como props. Extraída de App.tsx (CH-03) tal cual, sin cambiar el DOM.
 */
import type { RefObject } from "react";
import {
  type BootstrapReport,
  type ConversationSearchHit,
  type ConversationSummary,
  type ConversationView,
  type Loadable,
  type ProjectSummary
} from "../domain";
import type { DialogState } from "../dialogs";
import type { WorkspaceDestination } from "../navegacion";

type Props = {
  appVersion: string;
  bootstrap: Loadable<BootstrapReport>;
  conversation: Loadable<ConversationView> | null;
  conversations: ConversationSummary[];
  projects: ProjectSummary[];
  /** Proyecto seleccionado según la navegación, si lo hay. */
  selectedProject: ProjectSummary | null;
  selectedProjectId: string | null;
  setSelectedProjectId: (id: string | null) => void;
  workspaceDestination: WorkspaceDestination;
  openWorkspaceDestination: (destination: WorkspaceDestination) => void;
  searchInputRef: RefObject<HTMLInputElement | null>;
  searchQuery: string;
  setSearchQuery: (query: string) => void;
  searchIncludesArchived: boolean;
  setSearchIncludesArchived: (value: boolean) => void;
  searchResults: ConversationSearchHit[];
  /** Conversaciones visibles ya filtradas y ordenadas (H21). */
  visibleConversationList: ConversationSummary[];
  createConversation: () => void;
  openConversation: (conversationId: string) => Promise<void>;
  openSearchHit: (hit: ConversationSearchHit) => Promise<void>;
  openDialog: (dialog: DialogState) => void;
  openProjectKnowledge: (project: ProjectSummary) => Promise<void>;
  navigationError: string | null;
  setKeyboardHelpOpen: (open: boolean) => void;
};

export function BarraLateral({
  appVersion,
  bootstrap,
  conversation,
  conversations,
  projects,
  selectedProject,
  selectedProjectId,
  setSelectedProjectId,
  workspaceDestination,
  openWorkspaceDestination,
  searchInputRef,
  searchQuery,
  setSearchQuery,
  searchIncludesArchived,
  setSearchIncludesArchived,
  searchResults,
  visibleConversationList,
  createConversation,
  openConversation,
  openSearchHit,
  openDialog,
  openProjectKnowledge,
  navigationError,
  setKeyboardHelpOpen
}: Props) {
  return (
    <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">C</span>
          <div><strong>ChatyGPT</strong><small>Espacio personal · v{appVersion}</small></div>
        </div>

        <button
          className="new-chat"
          onClick={createConversation}
          aria-keyshortcuts="Control+N"
          title="Nueva conversación (Ctrl+N)"
        >
          ＋ Nueva conversación
        </button>

        <label className="search-box">
          <span>⌕</span>
          <input
            ref={searchInputRef}
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
            placeholder="Buscar conversaciones"
            aria-label="Buscar conversaciones"
            aria-keyshortcuts="Control+F /"
          />
          {searchQuery && (
            <button onClick={() => setSearchQuery("")} aria-label="Limpiar búsqueda">×</button>
          )}
        </label>
        {searchQuery.trim() && (
          <label className="search-scope">
            <input
              type="checkbox"
              checked={searchIncludesArchived}
              onChange={(event) => setSearchIncludesArchived(event.target.checked)}
            />
            Incluir archivadas
          </label>
        )}

        <nav aria-label="Navegación principal">
          <p className="nav-label">Espacio</p>
          {([
            ["chats", "Chats"],
            ["projects", "Proyectos"],
            ["gpts", "GPTs"],
            ["workflows", "Flujos"],
            ["athena", "Athena"],
            ["automations", "Automatizaciones"],
            ["settings", "Ajustes"],
            ["archive", "Archivo"],
          ] as const).map(([destination, label], index) => (
            <button
              key={destination}
              className={`nav-item ${conversation === null && workspaceDestination === destination ? "active" : ""}`}
              onClick={() => openWorkspaceDestination(destination)}
              aria-current={conversation === null && workspaceDestination === destination ? "page" : undefined}
              aria-keyshortcuts={`Alt+${index + 1}`}
            >
              {label}
            </button>
          ))}

          {(workspaceDestination === "chats" || workspaceDestination === "projects") && (
            <>
              <div className="nav-label-row">
                <p className="nav-label">Proyectos</p>
                <button
                  className="icon-button"
                  onClick={() => openDialog({ kind: "project-create" })}
                  aria-label="Crear proyecto"
                >
                  ＋
                </button>
              </div>
              <button
                className={`project-link ${selectedProjectId === null ? "active" : ""}`}
                onClick={() => setSelectedProjectId(null)}
              >
                <span>Todos los chats</span><small>{conversations.length}</small>
              </button>
              <button
                className={`project-link ${selectedProjectId === "unassigned" ? "active" : ""}`}
                onClick={() => setSelectedProjectId("unassigned")}
              >
                <span>Sin proyecto</span>
                <small>{conversations.filter((item) => !item.projectId).length}</small>
              </button>
              {projects.map((project) => (
                <div className="project-row" key={project.id}>
                  <button
                    className={`project-link ${selectedProjectId === project.id ? "active" : ""}`}
                    onClick={() => setSelectedProjectId(project.id)}
                  >
                    <span>◇ {project.name}</span><small>{project.conversationCount}</small>
                  </button>
                  {selectedProjectId === project.id && (
                    <button
                      className="project-menu"
                      onClick={() => openDialog({ kind: "project-rename", project })}
                      aria-label={`Gestionar ${project.name}`}
                    >
                      •••
                    </button>
                  )}
                </div>
              ))}
            </>
          )}

          <p className="nav-label">
            {searchQuery.trim() ? "Resultados" : selectedProject?.name ?? "Recientes"}
          </p>
          {visibleConversationList.length === 0 ? (
            <div className="empty-nav">
              {searchQuery.trim()
                ? "No hay conversaciones que coincidan."
                : "No hay conversaciones en esta sección."}
            </div>
          ) : visibleConversationList.map((item) => {
            const hit = searchQuery.trim()
              ? searchResults.find((result) => result.id === item.id)
              : undefined;
            return (
              <button
                key={item.id}
                className={`conversation-link ${
                  conversation?.state === "ready" && conversation.value.id === item.id
                    ? "active"
                    : ""
                }${hit ? " search-result" : ""}`}
                onClick={() => (hit ? void openSearchHit(hit) : void openConversation(item.id))}
                aria-current={
                  conversation?.state === "ready" && conversation.value.id === item.id
                    ? "page"
                    : undefined
                }
              >
                <span>
                  {item.title}
                  {hit?.archived && <em className="search-archived"> · archivada</em>}
                </span>
                {hit?.snippet && <small className="search-snippet">{hit.snippet}</small>}
              </button>
            );
          })}
        </nav>

        {selectedProject && (
          <div className="project-actions">
            <button onClick={() => void openProjectKnowledge(selectedProject)}>
              Ver conocimiento
            </button>
            <button
              onClick={() => openDialog({
                kind: "project-instructions",
                project: selectedProject
              })}
            >
              {selectedProject.instructions ? "Editar instrucciones" : "Añadir instrucciones"}
            </button>
            <button
              onClick={() => openDialog({ kind: "project-rename", project: selectedProject })}
            >
              Renombrar
            </button>
            <button
              className="danger-text"
              onClick={() => openDialog({ kind: "project-archive", project: selectedProject })}
            >
              Archivar
            </button>
          </div>
        )}

        {navigationError && <p className="sidebar-error">{navigationError}</p>}
        <button
          className="keyboard-help-button"
          onClick={() => setKeyboardHelpOpen(true)}
          aria-haspopup="dialog"
          aria-keyshortcuts="Shift+/"
        >
          <span>Atajos de teclado</span>
          <kbd>?</kbd>
        </button>
        <div
          className="sidebar-footer"
          title={
            bootstrap.state === "ready" && bootstrap.value.logPath
              ? `Registro de diagnóstico: ${bootstrap.value.logPath}`
              : undefined
          }
        >
          <span className={`status-dot ${bootstrap.state === "ready" ? "ok" : ""}`} />
          {bootstrap.state === "ready"
            ? `ChatyGPT ${bootstrap.value.appVersion} · Datos locales · esquema ${bootstrap.value.schemaVersion}`
            : "Preparando datos locales"}
        </div>
      </aside>
  );
}
