import { useEffect, useMemo, useState } from "react";
import {
  isTaskBlockingConversation,
  isTaskPollingComplete,
  isTerminalTask,
  type BootstrapReport,
  type BrokerDiagnostic,
  type ConversationSummary,
  type ConversationView,
  type LocalTaskSnapshot,
  type ProjectSummary
} from "./domain";
import { platform } from "./platform";

type Loadable<T> =
  | { state: "loading" }
  | { state: "ready"; value: T }
  | { state: "error"; message: string };

type DialogState =
  | { kind: "project-create" }
  | { kind: "project-rename"; project: ProjectSummary }
  | { kind: "project-archive"; project: ProjectSummary }
  | { kind: "conversation-rename"; conversation: ConversationView }
  | { kind: "conversation-archive"; conversation: ConversationView }
  | { kind: "conversation-delete"; conversation: ConversationView };

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function dialogCopy(dialog: DialogState): {
  title: string;
  description: string;
  fieldLabel?: string;
  initialValue?: string;
  destructive?: boolean;
  action: string;
} {
  switch (dialog.kind) {
    case "project-create":
      return {
        title: "Nuevo proyecto",
        description: "Agrupa conversaciones relacionadas sin mover sus datos fuera de SQLite.",
        fieldLabel: "Nombre del proyecto",
        action: "Crear proyecto"
      };
    case "project-rename":
      return {
        title: "Renombrar proyecto",
        description: "Las conversaciones asociadas conservarán su relación con el proyecto.",
        fieldLabel: "Nombre del proyecto",
        initialValue: dialog.project.name,
        action: "Guardar"
      };
    case "project-archive":
      return {
        title: "Archivar proyecto",
        description:
          "El proyecto desaparecerá de la barra lateral. Sus conversaciones seguirán disponibles sin proyecto.",
        destructive: true,
        action: "Archivar"
      };
    case "conversation-rename":
      return {
        title: "Renombrar conversación",
        description: "El contenido y el historial no cambiarán.",
        fieldLabel: "Título",
        initialValue: dialog.conversation.title,
        action: "Guardar"
      };
    case "conversation-archive":
      return {
        title: "Archivar conversación",
        description:
          "La conversación saldrá de la lista activa, pero sus mensajes se conservarán localmente.",
        destructive: true,
        action: "Archivar"
      };
    case "conversation-delete":
      return {
        title: "Eliminar conversación",
        description:
          "La conversación quedará marcada como eliminada. Esta acción no borra físicamente los registros todavía.",
        destructive: true,
        action: "Eliminar"
      };
  }
}

export function App() {
  const [bootstrap, setBootstrap] = useState<Loadable<BootstrapReport>>({ state: "loading" });
  const [broker, setBroker] = useState<Loadable<BrokerDiagnostic> | null>(null);
  const [smokeTask, setSmokeTask] = useState<Loadable<LocalTaskSnapshot> | null>(null);
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [searchResults, setSearchResults] = useState<ConversationSummary[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [conversation, setConversation] = useState<Loadable<ConversationView> | null>(null);
  const [activeTurn, setActiveTurn] = useState<Loadable<LocalTaskSnapshot> | null>(null);
  const [activeTurnConversationId, setActiveTurnConversationId] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [dialog, setDialog] = useState<DialogState | null>(null);
  const [dialogValue, setDialogValue] = useState("");
  const [dialogBusy, setDialogBusy] = useState(false);
  const [navigationError, setNavigationError] = useState<string | null>(null);

  const reloadNavigation = async () => {
    const [nextConversations, nextProjects] = await Promise.all([
      platform.listConversations(),
      platform.listProjects()
    ]);
    setConversations(nextConversations);
    setProjects(nextProjects);
  };

  const loadConversation = async (conversationId: string) => {
    const view = await platform.getConversation(conversationId);
    setConversation({ state: "ready", value: view });
    const pending = [...view.messages]
      .reverse()
      .find((message) => message.status === "pending" && message.brokerTaskId);
    if (pending?.brokerTaskId) {
      try {
        const task = await platform.getLocalTask(pending.brokerTaskId);
        setActiveTurn({ state: "ready", value: task });
        setActiveTurnConversationId(view.id);
      } catch {
        setActiveTurn(null);
        setActiveTurnConversationId(null);
      }
    } else if (activeTurnConversationId !== view.id) {
      setActiveTurn(null);
      setActiveTurnConversationId(null);
    }
  };

  useEffect(() => {
    platform.bootstrap()
      .then(async (value) => {
        setBootstrap({ state: "ready", value });
        const [items, projectItems] = await Promise.all([
          platform.listConversations(),
          platform.listProjects()
        ]);
        setConversations(items);
        setProjects(projectItems);
        if (items[0]) {
          await loadConversation(items[0].id);
        }
      })
      .catch((error) => setBootstrap({ state: "error", message: describeError(error) }));
  }, []);

  useEffect(() => {
    const query = searchQuery.trim();
    if (!query) {
      setSearchResults([]);
      return;
    }
    const timeout = window.setTimeout(() => {
      platform.searchConversations(query)
        .then(setSearchResults)
        .catch((error) => setNavigationError(describeError(error)));
    }, 250);
    return () => window.clearTimeout(timeout);
  }, [searchQuery]);

  useEffect(() => {
    if (!dialog) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !dialogBusy) {
        setDialog(null);
      }
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [dialog, dialogBusy]);

  useEffect(() => {
    if (smokeTask?.state !== "ready" || isTaskPollingComplete(smokeTask.value)) {
      return;
    }
    const localTaskId = smokeTask.value.id;
    const interval = window.setInterval(() => {
      platform.getLocalTask(localTaskId)
        .then((value) => setSmokeTask({ state: "ready", value }))
        .catch((error) => setSmokeTask({ state: "error", message: describeError(error) }));
    }, 1_000);
    return () => window.clearInterval(interval);
  }, [
    smokeTask?.state === "ready" ? smokeTask.value.id : null,
    smokeTask?.state === "ready" ? smokeTask.value.remoteStatus : null
  ]);

  useEffect(() => {
    if (activeTurn?.state !== "ready" || isTaskPollingComplete(activeTurn.value)) {
      return;
    }
    const localTaskId = activeTurn.value.id;
    const turnConversationId = activeTurnConversationId;
    const interval = window.setInterval(() => {
      platform.getLocalTask(localTaskId)
        .then(async (value) => {
          setActiveTurn({ state: "ready", value });
          if (isTaskPollingComplete(value)) {
            await reloadNavigation();
            if (
              turnConversationId &&
              conversation?.state === "ready" &&
              conversation.value.id === turnConversationId
            ) {
              await loadConversation(turnConversationId);
            }
          }
        })
        .catch((error) => setActiveTurn({ state: "error", message: describeError(error) }));
    }, 1_000);
    return () => window.clearInterval(interval);
  }, [
    activeTurn?.state === "ready" ? activeTurn.value.id : null,
    activeTurn?.state === "ready" ? activeTurn.value.remoteStatus : null,
    activeTurnConversationId,
    conversation?.state === "ready" ? conversation.value.id : null
  ]);

  const visibleConversations = useMemo(() => {
    const source = searchQuery.trim() ? searchResults : conversations;
    if (searchQuery.trim() || selectedProjectId === null) {
      return source;
    }
    if (selectedProjectId === "unassigned") {
      return source.filter((item) => !item.projectId);
    }
    return source.filter((item) => item.projectId === selectedProjectId);
  }, [conversations, searchQuery, searchResults, selectedProjectId]);

  const selectedProject =
    projects.find((project) => project.id === selectedProjectId) ?? null;
  const currentTurn =
    conversation?.state === "ready" &&
    activeTurnConversationId === conversation.value.id
      ? activeTurn
      : null;
  const currentTurnBlocks =
    currentTurn?.state === "loading" ||
    (currentTurn?.state === "ready" && isTaskBlockingConversation(currentTurn.value));

  const checkBroker = async () => {
    setBroker({ state: "loading" });
    try {
      setBroker({ state: "ready", value: await platform.diagnoseBroker() });
    } catch (error) {
      setBroker({ state: "error", message: describeError(error) });
    }
  };

  const startSmokeTask = async () => {
    setSmokeTask({ state: "loading" });
    try {
      setSmokeTask({ state: "ready", value: await platform.startSmokeTask() });
    } catch (error) {
      setSmokeTask({ state: "error", message: describeError(error) });
    }
  };

  const cancelSmokeTask = async () => {
    if (smokeTask?.state !== "ready") return;
    try {
      setSmokeTask({
        state: "ready",
        value: await platform.cancelLocalTask(smokeTask.value.id)
      });
    } catch (error) {
      setSmokeTask({ state: "error", message: describeError(error) });
    }
  };

  const openConversation = async (conversationId: string) => {
    setConversation({ state: "loading" });
    setNavigationError(null);
    try {
      await loadConversation(conversationId);
    } catch (error) {
      setConversation({ state: "error", message: describeError(error) });
    }
  };

  const createConversation = async () => {
    try {
      const projectId =
        selectedProjectId && selectedProjectId !== "unassigned"
          ? selectedProjectId
          : undefined;
      const created = await platform.createConversation(undefined, projectId);
      await reloadNavigation();
      await openConversation(created.id);
    } catch (error) {
      setNavigationError(describeError(error));
    }
  };

  const sendTurn = async () => {
    if (conversation?.state !== "ready" || !draft.trim()) return;
    const conversationId = conversation.value.id;
    const text = draft;
    setDraft("");
    setActiveTurn({ state: "loading" });
    setActiveTurnConversationId(conversationId);
    try {
      const task = await platform.sendChatTurn(conversationId, text);
      setActiveTurn({ state: "ready", value: task });
      await loadConversation(conversationId);
      await reloadNavigation();
    } catch (error) {
      setActiveTurn({ state: "error", message: describeError(error) });
      setDraft(text);
    }
  };

  const cancelActiveTurn = async () => {
    if (currentTurn?.state !== "ready") return;
    try {
      const task = await platform.cancelLocalTask(currentTurn.value.id);
      setActiveTurn({ state: "ready", value: task });
      if (conversation?.state === "ready") {
        await loadConversation(conversation.value.id);
      }
    } catch (error) {
      setActiveTurn({ state: "error", message: describeError(error) });
    }
  };

  const moveCurrentConversation = async (projectId: string) => {
    if (conversation?.state !== "ready") return;
    try {
      await platform.moveConversation(
        conversation.value.id,
        projectId === "unassigned" ? undefined : projectId
      );
      await Promise.all([
        loadConversation(conversation.value.id),
        reloadNavigation()
      ]);
    } catch (error) {
      setNavigationError(describeError(error));
    }
  };

  const openDialog = (nextDialog: DialogState) => {
    const copy = dialogCopy(nextDialog);
    setDialog(nextDialog);
    setDialogValue(copy.initialValue ?? "");
    setNavigationError(null);
  };

  const submitDialog = async () => {
    if (!dialog) return;
    const copy = dialogCopy(dialog);
    if (copy.fieldLabel && !dialogValue.trim()) return;
    setDialogBusy(true);
    try {
      switch (dialog.kind) {
        case "project-create": {
          const project = await platform.createProject(dialogValue.trim());
          setSelectedProjectId(project.id);
          break;
        }
        case "project-rename":
          await platform.renameProject(dialog.project.id, dialogValue.trim());
          break;
        case "project-archive":
          await platform.archiveProject(dialog.project.id);
          if (selectedProjectId === dialog.project.id) {
            setSelectedProjectId(null);
          }
          break;
        case "conversation-rename":
          await platform.renameConversation(
            dialog.conversation.id,
            dialogValue.trim()
          );
          await loadConversation(dialog.conversation.id);
          break;
        case "conversation-archive":
          await platform.archiveConversation(dialog.conversation.id);
          setConversation(null);
          break;
        case "conversation-delete":
          await platform.deleteConversation(dialog.conversation.id);
          setConversation(null);
          break;
      }
      await reloadNavigation();
      setDialog(null);
    } catch (error) {
      setNavigationError(describeError(error));
    } finally {
      setDialogBusy(false);
    }
  };

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">C</span>
          <div><strong>ChatyGPT</strong><small>Espacio personal</small></div>
        </div>

        <button className="new-chat" onClick={createConversation}>＋ Nueva conversación</button>

        <label className="search-box">
          <span>⌕</span>
          <input
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
            placeholder="Buscar conversaciones"
            aria-label="Buscar conversaciones"
          />
          {searchQuery && (
            <button onClick={() => setSearchQuery("")} aria-label="Limpiar búsqueda">×</button>
          )}
        </label>

        <nav aria-label="Navegación principal">
          <p className="nav-label">Espacio</p>
          <button
            className={`nav-item ${conversation === null ? "active" : ""}`}
            onClick={() => setConversation(null)}
          >
            ◌ Inicio
          </button>

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

          <p className="nav-label">
            {searchQuery.trim() ? "Resultados" : selectedProject?.name ?? "Recientes"}
          </p>
          {visibleConversations.length === 0 ? (
            <div className="empty-nav">
              {searchQuery.trim()
                ? "No hay conversaciones que coincidan."
                : "No hay conversaciones en esta sección."}
            </div>
          ) : visibleConversations.map((item) => (
            <button
              key={item.id}
              className={`conversation-link ${
                conversation?.state === "ready" && conversation.value.id === item.id
                  ? "active"
                  : ""
              }`}
              onClick={() => openConversation(item.id)}
            >
              {item.title}
            </button>
          ))}
        </nav>

        {selectedProject && (
          <div className="project-actions">
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
        <div className="sidebar-footer">
          <span className={`status-dot ${bootstrap.state === "ready" ? "ok" : ""}`} />
          {bootstrap.state === "ready"
            ? `Datos locales · esquema ${bootstrap.value.schemaVersion}`
            : "Preparando datos locales"}
        </div>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <div>
            <span className="eyebrow">
              {conversation?.state === "ready" ? "Conversación local" : "Fase 1 · Núcleo"}
            </span>
            <h1>
              {conversation?.state === "ready"
                ? conversation.value.title
                : "Tu IA, organizada y durable."}
            </h1>
          </div>
          {conversation?.state === "ready" ? (
            <div className="conversation-toolbar">
              <select
                value={conversation.value.projectId ?? "unassigned"}
                onChange={(event) => void moveCurrentConversation(event.target.value)}
                aria-label="Proyecto de la conversación"
              >
                <option value="unassigned">Sin proyecto</option>
                {projects.map((project) => (
                  <option key={project.id} value={project.id}>{project.name}</option>
                ))}
              </select>
              <button
                onClick={() =>
                  openDialog({ kind: "conversation-rename", conversation: conversation.value })
                }
              >
                Renombrar
              </button>
              <button
                disabled={Boolean(currentTurnBlocks)}
                onClick={() =>
                  openDialog({ kind: "conversation-archive", conversation: conversation.value })
                }
              >
                Archivar
              </button>
              <button
                className="danger-text"
                disabled={Boolean(currentTurnBlocks)}
                onClick={() =>
                  openDialog({ kind: "conversation-delete", conversation: conversation.value })
                }
              >
                Eliminar
              </button>
            </div>
          ) : (
            <span className="version">v0.1.0</span>
          )}
        </header>

        <div className="content">
          {conversation?.state === "ready" ? (
            <section className="chat-surface">
              <div className="message-list" aria-live="polite">
                {conversation.value.messages.length === 0 && (
                  <div className="chat-empty">
                    <span className="pill">
                      {conversation.value.projectId ? "Conversación de proyecto" : "Nueva conversación"}
                    </span>
                    <h2>¿En qué quieres trabajar?</h2>
                    <p>El mensaje y su contexto se guardarán antes de contactar con Broker AI.</p>
                  </div>
                )}
                {conversation.value.messages.map((message) => (
                  <article key={message.id} className={`message ${message.role}`}>
                    <span className="message-role">
                      {message.role === "user" ? "Tú" : "ChatyGPT"}
                    </span>
                    {message.status === "pending" ? (
                      <div className="real-progress">
                        <span /> Esperando resultado · {
                          currentTurn?.state === "ready"
                            ? currentTurn.value.remoteStatus
                            : message.taskRemoteStatus ?? "recuperando"
                        }
                      </div>
                    ) : message.text ? (
                      <div className="message-text">{message.text}</div>
                    ) : message.error ? (
                      <div className="error">{JSON.stringify(message.error)}</div>
                    ) : null}
                  </article>
                ))}
              </div>
              <div className="composer">
                <textarea
                  value={draft}
                  onChange={(event) => setDraft(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" && !event.shiftKey) {
                      event.preventDefault();
                      void sendTurn();
                    }
                  }}
                  placeholder="Escribe un mensaje…"
                  rows={3}
                  disabled={Boolean(currentTurnBlocks)}
                />
                <div className="composer-footer">
                  <span>Enter para enviar · Shift+Enter para nueva línea</span>
                  <div className="task-actions">
                    {currentTurn?.state === "ready" &&
                      isTaskBlockingConversation(currentTurn.value) &&
                      currentTurn.value.remoteTaskId && (
                        <button className="secondary danger" onClick={cancelActiveTurn}>
                          Cancelar
                        </button>
                      )}
                    <button
                      className="primary"
                      onClick={sendTurn}
                      disabled={
                        !draft.trim() ||
                        broker?.state !== "ready" ||
                        !broker.value.ready ||
                        Boolean(currentTurnBlocks)
                      }
                    >
                      Enviar
                    </button>
                  </div>
                </div>
                {currentTurn?.state === "error" && (
                  <p className="error">{currentTurn.message}</p>
                )}
              </div>
            </section>
          ) : conversation?.state === "loading" ? (
            <section className="hero-card"><p>Abriendo conversación…</p></section>
          ) : conversation?.state === "error" ? (
            <section className="hero-card"><p className="error">{conversation.message}</p></section>
          ) : (
            <>
              <section className="hero-card">
                <div>
                  <span className="pill">Local-first</span>
                  <h2>Conversaciones organizadas sin perder trazabilidad.</h2>
                  <p>
                    Busca en el historial, agrupa chats en proyectos y gestiona su ciclo
                    de vida sin modificar Broker AI.
                  </p>
                </div>
                <div className="orb" aria-hidden="true"><span /></div>
              </section>

              <div className="grid">
                <article className="panel">
                  <div className="panel-heading">
                    <div><span className="kicker">Persistencia</span><h3>Estado local</h3></div>
                    <span className={`badge ${bootstrap.state === "ready" ? "success" : ""}`}>
                      {bootstrap.state === "loading"
                        ? "Inicializando"
                        : bootstrap.state === "ready"
                          ? "Operativa"
                          : "Error"}
                    </span>
                  </div>
                  {bootstrap.state === "ready" && (
                    <dl className="facts">
                      <div><dt>Esquema</dt><dd>{bootstrap.value.schemaVersion}</dd></div>
                      <div><dt>Conversaciones</dt><dd>{conversations.length}</dd></div>
                      <div><dt>Proyectos</dt><dd>{projects.length}</dd></div>
                    </dl>
                  )}
                  {bootstrap.state === "error" && <p className="error">{bootstrap.message}</p>}
                </article>

                <article className="panel">
                  <div className="panel-heading">
                    <div><span className="kicker">Inferencia</span><h3>Broker AI</h3></div>
                    {broker?.state === "ready" && (
                      <span className={`badge ${broker.value.ready ? "success" : "warning"}`}>
                        {broker.value.ready ? "Listo" : "No disponible"}
                      </span>
                    )}
                  </div>
                  <p className="muted">
                    Comprueba salud y capacidades reales sin crear una inferencia.
                  </p>
                  <button
                    className="primary"
                    onClick={checkBroker}
                    disabled={broker?.state === "loading"}
                  >
                    {broker?.state === "loading" ? "Comprobando…" : "Comprobar conexión"}
                  </button>
                  {broker?.state === "ready" && (
                    <div className="diagnostic">
                      <strong>{broker.value.message}</strong>
                      <span>
                        {broker.value.contractVersion
                          ? `Contrato ${broker.value.contractVersion}`
                          : broker.value.baseUrl}
                      </span>
                      <span>{broker.value.latencyMs} ms</span>
                    </div>
                  )}
                  {broker?.state === "error" && <p className="error">{broker.message}</p>}
                </article>
              </div>

              <section className="task-card">
                <div className="panel-heading">
                  <div>
                    <span className="kicker">Recorrido durable</span>
                    <h3>Prueba controlada de inferencia</h3>
                  </div>
                  {smokeTask?.state === "ready" && (
                    <span className={`badge ${
                      isTerminalTask(smokeTask.value) ? "success" : "warning"
                    }`}>
                      {smokeTask.value.remoteStatus}
                    </span>
                  )}
                </div>
                <p className="muted">
                  Persiste la petición antes de enviarla y limita la ejecución a Ollama local.
                </p>
                <div className="task-actions">
                  <button
                    className="primary"
                    onClick={startSmokeTask}
                    disabled={
                      broker?.state !== "ready" ||
                      !broker.value.ready ||
                      smokeTask?.state === "loading"
                    }
                  >
                    {smokeTask?.state === "loading" ? "Creando…" : "Ejecutar prueba durable"}
                  </button>
                  {smokeTask?.state === "ready" &&
                    isTaskBlockingConversation(smokeTask.value) &&
                    smokeTask.value.remoteTaskId && (
                      <button className="secondary danger" onClick={cancelSmokeTask}>
                        Cancelar
                      </button>
                    )}
                </div>
                {smokeTask?.state === "ready" && smokeTask.value.result && (
                  <pre className="result-preview">
                    {String(
                      smokeTask.value.result.result_markdown ??
                      JSON.stringify(smokeTask.value.result, null, 2)
                    )}
                  </pre>
                )}
                {smokeTask?.state === "error" && (
                  <p className="error">{smokeTask.message}</p>
                )}
              </section>
            </>
          )}
        </div>
      </section>

      {dialog && (
        <div className="modal-backdrop" role="presentation">
          <section className="modal" role="dialog" aria-modal="true" aria-labelledby="dialog-title">
            <span className="kicker">Gestión local</span>
            <h2 id="dialog-title">{dialogCopy(dialog).title}</h2>
            <p>{dialogCopy(dialog).description}</p>
            {dialogCopy(dialog).fieldLabel && (
              <label>
                <span>{dialogCopy(dialog).fieldLabel}</span>
                <input
                  autoFocus
                  value={dialogValue}
                  onChange={(event) => setDialogValue(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter") void submitDialog();
                    if (event.key === "Escape") setDialog(null);
                  }}
                  maxLength={120}
                />
              </label>
            )}
            <div className="modal-actions">
              <button className="secondary" onClick={() => setDialog(null)} disabled={dialogBusy}>
                Cancelar
              </button>
              <button
                className={dialogCopy(dialog).destructive ? "danger-button" : "primary"}
                onClick={submitDialog}
                disabled={
                  dialogBusy ||
                  Boolean(dialogCopy(dialog).fieldLabel && !dialogValue.trim())
                }
              >
                {dialogBusy ? "Guardando…" : dialogCopy(dialog).action}
              </button>
            </div>
          </section>
        </div>
      )}
    </main>
  );
}
