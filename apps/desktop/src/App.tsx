import { useEffect, useMemo, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  canSendMessage,
  isTaskBlockingConversation,
  isTaskPollingComplete,
  isTerminalTask,
  type BootstrapReport,
  type AttachmentView,
  type AuditEventView,
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
  const [auditEvents, setAuditEvents] = useState<Loadable<AuditEventView[]>>({ state: "loading" });
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
  const [attachments, setAttachments] = useState<AttachmentView[]>([]);
  const [draftAttachmentIds, setDraftAttachmentIds] = useState<string[]>([]);
  const [attachmentBusy, setAttachmentBusy] = useState(false);
  const [attachmentError, setAttachmentError] = useState<string | null>(null);
  const [toolsEnabled, setToolsEnabled] = useState(false);
  const [sandboxEnabled, setSandboxEnabled] = useState(false);
  const [toolDecisions, setToolDecisions] = useState<Record<string, boolean>>({});
  const [toolDecisionBusy, setToolDecisionBusy] = useState(false);
  const [dialog, setDialog] = useState<DialogState | null>(null);
  const [dialogValue, setDialogValue] = useState("");
  const [dialogBusy, setDialogBusy] = useState(false);
  const [navigationError, setNavigationError] = useState<string | null>(null);
  const [exportBusy, setExportBusy] = useState(false);
  const [exportNotice, setExportNotice] = useState<string | null>(null);
  const [recoveryNoticeDismissed, setRecoveryNoticeDismissed] = useState(false);
  const currentTurn =
    conversation?.state === "ready" &&
    activeTurnConversationId === conversation.value.id
      ? activeTurn
      : null;
  const currentTurnBlocks =
    currentTurn?.state === "loading" ||
    (currentTurn?.state === "ready" && isTaskBlockingConversation(currentTurn.value));

  const reloadNavigation = async () => {
    const [nextConversations, nextProjects] = await Promise.all([
      platform.listConversations(),
      platform.listProjects()
    ]);
    setConversations(nextConversations);
    setProjects(nextProjects);
    try {
      setAuditEvents({ state: "ready", value: await platform.listAuditEvents() });
    } catch (error) {
      setAuditEvents({ state: "error", message: describeError(error) });
    }
  };

  const loadConversation = async (conversationId: string) => {
    const [view, conversationAttachments] = await Promise.all([
      platform.getConversation(conversationId),
      platform.listAttachments(conversationId)
    ]);
    setConversation({ state: "ready", value: view });
    setAttachments(conversationAttachments);
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
        try {
          setAuditEvents({ state: "ready", value: await platform.listAuditEvents() });
        } catch (error) {
          setAuditEvents({ state: "error", message: describeError(error) });
        }
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
    if (conversation?.state !== "ready") return;
    const conversationId = conversation.value.id;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    getCurrentWebviewWindow()
      .onDragDropEvent((event) => {
        if (event.payload.type === "drop" && event.payload.paths.length > 0) {
          void importAttachmentPaths(conversationId, event.payload.paths);
        }
      })
      .then((stop) => {
        if (disposed) stop();
        else unlisten = stop;
      })
      .catch((error) => setAttachmentError(describeError(error)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [conversation?.state === "ready" ? conversation.value.id : null]);

  useEffect(() => {
    if (conversation?.state !== "ready") return;
    if (!attachments.some((item) => !["ready", "failed"].includes(item.ingestionStatus))) {
      return;
    }
    const conversationId = conversation.value.id;
    const interval = window.setInterval(() => {
      platform.listAttachments(conversationId)
        .then(setAttachments)
        .catch((error) => setAttachmentError(describeError(error)));
    }, 1_000);
    return () => window.clearInterval(interval);
  }, [
    conversation?.state === "ready" ? conversation.value.id : null,
    attachments.map((item) => `${item.id}:${item.ingestionStatus}`).join("|")
  ]);

  useEffect(() => {
    setToolDecisions({});
  }, [
    currentTurn?.state === "ready" ? currentTurn.value.id : null,
    currentTurn?.state === "ready"
      ? currentTurn.value.pendingToolCalls.map((call) => call.toolCallId).join("|")
      : ""
  ]);

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
    activeTurn?.state === "ready" ? activeTurn.value.localState : null,
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
  const selectedAttachments = attachments.filter((item) =>
    draftAttachmentIds.includes(item.id)
  );
  const attachmentsBlockSend = selectedAttachments.some(
    (item) => item.ingestionStatus !== "ready"
  );
  const canSend = canSendMessage({
    hasConversation: conversation?.state === "ready",
    hasText: Boolean(draft.trim()),
    attachmentsReady: !attachmentsBlockSend,
    attachmentBusy,
    turnBlocking: Boolean(currentTurnBlocks)
  });

  async function importAttachmentPaths(conversationId: string, paths: string[]) {
    setAttachmentBusy(true);
    setAttachmentError(null);
    try {
      const importedIds: string[] = [];
      for (const path of paths) {
        const attachment = await platform.importAttachment(conversationId, path);
        importedIds.push(attachment.id);
      }
      setAttachments(await platform.listAttachments(conversationId));
      setDraftAttachmentIds((current) => [...new Set([...current, ...importedIds])]);
    } catch (error) {
      setAttachmentError(describeError(error));
    } finally {
      setAttachmentBusy(false);
    }
  }

  const chooseAttachments = async () => {
    if (conversation?.state !== "ready") return;
    try {
      const paths = await platform.pickAttachmentPaths();
      if (paths.length > 0) await importAttachmentPaths(conversation.value.id, paths);
    } catch (error) {
      setAttachmentError(describeError(error));
    }
  };

  const removeAttachment = async (attachmentId: string) => {
    if (conversation?.state !== "ready") return;
    try {
      await platform.removeAttachment(conversation.value.id, attachmentId);
      setAttachments((items) => items.filter((item) => item.id !== attachmentId));
      setDraftAttachmentIds((ids) => ids.filter((id) => id !== attachmentId));
    } catch (error) {
      setAttachmentError(describeError(error));
    }
  };

  const retryAttachment = async (attachmentId: string) => {
    try {
      const updated = await platform.retryAttachment(attachmentId);
      setAttachments((items) => items.map((item) => item.id === updated.id ? updated : item));
    } catch (error) {
      setAttachmentError(describeError(error));
    }
  };

  const checkBroker = async () => {
    setBroker({ state: "loading" });
    try {
      setBroker({ state: "ready", value: await platform.diagnoseBroker() });
    } catch (error) {
      setBroker({ state: "error", message: describeError(error) });
    }
  };

  const refreshAuditEvents = async () => {
    setAuditEvents({ state: "loading" });
    try {
      setAuditEvents({ state: "ready", value: await platform.listAuditEvents() });
    } catch (error) {
      setAuditEvents({ state: "error", message: describeError(error) });
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
    setAttachments([]);
    setDraftAttachmentIds([]);
    setAttachmentError(null);
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
    if (!canSend || conversation?.state !== "ready") return;
    const conversationId = conversation.value.id;
    const text = draft;
    setDraft("");
    setActiveTurn({ state: "loading" });
    setActiveTurnConversationId(conversationId);
    try {
      const attachmentIds = [...draftAttachmentIds];
      const task = await platform.sendChatTurn(
        conversationId,
        text,
        attachmentIds,
        toolsEnabled,
        sandboxEnabled
      );
      setSandboxEnabled(false);
      setActiveTurn({ state: "ready", value: task });
      await loadConversation(conversationId);
      await reloadNavigation();
    } catch (error) {
      setActiveTurn({ state: "error", message: describeError(error) });
      setDraft(text);
    }
  };

  const submitToolDecisions = async () => {
    if (currentTurn?.state !== "ready") return;
    const calls = currentTurn.value.pendingToolCalls;
    if (calls.length === 0 || calls.some((call) => toolDecisions[call.toolCallId] === undefined)) {
      return;
    }
    setToolDecisionBusy(true);
    try {
      const task = await platform.resolveToolCalls(
        currentTurn.value.id,
        calls.map((call) => ({
          toolCallId: call.toolCallId,
          approved: toolDecisions[call.toolCallId]
        }))
      );
      setActiveTurn({ state: "ready", value: task });
      await reloadNavigation();
      if (conversation?.state === "ready") {
        await loadConversation(conversation.value.id);
      }
    } catch (error) {
      setActiveTurn({ state: "error", message: describeError(error) });
    } finally {
      setToolDecisionBusy(false);
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

  const exportCurrentConversation = async () => {
    if (conversation?.state !== "ready") return;
    setExportBusy(true);
    setExportNotice(null);
    setNavigationError(null);
    try {
      const selection = await platform.pickExportPath(conversation.value.title);
      if (!selection) return;
      const report = await platform.exportConversation(
        conversation.value.id,
        selection.path,
        selection.existed
      );
      setExportNotice(`Exportación verificada: ${report.destinationPath}`);
    } catch (error) {
      setNavigationError(describeError(error));
    } finally {
      setExportBusy(false);
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
                onClick={exportCurrentConversation}
                disabled={exportBusy || Boolean(currentTurnBlocks)}
              >
                {exportBusy ? "Exportando…" : "Exportar Markdown"}
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
          {exportNotice && <p className="export-notice">{exportNotice}</p>}
          {bootstrap.state === "ready" &&
            !recoveryNoticeDismissed &&
            (bootstrap.value.recoveredTasks > 0 || bootstrap.value.recoveredAttachments > 0) && (
              <section className="recovery-notice" aria-label="Recuperación al iniciar">
                <div>
                  <span className="kicker">Recuperación automática</span>
                  <strong>
                    ChatyGPT reanudó {bootstrap.value.recoveredTasks} tarea(s) y {bootstrap.value.recoveredAttachments} adjunto(s).
                  </strong>
                  <p>Puedes seguir trabajando: el progreso continúa desde el último estado guardado.</p>
                  {bootstrap.value.recoveryItems.slice(0, 3).map((item, index) => (
                    <div className="recovery-item" key={`${item.updatedAt}-${index}`}>
                      <span>{item.conversationTitle ?? item.label} · {item.status}</span>
                      {item.conversationId && (
                        <button className="secondary" onClick={() => openConversation(item.conversationId!)}>
                          Abrir conversación
                        </button>
                      )}
                    </div>
                  ))}
                </div>
                <button
                  className="recovery-dismiss"
                  onClick={() => setRecoveryNoticeDismissed(true)}
                  aria-label="Ocultar aviso de recuperación"
                >
                  ×
                </button>
              </section>
            )}
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
                                  {source.mediaType ?? "Archivo adjunto"}
                                  {source.sizeBytes !== undefined &&
                                    ` · ${(source.sizeBytes / 1024).toFixed(1)} KB`}
                                </small>
                                {source.quoteText && <p>{source.quoteText}</p>}
                              </div>
                            </article>
                          ))}
                        </div>
                        <p className="source-disclaimer">
                          Archivos enviados al Broker en este turno. No implican una cita por frase.
                        </p>
                      </section>
                    )}
                  </article>
                ))}
              </div>
              {currentTurn?.state === "ready" &&
                currentTurn.value.pendingToolCalls.length > 0 && (
                  <section className="tool-confirmation" aria-label="Confirmación de herramientas">
                    <span className="kicker">Confirmación necesaria</span>
                    <h3>ChatyGPT quiere realizar una acción</h3>
                    <p>
                      Revisa cada propuesta. No se ejecutará ninguna acción hasta que decidas.
                    </p>
                    <div className="tool-call-list">
                      {currentTurn.value.pendingToolCalls.map((call) => (
                        <article key={call.toolCallId} className="tool-call-card">
                          <div>
                            <strong>
                              {call.name === "rename_conversation"
                                ? "Renombrar la conversación"
                                : call.name}
                            </strong>
                            <small>{JSON.stringify(call.arguments)}</small>
                          </div>
                          <div className="tool-decision-buttons">
                            <button
                              className={toolDecisions[call.toolCallId] === false ? "selected" : ""}
                              onClick={() => setToolDecisions((values) => ({
                                ...values,
                                [call.toolCallId]: false
                              }))}
                            >
                              Rechazar
                            </button>
                            <button
                              className={toolDecisions[call.toolCallId] === true ? "selected approve" : ""}
                              onClick={() => setToolDecisions((values) => ({
                                ...values,
                                [call.toolCallId]: true
                              }))}
                            >
                              Autorizar
                            </button>
                          </div>
                        </article>
                      ))}
                    </div>
                    <button
                      className="primary"
                      onClick={submitToolDecisions}
                      disabled={
                        toolDecisionBusy ||
                        currentTurn.value.pendingToolCalls.some(
                          (call) => toolDecisions[call.toolCallId] === undefined
                        )
                      }
                    >
                      {toolDecisionBusy ? "Reanudando…" : "Confirmar decisiones y continuar"}
                    </button>
                  </section>
                )}
              <div className="composer">
                <div className="attachment-row">
                  <button
                    className="attachment-picker"
                    onClick={chooseAttachments}
                    disabled={Boolean(currentTurnBlocks) || attachmentBusy}
                  >
                    {attachmentBusy ? "Importando…" : "+ Adjuntar archivos"}
                  </button>
                  <span>o arrástralos a esta ventana</span>
                </div>
                {attachments.length > 0 && (
                  <div className="attachment-list" aria-label="Archivos de la conversación">
                    {attachments.map((attachment) => {
                      const selected = draftAttachmentIds.includes(attachment.id);
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
                              {(attachment.sizeBytes / 1024).toFixed(1)} KB · {attachment.ingestionStatus}
                            </small>
                          </button>
                          {attachment.ingestionStatus === "failed" && (
                            <button onClick={() => retryAttachment(attachment.id)}>Reintentar</button>
                          )}
                          <button
                            className="attachment-remove"
                            onClick={() => removeAttachment(attachment.id)}
                            aria-label={`Quitar ${attachment.displayName}`}
                          >
                            ×
                          </button>
                        </div>
                      );
                    })}
                  </div>
                )}
                <textarea
                  value={draft}
                  onChange={(event) => setDraft(event.target.value)}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" && !event.shiftKey && canSend) {
                      event.preventDefault();
                      void sendTurn();
                    }
                  }}
                  placeholder="Escribe un mensaje…"
                  rows={3}
                  disabled={Boolean(currentTurnBlocks)}
                />
                {sandboxEnabled && (
                  <p className="sandbox-consent">
                    Este mensaje puede ejecutar Python en un contenedor desechable, sin red ni acceso a tus archivos. El permiso se desactiva al enviarlo.
                  </p>
                )}
                <div className="composer-footer">
                  <span>
                    Enter para enviar · Shift+Enter para nueva línea
                    {selectedAttachments.length > 0 &&
                      ` · ${selectedAttachments.length} adjunto(s) activo(s)`}
                  </span>
                  <div className="task-actions">
                    <label className="tools-toggle" title="Permite que el modelo proponga acciones locales confirmables">
                      <input
                        type="checkbox"
                        checked={toolsEnabled}
                        onChange={(event) => setToolsEnabled(event.target.checked)}
                        disabled={Boolean(currentTurnBlocks)}
                      />
                      Herramientas
                    </label>
                    <label
                      className="tools-toggle sandbox-toggle"
                      title={
                        broker?.state === "ready" && broker.value.sandboxRunCode
                          ? "Permite ejecutar Python aislado solo durante el próximo mensaje"
                          : "El sandbox no está disponible en Broker AI"
                      }
                    >
                      <input
                        type="checkbox"
                        checked={sandboxEnabled}
                        onChange={(event) => setSandboxEnabled(event.target.checked)}
                        disabled={
                          Boolean(currentTurnBlocks) ||
                          broker?.state !== "ready" ||
                          !broker.value.ready ||
                          !broker.value.sandboxRunCode
                        }
                      />
                      Código aislado · un turno
                    </label>
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
                      disabled={!canSend}
                    >
                      Enviar
                    </button>
                  </div>
                </div>
                {currentTurn?.state === "error" && (
                  <p className="error">{currentTurn.message}</p>
                )}
                {attachmentError && <p className="error">{attachmentError}</p>}
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
                      <span>
                        Código aislado: {broker.value.sandboxRunCode ? "disponible" : "no disponible"}
                      </span>
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
