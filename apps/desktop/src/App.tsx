import { lazy, Suspense, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  type Loadable,
  attachmentNeedsSandbox,
  attachmentSelectionOnConversationOpen,
  attachmentStatusLabel,
  brokerSupportsPreset,
  canSendMessage,
  canStartMemoryEdit,
  canUseSemanticMemory,
  shouldPollMemoryIndex,
  shouldPollMemorySearch,
  shouldReconcilePendingTurn,
  shouldReloadConversationAfterTurn,
  activeMemoriesForConversation,
  semanticReadyMemoriesForConversation,
  visibleConversations,
  canRevealContextSource,
  formatResponseDuration,
  formatResponseUsage,
  filterProjectKnowledge,
  filterScheduledRuns,
  filterScheduledTasks,
  isTaskBlockingConversation,
  isTaskPollingComplete,
  isTerminalTask,
  memoryUpdateNotice,
  shouldApplyContextLoad,
  shouldFollowConversationScroll,
  shouldOfferSandboxForPrompt,
  shouldRefreshSandboxDiagnostic,
  sandboxUnavailableGuidance,
  scheduledCalendarOccurrences,
  scheduledNotifications,
  scheduledRunDetail,
  scheduledTaskDuplicateDraft,
  taskFailureSummary,
  taskProgressSummary,
  authorizedFolderPurpose,
  brokerCredentialLabel,
  credentialSavedNotice,
  taskStatusLabel,
  customGptIconGlyph,
  customGptIconOptions,
  customGptVersionSummary,
  type BootstrapReport,
  type BrokerCredentialStatus,
  type AuditEventView,
  type ApiCredentialStatus,
  type AuthorizedFolderView,
  type BrokerDiagnostic,
  type ContextSnapshotView,
  type ConversationSearchHit,
  type ConversationSummary,
  type ConversationSummaryOverview,
  type EffectiveExecutionPolicy,
  type ConversationExecutionPreferences,
  type ComposerErrorGuidance,
  type ConversationView,
  type CustomGptPreview,
  type CustomGptApiAction,
  type CustomGptIcon,
  type CustomGptVersionView,
  type CustomGptView,
  type CustomGptApiActionPreview,
  type CustomGptApiActionTestResult,
  type LocalTaskSnapshot,
  type MemoryItemView,
  type MemoryOverview,
  type MemorySearchView,
  type ProjectKnowledgeOverview,
  type ProjectKnowledgeFilter,
  type ProjectSummary,
  type ScheduledCalendarOccurrence,
  type ScheduledNotificationView,
  type ScheduledHistoryPeriodFilter,
  type ScheduledHistorySort,
  type ScheduledHistoryStatusFilter,
  type ScheduledRunPageView,
  type ScheduledTaskTemplateView,
  type ScheduledTaskView,
  type TaskArtifact,
  type PerformanceReportView,
  type WindowsStartupStatus
} from "./domain";
import { platform } from "./platform";
import { MarkdownContent } from "./MarkdownContent";
import {
  applyAppearancePreference,
  loadAppearancePreference,
  persistAppearancePreference,
  subscribeToSystemAppearance,
  type AppearancePreference,
  type ResolvedAppearance
} from "./appearance";
import {
  loadImageDescriptionPreference,
  persistImageDescriptionPreference,
  type ImageDescriptionPreference
} from "./ingestionPreferences";
import { useAdjuntos } from "./adjuntos/useAdjuntos";
import { TarjetaAdjuntos } from "./adjuntos/TarjetaAdjuntos";
import { useCompositor } from "./compositor/useCompositor";
import { TarjetaOpcionesEjecucion } from "./compositor/TarjetaOpcionesEjecucion";
import { useConfirmaciones } from "./confirmaciones/useConfirmaciones";
import { TarjetaConfirmacionHerramientas } from "./confirmaciones/TarjetaConfirmacionHerramientas";
import { useListaMensajes } from "./lista/useListaMensajes";
import { useDialogos } from "./dialogos/useDialogos";
import { BarraLateral } from "./navegacion/BarraLateral";
import { ListaMensajes } from "./lista/ListaMensajes";
import { isEditableKeyboardTarget, keyboardShortcutAction } from "./keyboard";
import { AyudaTeclado } from "./paneles/AyudaTeclado";
import { Dialogo } from "./paneles/Dialogo";
import { VistaPreviaGpt } from "./paneles/VistaPreviaGpt";
import { ResumenConversacion } from "./paneles/ResumenConversacion";
import { ConocimientoProyecto } from "./paneles/ConocimientoProyecto";
import { ArchivoPanel } from "./paneles/ArchivoPanel";
import { CopiaSeguridad } from "./paneles/CopiaSeguridad";
import { ConexionKnowledgeOrchestrator } from "./paneles/ConexionKnowledgeOrchestrator";
import { TarjetaProyectos } from "./paneles/TarjetaProyectos";
import { TarjetaEstadoYBroker } from "./paneles/TarjetaEstadoYBroker";
import { TarjetaApariencia } from "./paneles/TarjetaApariencia";
import { TarjetaImagenesDocumentos } from "./paneles/TarjetaImagenesDocumentos";
import { TarjetaRendimiento } from "./paneles/TarjetaRendimiento";
import { TarjetaCredencialBroker } from "./paneles/TarjetaCredencialBroker";
import { TarjetaCarpetasAutorizadas } from "./paneles/TarjetaCarpetasAutorizadas";
import { TarjetaPruebaInferencia } from "./paneles/TarjetaPruebaInferencia";
import { TarjetaActividad } from "./paneles/TarjetaActividad";
import { MessageArtifacts } from "./MessageArtifacts";
import {
  operationFor,
  saveDrafts,
  withoutDraft,
  withRestoredDraft
} from "./borradores";
import { describeError, withoutExtendedPathPrefix } from "./errors";
import { useMemoria } from "./memoria/useMemoria";
import { TarjetaMemoria } from "./memoria/TarjetaMemoria";
import { useGpts } from "./gpts/useGpts";
import { TarjetaGpts } from "./gpts/TarjetaGpts";
import type { WorkspaceDestination } from "./navegacion";
import { TarjetaProgramacion } from "./programacion/TarjetaProgramacion";
import { useProgramacion } from "./programacion/useProgramacion";
import {
  sandboxDeniedByCustomGpt,
  sandboxDiagnosticFailure,
  sandboxSendDecision
} from "./composer";
import {
  canSaveScheduleTemplate,
  pendingScheduledRunNotifications,
  defaultScheduledLocalTime,
  freshScheduledLocalTime,
  resolvedSchedulerTimezone,
  validateScheduleDraft,
  schedulerCalendarConflictCount,
  schedulerCalendarDays,
  loadSchedulerReadNotifications,
  persistSchedulerReadNotifications,
  scheduledLocalTimeValue,
  scheduledRunLabel,
  schedulerReadNotificationsExist
} from "./schedulerView";
import {
  budgetVerdictLabel,
  budgetVerdictTone,
  formatDuration,
  isInteractionEntry,
  FLUSH_INTERVAL_MS,
  INTERACTION_THRESHOLD_MS,
  PerformanceSampleBuffer,
  type PerformanceMetric
} from "./performance";
import { ConfirmacionHost, confirmAction } from "./confirmacion";

// H26: las áreas más pesadas se cargan cuando se abren. El paquete inicial
// deja de arrastrar el editor de flujos y el área de Athena.
const WorkflowStudio = lazy(() =>
  import("./WorkflowStudio").then((module) => ({ default: module.WorkflowStudio }))
);
const AthenaArea = lazy(() =>
  import("./AthenaArea").then((module) => ({ default: module.AthenaArea }))
);

const WORKSPACE_TITLES: Record<WorkspaceDestination, string> = {
  chats: "Inicio",
  projects: "Proyectos",
  gpts: "GPTs personales",
  workflows: "Flujos",
  athena: "Athena",
  automations: "Automatizaciones",
  settings: "Ajustes",
  archive: "Archivo"
};

/** Qué se hace en cada área: la frase de cabecera era la misma en todas y no
 *  decía dónde estaba la persona (auditoría 14-oct, INC-06). */
const WORKSPACE_SUBTITLES: Record<WorkspaceDestination, string> = {
  chats: "Tu IA, organizada y durable.",
  projects: "Agrupa conversaciones y su conocimiento.",
  gpts: "Asistentes con tus instrucciones, versionados.",
  workflows: "Encadena pasos y publícalos.",
  athena: "Trabajo autónomo supervisado.",
  automations: "Tareas que se ejecutan solas a su hora.",
  settings: "Conexiones, permisos y datos de este equipo.",
  archive: "Conversaciones archivadas y papelera."
};

// La version sale del package.json al compilar: lo primero que hace falta
// para leer un informe de fallo es saber contra que build se estaba mirando.
const APP_VERSION = __APP_VERSION__;
const INITIAL_VISIBLE_MESSAGES = 80;

const classificationLongLabel = (classification: string) =>
  classification === "internal"
    ? "Uso personal"
    : classification === "public"
      ? "Contenido público"
      : classification === "confidential"
        ? "Confidencial"
        : "Solo modelos locales · máxima restricción";

/** Explica de dónde sale la privacidad que se aplicará (H01). */
const policySourceLabel = (policy: EffectiveExecutionPolicy) => {
  switch (policy.sources.dataClassification) {
    case "gpt":
      return `La exige el GPT «${policy.customGptName ?? "personal"}», más restrictivo que el chat.`;
    case "context":
      return "La hereda del historial reciente, que ya contiene información protegida.";
    case "memory":
      return "La imponen los recuerdos sensibles activos.";
    default:
      return "Es la opción elegida en este chat.";
  }
};

export function App() {
  const searchInputRef = useRef<HTMLInputElement>(null);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const activeModalRef = useRef<HTMLElement>(null);
  const followConversationScrollRef = useRef(true);
  const [bootstrap, setBootstrap] = useState<Loadable<BootstrapReport>>({ state: "loading" });
  const [appearancePreference, setAppearancePreference] =
    useState<AppearancePreference>(loadAppearancePreference);
  const [imageDescriptionPreference, setImageDescriptionPreference] =
    useState<ImageDescriptionPreference>(loadImageDescriptionPreference);
  const [resolvedAppearance, setResolvedAppearance] = useState<ResolvedAppearance>(() =>
    document.documentElement.dataset.theme === "dark" ? "dark" : "light"
  );
  const [keyboardHelpOpen, setKeyboardHelpOpen] = useState(false);
  /** Muestras de rendimiento pendientes de enviar, agrupadas para no medir caro. */
  const performanceBufferRef = useRef(new PerformanceSampleBuffer());
  /** El arranque se mide una sola vez por sesión. */
  const appStartRecordedRef = useRef(false);
  const [performanceReport, setPerformanceReport] =
    useState<Loadable<PerformanceReportView>>({ state: "loading" });
  const [performanceBusy, setPerformanceBusy] = useState(false);
  const [broker, setBroker] = useState<Loadable<BrokerDiagnostic> | null>(null);
  const [auditEvents, setAuditEvents] = useState<Loadable<AuditEventView[]>>({ state: "loading" });
  const [authorizedFolders, setAuthorizedFolders] =
    useState<Loadable<AuthorizedFolderView[]>>({ state: "loading" });
  const [folderBusy, setFolderBusy] = useState<string | null>(null);
  const [brokerCredential, setBrokerCredential] =
    useState<Loadable<BrokerCredentialStatus>>({ state: "loading" });
  const [credentialDraft, setCredentialDraft] = useState("");
  const [credentialBusy, setCredentialBusy] = useState(false);
  const [credentialNotice, setCredentialNotice] = useState<string | null>(null);
  // El planificador vive en su propio hook; aqui solo se recuperan los
  // nombres que el resto de la pantalla ya usaba.
  const programacion = useProgramacion(bootstrap.state === "ready");
  const {
    scheduledRunStatesRef,
    schedulerHistoryInitializedRef,
    schedulerReadStateExistedRef,
    setScheduleConversationId,
    setScheduledTaskTemplates,
    setScheduledTasks,
    setSchedulerReadIds
  } = programacion;

  useLayoutEffect(() => {
    persistAppearancePreference(appearancePreference);
    const synchronize = () =>
      setResolvedAppearance(applyAppearancePreference(appearancePreference));
    synchronize();
    return appearancePreference === "system"
      ? subscribeToSystemAppearance(synchronize)
      : undefined;
  }, [appearancePreference]);
  useEffect(() => {
    persistImageDescriptionPreference(imageDescriptionPreference);
  }, [imageDescriptionPreference]);
  const [conversation, setConversation] = useState<Loadable<ConversationView> | null>(null);
  const refreshAuditEvents = async () => {
    setAuditEvents({ state: "loading" });
    try {
      setAuditEvents({ state: "ready", value: await platform.listAuditEvents() });
    } catch (error) {
      setAuditEvents({ state: "error", message: describeError(error) });
    }
  };

  // Los GPT personales viven en su propio hook.
  const gpts = useGpts({
    broker,
    conversation,
    imageDescriptionPreference,
    refreshAuditEvents
  });
  const {
    customGpts,
    setCustomGpts,
    setApiCredentials,
    customGptPreview,
    setCustomGptPreview,
    customGptFiles,
    setCustomGptFiles,
    activeCustomGptKnowledge,
    setActiveCustomGptKnowledge,
    activeCustomGptFiles,
    setActiveCustomGptFiles
  } = gpts;

  // La memoria personal vive en su propio hook.
  const memoria = useMemoria({ refreshAuditEvents });
  const {
    memory,
    setMemory,
    setMemorySearchQuery,
    setMemorySearchProjectId,
    memorySearch,
    setMemorySearch
  } = memoria;
  const [smokeTask, setSmokeTask] = useState<Loadable<LocalTaskSnapshot> | null>(null);
  /** Ficheros que produjo la tarea (contrato 2.10, 8.3). Empieza como "idle"
   *  porque no se piden solos: son otra petición al Broker. */
  const [taskArtifacts, setTaskArtifacts] = useState<
    Loadable<TaskArtifact[]> | { state: "idle" }
  >({ state: "idle" });
  const [projects, setProjects] = useState<ProjectSummary[]>([]);
  const [projectKnowledge, setProjectKnowledge] =
    useState<Loadable<ProjectKnowledgeOverview> | null>(null);
  const [projectKnowledgeBusyId, setProjectKnowledgeBusyId] = useState<string | null>(null);
  const [projectKnowledgeActionError, setProjectKnowledgeActionError] =
    useState<string | null>(null);
  const [projectKnowledgeQuery, setProjectKnowledgeQuery] = useState("");
  const [projectKnowledgeFilter, setProjectKnowledgeFilter] =
    useState<ProjectKnowledgeFilter>("all");
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [searchResults, setSearchResults] = useState<ConversationSearchHit[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchIncludesArchived, setSearchIncludesArchived] = useState(false);
  /** Cada búsqueda lleva un número: solo se aplica la respuesta de la última (H22). */
  const searchSequenceRef = useRef(0);
  /** Mensaje al que llevar la vista tras abrir un resultado de búsqueda (H22). */
  const pendingMessageFocusRef = useRef<string | null>(null);
  /**
   * La última conversación que la persona eligió (H11).
   *
   * Una carga que termina tarde, o la aceptación de un envío hecho en otra
   * conversación, solo puede pintar su resultado si sigue siendo esta. Así
   * ninguna respuesta lenta cambia la pantalla después de la última decisión.
   */
  const selectedConversationRef = useRef<string | null>(null);
  const conversationLoadSequenceRef = useRef(0);
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [workspaceDestination, setWorkspaceDestination] =
    useState<WorkspaceDestination>("chats");
  // En ventanas estrechas el panel de contexto empieza cerrado: abierto dejaba
  // la conversación en una columna de pocas palabras (980×680, 28-sep-2026).
  const [contextInspectorOpen, setContextInspectorOpen] = useState(
    () => typeof window === "undefined" || window.innerWidth >= 1200
  );
  const [activeTurn, setActiveTurn] = useState<Loadable<LocalTaskSnapshot> | null>(null);
  const [activeTurnConversationId, setActiveTurnConversationId] = useState<string | null>(null);
  const turnHandoffReloadingRef = useRef(false);
  /** Aviso cuando un mensaje se envió pero la vista no pudo actualizarse (H12). */
  const [turnRefreshNotice, setTurnRefreshNotice] = useState<string | null>(null);
  const adjuntos = useAdjuntos({
    conversation,
    broker,
    imageDescriptionPreference,
    selectedConversationRef
  });
  const compositor = useCompositor({
    conversation,
    memoryItemCount: memory.state === "ready" ? memory.value.items.length : 0,
    selectedConversationRef
  });
  const [composerError, setComposerError] = useState<ComposerErrorGuidance | null>(null);
  const [toolsEnabled, setToolsEnabled] = useState(false);
  const [sandboxEnabled, setSandboxEnabled] = useState(false);
  const [semanticMemoryEnabled, setSemanticMemoryEnabled] = useState(false);
  const [researchMode, setResearchMode] = useState(false);
  const [sandboxSuggestionPending, setSandboxSuggestionPending] = useState(false);
  const [executionOptionsBusy, setExecutionOptionsBusy] = useState(false);
  const [navigationError, setNavigationError] = useState<string | null>(null);
  const [exportBusy, setExportBusy] = useState<"markdown" | "obsidian" | null>(null);
  const [exportNotice, setExportNotice] = useState<string | null>(null);
  // El aviso de una exportación es pasajero: se retira solo y al cambiar de
  // sección, en vez de quedarse pegado sobre lo que se abra después (INC-13).
  useEffect(() => {
    if (!exportNotice) return;
    const timer = window.setTimeout(() => setExportNotice(null), 15_000);
    return () => window.clearTimeout(timer);
  }, [exportNotice]);
  useEffect(() => setExportNotice(null), [workspaceDestination]);
  // El planificador no se desmonta al cambiar de área: la propuesta de fecha se
  // renueva al entrar si ya quedó en el pasado (auditoría 14-oct, INC-10).
  const { scheduleEditingId, setScheduleAt } = programacion;
  useEffect(() => {
    if (workspaceDestination === "automations" && !scheduleEditingId) {
      setScheduleAt((value) => freshScheduledLocalTime(value));
    }
  }, [workspaceDestination, scheduleEditingId, setScheduleAt]);
  const [recoveryNoticeDismissed, setRecoveryNoticeDismissed] = useState(false);
  const [contextPanel, setContextPanel] = useState<{
    taskId: string;
    data: Loadable<ContextSnapshotView>;
  } | null>(null);
  const [contextSourceAction, setContextSourceAction] = useState<{
    taskId: string;
    reference: string;
    state: "loading" | "success" | "error";
    message?: string;
  } | null>(null);
  const [summaryPanel, setSummaryPanel] =
    useState<Loadable<ConversationSummaryOverview> | null>(null);
  const [summaryDraft, setSummaryDraft] = useState("");
  const [summaryBusy, setSummaryBusy] = useState(false);
  const currentTurn =
    conversation?.state === "ready" &&
    activeTurnConversationId === conversation.value.id
      ? activeTurn
      : null;
  const currentTurnBlocks =
    currentTurn?.state === "loading" ||
    (currentTurn?.state === "ready" && isTaskBlockingConversation(currentTurn.value));
  const currentProgress =
    currentTurn?.state === "ready" ? taskProgressSummary(currentTurn.value) : null;
  const openConversationId = conversation?.state === "ready" ? conversation.value.id : null;
  const selectedCustomGpt =
    conversation?.state === "ready" &&
    customGpts.state === "ready" &&
    conversation.value.customGptId
      ? customGpts.value.find((item) => item.id === conversation.value.customGptId)
      : undefined;
  /** Valores que se aplicarán de verdad: la política efectiva, o los del chat mientras carga. */
  const appliedPreferences =
    compositor.executionPolicy?.state === "ready"
      ? compositor.executionPolicy.value
      : conversation?.state === "ready"
        ? conversation.value.executionPreferences
        : {
            dataClassification: "internal" as const,
            strategy: "single" as const,
            maxCostUsd: 0,
            priority: 100
          };
  const selectedGptAllowsRunCode =
    !selectedCustomGpt || selectedCustomGpt.toolPermissions.runCode === "confirm";
  const selectedGptAllowsRename =
    !selectedCustomGpt ||
    selectedCustomGpt.toolPermissions.renameConversation === "confirm";
  const conversationScrollSignal =
    conversation?.state === "ready"
      ? [
          conversation.value.id,
          ...conversation.value.messages.map(
            (message) =>
              `${message.id}:${message.status}:${message.text?.length ?? 0}:${
                message.error ? JSON.stringify(message.error) : ""
              }`
          ),
          currentProgress?.label ?? "",
          currentProgress?.completed ?? ""
        ].join("|")
      : "";

  const lista = useListaMensajes({
    conversation,
    setConversation,
    selectedConversationRef,
    conversationScrollSignal,
    followConversationScrollRef,
    pendingMessageFocusRef,
    setNavigationError
  });

  // Si la ventana se estrecha por debajo de 1200 px, el panel de contexto se
  // cierra para devolver el ancho a la conversación; se puede volver a abrir.
  useEffect(() => {
    let wide = window.innerWidth >= 1200;
    const onResize = () => {
      const nowWide = window.innerWidth >= 1200;
      if (wide && !nowWide) setContextInspectorOpen(false);
      wide = nowWide;
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  useEffect(() => {
    const customGptId =
      conversation?.state === "ready" ? conversation.value.customGptId : undefined;
    if (!customGptId) {
      setActiveCustomGptKnowledge(null);
      setActiveCustomGptFiles(null);
      return;
    }
    let current = true;
    setActiveCustomGptKnowledge({ state: "loading" });
    setActiveCustomGptFiles({ state: "loading" });
    void Promise.all([
      platform.getCustomGptKnowledge(customGptId),
      platform.listCustomGptFiles(customGptId)
    ])
      .then(([items, files]) => {
        if (current) {
          setActiveCustomGptKnowledge({ state: "ready", value: items });
          setActiveCustomGptFiles({ state: "ready", value: files });
        }
      })
      .catch((error) => {
        if (current) {
          const failed: Loadable<never[]> = {
            state: "error",
            message: describeError(error)
          };
          setActiveCustomGptKnowledge(failed);
          setActiveCustomGptFiles(failed);
        }
      });
    return () => {
      current = false;
    };
  }, [
    conversation?.state === "ready"
      ? `${conversation.value.id}:${conversation.value.customGptId ?? "none"}`
      : "no-conversation"
  ]);

  useEffect(() => {
    if (
      !customGptFiles ||
      customGptFiles.data.state !== "ready" ||
      !customGptFiles.data.value.some((file) =>
        ["local", "uploading", "received", "converting"].includes(file.ingestionStatus)
      )
    ) return;
    const customGptId = customGptFiles.customGptId;
    const timer = window.setInterval(() => {
      void platform.listCustomGptFiles(customGptId).then((files) => {
        setCustomGptFiles((current) =>
          current?.customGptId === customGptId
            ? { customGptId, data: { state: "ready", value: files } }
            : current
        );
        if (
          conversation?.state === "ready" &&
          conversation.value.customGptId === customGptId
        ) {
          setActiveCustomGptFiles({ state: "ready", value: files });
        }
      });
    }, 1_200);
    return () => window.clearInterval(timer);
  }, [
    customGptFiles?.customGptId,
    customGptFiles?.data.state === "ready"
      ? customGptFiles.data.value
          .map((file) => `${file.id}:${file.ingestionStatus}:${file.contextStatus}`)
          .join("|")
      : customGptFiles?.data.state
  ]);

  useEffect(() => {
    const customGptId =
      conversation?.state === "ready" ? conversation.value.customGptId : undefined;
    if (
      !customGptId ||
      activeCustomGptFiles?.state !== "ready" ||
      !activeCustomGptFiles.value.some((file) =>
        ["local", "uploading", "received", "converting"].includes(file.ingestionStatus)
      )
    ) return;
    const timer = window.setInterval(() => {
      void platform
        .listCustomGptFiles(customGptId)
        .then((files) => setActiveCustomGptFiles({ state: "ready", value: files }));
    }, 1_200);
    return () => window.clearInterval(timer);
  }, [
    conversation?.state === "ready"
      ? `${conversation.value.id}:${conversation.value.customGptId ?? "none"}`
      : "no-conversation",
    activeCustomGptFiles?.state === "ready"
      ? activeCustomGptFiles.value
          .map((file) => `${file.id}:${file.ingestionStatus}`)
          .join("|")
      : activeCustomGptFiles?.state
  ]);

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
    try {
      setMemory({ state: "ready", value: await platform.getMemoryOverview() });
    } catch (error) {
      setMemory({ state: "error", message: describeError(error) });
    }
    try {
      setAuthorizedFolders({
        state: "ready",
        value: await platform.listAuthorizedFolders()
      });
    } catch (error) {
      setAuthorizedFolders({ state: "error", message: describeError(error) });
    }
    try {
      setBrokerCredential({ state: "ready", value: await platform.getBrokerCredential() });
    } catch (error) {
      setBrokerCredential({ state: "error", message: describeError(error) });
    }
    try {
      setApiCredentials({ state: "ready", value: await platform.listApiCredentials() });
    } catch (error) {
      setApiCredentials({ state: "error", message: describeError(error) });
    }
  };

  const saveBrokerCredential = async () => {
    // El valor solo existe en memoria hasta que Windows lo cifra; después se
    // borra del formulario para no dejarlo visible en pantalla.
    setCredentialBusy(true);
    setCredentialNotice(null);
    try {
      const status = await platform.setBrokerCredential(credentialDraft);
      setBrokerCredential({ state: "ready", value: status });
      setCredentialDraft("");
      // Se comprueba en el acto, como hace el lanzador: un token inventado no
      // debe descubrirse después, al enviar el primer mensaje (INC-03).
      let diagnostic: BrokerDiagnostic | null = null;
      try {
        diagnostic = await platform.diagnoseBroker();
        setBroker({ state: "ready", value: diagnostic });
      } catch {
        diagnostic = null;
      }
      setCredentialNotice(credentialSavedNotice(diagnostic));
    } catch (error) {
      setCredentialNotice(describeError(error));
    } finally {
      setCredentialBusy(false);
    }
  };

  const removeBrokerCredential = async () => {
    // La orden de Rust exige confirmación explícita. Hasta ahora el frontend la
    // afirmaba por su cuenta, de modo que la comprobación no protegía nada:
    // quien decide es la persona, y aquí es donde se le pregunta.
    if (
      !await confirmAction(
        "¿Retirar la credencial de Broker AI de este equipo? Tendrás que volver a introducirla para enviar mensajes."
      )
    ) {
      return;
    }
    setCredentialBusy(true);
    setCredentialNotice(null);
    try {
      const status = await platform.clearBrokerCredential();
      setBrokerCredential({ state: "ready", value: status });
      setCredentialNotice("Credencial retirada de este equipo.");
    } catch (error) {
      setCredentialNotice(describeError(error));
    } finally {
      setCredentialBusy(false);
    }
  };

  const revokeFolder = async (folderId: string) => {
    // Misma razón que al retirar la credencial: revocar una carpeta autorizada
    // es una decisión de la persona, no un trámite que el frontend dé por hecho.
    if (
      !await confirmAction(
        "¿Revocar todos los permisos de esta carpeta? Las próximas exportaciones, lecturas o modificaciones volverán a pedir autorización."
      )
    ) {
      return;
    }
    setFolderBusy(folderId);
    try {
      setAuthorizedFolders({
        state: "ready",
        value: await platform.revokeAuthorizedFolder(folderId)
      });
    } catch (error) {
      setAuthorizedFolders({ state: "error", message: describeError(error) });
    } finally {
      setFolderBusy(null);
    }
  };

  /**
   * Anota una duración observada.
   *
   * Es deliberadamente infalible: medir no puede alterar ni interrumpir la
   * acción medida, de modo que una muestra inadmisible simplemente se descarta.
   */
  const recordSample = (metric: PerformanceMetric, durationMs: number) => {
    performanceBufferRef.current.push(metric, durationMs);
  };

  /** Mensajes a pedir al recargar: lo ya cargado, para no perder el punto de lectura. */
  const loadedMessageCount = (conversationId: string) =>
    conversation?.state === "ready" && conversation.value.id === conversationId
      ? Math.max(INITIAL_VISIBLE_MESSAGES, conversation.value.messages.length)
      : INITIAL_VISIBLE_MESSAGES;

  /**
   * Carga una conversación y la pinta solo si sigue siendo la elegida (H11).
   *
   * Devuelve `false` cuando la respuesta llegó tarde y se descartó: no es un
   * error, es que la persona ya está en otra parte.
   */
  const loadConversation = async (
    conversationId: string,
    selectConversationAttachments = false
  ): Promise<boolean> => {
    const sequence = ++conversationLoadSequenceRef.current;
    const [view, conversationAttachments, conversationProjectFiles] = await Promise.all([
      platform.getConversation(conversationId, loadedMessageCount(conversationId)),
      platform.listAttachments(conversationId),
      platform.listProjectFiles(conversationId)
    ]);
    if (
      selectedConversationRef.current !== conversationId ||
      sequence !== conversationLoadSequenceRef.current
    ) {
      return false;
    }
    setConversation({ state: "ready", value: view });
    adjuntos.setAttachments(conversationAttachments);
    adjuntos.setProjectFiles(conversationProjectFiles);
    if (selectConversationAttachments) {
      adjuntos.setDraftAttachmentIds(attachmentSelectionOnConversationOpen(conversationAttachments));
    }
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
    return true;
  };

  useEffect(() => {
    platform.bootstrap()
      .then(async (value) => {
        setBootstrap({ state: "ready", value });
        setBroker({ state: "loading" });
        platform.diagnoseBroker()
          .then((diagnostic) => setBroker({ state: "ready", value: diagnostic }))
          .catch((error) => setBroker({ state: "error", message: describeError(error) }));
        const [items, projectItems] = await Promise.all([
          platform.listConversations(),
          platform.listProjects()
        ]);
        setConversations(items);
        setProjects(projectItems);
        setScheduleConversationId((current) => current || items[0]?.id || "");
        try {
          const [taskItems, templateItems] = await Promise.all([
            platform.listScheduledTasks(),
            platform.listScheduledTaskTemplates()
          ]);
          for (const task of taskItems) {
            for (const run of task.runs) {
              scheduledRunStatesRef.current.set(run.id, run.status);
            }
          }
          if (!schedulerReadStateExistedRef.current) {
            const existingTerminalIds = new Set(
              scheduledNotifications(taskItems).map((item) => item.id)
            );
            setSchedulerReadIds(existingTerminalIds);
            persistSchedulerReadNotifications(existingTerminalIds);
            schedulerReadStateExistedRef.current = true;
          }
          schedulerHistoryInitializedRef.current = true;
          setScheduledTasks({ state: "ready", value: taskItems });
          setScheduledTaskTemplates({ state: "ready", value: templateItems });
        } catch (error) {
          setScheduledTasks({ state: "error", message: describeError(error) });
          setScheduledTaskTemplates({ state: "error", message: describeError(error) });
        }
        try {
          setAuditEvents({ state: "ready", value: await platform.listAuditEvents() });
        } catch (error) {
          setAuditEvents({ state: "error", message: describeError(error) });
        }
        try {
          setMemory({ state: "ready", value: await platform.getMemoryOverview() });
          const latestSearch = await platform.getLatestMemorySearch();
          if (latestSearch) {
            setMemorySearch({ state: "ready", value: latestSearch });
            setMemorySearchQuery(latestSearch.query);
            setMemorySearchProjectId(latestSearch.projectId ?? "global");
          }
        } catch (error) {
          setMemory({ state: "error", message: describeError(error) });
        }
        try {
          setCustomGpts({ state: "ready", value: await platform.listCustomGpts() });
        } catch (error) {
          setCustomGpts({ state: "error", message: describeError(error) });
        }
        // Los dos paneles de seguridad se cargaban únicamente desde
        // `reloadNavigation`, que solo se ejecuta tras una acción de la persona.
        // Al abrir la aplicación se quedaban en «Comprobando credencial…» y
        // «Cargando permisos…» para siempre: quien solo quisiera revisar su
        // credencial o revocar una carpeta no llegaba a verlas nunca.
        try {
          setBrokerCredential({
            state: "ready",
            value: await platform.getBrokerCredential()
          });
        } catch (error) {
          setBrokerCredential({ state: "error", message: describeError(error) });
        }
        try {
          setAuthorizedFolders({
            state: "ready",
            value: await platform.listAuthorizedFolders()
          });
        } catch (error) {
          setAuthorizedFolders({ state: "error", message: describeError(error) });
        }
        try {
          setApiCredentials({ state: "ready", value: await platform.listApiCredentials() });
        } catch (error) {
          setApiCredentials({ state: "error", message: describeError(error) });
        }
        if (items[0]) {
          selectedConversationRef.current = items[0].id;
          await loadConversation(items[0].id, true);
        }
        // La aplicación es usable a partir de aquí: hay navegación y, si existe,
        // una conversación en pantalla. `performance.now()` se cuenta desde que
        // la vista web empieza a cargar, no desde que arranca el proceso.
        if (!appStartRecordedRef.current) {
          appStartRecordedRef.current = true;
          recordSample("app_start", performance.now());
        }
      })
      .catch((error) => setBootstrap({ state: "error", message: describeError(error) }));
    platform.getPerformanceReport()
      .then((value) => setPerformanceReport({ state: "ready", value }))
      .catch((error) =>
        setPerformanceReport({ state: "error", message: describeError(error) })
      );
  }, []);

  /**
   * Observa la respuesta de la interfaz a las interacciones reales.
   *
   * El umbral de 16 ms es el mínimo que admite la API: las interacciones más
   * rápidas no llegan a observarse, por lo que los percentiles calculados son
   * un límite superior y nunca una cifra optimista.
   */
  useEffect(() => {
    if (typeof PerformanceObserver === "undefined") return;
    if (!PerformanceObserver.supportedEntryTypes?.includes("event")) return;
    const observer = new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) {
        const interaction = entry as PerformanceEntry & { interactionId?: number };
        if (isInteractionEntry(interaction)) {
          recordSample("ui_response", interaction.duration);
        }
      }
    });
    try {
      observer.observe({
        type: "event",
        buffered: true,
        durationThreshold: INTERACTION_THRESHOLD_MS
      } as PerformanceObserverInit);
    } catch {
      // WebView2 sin Event Timing: la métrica queda sin muestras y, por tanto,
      // sin veredicto. Es preferible a inventar una medida sustitutiva.
      return;
    }
    return () => observer.disconnect();
  }, []);

  /** Vacía el búfer por lotes y refresca el informe cuando Inicio está visible. */
  useEffect(() => {
    if (bootstrap.state !== "ready") return;
    const homeVisible = conversation === null;
    const flush = async () => {
      const batches = performanceBufferRef.current.drain();
      if (batches.length === 0) return;
      try {
        for (const batch of batches) {
          await platform.recordPerformanceSamples(batch.metric, batch.durationsMs);
        }
      } catch {
        // Perder muestras no degrada la aplicación: el informe simplemente
        // describe menos ejecuciones. No se reencolan para no acumularlas
        // indefinidamente si el fallo es persistente.
        return;
      }
      if (!homeVisible) return;
      try {
        setPerformanceReport({
          state: "ready",
          value: await platform.getPerformanceReport()
        });
      } catch (error) {
        setPerformanceReport({ state: "error", message: describeError(error) });
      }
    };
    const timer = window.setInterval(() => void flush(), FLUSH_INTERVAL_MS);
    return () => {
      window.clearInterval(timer);
      void flush();
    };
  }, [bootstrap.state, conversation === null]);

  /** Al volver a Inicio/Ajustes, relee las muestras ya persistidas. */
  useEffect(() => {
    if (bootstrap.state !== "ready" || conversation !== null) return;
    platform.getPerformanceReport()
      .then((value) => setPerformanceReport({ state: "ready", value }))
      .catch((error) =>
        setPerformanceReport({ state: "error", message: describeError(error) })
      );
  }, [bootstrap.state, conversation === null]);



  useEffect(() => {
    const query = searchQuery.trim();
    const sequence = ++searchSequenceRef.current;
    if (!query) {
      setSearchResults([]);
      return;
    }
    const timeout = window.setTimeout(() => {
      // Se cronometra la consulta, no la espera deliberada de 250 ms que evita
      // preguntar a SQLite en cada tecla.
      const startedAt = performance.now();
      platform.searchConversations(query, searchIncludesArchived)
        .then((results) => {
          // Una consulta anterior que termina tarde no pisa la vigente (H22).
          if (sequence !== searchSequenceRef.current) return;
          recordSample("conversation_search", performance.now() - startedAt);
          setSearchResults(results);
        })
        .catch((error) => {
          if (sequence === searchSequenceRef.current) setNavigationError(describeError(error));
        });
    }, 250);
    return () => window.clearTimeout(timeout);
  }, [searchQuery, searchIncludesArchived]);

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
    if (memory.state !== "ready" || !shouldPollMemoryIndex(memory.value.items)) {
      return;
    }
    const interval = window.setInterval(() => {
      platform.getMemoryOverview()
        .then((value) => setMemory({ state: "ready", value }))
        .catch((error) => setMemory({ state: "error", message: describeError(error) }));
    }, 1_000);
    return () => window.clearInterval(interval);
  }, [
    memory.state === "ready"
      ? memory.value.items.map((item) => `${item.id}:${item.embeddingStatus}`).join("|")
      : ""
  ]);

  useEffect(() => {
    if (memorySearch?.state !== "ready" || !shouldPollMemorySearch(memorySearch.value)) {
      return;
    }
    const searchId = memorySearch.value.id;
    const interval = window.setInterval(() => {
      platform.getMemorySearch(searchId)
        .then((value) => setMemorySearch({ state: "ready", value }))
        .catch((error) => setMemorySearch({ state: "error", message: describeError(error) }));
    }, 1_000);
    return () => window.clearInterval(interval);
  }, [
    memorySearch?.state === "ready" ? `${memorySearch.value.id}:${memorySearch.value.status}` : ""
  ]);

  useEffect(() => {
    if (activeTurn?.state !== "ready") {
      return;
    }
    const localTaskId = activeTurn.value.id;
    const turnConversationId = activeTurnConversationId;
    const needsTaskHandoff =
      conversation?.state === "ready" &&
      shouldReconcilePendingTurn({
        task: activeTurn.value,
        messages: conversation.value.messages
      });
    if (isTaskPollingComplete(activeTurn.value) && !needsTaskHandoff) {
      return;
    }
    const interval = window.setInterval(() => {
      if (needsTaskHandoff) {
        if (
          !turnHandoffReloadingRef.current &&
          turnConversationId &&
          shouldReloadConversationAfterTurn({
            turnConversationId,
            openConversationId:
              conversation?.state === "ready" ? conversation.value.id : null
          })
        ) {
          turnHandoffReloadingRef.current = true;
          loadConversation(turnConversationId)
            .catch((error) =>
              setActiveTurn({ state: "error", message: describeError(error) })
            )
            .finally(() => {
              turnHandoffReloadingRef.current = false;
            });
        }
        return;
      }
      platform.getLocalTask(localTaskId)
        .then(async (value) => {
          setActiveTurn({ state: "ready", value });
          if (isTaskPollingComplete(value)) {
            await reloadNavigation();
            if (
              shouldReloadConversationAfterTurn({
                turnConversationId,
                openConversationId:
                  conversation?.state === "ready" ? conversation.value.id : null
              }) &&
              turnConversationId
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
    conversation?.state === "ready" ? conversation.value.id : null,
    conversation?.state === "ready"
      ? conversation.value.messages
          .filter((message) => message.status === "pending")
          .map((message) => `${message.id}:${message.brokerTaskId ?? ""}`)
          .join("|")
      : ""
  ]);

  const openSearchHit = async (hit: ConversationSearchHit) => {
    pendingMessageFocusRef.current = hit.matchedMessageId ?? null;
    await openConversation(hit.id);
  };

  const visibleConversationList = useMemo(
    () =>
      visibleConversations({
        conversations,
        searchResults,
        searchQuery,
        selectedProjectId
      }),
    [conversations, searchQuery, searchResults, selectedProjectId]
  );

  const selectedProject =
    projects.find((project) => project.id === selectedProjectId) ?? null;
  const selectedAttachments = adjuntos.selectedAttachments;
  const selectedAttachmentsNeedSandbox = selectedAttachments.some(attachmentNeedsSandbox);
  const attachmentsBlockSend = selectedAttachments.some(
    (item) => item.ingestionStatus !== "ready"
  );
  const canSend = canSendMessage({
    hasConversation: conversation?.state === "ready",
    hasText: Boolean(compositor.draft.trim()),
    attachmentsReady: !attachmentsBlockSend,
    attachmentBusy: adjuntos.attachmentBusy || adjuntos.cameraOpen,
    turnBlocking: Boolean(currentTurnBlocks)
  });
  const sandboxAvailable =
    broker?.state === "ready" && broker.value.ready && Boolean(broker.value.sandboxRunCode);
  const sandboxCapabilityKnown =
    broker?.state === "ready" && broker.value.capabilitiesVerified !== false;
  const activeGlobalMemoryCount =
    memory.state === "ready" && memory.value.enabled && conversation?.state === "ready"
      ? activeMemoriesForConversation(memory.value.items, conversation.value.projectId).length
      : 0;
  const activeCustomGptMemoryCount =
    activeCustomGptKnowledge?.state === "ready"
      ? activeCustomGptKnowledge.value.filter((item) => item.enabled).length
      : 0;
  const activeMemoryCount = activeGlobalMemoryCount + activeCustomGptMemoryCount;
  const activeCustomGptFileCount =
    activeCustomGptFiles?.state === "ready"
      ? activeCustomGptFiles.value.filter(
          (file) => file.ingestionStatus === "ready"
        ).length
      : 0;
  const readyCustomGptMemories =
    activeCustomGptKnowledge?.state === "ready"
      ? activeCustomGptKnowledge.value.filter(
          (item) => item.enabled && item.embeddingStatus === "ready"
        ).length
      : 0;
  const semanticMemoryReady = canUseSemanticMemory({
    memoryEnabled:
      (memory.state === "ready" && memory.value.enabled) ||
      Boolean(conversation?.state === "ready" && conversation.value.customGptId),
    hasConversation: conversation?.state === "ready",
    readyEligibleMemories:
      memory.state === "ready" && conversation?.state === "ready"
        ? (memory.value.enabled
            ? semanticReadyMemoriesForConversation(
                memory.value.items,
                conversation.value.projectId
              ).length
            : 0) + readyCustomGptMemories
        : readyCustomGptMemories
  });
  const semanticDocumentsReady = selectedAttachments.some(
    (attachment) => attachment.semanticIndexedChunks > 0
  ) || (
    activeCustomGptFiles?.state === "ready" &&
    activeCustomGptFiles.value.some(
      (attachment) => attachment.semanticIndexedChunks > 0
    )
  );

  const updateExecutionPreferences = async (
    patch: Partial<ConversationExecutionPreferences>
  ) => {
    if (conversation?.state !== "ready") return;
    const conversationId = conversation.value.id;
    const next = { ...conversation.value.executionPreferences, ...patch };
    setExecutionOptionsBusy(true);
    setNavigationError(null);
    try {
      const updated = await platform.updateConversationExecutionPreferences(
        conversationId,
        next
      );
      setConversation((current) =>
        current?.state === "ready" && current.value.id === conversationId
          ? {
              state: "ready",
              value: { ...current.value, executionPreferences: updated }
            }
          : current
      );
    } catch (error) {
      setNavigationError(describeError(error));
    } finally {
      setExecutionOptionsBusy(false);
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


  const startSmokeTask = async () => {
    setSmokeTask({ state: "loading" });
    // Los ficheros listados pertenecen a la tarea anterior: enseñarlos junto a
    // una nueva los atribuiría a una ejecución que no los produjo.
    setTaskArtifacts({ state: "idle" });
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

  /** Lista los ficheros que produjo la tarea (contrato 2.10, 8.3).
   *
   *  La respuesta de texto llega en `result`, pero lo que un modelo devuelve
   *  como fichero —una imagen, sobre todo— no cabe ahí: el resultado se lee
   *  entero en cada sondeo del estado. Va aparte, y hay que pedirlo.
   */
  const loadTaskArtifacts = async () => {
    if (smokeTask?.state !== "ready" || !smokeTask.value.remoteTaskId) return;
    setTaskArtifacts({ state: "loading" });
    try {
      setTaskArtifacts({
        state: "ready",
        value: await platform.listTaskArtifacts(smokeTask.value.remoteTaskId)
      });
    } catch (error) {
      setTaskArtifacts({ state: "error", message: describeError(error) });
    }
  };

  /** Guarda un fichero de la tarea en el almacén local de adjuntos.
   *
   *  Un artefacto que el Broker ya podó responde 410 y no 404: existió y se
   *  borró a propósito. El mensaje que llega del backend lo dice así, y se
   *  muestra tal cual en vez de traducirlo a un "no encontrado" que mandaría a
   *  buscar donde no hay nada.
   */
  const saveArtifact = async (artifactId: string) => {
    if (smokeTask?.state !== "ready" || !smokeTask.value.remoteTaskId) return;
    try {
      const path = await platform.saveTaskArtifact(
        smokeTask.value.remoteTaskId,
        artifactId
      );
      // Sin ventanas de alerta: el aviso queda visible y no bloquea (H23).
      setExportNotice(`Fichero conservado en este equipo: ${path}`);
    } catch (error) {
      setTaskArtifacts({ state: "error", message: describeError(error) });
    }
  };

  /** Descarta las mediciones acumuladas, incluidas las aún no enviadas. */
  const clearPerformanceSamples = async () => {
    // Borrar las mediciones es irreversible y deja las métricas sin veredicto.
    if (
      !await confirmAction(
        `¿Vaciar las mediciones de rendimiento? Las ${
          performanceReport.state === "ready" ? performanceReport.value.metrics.length : 5
        } métricas volverán a quedar sin medir.`
      )
    ) {
      return;
    }
    setPerformanceBusy(true);
    try {
      performanceBufferRef.current.drain();
      setPerformanceReport({
        state: "ready",
        value: await platform.clearPerformanceSamples()
      });
    } catch (error) {
      setPerformanceReport({ state: "error", message: describeError(error) });
    } finally {
      setPerformanceBusy(false);
    }
  };

  const openConversation = async (conversationId: string) => {
    selectedConversationRef.current = conversationId;
    setWorkspaceDestination("chats");
    followConversationScrollRef.current = true;
    setTurnRefreshNotice(null);
    setConversation({ state: "loading" });
    adjuntos.setAttachments([]);
    adjuntos.setProjectFiles([]);
    adjuntos.setDraftAttachmentIds([]);
    adjuntos.setAttachmentError(null);
    setNavigationError(null);
    const startedAt = performance.now();
    try {
      if (await loadConversation(conversationId, true)) {
        // Solo se mide la apertura completada: una que falla describe el
        // error, y una descartada por otra elección no mide nada.
        recordSample("conversation_open", performance.now() - startedAt);
      }
    } catch (error) {
      if (selectedConversationRef.current === conversationId) {
        setConversation({ state: "error", message: describeError(error) });
      }
    }
  };


  const authorizeGptReadFolder = async () => {
    setFolderBusy("new-read-folder");
    try {
      const selected = await platform.pickGptReadFolder();
      if (selected) {
        setAuthorizedFolders({
          state: "ready",
          value: await platform.listAuthorizedFolders()
        });
      }
    } catch (error) {
      setAuthorizedFolders({ state: "error", message: describeError(error) });
    } finally {
      setFolderBusy(null);
    }
  };

  const authorizeGptModifyFolder = async () => {
    setFolderBusy("new-modify-folder");
    try {
      const selected = await platform.pickGptModifyFolder();
      if (selected) {
        setAuthorizedFolders({
          state: "ready",
          value: await platform.listAuthorizedFolders()
        });
      }
    } catch (error) {
      setAuthorizedFolders({ state: "error", message: describeError(error) });
    } finally {
      setFolderBusy(null);
    }
  };

  const authorizeAthenaFolder = async (): Promise<AuthorizedFolderView | null> => {
    try {
      const selected = await platform.pickAthenaFolder();
      if (selected) {
        setAuthorizedFolders({
          state: "ready",
          value: await platform.listAuthorizedFolders()
        });
      }
      return selected;
    } catch (error) {
      setAuthorizedFolders({ state: "error", message: describeError(error) });
      throw error;
    }
  };

  const openWorkspaceDestination = (destination: WorkspaceDestination) => {
    selectedConversationRef.current = null;
    setWorkspaceDestination(destination);
    setConversation(null);
    setNavigationError(null);
    if (destination !== "projects") setSelectedProjectId(null);
  };

  const openBrokerCredentialSettings = () => {
    openWorkspaceDestination("settings");
    window.requestAnimationFrame(() => {
      const tokenInput = document.getElementById("broker-token");
      tokenInput?.scrollIntoView?.({ block: "center", behavior: "smooth" });
      tokenInput?.focus();
    });
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

  const toggleTaskContext = async (taskId: string) => {
    setContextSourceAction(null);
    if (contextPanel?.taskId === taskId) {
      setContextPanel(null);
      return;
    }
    setContextPanel({ taskId, data: { state: "loading" } });
    try {
      const value = await platform.getTaskContext(taskId);
      setContextPanel((current) =>
        shouldApplyContextLoad(current?.taskId, taskId)
          ? { taskId, data: { state: "ready", value } }
          : current
      );
    } catch (error) {
      setContextPanel((current) =>
        shouldApplyContextLoad(current?.taskId, taskId)
          ? {
              taskId,
              data: { state: "error", message: describeError(error) }
            }
          : current
      );
    }
  };

  const revealContextSource = async (taskId: string, sourceReference: string) => {
    setContextSourceAction({ taskId, reference: sourceReference, state: "loading" });
    try {
      const displayName = await platform.revealContextSource(taskId, sourceReference);
      setContextSourceAction({
        taskId,
        reference: sourceReference,
        state: "success",
        message: `${displayName} está seleccionado en el Explorador de Windows.`
      });
    } catch (error) {
      setContextSourceAction({
        taskId,
        reference: sourceReference,
        state: "error",
        message: describeError(error)
      });
    }
  };

  const sendTurn = async (sandboxOverride?: boolean, skipSandboxSuggestion = false) => {
    if (!canSend || conversation?.state !== "ready") return;
    const visibleStartBeganAt = performance.now();
    setComposerError(null);
    const conversationId = conversation.value.id;
    const text = compositor.draft;
    const useSandbox = sandboxOverride ?? sandboxEnabled;
    const requestsCodeExecution =
      shouldOfferSandboxForPrompt(text) || selectedAttachmentsNeedSandbox;
    const gptDenial = sandboxDeniedByCustomGpt({
      useSandbox,
      gptAllowsRunCode: selectedGptAllowsRunCode
    });
    if (gptDenial) {
      setComposerError(gptDenial);
      return;
    }
    let sandboxCanRun = sandboxAvailable;
    let sandboxKnown = sandboxCapabilityKnown;
    let diagnosticMessage: string | undefined;
    if (shouldRefreshSandboxDiagnostic({
      requiresCodeExecution: requestsCodeExecution,
      sandboxEnabledForTurn: useSandbox,
      sandboxAvailable: sandboxCanRun,
      skipSuggestion: skipSandboxSuggestion
    })) {
      try {
        const diagnostic = await platform.diagnoseBroker();
        setBroker({ state: "ready", value: diagnostic });
        sandboxCanRun = diagnostic.ready && Boolean(diagnostic.sandboxRunCode);
        sandboxKnown = diagnostic.capabilitiesVerified !== false;
        diagnosticMessage = diagnostic.ready ? undefined : diagnostic.message;
      } catch (error) {
        setComposerError(sandboxDiagnosticFailure(describeError(error)));
        return;
      }
    }
    const decision = sandboxSendDecision({
      skipSuggestion: skipSandboxSuggestion,
      useSandbox,
      requestsCodeExecution,
      sandboxAvailable: sandboxCanRun,
      sandboxCapabilityKnown: sandboxKnown,
      attachmentsNeedSandbox: selectedAttachmentsNeedSandbox,
      diagnosticMessage
    });
    if (decision.kind === "suggest-sandbox") {
      setSandboxSuggestionPending(true);
      return;
    }
    if (decision.kind === "blocked") {
      setComposerError(decision.error);
      return;
    }
    setSandboxSuggestionPending(false);
    followConversationScrollRef.current = true;
    // El identificador acompaña al texto exacto: si hay que repetir este mismo
    // envío, Rust devuelve la tarea ya creada en lugar de duplicarla (H12).
    const operation = operationFor(compositor.draftsRef.current, conversationId, text);
    compositor.draftsRef.current = operation.store;
    compositor.setDraft("");
    setTurnRefreshNotice(null);
    setActiveTurn({ state: "loading" });
    setActiveTurnConversationId(conversationId);
    const afterPaint = () => {
      recordSample("remote_operation_start", performance.now() - visibleStartBeganAt);
    };
    if (typeof requestAnimationFrame === "function") {
      requestAnimationFrame(afterPaint);
    } else {
      window.setTimeout(afterPaint, 0);
    }
    let task: LocalTaskSnapshot;
    try {
      const attachmentIds = [...adjuntos.draftAttachmentIds];
      task = await platform.sendChatTurn(
        conversationId,
        text,
        attachmentIds,
        toolsEnabled,
        useSandbox,
        semanticMemoryEnabled && semanticMemoryReady,
        researchMode,
        operation.operationId
      );
    } catch (error) {
      // El envío no se aceptó: el texto vuelve a SU conversación, aunque la
      // persona esté ya en otra, y conserva su operación por si se repite.
      compositor.draftsRef.current = withRestoredDraft(
        compositor.draftsRef.current,
        conversationId,
        text,
        operation.operationId
      );
      saveDrafts(compositor.draftsRef.current);
      if (selectedConversationRef.current === conversationId) {
        setActiveTurn({ state: "error", message: describeError(error) });
        compositor.setDraft(text);
        setSandboxEnabled(useSandbox);
      }
      return;
    }
    // Aceptado y guardado de forma durable: a partir de aquí nada puede
    // devolver el texto como «no enviado» (H12).
    compositor.draftsRef.current = withoutDraft(compositor.draftsRef.current, conversationId);
    saveDrafts(compositor.draftsRef.current);
    setSandboxEnabled(false);
    setResearchMode(false);
    setActiveTurn({ state: "ready", value: task });
    try {
      // Solo se recarga si la persona sigue aquí: la aceptación tardía de un
      // envío no puede sacarla de la conversación que abrió después (H11).
      if (selectedConversationRef.current === conversationId) {
        await loadConversation(conversationId);
      }
      await reloadNavigation();
    } catch (error) {
      if (selectedConversationRef.current === conversationId) {
        setTurnRefreshNotice(
          `El mensaje se envió y está guardado, pero no se pudo actualizar la vista: ${describeError(error)}`
        );
      }
    }
  };

  /** Reintenta solo la lectura, nunca el envío (H12). */
  const refreshAfterAcceptedTurn = async () => {
    if (conversation?.state !== "ready") return;
    try {
      await loadConversation(conversation.value.id);
      await reloadNavigation();
      setTurnRefreshNotice(null);
    } catch (error) {
      setTurnRefreshNotice(
        `El mensaje se envió y está guardado, pero no se pudo actualizar la vista: ${describeError(error)}`
      );
    }
  };

  const confirmaciones = useConfirmaciones({
    currentTurn,
    conversation,
    setActiveTurn,
    reloadNavigation,
    loadConversation
  });

  const cancelActiveTurn = async () => {
    if (currentTurn?.state !== "ready") return;
    const conversationId = activeTurnConversationId;
    try {
      // Funciona también sin identidad remota: la intención se guarda y el
      // envío no llegará a salir, o se cancelará en el Broker al reconciliar.
      const task = await platform.cancelLocalTask(currentTurn.value.id);
      setActiveTurn({ state: "ready", value: task });
      if (conversationId && selectedConversationRef.current === conversationId) {
        await loadConversation(conversationId);
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

  const selectConversationCustomGpt = async (customGptId: string) => {
    if (conversation?.state !== "ready") return;
    const conversationId = conversation.value.id;
    setNavigationError(null);
    try {
      const updated = await platform.setConversationCustomGpt(
        conversationId,
        customGptId === "none" ? undefined : customGptId
      );
      setConversation({ state: "ready", value: updated });
      const selected =
        customGpts.state === "ready"
          ? customGpts.value.find((item) => item.id === updated.customGptId)
          : undefined;
      if (selected?.toolPermissions.runCode === "deny") {
        setSandboxEnabled(false);
      }
      if (selected?.toolPermissions.renameConversation === "deny") {
        setToolsEnabled(false);
      }
      await refreshAuditEvents();
    } catch (error) {
      setNavigationError(describeError(error));
    }
  };

  const exportCurrentConversation = async () => {
    if (conversation?.state !== "ready") return;
    setExportBusy("markdown");
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
      setExportBusy(null);
    }
  };

  const exportCurrentConversationToObsidian = async () => {
    if (conversation?.state !== "ready") return;
    setExportBusy("obsidian");
    setExportNotice(null);
    setNavigationError(null);
    try {
      const vaultPath = await platform.pickObsidianVault();
      if (!vaultPath) return;
      let report;
      try {
        report = await platform.exportConversationToObsidian(
          conversation.value.id,
          vaultPath,
          false
        );
      } catch (error) {
        const message = describeError(error);
        if (
          !message.includes("confirma para reemplazarlo") ||
          !await confirmAction(
            "La nota o alguno de sus adjuntos tiene cambios hechos fuera de ChatyGPT. ¿Quieres reemplazarlos con la versión local?"
          )
        ) {
          throw error;
        }
        report = await platform.exportConversationToObsidian(
          conversation.value.id,
          vaultPath,
          true
        );
      }
      const copied = report.attachmentCount - report.reusedAttachmentCount;
      setExportNotice(
        `Obsidian actualizado · ${copied} adjunto(s) copiado(s) · ${
          report.reusedAttachmentCount
        } reutilizado(s) · ${
          report.projectIndexUpdated ? "índice de proyecto actualizado · " : ""
        }${report.approvedMemoryCount} recuerdo(s) aprobado(s): ${report.destinationPath}`
      );
    } catch (error) {
      setNavigationError(describeError(error));
    } finally {
      setExportBusy(null);
    }
  };

  const showSummaryOverview = (overview: ConversationSummaryOverview) => {
    setSummaryPanel({ state: "ready", value: overview });
    setSummaryDraft(overview.candidate?.draftText ?? "");
  };

  const openSummaryPanel = async () => {
    if (conversation?.state !== "ready") return;
    setSummaryPanel({ state: "loading" });
    try {
      showSummaryOverview(await platform.getConversationSummary(conversation.value.id));
    } catch (error) {
      setSummaryPanel({ state: "error", message: describeError(error) });
    }
  };

  const generateSummary = async () => {
    if (conversation?.state !== "ready") return;
    setSummaryBusy(true);
    try {
      showSummaryOverview(await platform.startConversationSummary(conversation.value.id));
      await reloadNavigation();
    } catch (error) {
      setSummaryPanel({ state: "error", message: describeError(error) });
    } finally {
      setSummaryBusy(false);
    }
  };

  const saveSummaryDraft = async () => {
    if (summaryPanel?.state !== "ready" || !summaryPanel.value.candidate) return;
    setSummaryBusy(true);
    try {
      showSummaryOverview(
        await platform.updateConversationSummary(
          summaryPanel.value.candidate.id,
          summaryDraft
        )
      );
    } catch (error) {
      setSummaryPanel({ state: "error", message: describeError(error) });
    } finally {
      setSummaryBusy(false);
    }
  };

  const approveSummaryDraft = async () => {
    if (summaryPanel?.state !== "ready" || !summaryPanel.value.candidate) return;
    setSummaryBusy(true);
    try {
      await platform.updateConversationSummary(
        summaryPanel.value.candidate.id,
        summaryDraft
      );
      showSummaryOverview(
        await platform.approveConversationSummary(summaryPanel.value.candidate.id)
      );
      await reloadNavigation();
    } catch (error) {
      setSummaryPanel({ state: "error", message: describeError(error) });
    } finally {
      setSummaryBusy(false);
    }
  };

  useEffect(() => {
    if (
      summaryPanel?.state !== "ready" ||
      summaryPanel.value.candidate?.status !== "generating" ||
      !summaryPanel.value.candidate.brokerTaskId ||
      conversation?.state !== "ready"
    ) {
      return;
    }
    const taskId = summaryPanel.value.candidate.brokerTaskId;
    const conversationId = conversation.value.id;
    const interval = window.setInterval(() => {
      void platform.getLocalTask(taskId).then(async (task) => {
        if (isTerminalTask(task)) {
          window.clearInterval(interval);
          showSummaryOverview(await platform.getConversationSummary(conversationId));
          await reloadNavigation();
        }
      }).catch((error) => {
        window.clearInterval(interval);
        setSummaryPanel({ state: "error", message: describeError(error) });
      });
    }, 1_000);
    return () => window.clearInterval(interval);
  }, [summaryPanel, conversation]);

  const {
    dialog,
    setDialog,
    dialogValue,
    setDialogValue,
    dialogBusy,
    dialogBusyRef,
    openDialog,
    submitDialog
  } = useDialogos({
    selectedProjectId,
    setSelectedProjectId,
    setConversation,
    setActiveTurn,
    setActiveTurnConversationId,
    followConversationScrollRef,
    reloadNavigation,
    loadConversation,
    openConversation,
    setNavigationError
  });

  const openProjectKnowledge = async (project: ProjectSummary) => {
    setProjectKnowledgeQuery("");
    setProjectKnowledgeFilter("all");
    setProjectKnowledge({ state: "loading" });
    setProjectKnowledgeActionError(null);
    try {
      setProjectKnowledge({
        state: "ready",
        value: await platform.getProjectKnowledge(project.id)
      });
    } catch (error) {
      setProjectKnowledge({ state: "error", message: describeError(error) });
    }
  };

  const removeFileFromProjectKnowledge = async (
    projectId: string,
    attachmentId: string,
    displayName: string
  ) => {
    if (!await confirmAction(
      `¿Retirar "${displayName}" de la biblioteca del proyecto?\n\n`
      + "Seguirá disponible en los chats que ya lo utilizan."
    )) {
      return;
    }
    setProjectKnowledgeBusyId(attachmentId);
    setProjectKnowledgeActionError(null);
    try {
      const next = await platform.removeProjectFile(projectId, attachmentId);
      setProjectKnowledge({ state: "ready", value: next });
      adjuntos.setProjectFiles((files) => files.filter((file) => file.id !== attachmentId));
      await reloadNavigation();
    } catch (error) {
      setProjectKnowledgeActionError(describeError(error));
    } finally {
      setProjectKnowledgeBusyId(null);
    }
  };

  const toggleProjectMemoryFromKnowledge = async (
    projectId: string,
    memoryId: string,
    enabled: boolean
  ) => {
    setProjectKnowledgeBusyId(memoryId);
    setProjectKnowledgeActionError(null);
    try {
      const next = await platform.setProjectMemoryItemEnabled(
        projectId,
        memoryId,
        enabled
      );
      setProjectKnowledge({ state: "ready", value: next });
      await reloadNavigation();
    } catch (error) {
      setProjectKnowledgeActionError(describeError(error));
    } finally {
      setProjectKnowledgeBusyId(null);
    }
  };

  const openConversationFromProjectKnowledge = async (conversationId: string) => {
    setProjectKnowledge(null);
    await openConversation(conversationId);
  };

  const filteredProjectKnowledge = useMemo(
    () => projectKnowledge?.state === "ready"
      ? filterProjectKnowledge(
          projectKnowledge.value,
          projectKnowledgeQuery,
          projectKnowledgeFilter
        )
      : null,
    [projectKnowledge, projectKnowledgeQuery, projectKnowledgeFilter]
  );

  const activeModalKind = keyboardHelpOpen
    ? "keyboard-help"
    : dialog
      ? "dialog"
      : customGptPreview
        ? "custom-gpt-preview"
        : projectKnowledge
          ? "project-knowledge"
          : summaryPanel
            ? "summary"
            : null;

  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      if (event.repeat) return;
      const action = keyboardShortcutAction({
        key: event.key,
        ctrlKey: event.ctrlKey,
        shiftKey: event.shiftKey,
        altKey: event.altKey,
        metaKey: event.metaKey,
        isComposing: event.isComposing,
        editableTarget: isEditableKeyboardTarget(event.target),
        modalOpen: activeModalKind !== null
      });
      if (!action) return;
      if (action === "focus-composer" && !composerRef.current) return;
      event.preventDefault();
      switch (action) {
        case "new-conversation":
          void createConversation();
          break;
        case "focus-search":
          searchInputRef.current?.focus();
          searchInputRef.current?.select();
          break;
        case "focus-composer":
          composerRef.current?.focus();
          break;
        case "go-home":
          setConversation(null);
          break;
        case "open-help":
          setKeyboardHelpOpen(true);
          break;
      }
    };
    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, [activeModalKind, selectedProjectId]);

  useEffect(() => {
    if (!activeModalKind) return;
    const previousFocus = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    const modal = activeModalRef.current;
    const focusableSelector =
      'button:not([disabled]), input:not([disabled]), textarea:not([disabled]), select:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])';
    const frame = window.requestAnimationFrame(() => {
      modal?.querySelector<HTMLElement>("[autofocus]")?.focus();
      if (document.activeElement === previousFocus) {
        modal?.querySelector<HTMLElement>(focusableSelector)?.focus();
      }
    });
    const containFocus = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        if (activeModalKind === "keyboard-help") setKeyboardHelpOpen(false);
        if (activeModalKind === "dialog" && !dialogBusyRef.current) setDialog(null);
        if (activeModalKind === "custom-gpt-preview") setCustomGptPreview(null);
        if (activeModalKind === "project-knowledge") setProjectKnowledge(null);
        if (activeModalKind === "summary") setSummaryPanel(null);
        return;
      }
      if (event.key !== "Tab" || !modal) return;
      const focusable = [...modal.querySelectorAll<HTMLElement>(focusableSelector)];
      if (focusable.length === 0) {
        event.preventDefault();
        modal.focus();
        return;
      }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", containFocus);
    return () => {
      window.cancelAnimationFrame(frame);
      window.removeEventListener("keydown", containFocus);
      previousFocus?.focus();
    };
  }, [activeModalKind]);

  return (
    <div className="app-shell">
      <a className="skip-link" href="#main-content">
        Saltar al contenido principal
      </a>
      <BarraLateral
        appVersion={APP_VERSION}
        bootstrap={bootstrap}
        conversation={conversation}
        conversations={conversations}
        projects={projects}
        selectedProject={selectedProject}
        selectedProjectId={selectedProjectId}
        setSelectedProjectId={setSelectedProjectId}
        workspaceDestination={workspaceDestination}
        openWorkspaceDestination={openWorkspaceDestination}
        searchInputRef={searchInputRef}
        searchQuery={searchQuery}
        setSearchQuery={setSearchQuery}
        searchIncludesArchived={searchIncludesArchived}
        setSearchIncludesArchived={setSearchIncludesArchived}
        searchResults={searchResults}
        visibleConversationList={visibleConversationList}
        createConversation={createConversation}
        openConversation={openConversation}
        openSearchHit={openSearchHit}
        openDialog={openDialog}
        openProjectKnowledge={openProjectKnowledge}
        navigationError={navigationError}
        setKeyboardHelpOpen={setKeyboardHelpOpen}
      />

      <section className="workspace" aria-label="Espacio de trabajo">
        <header className="topbar">
          <div>
            <span className="eyebrow">
              {conversation?.state === "ready"
                ? "Conversación local"
                : WORKSPACE_TITLES[workspaceDestination]}
            </span>
            <h1>
              {conversation?.state === "ready"
                ? conversation.value.title
                : WORKSPACE_SUBTITLES[workspaceDestination]}
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
              <select
                value={conversation.value.customGptId ?? "none"}
                onChange={(event) => void selectConversationCustomGpt(event.target.value)}
                aria-label="GPT personal de la conversación"
                title="El GPT elegido se aplicará a los próximos mensajes"
                disabled={
                  Boolean(currentTurnBlocks) ||
                  customGpts.state !== "ready"
                }
              >
                <option value="none">Sin GPT personal</option>
                {customGpts.state === "ready" &&
                  customGpts.value.map((customGpt) => (
                    <option key={customGpt.id} value={customGpt.id}>
                      {customGptIconGlyph(customGpt.iconRef)} {customGpt.name} · v{customGpt.versionNo}
                    </option>
                  ))}
              </select>
              <button
                className="context-inspector-toggle"
                onClick={() => setContextInspectorOpen((open) => !open)}
                aria-pressed={contextInspectorOpen}
              >
                {contextInspectorOpen ? "Ocultar contexto" : "Contexto"}
              </button>
              <details className="conversation-more">
                <summary aria-label="Más acciones de conversación">Más</summary>
                <div className="conversation-more-menu">
                  <button
                    onClick={() =>
                      openDialog({ kind: "conversation-rename", conversation: conversation.value })
                    }
                  >
                    Renombrar
                  </button>
                  <button
                    className="export-action"
                    onClick={exportCurrentConversation}
                    disabled={Boolean(exportBusy) || Boolean(currentTurnBlocks)}
                  >
                    {exportBusy === "markdown" ? "Exportando…" : "Exportar Markdown"}
                  </button>
                  <button
                    className="export-action export-obsidian"
                    onClick={exportCurrentConversationToObsidian}
                    disabled={Boolean(exportBusy) || Boolean(currentTurnBlocks)}
                  >
                    {exportBusy === "obsidian" ? "Preparando…" : "Exportar a Obsidian"}
                  </button>
                  <button onClick={() => void openSummaryPanel()} disabled={Boolean(currentTurnBlocks)}>
                    Ver resumen
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
              </details>
            </div>
          ) : (
            <span className="version">v{APP_VERSION}</span>
          )}
        </header>

        <main id="main-content" tabIndex={-1}
          className={`content ${conversation?.state === "ready" ? "conversation-content" : "home-content"}`}>
          {exportNotice && (
            <p className="export-notice" role="status">{withoutExtendedPathPrefix(exportNotice)}</p>
          )}
          {bootstrap.state === "ready" && bootstrap.value.restoreNotice && (
            <p className="restore-notice" role="status">{bootstrap.value.restoreNotice}</p>
          )}
          {broker?.state === "ready" &&
            (broker.value.credential === "rejected" || broker.value.credential === "missing") && (
            <div className="credential-banner" role="alert">
              <span>
                {broker.value.credential === "missing"
                  ? "Falta la credencial de Broker AI. Tus conversaciones siguen aquí; para enviar mensajes introduce su token."
                  : "Broker AI rechaza la credencial guardada (puede que su token haya cambiado). Tus conversaciones siguen aquí; para enviar mensajes introduce el token actual."}
              </span>
              <button className="secondary" onClick={openBrokerCredentialSettings}>
                Ir a la credencial
              </button>
            </div>
          )}
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
                      <span>{item.conversationTitle ?? item.label} · {taskStatusLabel(item.status)}</span>
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
            <div className={`chat-workspace ${contextInspectorOpen ? "" : "inspector-collapsed"}`}>
              <section className="chat-surface">
              <ListaMensajes
                lista={lista}
                conversation={conversation.value}
                currentTurn={currentTurn}
                currentProgress={currentProgress}
                selectedCustomGpt={selectedCustomGpt}
                currentTurnBlocks={Boolean(currentTurnBlocks)}
                setDraft={compositor.setDraft}
                followConversationScrollRef={followConversationScrollRef}
                contextPanel={contextPanel}
                toggleTaskContext={toggleTaskContext}
                contextSourceAction={contextSourceAction}
                revealContextSource={revealContextSource}
              />
              {currentTurn?.state === "ready" &&
                currentTurn.value.pendingToolCalls.length > 0 && (
                  <TarjetaConfirmacionHerramientas
                    confirmaciones={confirmaciones}
                    turn={currentTurn.value}
                  />
                )}
              <div className="composer">
                <TarjetaAdjuntos
                  adjuntos={adjuntos}
                  conversation={conversation.value}
                  currentTurnBlocks={Boolean(currentTurnBlocks)}
                />
                <textarea
                  ref={composerRef}
                  value={compositor.draft}
                  onChange={(event) => {
                    compositor.setDraft(event.target.value);
                    setSandboxSuggestionPending(false);
                    setComposerError(null);
                  }}
                  onKeyDown={(event) =>
                    compositor.composerKeyDown(event, { canSend, onSend: () => void sendTurn() })
                  }
                  placeholder="Escribe un mensaje…"
                  aria-label="Mensaje para ChatyGPT"
                  aria-keyshortcuts="Control+Shift+M"
                  rows={3}
                  disabled={Boolean(currentTurnBlocks)}
                />
                {composerError && (
                  <div className="composer-error" role="alert" aria-live="assertive">
                    <div>
                      <strong>{composerError.title}</strong>
                      <span>{composerError.detail}</span>
                      <small>{composerError.action}</small>
                    </div>
                    <button
                      className="secondary"
                      onClick={() => void sendTurn()}
                      disabled={Boolean(currentTurnBlocks)}
                    >
                      Volver a comprobar
                    </button>
                  </div>
                )}
                {sandboxEnabled && (
                  <p className="sandbox-consent">
                    Este mensaje puede ejecutar Python en un contenedor desechable, sin red ni acceso a tus archivos. El permiso se desactiva al enviarlo.
                  </p>
                )}
                {sandboxSuggestionPending && (
                  <div className="sandbox-suggestion" role="alert">
                    <div>
                      <strong>
                        {!selectedGptAllowsRunCode
                          ? "Este GPT no puede ejecutar código"
                          : selectedAttachmentsNeedSandbox
                          ? "¿Quieres analizar el archivo con código?"
                          : "¿Quieres que ChatyGPT ejecute y pruebe el código?"}
                      </strong>
                      <span>
                        {!selectedGptAllowsRunCode
                          ? "Puedes enviar el mensaje sin ejecutar, editar el GPT o seleccionar otro."
                          : selectedAttachmentsNeedSandbox
                          ? "Las hojas de cálculo y los CSV necesitan el contenedor aislado para calcular resultados."
                          : "Necesita permiso para usar el contenedor aislado durante este mensaje."}
                      </span>
                    </div>
                    <div className="task-actions">
                      {selectedGptAllowsRunCode && (
                        <button
                          className="primary"
                          onClick={() => void sendTurn(true, true)}
                        >
                          Permitir y enviar
                        </button>
                      )}
                      <button
                        className="secondary"
                        onClick={() => {
                          if (selectedAttachmentsNeedSandbox) {
                            setSandboxSuggestionPending(false);
                          } else {
                            void sendTurn(false, true);
                          }
                        }}
                      >
                        {selectedAttachmentsNeedSandbox ? "Cancelar" : "Enviar sin ejecutar"}
                      </button>
                    </div>
                  </div>
                )}
                {compositor.executionPolicy?.state === "ready" && compositor.executionPolicy.value.notes.length > 0 && (
                  // Lo que el chat no decide se ve sin desplegar nada: quien va a
                  // enviar tiene que saber antes qué privacidad y gasto se aplican.
                  <div className="effective-policy" role="note" aria-label="Qué se aplicará al próximo mensaje">
                    <strong>Qué se aplicará al próximo mensaje</strong>
                    <ul>
                      {compositor.executionPolicy.value.notes.map((note) => (
                        <li key={note}>{note}</li>
                      ))}
                    </ul>
                  </div>
                )}
                <TarjetaOpcionesEjecucion
                  policy={compositor.executionPolicy}
                  preferences={conversation.value.executionPreferences}
                  appliedPreferences={appliedPreferences}
                  updatePreferences={updateExecutionPreferences}
                  optionsBusy={executionOptionsBusy}
                  turnBlocks={Boolean(currentTurnBlocks)}
                  broker={broker}
                />
                <div className="composer-footer">
                  <span>
                    Enter para enviar · Shift+Enter para nueva línea
                    {selectedAttachments.length > 0 &&
                      ` · ${selectedAttachments.length} adjunto(s) activo(s)`}
                    {activeMemoryCount > 0 && ` · ${activeMemoryCount} recuerdo(s) activo(s)`}
                    {activeCustomGptFileCount > 0 &&
                      ` · ${activeCustomGptFileCount} archivo(s) del GPT`}
                    {semanticMemoryEnabled && semanticMemoryReady && " · selección semántica activa"}
                    {semanticDocumentsReady && " · búsqueda semántica documental activa"}
                  </span>
                  <div className="task-actions">
                    <label
                      className="tools-toggle research-toggle"
                      title="Realiza varias búsquedas, contrasta fuentes y prepara un informe con citas. Se desactiva después de enviar."
                    >
                      <input
                        type="checkbox"
                        checked={researchMode}
                        onChange={(event) => setResearchMode(event.target.checked)}
                        disabled={Boolean(currentTurnBlocks)}
                      />
                      Investigación profunda · un turno
                    </label>
                    <label
                      className="tools-toggle"
                      title={
                        semanticMemoryReady
                          ? "Busca y usa solo los recuerdos relacionados con el próximo mensaje"
                          : "Activa la memoria y espera a que al menos un recuerdo tenga el índice preparado"
                      }
                    >
                      <input
                        type="checkbox"
                        checked={semanticMemoryEnabled && semanticMemoryReady}
                        onChange={(event) => setSemanticMemoryEnabled(event.target.checked)}
                        disabled={Boolean(currentTurnBlocks) || !semanticMemoryReady}
                      />
                      Buscar recuerdos
                    </label>
                    <label
                      className="tools-toggle"
                      title="Cada mensaje adjunta todo el conocimiento publicado en Knowledge Orchestrator y pide una respuesta que lo sintetice. Se configura en Inicio."
                    >
                      <input
                        type="checkbox"
                        checked={conversation.value.executionPreferences.knowledgeOrchestrator ?? false}
                        onChange={(event) => void updateExecutionPreferences({
                          knowledgeOrchestrator: event.target.checked
                        })}
                        disabled={Boolean(currentTurnBlocks) || executionOptionsBusy}
                      />
                      Knowledge Orchestrator
                    </label>
                    {researchMode &&
                      ((semanticMemoryEnabled && semanticMemoryReady) || semanticDocumentsReady) && (
                        <span className="research-mode-note">
                          Primero se recupera el contexto relacionado y después la
                          investigación parte de él. Las herramientas quedan fijadas al
                          enviar, así que un reinicio la retoma tal y como la autorizaste.
                        </span>
                      )}
                    <label
                      className="tools-toggle"
                      title={
                        selectedGptAllowsRename
                          ? "Permite que el modelo proponga renombrar el chat, siempre con confirmación"
                          : "La versión seleccionada del GPT mantiene esta herramienta denegada"
                      }
                    >
                      <input
                        type="checkbox"
                        checked={toolsEnabled && selectedGptAllowsRename}
                        onChange={(event) => setToolsEnabled(event.target.checked)}
                        disabled={Boolean(currentTurnBlocks) || !selectedGptAllowsRename}
                      />
                      Herramientas
                    </label>
                    <label
                      className="tools-toggle sandbox-toggle"
                      title={
                        !selectedGptAllowsRunCode
                          ? "La versión seleccionada del GPT mantiene Código aislado denegado"
                          : broker?.state === "ready" && broker.value.sandboxRunCode
                          ? "Permite ejecutar Python aislado solo durante el próximo mensaje"
                          : "El sandbox no está disponible en Broker AI"
                      }
                    >
                      <input
                        type="checkbox"
                        checked={sandboxEnabled && selectedGptAllowsRunCode}
                        onChange={(event) => setSandboxEnabled(event.target.checked)}
                        disabled={
                          Boolean(currentTurnBlocks) ||
                          !selectedGptAllowsRunCode ||
                          broker?.state !== "ready" ||
                          !broker.value.ready ||
                          !broker.value.sandboxRunCode
                        }
                      />
                      Código aislado · un turno
                    </label>
                    {currentTurn?.state === "ready" &&
                      isTaskBlockingConversation(currentTurn.value) && (
                        <button
                          className="secondary danger"
                          onClick={cancelActiveTurn}
                          disabled={Boolean(currentTurn.value.cancelRequested)}
                        >
                          {currentTurn.value.cancelRequested ? "Cancelación solicitada" : "Cancelar"}
                        </button>
                      )}
                    <button
                      className="primary"
                      onClick={() => void sendTurn()}
                      disabled={!canSend}
                    >
                      Enviar
                    </button>
                  </div>
                </div>
                {currentTurn?.state === "loading" && (
                  <p className="composer-status" role="status">
                    <span aria-hidden="true" /> Preparando y guardando el mensaje…
                  </p>
                )}
                {currentTurn?.state === "error" && (
                  <p className="error">{currentTurn.message}</p>
                )}
                {turnRefreshNotice && (
                  <div className="composer-refresh-notice" role="status">
                    <span>{turnRefreshNotice}</span>
                    <button className="secondary" onClick={() => void refreshAfterAcceptedTurn()}>
                      Actualizar la vista
                    </button>
                  </div>
                )}
                {adjuntos.attachmentError && <p className="error">{adjuntos.attachmentError}</p>}
              </div>
              </section>
              {contextInspectorOpen && (
                <aside className="context-inspector" aria-label="Contexto activo">
                  <div className="context-inspector-heading">
                    <div>
                      <span className="kicker">Contexto activo</span>
                      <h2>Qué verá el modelo</h2>
                    </div>
                    <button
                      className="context-close"
                      onClick={() => setContextInspectorOpen(false)}
                      aria-label="Cerrar panel de contexto"
                    >
                      ×
                    </button>
                  </div>

                  <details className="context-group" open>
                    <summary>
                      <span>Este turno</span>
                      <small>{selectedAttachments.length}</small>
                    </summary>
                    <div className="context-group-body">
                      {selectedAttachments.length === 0 ? (
                        <p className="context-empty">Ningún archivo activo para el próximo mensaje.</p>
                      ) : selectedAttachments.map((attachment) => (
                        <article className="context-item" key={attachment.id}>
                          <div>
                            <strong>{attachment.displayName}</strong>
                            <small>{attachmentStatusLabel(attachment.ingestionStatus)}</small>
                          </div>
                          <button
                            onClick={() => adjuntos.setDraftAttachmentIds((ids) =>
                              ids.filter((id) => id !== attachment.id)
                            )}
                            aria-label={`Desactivar ${attachment.displayName} para el próximo mensaje`}
                          >
                            ×
                          </button>
                        </article>
                      ))}
                    </div>
                  </details>

                  <details className="context-group" open>
                    <summary>
                      <span>Proyecto</span>
                      <small>{conversation.value.projectId ? 1 : 0}</small>
                    </summary>
                    <div className="context-group-body">
                      <article className="context-item context-item-static">
                        <div>
                          <strong>
                            {projects.find((project) => project.id === conversation.value.projectId)?.name
                              ?? "Sin proyecto"}
                          </strong>
                          <small>
                            {conversation.value.projectId
                              ? "Instrucciones y archivos compartidos del proyecto"
                              : "Esta conversación no comparte contexto de proyecto"}
                          </small>
                        </div>
                      </article>
                    </div>
                  </details>

                  <details className="context-group" open>
                    <summary>
                      <span>Memoria</span>
                      <small>{activeMemoryCount}</small>
                    </summary>
                    <div className="context-group-body">
                      <article className="context-item context-item-static">
                        <div>
                          <strong>
                            {activeMemoryCount > 0
                              ? `${activeMemoryCount} recuerdo(s) disponible(s)`
                              : "Sin recuerdos activos"}
                          </strong>
                          <small>
                            {semanticMemoryEnabled && semanticMemoryReady
                              ? "La búsqueda semántica está activa para este turno"
                              : "Puedes activar la búsqueda de recuerdos al enviar"}
                          </small>
                        </div>
                      </article>
                    </div>
                  </details>

                  <section className="privacy-context" aria-label="Privacidad del próximo mensaje">
                    <span>Privacidad del próximo mensaje</span>
                    <strong>{classificationLongLabel(appliedPreferences.dataClassification)}</strong>
                    <small>
                      {appliedPreferences.dataClassification === "confidential" ||
                      appliedPreferences.dataClassification === "local_only"
                        ? "Solo modelos locales del Broker; nunca proveedores cloud ni herramientas con salida a red."
                        : "Puede usar proveedores locales o cloud según el enrutamiento."}
                    </small>
                    {compositor.executionPolicy?.state === "ready" && (
                      <>
                        <small>{policySourceLabel(compositor.executionPolicy.value)}</small>
                        <small>
                          Destino: Broker AI en {compositor.executionPolicy.value.destination.host}
                          {compositor.executionPolicy.value.destination.localMachine
                            ? " (este equipo)"
                            : compositor.executionPolicy.value.destination.encrypted
                              ? " · conexión cifrada"
                              : " · conexión sin cifrar"}
                        </small>
                        {compositor.executionPolicy.value.destination.warning && (
                          <small className="policy-warning">
                            {compositor.executionPolicy.value.destination.warning}
                          </small>
                        )}
                      </>
                    )}
                  </section>

                  <button
                    className="manage-context-button"
                    onClick={() => {
                      const controls = document.querySelector<HTMLDetailsElement>(".execution-settings");
                      if (controls) {
                        controls.open = true;
                        controls.scrollIntoView({ behavior: "smooth", block: "nearest" });
                      }
                    }}
                  >
                    Gestionar contexto
                  </button>
                </aside>
              )}
            </div>
          ) : conversation?.state === "loading" ? (
            <section className="hero-card"><p>Abriendo conversación…</p></section>
          ) : conversation?.state === "error" ? (
            <section className="hero-card"><p className="error">{conversation.message}</p></section>
          ) : (
            <div className={`home-workspace home-${workspaceDestination}`}>
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

              <TarjetaProyectos
                projects={projects}
                openDialog={openDialog}
                setSelectedProjectId={setSelectedProjectId}
                setWorkspaceDestination={setWorkspaceDestination}
                openProjectKnowledge={openProjectKnowledge}
              />

              {workspaceDestination === "workflows" && (
                <Suspense fallback={<p className="muted">Cargando el editor de flujos…</p>}>
                  <WorkflowStudio
                    projects={projects}
                    customGpts={customGpts.state === "ready" ? customGpts.value : []}
                    onOpenBrokerCredential={openBrokerCredentialSettings}
                    onOpenAutomations={() => setWorkspaceDestination("automations")}
                  />
                </Suspense>
              )}

              {workspaceDestination === "athena" && (
                <Suspense fallback={<p className="muted">Cargando el área de Athena…</p>}>
                  <AthenaArea
                    carpetas={
                      authorizedFolders.state === "ready"
                        ? authorizedFolders.value.filter(
                            (folder) =>
                              !folder.revokedAt &&
                              folder.permissions?.athena === true
                          )
                        : []
                    }
                    carpetasCargando={authorizedFolders.state === "loading"}
                    carpetasError={
                      authorizedFolders.state === "error" ? authorizedFolders.message : null
                    }
                    onAutorizarCarpeta={authorizeAthenaFolder}
                  />
                </Suspense>
              )}

              {workspaceDestination === "archive" && (
                <ArchivoPanel
                  onChanged={reloadNavigation}
                  onOpenConversation={(conversationId) => void openConversation(conversationId)}
                />
              )}

              <CopiaSeguridad />

              <TarjetaEstadoYBroker
                bootstrap={bootstrap}
                conversations={conversations}
                projects={projects}
                broker={broker}
                checkBroker={checkBroker}
              />

              <TarjetaApariencia
                appearancePreference={appearancePreference}
                setAppearancePreference={setAppearancePreference}
                resolvedAppearance={resolvedAppearance}
              />

              <TarjetaImagenesDocumentos
                imageDescriptionPreference={imageDescriptionPreference}
                setImageDescriptionPreference={setImageDescriptionPreference}
              />

              <TarjetaRendimiento
                performanceReport={performanceReport}
                performanceBusy={performanceBusy}
                clearPerformanceSamples={clearPerformanceSamples}
              />

              <TarjetaCredencialBroker
                brokerCredential={brokerCredential}
                credentialDraft={credentialDraft}
                setCredentialDraft={setCredentialDraft}
                credentialBusy={credentialBusy}
                credentialNotice={credentialNotice}
                saveBrokerCredential={saveBrokerCredential}
                removeBrokerCredential={removeBrokerCredential}
              />

              <ConexionKnowledgeOrchestrator />

              <TarjetaCarpetasAutorizadas
                authorizedFolders={authorizedFolders}
                folderBusy={folderBusy}
                authorizeGptReadFolder={authorizeGptReadFolder}
                authorizeGptModifyFolder={authorizeGptModifyFolder}
                revokeFolder={revokeFolder}
              />

              <TarjetaProgramacion
                programacion={programacion}
                conversations={conversations}
                openConversation={openConversation}
                setWorkspaceDestination={setWorkspaceDestination}
              />

              <TarjetaGpts gpts={gpts} projects={projects} openDialog={openDialog} />

              <TarjetaMemoria memoria={memoria} projects={projects} />

              <TarjetaPruebaInferencia
                smokeTask={smokeTask}
                broker={broker}
                taskArtifacts={taskArtifacts}
                startSmokeTask={startSmokeTask}
                cancelSmokeTask={cancelSmokeTask}
                loadTaskArtifacts={loadTaskArtifacts}
                saveArtifact={saveArtifact}
              />

              <TarjetaActividad
                auditEvents={auditEvents}
                refreshAuditEvents={refreshAuditEvents}
              />
            </div>
          )}
        </main>
      </section>

      <VistaPreviaGpt
        customGptPreview={customGptPreview}
        activeModalRef={activeModalRef}
        setCustomGptPreview={setCustomGptPreview}
      />
      <AyudaTeclado
        keyboardHelpOpen={keyboardHelpOpen}
        activeModalRef={activeModalRef}
        setKeyboardHelpOpen={setKeyboardHelpOpen}
      />

      <ConfirmacionHost />

      <Dialogo
        dialog={dialog}
        dialogValue={dialogValue}
        dialogBusy={dialogBusy}
        activeModalRef={activeModalRef}
        setDialog={setDialog}
        setDialogValue={setDialogValue}
        submitDialog={submitDialog}
      />

      <ConocimientoProyecto
        projectKnowledge={projectKnowledge}
        filteredProjectKnowledge={filteredProjectKnowledge}
        projectKnowledgeQuery={projectKnowledgeQuery}
        projectKnowledgeFilter={projectKnowledgeFilter}
        projectKnowledgeBusyId={projectKnowledgeBusyId}
        projectKnowledgeActionError={projectKnowledgeActionError}
        activeModalRef={activeModalRef}
        setProjectKnowledge={setProjectKnowledge}
        setProjectKnowledgeQuery={setProjectKnowledgeQuery}
        setProjectKnowledgeFilter={setProjectKnowledgeFilter}
        openDialog={openDialog}
        openConversationFromProjectKnowledge={openConversationFromProjectKnowledge}
        removeFileFromProjectKnowledge={removeFileFromProjectKnowledge}
        toggleProjectMemoryFromKnowledge={toggleProjectMemoryFromKnowledge}
      />

      <ResumenConversacion
        summaryPanel={summaryPanel}
        summaryDraft={summaryDraft}
        summaryBusy={summaryBusy}
        activeModalRef={activeModalRef}
        setSummaryPanel={setSummaryPanel}
        setSummaryDraft={setSummaryDraft}
        generateSummary={generateSummary}
        saveSummaryDraft={saveSummaryDraft}
        approveSummaryDraft={approveSummaryDraft}
      />
    </div>
  );
}
