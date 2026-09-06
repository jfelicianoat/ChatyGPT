import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  attachmentFailureGuidance,
  type Loadable,
  attachmentContextSummary,
  attachmentNeedsSandbox,
  attachmentSelectionOnConversationOpen,
  attachmentImagePolicyLabel,
  attachmentStatusLabel,
  brokerSupportsPreset,
  brokerAttachmentExtensions,
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
  confirmationSummary,
  formatResponseDuration,
  formatResponseUsage,
  filterProjectKnowledge,
  filterScheduledRuns,
  filterScheduledTasks,
  isTaskBlockingConversation,
  isTaskPollingComplete,
  isTerminalTask,
  memoryUpdateNotice,
  projectFilesAvailableToConversation,
  progressiveConversationWindow,
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
  customGptIconGlyph,
  customGptIconOptions,
  customGptVersionSummary,
  type BootstrapReport,
  type BrokerCredentialStatus,
  type AttachmentView,
  type AuditEventView,
  type ApiCredentialStatus,
  type AuthorizedFolderView,
  type BrokerDiagnostic,
  type ContextSnapshotView,
  type ConversationSummary,
  type ConversationSummaryOverview,
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
  type PerformanceReportView,
  type WindowsStartupStatus
} from "./domain";
import { platform } from "./platform";
import { MarkdownContent } from "./MarkdownContent";
import {
  captureDisplayName,
  captureScreenFrame,
  captureVideoFrame,
  cropCapturedFrame,
  normalizeCropSelection,
  type CropSelection,
  type CapturedScreenFrame
} from "./screenCapture";
import { cameraFailureMessage, openCameraStream } from "./cameraCapture";
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
  shouldDescribeImages,
  type ImageDescriptionPreference
} from "./ingestionPreferences";
import { isEditableKeyboardTarget, keyboardShortcutAction } from "./keyboard";
import { AthenaArea } from "./AthenaArea";
import { AyudaTeclado } from "./paneles/AyudaTeclado";
import { Dialogo } from "./paneles/Dialogo";
import { VistaPreviaGpt } from "./paneles/VistaPreviaGpt";
import { ResumenConversacion } from "./paneles/ResumenConversacion";
import { ConocimientoProyecto } from "./paneles/ConocimientoProyecto";
import { WorkflowStudio } from "./WorkflowStudio";
import { dialogCopy, type DialogState } from "./dialogs";
import { describeError } from "./errors";
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

type ScreenCapturePreview = CapturedScreenFrame & {
  conversationId: string;
  previewUrl: string;
  source: "screen" | "camera";
};

// La version sale del package.json al compilar: lo primero que hace falta
// para leer un informe de fallo es saber contra que build se estaba mirando.
const APP_VERSION = __APP_VERSION__;
const INITIAL_VISIBLE_MESSAGES = 80;
const EARLIER_MESSAGE_PAGE_SIZE = 50;

export function App() {
  const messageListRef = useRef<HTMLDivElement>(null);
  const prependScrollRef = useRef<{ scrollHeight: number; scrollTop: number } | null>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const activeModalRef = useRef<HTMLElement>(null);
  const dialogBusyRef = useRef(false);
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
  const [taskArtifacts, setTaskArtifacts] = useState<Loadable<TaskArtifact[]>>({
    state: "idle"
  });
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
  const [searchResults, setSearchResults] = useState<ConversationSummary[]>([]);
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedProjectId, setSelectedProjectId] = useState<string | null>(null);
  const [workspaceDestination, setWorkspaceDestination] =
    useState<WorkspaceDestination>("chats");
  const [messageWindow, setMessageWindow] = useState({
    conversationId: null as string | null,
    limit: INITIAL_VISIBLE_MESSAGES
  });
  const [contextInspectorOpen, setContextInspectorOpen] = useState(true);
  const [activeTurn, setActiveTurn] = useState<Loadable<LocalTaskSnapshot> | null>(null);
  const [activeTurnConversationId, setActiveTurnConversationId] = useState<string | null>(null);
  const turnHandoffReloadingRef = useRef(false);
  const [draft, setDraft] = useState("");
  const [attachments, setAttachments] = useState<AttachmentView[]>([]);
  const [projectFiles, setProjectFiles] = useState<AttachmentView[]>([]);
  const [draftAttachmentIds, setDraftAttachmentIds] = useState<string[]>([]);
  const [attachmentBusy, setAttachmentBusy] = useState(false);
  const [screenCaptureBusy, setScreenCaptureBusy] = useState(false);
  const [screenCapturePreview, setScreenCapturePreview] =
    useState<ScreenCapturePreview | null>(null);
  const screenCaptureUrlRef = useRef<string | null>(null);
  const [cropMode, setCropMode] = useState(false);
  const [cropSelection, setCropSelection] = useState<CropSelection | null>(null);
  const cropStartRef = useRef<{ x: number; y: number } | null>(null);
  const [cameraOpen, setCameraOpen] = useState(false);
  const [cameraReady, setCameraReady] = useState(false);
  const [cameraBusy, setCameraBusy] = useState(false);
  const [cameraError, setCameraError] = useState<string | null>(null);
  const [cameraConversationId, setCameraConversationId] = useState<string | null>(null);
  const cameraVideoRef = useRef<HTMLVideoElement | null>(null);
  const cameraStreamRef = useRef<MediaStream | null>(null);
  const [projectFileBusyId, setProjectFileBusyId] = useState<string | null>(null);
  const [attachmentContextRetryId, setAttachmentContextRetryId] = useState<string | null>(null);
  const [attachmentSemanticRetryId, setAttachmentSemanticRetryId] = useState<string | null>(null);
  const [attachmentError, setAttachmentError] = useState<string | null>(null);
  const [composerError, setComposerError] = useState<ComposerErrorGuidance | null>(null);
  const [toolsEnabled, setToolsEnabled] = useState(false);
  const [sandboxEnabled, setSandboxEnabled] = useState(false);
  const [semanticMemoryEnabled, setSemanticMemoryEnabled] = useState(false);
  const [researchMode, setResearchMode] = useState(false);
  const [sandboxSuggestionPending, setSandboxSuggestionPending] = useState(false);
  const [executionOptionsBusy, setExecutionOptionsBusy] = useState(false);
  const [toolDecisions, setToolDecisions] = useState<Record<string, boolean>>({});
  const [toolDecisionBusy, setToolDecisionBusy] = useState(false);
  const [dialog, setDialog] = useState<DialogState | null>(null);
  const [dialogValue, setDialogValue] = useState("");
  const [dialogBusy, setDialogBusy] = useState(false);
  const [navigationError, setNavigationError] = useState<string | null>(null);
  const [exportBusy, setExportBusy] = useState<"markdown" | "obsidian" | null>(null);
  const [exportNotice, setExportNotice] = useState<string | null>(null);
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
  const effectiveMessageLimit =
    messageWindow.conversationId === openConversationId
      ? messageWindow.limit
      : INITIAL_VISIBLE_MESSAGES;
  const progressiveMessages =
    conversation?.state === "ready"
      ? progressiveConversationWindow(conversation.value.messages, effectiveMessageLimit)
      : { visibleItems: [], hiddenCount: 0 };
  const selectedCustomGpt =
    conversation?.state === "ready" &&
    customGpts.state === "ready" &&
    conversation.value.customGptId
      ? customGpts.value.find((item) => item.id === conversation.value.customGptId)
      : undefined;
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

  useLayoutEffect(() => {
    const pending = prependScrollRef.current;
    const messageList = messageListRef.current;
    if (!pending || !messageList) return;
    messageList.scrollTop =
      pending.scrollTop + Math.max(0, messageList.scrollHeight - pending.scrollHeight);
    prependScrollRef.current = null;
  }, [effectiveMessageLimit]);

  useLayoutEffect(() => {
    const messageList = messageListRef.current;
    if (!messageList || !followConversationScrollRef.current) return;
    messageList.scrollTop = messageList.scrollHeight;
  }, [conversationScrollSignal]);

  useEffect(() => {
    if (messageWindow.conversationId === openConversationId) return;
    prependScrollRef.current = null;
    setMessageWindow({
      conversationId: openConversationId,
      limit: INITIAL_VISIBLE_MESSAGES
    });
  }, [openConversationId, messageWindow.conversationId]);

  useEffect(
    () => () => {
      if (screenCaptureUrlRef.current) {
        URL.revokeObjectURL(screenCaptureUrlRef.current);
      }
      cameraStreamRef.current?.getTracks().forEach((track) => track.stop());
    },
    []
  );

  useEffect(() => {
    dialogBusyRef.current = dialogBusy;
  }, [dialogBusy]);

  useEffect(() => {
    if (!cameraOpen) return;
    const video = cameraVideoRef.current;
    const stream = cameraStreamRef.current;
    if (!video || !stream) return;
    video.srcObject = stream;
    void video.play().catch((error) => {
      setCameraError(cameraFailureMessage(error));
      stopCamera();
    });
  }, [cameraOpen]);

  useEffect(() => {
    const currentConversationId =
      conversation?.state === "ready" ? conversation.value.id : null;
    if (cameraConversationId && cameraConversationId !== currentConversationId) {
      stopCamera();
    }
  }, [conversation, cameraConversationId]);

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
      setCredentialNotice("Credencial guardada y cifrada para tu cuenta de Windows.");
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
      !window.confirm(
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
      !window.confirm(
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

  const loadConversation = async (
    conversationId: string,
    selectConversationAttachments = false
  ) => {
    const [view, conversationAttachments, conversationProjectFiles] = await Promise.all([
      platform.getConversation(conversationId),
      platform.listAttachments(conversationId),
      platform.listProjectFiles(conversationId)
    ]);
    setConversation({ state: "ready", value: view });
    setAttachments(conversationAttachments);
    setProjectFiles(conversationProjectFiles);
    if (selectConversationAttachments) {
      setDraftAttachmentIds(attachmentSelectionOnConversationOpen(conversationAttachments));
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
    if (!query) {
      setSearchResults([]);
      return;
    }
    const timeout = window.setTimeout(() => {
      // Se cronometra la consulta, no la espera deliberada de 250 ms que evita
      // preguntar a SQLite en cada tecla.
      const startedAt = performance.now();
      platform.searchConversations(query)
        .then((results) => {
          recordSample("conversation_search", performance.now() - startedAt);
          setSearchResults(results);
        })
        .catch((error) => setNavigationError(describeError(error)));
    }, 250);
    return () => window.clearTimeout(timeout);
  }, [searchQuery]);

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
    if (!attachments.some((item) =>
      !["ready", "failed"].includes(item.ingestionStatus)
      || ["pending", "preparing"].includes(item.contextStatus)
      || ["pending", "indexing"].includes(item.semanticIndexStatus)
    )) {
      return;
    }
    const conversationId = conversation.value.id;
    const interval = window.setInterval(() => {
      Promise.all([
        platform.listAttachments(conversationId),
        platform.listProjectFiles(conversationId)
      ])
        .then(([nextAttachments, nextProjectFiles]) => {
          setAttachments(nextAttachments);
          setProjectFiles(nextProjectFiles);
        })
        .catch((error) => setAttachmentError(describeError(error)));
    }, 1_000);
    return () => window.clearInterval(interval);
  }, [
    conversation?.state === "ready" ? conversation.value.id : null,
    attachments
      .map((item) =>
        `${item.id}:${item.ingestionStatus}:${item.contextStatus}:${item.chunkCount}`
      )
      .join("|")
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
  const selectedAttachments = attachments.filter((item) =>
    draftAttachmentIds.includes(item.id)
  );
  const selectedAttachmentsNeedSandbox = selectedAttachments.some(attachmentNeedsSandbox);
  const attachmentsBlockSend = selectedAttachments.some(
    (item) => item.ingestionStatus !== "ready"
  );
  const canSend = canSendMessage({
    hasConversation: conversation?.state === "ready",
    hasText: Boolean(draft.trim()),
    attachmentsReady: !attachmentsBlockSend,
    attachmentBusy: attachmentBusy || cameraOpen,
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
  const availableProjectFiles = projectFilesAvailableToConversation(attachments, projectFiles);

  async function importAttachmentPaths(conversationId: string, paths: string[]) {
    setAttachmentBusy(true);
    setAttachmentError(null);
    try {
      const importedIds: string[] = [];
      for (const path of paths) {
        const attachment = await platform.importAttachment(
          conversationId,
          path,
          shouldDescribeImages(imageDescriptionPreference)
        );
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
      const paths = await platform.pickAttachmentPaths(
        broker?.state === "ready" ? brokerAttachmentExtensions(broker.value) : []
      );
      if (paths.length > 0) await importAttachmentPaths(conversation.value.id, paths);
    } catch (error) {
      setAttachmentError(describeError(error));
    }
  };

  function discardScreenCapture() {
    if (screenCaptureUrlRef.current) {
      URL.revokeObjectURL(screenCaptureUrlRef.current);
      screenCaptureUrlRef.current = null;
    }
    cropStartRef.current = null;
    setCropMode(false);
    setCropSelection(null);
    setScreenCapturePreview(null);
  }

  function cropPointerPosition(event: React.PointerEvent<HTMLDivElement>) {
    const bounds = event.currentTarget.getBoundingClientRect();
    return {
      x: (event.clientX - bounds.left) / bounds.width,
      y: (event.clientY - bounds.top) / bounds.height
    };
  }

  const beginCropSelection = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!cropMode) return;
    const point = cropPointerPosition(event);
    cropStartRef.current = point;
    setCropSelection(null);
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const updateCropSelection = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!cropMode || !cropStartRef.current) return;
    const point = cropPointerPosition(event);
    setCropSelection(
      normalizeCropSelection(
        cropStartRef.current.x,
        cropStartRef.current.y,
        point.x,
        point.y,
        0
      )
    );
  };

  const finishCropSelection = (event: React.PointerEvent<HTMLDivElement>) => {
    if (!cropMode || !cropStartRef.current) return;
    const point = cropPointerPosition(event);
    setCropSelection(
      normalizeCropSelection(
        cropStartRef.current.x,
        cropStartRef.current.y,
        point.x,
        point.y
      )
    );
    cropStartRef.current = null;
    if (event.currentTarget.hasPointerCapture(event.pointerId)) {
      event.currentTarget.releasePointerCapture(event.pointerId);
    }
  };

  const applyScreenCaptureCrop = async () => {
    if (!screenCapturePreview || !cropSelection) return;
    setScreenCaptureBusy(true);
    setAttachmentError(null);
    try {
      const frame = await cropCapturedFrame(screenCapturePreview, cropSelection);
      if (screenCaptureUrlRef.current) URL.revokeObjectURL(screenCaptureUrlRef.current);
      const previewUrl = URL.createObjectURL(frame.blob);
      screenCaptureUrlRef.current = previewUrl;
      setScreenCapturePreview({
        ...frame,
        conversationId: screenCapturePreview.conversationId,
        previewUrl,
        source: screenCapturePreview.source
      });
      setCropMode(false);
      setCropSelection(null);
    } catch (error) {
      setAttachmentError(describeError(error));
    } finally {
      setScreenCaptureBusy(false);
    }
  };

  const takeScreenCapture = async () => {
    if (conversation?.state !== "ready") return;
    setScreenCaptureBusy(true);
    setAttachmentError(null);
    try {
      const frame = await captureScreenFrame();
      discardScreenCapture();
      const previewUrl = URL.createObjectURL(frame.blob);
      screenCaptureUrlRef.current = previewUrl;
      setScreenCapturePreview({
        ...frame,
        conversationId: conversation.value.id,
        previewUrl,
        source: "screen"
      });
    } catch (error) {
      if (error instanceof DOMException && error.name === "NotAllowedError") {
        return;
      }
      setAttachmentError(describeError(error));
    } finally {
      setScreenCaptureBusy(false);
    }
  };

  function stopCamera() {
    cameraStreamRef.current?.getTracks().forEach((track) => track.stop());
    cameraStreamRef.current = null;
    if (cameraVideoRef.current) {
      cameraVideoRef.current.srcObject = null;
    }
    setCameraOpen(false);
    setCameraReady(false);
    setCameraConversationId(null);
  }

  const openCamera = async () => {
    if (conversation?.state !== "ready") return;
    setCameraBusy(true);
    setCameraError(null);
    stopCamera();
    try {
      const stream = await openCameraStream();
      discardScreenCapture();
      cameraStreamRef.current = stream;
      setCameraConversationId(conversation.value.id);
      setCameraOpen(true);
    } catch (error) {
      setCameraError(cameraFailureMessage(error));
    } finally {
      setCameraBusy(false);
    }
  };

  const takeCameraPhoto = async () => {
    if (
      conversation?.state !== "ready" ||
      !cameraVideoRef.current ||
      cameraConversationId !== conversation.value.id
    ) {
      stopCamera();
      return;
    }
    setCameraBusy(true);
    setCameraError(null);
    try {
      const frame = await captureVideoFrame(
        cameraVideoRef.current,
        captureDisplayName(new Date(), "foto")
      );
      stopCamera();
      discardScreenCapture();
      const previewUrl = URL.createObjectURL(frame.blob);
      screenCaptureUrlRef.current = previewUrl;
      setScreenCapturePreview({
        ...frame,
        conversationId: conversation.value.id,
        previewUrl,
        source: "camera"
      });
    } catch (error) {
      setCameraError(cameraFailureMessage(error));
    } finally {
      setCameraBusy(false);
    }
  };

  const attachScreenCapture = async () => {
    if (
      conversation?.state !== "ready" ||
      !screenCapturePreview ||
      screenCapturePreview.conversationId !== conversation.value.id
    ) {
      discardScreenCapture();
      return;
    }
    setAttachmentBusy(true);
    setAttachmentError(null);
    try {
      const bytes = Array.from(
        new Uint8Array(await screenCapturePreview.blob.arrayBuffer())
      );
      const attachment = await platform.importCapturedImage(
        conversation.value.id,
        screenCapturePreview.displayName,
        bytes
      );
      setAttachments(await platform.listAttachments(conversation.value.id));
      setDraftAttachmentIds((current) => [...new Set([...current, attachment.id])]);
      discardScreenCapture();
    } catch (error) {
      setAttachmentError(describeError(error));
    } finally {
      setAttachmentBusy(false);
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

  const setAttachmentProjectSharing = async (attachmentId: string, enabled: boolean) => {
    if (conversation?.state !== "ready") return;
    setProjectFileBusyId(attachmentId);
    setAttachmentError(null);
    try {
      setProjectFiles(await platform.setProjectFile(
        conversation.value.id,
        attachmentId,
        enabled
      ));
    } catch (error) {
      setAttachmentError(describeError(error));
    } finally {
      setProjectFileBusyId(null);
    }
  };

  const addProjectFileToConversation = async (attachmentId: string) => {
    if (conversation?.state !== "ready") return;
    setProjectFileBusyId(attachmentId);
    setAttachmentError(null);
    try {
      const nextAttachments = await platform.useProjectFile(
        conversation.value.id,
        attachmentId
      );
      setAttachments(nextAttachments);
      setDraftAttachmentIds((ids) => [...new Set([...ids, attachmentId])]);
    } catch (error) {
      setAttachmentError(describeError(error));
    } finally {
      setProjectFileBusyId(null);
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

  const retryAttachmentContext = async (attachmentId: string) => {
    setAttachmentContextRetryId(attachmentId);
    setAttachmentError(null);
    try {
      const updated = await platform.retryAttachmentContext(attachmentId);
      setAttachments((items) => items.map((item) => item.id === updated.id ? updated : item));
    } catch (error) {
      setAttachmentError(describeError(error));
    } finally {
      setAttachmentContextRetryId(null);
    }
  };

  const retryAttachmentSemanticIndex = async (attachmentId: string) => {
    setAttachmentSemanticRetryId(attachmentId);
    setAttachmentError(null);
    try {
      const updated = await platform.retryAttachmentSemanticIndex(attachmentId);
      setAttachments((items) => items.map((item) => item.id === updated.id ? updated : item));
    } catch (error) {
      setAttachmentError(describeError(error));
    } finally {
      setAttachmentSemanticRetryId(null);
    }
  };

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
      window.alert(`Guardado en:\n${path}`);
    } catch (error) {
      setTaskArtifacts({ state: "error", message: describeError(error) });
    }
  };

  /** Descarta las mediciones acumuladas, incluidas las aún no enviadas. */
  const clearPerformanceSamples = async () => {
    // Borrar las mediciones es irreversible y deja las métricas sin veredicto.
    if (
      !window.confirm(
        "¿Vaciar las mediciones de rendimiento? Las cuatro métricas volverán a quedar sin medir."
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
    setWorkspaceDestination("chats");
    followConversationScrollRef.current = true;
    setConversation({ state: "loading" });
    setAttachments([]);
    setProjectFiles([]);
    setDraftAttachmentIds([]);
    setAttachmentError(null);
    setNavigationError(null);
    const startedAt = performance.now();
    try {
      await loadConversation(conversationId, true);
      // Solo se mide la apertura completada: una que falla describe el error,
      // no el rendimiento.
      recordSample("conversation_open", performance.now() - startedAt);
    } catch (error) {
      setConversation({ state: "error", message: describeError(error) });
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

  const revealEarlierMessages = () => {
    if (conversation?.state !== "ready" || progressiveMessages.hiddenCount === 0) return;
    const messageList = messageListRef.current;
    if (messageList) {
      prependScrollRef.current = {
        scrollHeight: messageList.scrollHeight,
        scrollTop: messageList.scrollTop
      };
    }
    followConversationScrollRef.current = false;
    setMessageWindow({
      conversationId: conversation.value.id,
      limit: Math.min(
        conversation.value.messages.length,
        effectiveMessageLimit + EARLIER_MESSAGE_PAGE_SIZE
      )
    });
  };

  const sendTurn = async (sandboxOverride?: boolean, skipSandboxSuggestion = false) => {
    if (!canSend || conversation?.state !== "ready") return;
    const visibleStartBeganAt = performance.now();
    setComposerError(null);
    const conversationId = conversation.value.id;
    const text = draft;
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
    setDraft("");
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
    try {
      const attachmentIds = [...draftAttachmentIds];
      const task = await platform.sendChatTurn(
        conversationId,
        text,
        attachmentIds,
        toolsEnabled,
        useSandbox,
        semanticMemoryEnabled && semanticMemoryReady,
        researchMode
      );
      setSandboxEnabled(false);
      setResearchMode(false);
      setActiveTurn({ state: "ready", value: task });
      await loadConversation(conversationId);
      await reloadNavigation();
    } catch (error) {
      setActiveTurn({ state: "error", message: describeError(error) });
      setDraft(text);
      setSandboxEnabled(useSandbox);
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
          !window.confirm(
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

  const openDialog = (nextDialog: DialogState) => {
    const copy = dialogCopy(nextDialog);
    setDialog(nextDialog);
    setDialogValue(copy.initialValue ?? "");
    setNavigationError(null);
  };

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
    if (!window.confirm(
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
      setProjectFiles((files) => files.filter((file) => file.id !== attachmentId));
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

  const submitDialog = async () => {
    if (!dialog) return;
    const copy = dialogCopy(dialog);
    if (copy.fieldLabel && !copy.allowEmpty && !dialogValue.trim()) return;
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
        case "project-instructions":
          await platform.updateProjectInstructions(dialog.project.id, dialogValue);
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
        case "custom-gpt-test": {
          // La prueba usa exactamente el mismo recorrido durable que un chat
          // normal: queda en SQLite, aparece en Recientes y puede reanudarse.
          const prompt = dialogValue.trim();
          const created = await platform.createConversation(
            `Prueba · ${dialog.customGpt.name}`,
            dialog.customGpt.defaultProjectId ?? undefined
          );
          await platform.setConversationCustomGpt(created.id, dialog.customGpt.id);
          setDialog(null);
          await reloadNavigation();
          await openConversation(created.id);
          followConversationScrollRef.current = true;
          setActiveTurn({ state: "loading" });
          setActiveTurnConversationId(created.id);
          try {
            const task = await platform.sendChatTurn(
              created.id,
              prompt,
              [],
              false,
              false,
              false,
              false
            );
            setActiveTurn({ state: "ready", value: task });
            await loadConversation(created.id);
          } catch (error) {
            // El chat ya existe: mostramos el fallo dentro de él para que no se
            // pierda el contexto ni parezca que el botón no hizo nada.
            setActiveTurn({ state: "error", message: describeError(error) });
          }
          break;
        }
      }
      await reloadNavigation();
      setDialog(null);
    } catch (error) {
      setNavigationError(describeError(error));
    } finally {
      setDialogBusy(false);
    }
  };


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
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">C</span>
          <div><strong>ChatyGPT</strong><small>Espacio personal · v{APP_VERSION}</small></div>
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
                <button
                  className="secondary"
                  disabled={folderBusy === "new-modify-folder"}
                  onClick={() => void authorizeGptModifyFolder()}
                >
                  {folderBusy === "new-modify-folder"
                    ? "Abriendo selector…"
                    : "Autorizar carpeta para modificar"}
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
          ) : visibleConversationList.map((item) => (
            <button
              key={item.id}
              className={`conversation-link ${
                conversation?.state === "ready" && conversation.value.id === item.id
                  ? "active"
                  : ""
              }`}
              onClick={() => openConversation(item.id)}
              aria-current={
                conversation?.state === "ready" && conversation.value.id === item.id
                  ? "page"
                  : undefined
              }
            >
              {item.title}
            </button>
          ))}
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
            ? `Datos locales · esquema ${bootstrap.value.schemaVersion}`
            : "Preparando datos locales"}
        </div>
      </aside>

      <section className="workspace" aria-label="Espacio de trabajo">
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
            <div className={`chat-workspace ${contextInspectorOpen ? "" : "inspector-collapsed"}`}>
              <section className="chat-surface">
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
                {conversation.value.messages.length === 0 && (
                  <div className="chat-empty">
                    <span className="pill">
                      {conversation.value.projectId ? "Conversación de proyecto" : "Nueva conversación"}
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
                {progressiveMessages.hiddenCount > 0 && (
                  <div className="earlier-messages">
                    <button className="secondary" onClick={revealEarlierMessages}>
                      Mostrar {Math.min(EARLIER_MESSAGE_PAGE_SIZE, progressiveMessages.hiddenCount)} mensajes anteriores
                    </button>
                    <small>{progressiveMessages.hiddenCount} mensajes anteriores todavía ocultos</small>
                  </div>
                )}
                {progressiveMessages.visibleItems.map((message) => (
                  <article key={message.id} className={`message ${message.role}`}>
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
                      conversation.value.researchRuns
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
              {currentTurn?.state === "ready" &&
                currentTurn.value.pendingToolCalls.length > 0 && (
                  <section className="tool-confirmation" aria-label="Confirmación de herramientas">
                    <span className="kicker">Confirmación necesaria</span>
                    <h3>ChatyGPT quiere realizar una acción</h3>
                    <p>
                      Revisa cada propuesta. No se ejecutará ninguna acción hasta que decidas.
                    </p>
                    <div className="tool-call-list">
                      {currentTurn.value.pendingToolCalls.map((call) => {
                        const detail = confirmationSummary(call);
                        return (
                        <article key={call.toolCallId} className="tool-call-card">
                          <div className="tool-call-disclosure">
                            <strong>{detail.action}</strong>
                            <dl>
                              <div>
                                <dt>Herramienta</dt>
                                <dd>{detail.tool}</dd>
                              </div>
                              <div>
                                <dt>Recursos afectados</dt>
                                <dd>{detail.resource}</dd>
                              </div>
                              <div>
                                <dt>Datos que se enviarán</dt>
                                <dd>
                                  {detail.data.length === 0
                                    ? "Ninguno declarado"
                                    : detail.data.map((datum) => (
                                        <span key={datum.label}>
                                          {datum.label}: {datum.value}
                                        </span>
                                      ))}
                                </dd>
                              </div>
                              <div>
                                <dt>Destino</dt>
                                <dd>{detail.destination}</dd>
                              </div>
                              <div>
                                <dt>Alcance</dt>
                                <dd>{detail.scope}</dd>
                              </div>
                              <div>
                                <dt>Consecuencias</dt>
                                <dd>{detail.consequences}</dd>
                              </div>
                            </dl>
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
                              Autorizar una vez
                            </button>
                          </div>
                        </article>
                        );
                      })}
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
                {cameraOpen && conversation.value.id === cameraConversationId && (
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
                  conversation.value.id === screenCapturePreview.conversationId && (
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
                  <section className="project-file-library" aria-label="Archivos del proyecto">
                    <div>
                      <strong>Archivos del proyecto</strong>
                      <small>Reutiliza contexto sin volver a subir el archivo.</small>
                    </div>
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
                  </section>
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
                          {conversation.value.projectId && (
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
                <textarea
                  ref={composerRef}
                  value={draft}
                  onChange={(event) => {
                    setDraft(event.target.value);
                    setSandboxSuggestionPending(false);
                    setComposerError(null);
                  }}
                  onKeyDown={(event) => {
                    if (event.key === "Enter" && !event.shiftKey && canSend) {
                      event.preventDefault();
                      void sendTurn();
                    }
                  }}
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
                <details className="execution-settings">
                  <summary>
                    <span>Opciones de ejecución</span>
                    <small>
                      {conversation.value.executionPreferences.strategy === "auto"
                        ? "Automática"
                        : conversation.value.executionPreferences.strategy === "mixture_of_agents"
                          ? "Análisis en equipo"
                          : "Respuesta directa"}
                      {" · "}
                      {conversation.value.executionPreferences.dataClassification === "internal"
                        ? "Uso personal"
                        : conversation.value.executionPreferences.dataClassification === "public"
                          ? "Público"
                          : conversation.value.executionPreferences.dataClassification === "confidential"
                            ? "Confidencial"
                            : "Solo local"}
                      {" · "}
                      hasta {conversation.value.executionPreferences.maxCostUsd.toFixed(2)} USD
                      {" · "}
                      {conversation.value.executionPreferences.priority <= 25
                        ? "Prioridad alta"
                        : conversation.value.executionPreferences.priority >= 250
                          ? "Prioridad baja"
                          : "Prioridad normal"}
                    </small>
                  </summary>
                  <div className="execution-settings-grid">
                    <label>
                      <span>Privacidad</span>
                      <select
                        value={conversation.value.executionPreferences.dataClassification}
                        onChange={(event) => void updateExecutionPreferences({
                          dataClassification: event.target.value as ConversationExecutionPreferences["dataClassification"]
                        })}
                        disabled={Boolean(currentTurnBlocks) || executionOptionsBusy}
                      >
                        <option value="internal">Uso personal · local o cloud</option>
                        <option value="public">Contenido público · local o cloud</option>
                        <option value="confidential">Confidencial · solo modelos locales</option>
                        <option value="local_only">Solo en este equipo</option>
                      </select>
                      <small>Decide si el contenido puede salir a proveedores cloud.</small>
                    </label>
                    <label>
                      <span>Forma de responder</span>
                      <select
                        value={conversation.value.executionPreferences.strategy}
                        onChange={(event) => void updateExecutionPreferences({
                          strategy: event.target.value as ConversationExecutionPreferences["strategy"]
                        })}
                        disabled={
                          Boolean(currentTurnBlocks) ||
                          executionOptionsBusy ||
                          broker?.state !== "ready"
                        }
                      >
                        <option value="single">Respuesta directa</option>
                        <option
                          value="auto"
                          disabled={
                            broker?.state === "ready" &&
                            !broker.value.strategies.includes("auto")
                          }
                        >
                          Automática · el Broker decide
                        </option>
                        <option
                          value="mixture_of_agents"
                          disabled={
                            broker?.state === "ready" &&
                            !broker.value.strategies.includes("mixture_of_agents")
                          }
                        >
                          Análisis en equipo
                        </option>
                      </select>
                      <small>Solo aparecen como utilizables las estrategias anunciadas por el Broker.</small>
                    </label>
                    <label>
                      <span>Prioridad en la cola</span>
                      <select
                        value={conversation.value.executionPreferences.priority}
                        onChange={(event) => void updateExecutionPreferences({
                          priority: Number(event.target.value)
                        })}
                        disabled={Boolean(currentTurnBlocks) || executionOptionsBusy}
                      >
                        <option value={25}>Alta</option>
                        <option value={100}>Normal</option>
                        <option value={250}>Baja</option>
                      </select>
                      <small>Las tareas con prioridad alta pasan antes cuando esperan recursos.</small>
                    </label>
                    <label>
                      <span>Límite por petición</span>
                      <select
                        value={conversation.value.executionPreferences.maxCostUsd}
                        onChange={(event) => void updateExecutionPreferences({
                          maxCostUsd: Number(event.target.value)
                        })}
                        disabled={Boolean(currentTurnBlocks) || executionOptionsBusy}
                      >
                        <option value={0}>0 USD · sin modelos de pago</option>
                        <option value={0.1}>Hasta 0,10 USD</option>
                        <option value={0.5}>Hasta 0,50 USD</option>
                        <option value={1}>Hasta 1,00 USD</option>
                      </select>
                      <small>Es un corte máximo, no una estimación del coste final.</small>
                    </label>
                    <label>
                      <span>Profundidad</span>
                      <select
                        value={conversation.value.executionPreferences.preset}
                        onChange={(event) => void updateExecutionPreferences({
                          preset: event.target.value as ConversationExecutionPreferences["preset"]
                        })}
                        disabled={
                          Boolean(currentTurnBlocks) ||
                          executionOptionsBusy ||
                          conversation.value.executionPreferences.strategy !== "mixture_of_agents"
                        }
                      >
                        <option value="fast">Normal</option>
                        <option
                          value="slow"
                          disabled={
                            broker?.state === "ready" &&
                            !brokerSupportsPreset(broker.value, "mixture_of_agents", "slow")
                          }
                        >
                          Exhaustiva
                        </option>
                      </select>
                      <small>La profundidad exhaustiva solo se aplica al análisis en equipo.</small>
                    </label>
                    <label className="execution-setting-check">
                      <input
                        type="checkbox"
                        checked={conversation.value.executionPreferences.longContext === "map_reduce"}
                        onChange={(event) => void updateExecutionPreferences({
                          longContext: event.target.checked ? "map_reduce" : "fail"
                        })}
                        disabled={
                          Boolean(currentTurnBlocks) ||
                          executionOptionsBusy ||
                          broker?.state !== "ready" ||
                          !broker.value.longContextMapReduce ||
                          conversation.value.executionPreferences.strategy === "mixture_of_agents"
                        }
                      />
                      <span>
                        Dividir documentos que no caben
                        <small>
                          Autoriza map-reduce; nunca se activa mediante truncado silencioso.
                        </small>
                      </span>
                    </label>
                    <p className="execution-settings-note">
                      Las herramientas usan temporalmente el modo agente. En “Análisis en equipo”,
                      Código aislado se entrega a los modelos que preparan las propuestas.
                    </p>
                  </div>
                </details>
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
                      isTaskBlockingConversation(currentTurn.value) &&
                      currentTurn.value.remoteTaskId && (
                        <button className="secondary danger" onClick={cancelActiveTurn}>
                          Cancelar
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
                {attachmentError && <p className="error">{attachmentError}</p>}
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
                            onClick={() => setDraftAttachmentIds((ids) =>
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

                  <section className="privacy-context">
                    <span>Privacidad</span>
                    <strong>
                      {conversation.value.executionPreferences.dataClassification === "internal"
                        ? "Uso personal"
                        : conversation.value.executionPreferences.dataClassification === "public"
                          ? "Contenido público"
                          : conversation.value.executionPreferences.dataClassification === "confidential"
                            ? "Confidencial"
                            : "Solo en este equipo"}
                    </strong>
                    <small>
                      {conversation.value.executionPreferences.dataClassification === "confidential" ||
                      conversation.value.executionPreferences.dataClassification === "local_only"
                        ? "Solo se usarán modelos locales."
                        : "Puede usar proveedores locales o cloud según el enrutamiento."}
                    </small>
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

              <section className="projects-card" aria-labelledby="projects-heading">
                <div className="panel-heading">
                  <div>
                    <span className="kicker">Organización</span>
                    <h3 id="projects-heading">Proyectos</h3>
                  </div>
                  <button className="primary" onClick={() => openDialog({ kind: "project-create" })}>
                    Crear proyecto
                  </button>
                </div>
                {projects.length === 0 ? (
                  <p className="muted">Crea un proyecto para reunir chats, instrucciones y archivos relacionados.</p>
                ) : (
                  <div className="project-home-list">
                    {projects.map((project) => (
                      <article key={project.id}>
                        <div>
                          <strong>{project.name}</strong>
                          <small>{project.conversationCount} conversación(es)</small>
                        </div>
                        <div className="task-actions">
                          <button
                            className="secondary"
                            onClick={() => {
                              setSelectedProjectId(project.id);
                              setWorkspaceDestination("chats");
                            }}
                          >
                            Ver chats
                          </button>
                          <button className="secondary" onClick={() => void openProjectKnowledge(project)}>
                            Conocimiento
                          </button>
                        </div>
                      </article>
                    ))}
                  </div>
                )}
              </section>

              <WorkflowStudio
                projects={projects}
                customGpts={customGpts.state === "ready" ? customGpts.value : []}
                onOpenBrokerCredential={openBrokerCredentialSettings}
                onOpenAutomations={() => setWorkspaceDestination("automations")}
              />

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
                      <span>
                        Carriles: {broker.value.workLanes.length > 0
                          ? broker.value.workLanes.join(", ")
                          : "no declarados"}
                      </span>
                      <span>
                        Frontera de datos: {broker.value.derivedDataBoundary
                          ? "derivada por clasificación"
                          : "compatibilidad explícita"}
                      </span>
                      <span>
                        Dependencias entre tareas: {broker.value.taskDependencies
                          ? "disponibles"
                          : "no anunciadas"}
                      </span>
                      {(broker.value.agentSkillsEgress?.length ?? 0) > 0 && (
                        <span>
                          Herramientas con salida a Internet: {broker.value.agentSkillsEgress?.join(", ")}
                        </span>
                      )}
                      <span>
                        Documentos largos: {broker.value.longContextMapReduce
                          ? "map-reduce disponible"
                          : "sin map-reduce"}
                      </span>
                      <span>
                        Exclusividad de contenido: {broker.value.contentExclusivity === undefined
                          ? "no consta"
                          : broker.value.contentExclusivity
                            ? "solo la ve el modelo que responde"
                            : "el Broker puede sondear otro modelo local"}
                      </span>
                      <span>
                        Ejecución demostrable: {broker.value.demonstrableExecution === undefined
                          ? "no consta"
                          : broker.value.demonstrableExecution
                            ? "contrato 2.10"
                            : "no la declara"}
                      </span>
                    </div>
                  )}
                  {broker?.state === "error" && <p className="error">{broker.message}</p>}
                </article>
              </div>

              <section className="appearance-card" aria-labelledby="appearance-heading">
                <div>
                  <span className="kicker">Preferencias locales</span>
                  <h3 id="appearance-heading">Apariencia</h3>
                  <p>
                    Elige el aspecto de ChatyGPT. La opción se conserva únicamente en este equipo.
                  </p>
                </div>
                <div className="appearance-options" role="radiogroup" aria-label="Tema de la aplicación">
                  {([
                    ["system", "Windows", "Sigue el tema del sistema"],
                    ["light", "Claro", "Fondo luminoso"],
                    ["dark", "Oscuro", "Menos luz en pantalla"]
                  ] as const).map(([value, label, description]) => (
                    <button
                      key={value}
                      type="button"
                      role="radio"
                      aria-checked={appearancePreference === value}
                      className={appearancePreference === value ? "active" : ""}
                      onClick={() => setAppearancePreference(value)}
                    >
                      <strong>{label}</strong>
                      <span>{description}</span>
                    </button>
                  ))}
                </div>
                <small aria-live="polite">
                  Tema visible: {resolvedAppearance === "dark" ? "oscuro" : "claro"}
                  {appearancePreference === "system" ? " · cambia con Windows" : ""}.
                </small>
              </section>

              <section
                className="appearance-card attachment-processing-card"
                aria-labelledby="attachment-processing-heading"
              >
                <div>
                  <span className="kicker">Procesamiento de archivos</span>
                  <h3 id="attachment-processing-heading">Imágenes en documentos</h3>
                  <p>
                    Decide si el Broker debe describir las figuras con un modelo de visión
                    cuando adjuntes un archivo nuevo.
                  </p>
                </div>
                <div
                  className="appearance-options"
                  role="radiogroup"
                  aria-label="Tratamiento de imágenes en documentos"
                >
                  {([
                    ["describe", "Con imágenes", "Extrae y describe figuras; tarda más"],
                    ["ignore", "Sin imágenes", "Ignora figuras y acelera la conversión"]
                  ] as const).map(([value, label, description]) => (
                    <button
                      key={value}
                      type="button"
                      role="radio"
                      aria-checked={imageDescriptionPreference === value}
                      className={imageDescriptionPreference === value ? "active" : ""}
                      onClick={() => setImageDescriptionPreference(value)}
                    >
                      <strong>{label}</strong>
                      <span>{description}</span>
                    </button>
                  ))}
                </div>
                <small aria-live="polite">
                  Se aplicará a los próximos archivos. Las imágenes adjuntadas directamente
                  y las capturas siempre se procesan.
                </small>
              </section>

              <section className="performance-card" aria-labelledby="performance-heading">
                <div>
                  <span className="kicker">Medición local</span>
                  <h3 id="performance-heading">Rendimiento</h3>
                  <p>
                    Mediciones tomadas en este equipo mientras usas la aplicación. Se
                    guardan únicamente duraciones: ni textos, ni títulos, ni rutas. Cada
                    objetivo se compara con el percentil 95 de las muestras conservadas.
                  </p>
                </div>
                {performanceReport.state === "loading" && <small>Cargando mediciones…</small>}
                {performanceReport.state === "error" && (
                  <small role="alert">{performanceReport.message}</small>
                )}
                {performanceReport.state === "ready" && (
                  <>
                    <div className="performance-grid">
                      {performanceReport.value.metrics.map((summary) => (
                        <article key={summary.metric} className="performance-metric">
                          <header>
                            <strong>{summary.label}</strong>
                            <span
                              className={`badge ${budgetVerdictTone(summary.meetsBudget)}`}
                            >
                              {budgetVerdictLabel(summary.meetsBudget)}
                            </span>
                          </header>
                          <dl>
                            <div>
                              <dt>Objetivo</dt>
                              <dd>≤ {formatDuration(summary.budgetMs)} (p95)</dd>
                            </div>
                            <div>
                              <dt>p95</dt>
                              <dd>{formatDuration(summary.p95Ms)}</dd>
                            </div>
                            <div>
                              <dt>Mediana</dt>
                              <dd>{formatDuration(summary.p50Ms)}</dd>
                            </div>
                            <div>
                              <dt>Peor caso</dt>
                              <dd>{formatDuration(summary.maxMs)}</dd>
                            </div>
                            <div>
                              <dt>Muestras</dt>
                              <dd>{summary.samples}</dd>
                            </div>
                          </dl>
                          <small>{summary.description}</small>
                        </article>
                      ))}
                    </div>
                    <div className="performance-actions">
                      <small aria-live="polite">
                        {performanceReport.value.totalSamples} muestras conservadas ·
                        se guardan como máximo {performanceReport.value.sampleLimit} por
                        métrica y las antiguas se descartan.
                      </small>
                      <button
                        className="secondary"
                        disabled={
                          performanceBusy || performanceReport.value.totalSamples === 0
                        }
                        onClick={() => void clearPerformanceSamples()}
                      >
                        {performanceBusy ? "Vaciando…" : "Vaciar mediciones"}
                      </button>
                    </div>
                  </>
                )}
              </section>

              <section className="broker-credential" aria-labelledby="credential-heading">
                <div>
                  <span className="kicker">Seguridad local</span>
                  <h3 id="credential-heading">Credencial de Broker AI</h3>
                  <p>
                    El token se cifra con Windows para tu cuenta de usuario. No se guarda
                    en la base de datos, ni en los registros, ni en el script de inicio, y
                    nunca se vuelve a mostrar.
                  </p>
                </div>
                {brokerCredential.state === "loading" && <small>Comprobando credencial…</small>}
                {brokerCredential.state === "error" && (
                  <small role="alert">{brokerCredential.message}</small>
                )}
                {brokerCredential.state === "ready" && (
                  <>
                    <div className="credential-state">
                      <span className={`badge ${brokerCredential.value.protected ? "ok" : ""}`}>
                        {brokerCredentialLabel(brokerCredential.value)}
                      </span>
                      <small>{brokerCredential.value.message}</small>
                    </div>
                    <div className="credential-form">
                      <label htmlFor="broker-token">Token administrativo</label>
                      <input
                        id="broker-token"
                        type="password"
                        autoComplete="off"
                        spellCheck={false}
                        placeholder="Pega aquí el token de Broker AI"
                        value={credentialDraft}
                        onChange={(event) => setCredentialDraft(event.target.value)}
                      />
                      <div className="credential-actions">
                        <button
                          className="primary"
                          disabled={credentialBusy || credentialDraft.trim().length === 0}
                          onClick={() => void saveBrokerCredential()}
                        >
                          {credentialBusy ? "Guardando…" : "Guardar credencial"}
                        </button>
                        {brokerCredential.value.protected && (
                          <button
                            className="secondary"
                            disabled={credentialBusy}
                            onClick={() => void removeBrokerCredential()}
                          >
                            Retirar
                          </button>
                        )}
                      </div>
                    </div>
                    {credentialNotice && <small aria-live="polite">{credentialNotice}</small>}
                  </>
                )}
              </section>

              <section className="authorized-folders" aria-labelledby="folders-heading">
                <div>
                  <span className="kicker">Permisos locales</span>
                  <h3 id="folders-heading">Carpetas autorizadas</h3>
                    <p>
                    La escritura y la lectura para GPTs son permisos distintos. Leer siempre
                    requiere una confirmación adicional en el chat y usa modelos locales.
                  </p>
                </div>
                <button
                  className="secondary"
                  disabled={folderBusy === "new-read-folder"}
                  onClick={() => void authorizeGptReadFolder()}
                >
                  {folderBusy === "new-read-folder"
                    ? "Abriendo selector…"
                    : "Autorizar carpeta para lectura"}
                </button>
                {authorizedFolders.state === "loading" && <small>Cargando permisos…</small>}
                {authorizedFolders.state === "error" && (
                  <small role="alert">{authorizedFolders.message}</small>
                )}
                {authorizedFolders.state === "ready" &&
                  (authorizedFolders.value.length === 0 ? (
                    <small>
                      Todavía no has autorizado ninguna carpeta. Aparecerán aquí en cuanto
                      exportes por primera vez.
                    </small>
                  ) : (
                    <ul className="authorized-folder-list">
                      {authorizedFolders.value.map((folder) => (
                        <li key={folder.id} className={folder.revokedAt ? "revoked" : ""}>
                          <div>
                            <strong>{folder.displayName}</strong>
                            <span>{authorizedFolderPurpose(folder)}</span>
                            <small>
                              {folder.revokedAt
                                ? `Revocada el ${folder.revokedAt}`
                                : `Autorizada el ${folder.grantedAt}`}
                            </small>
                          </div>
                          {!folder.revokedAt && (
                            <button
                              className="secondary"
                              disabled={folderBusy === folder.id}
                              onClick={() => void revokeFolder(folder.id)}
                            >
                              {folderBusy === folder.id ? "Revocando…" : "Revocar"}
                            </button>
                          )}
                        </li>
                      ))}
                    </ul>
                  ))}
              </section>

              <TarjetaProgramacion
                programacion={programacion}
                conversations={conversations}
                openConversation={openConversation}
                setWorkspaceDestination={setWorkspaceDestination}
              />

              <TarjetaGpts gpts={gpts} projects={projects} openDialog={openDialog} />

              <TarjetaMemoria memoria={memoria} projects={projects} />

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
                      smokeTask.value.result.assistant_content ??
                      smokeTask.value.result.result_markdown ??
                      JSON.stringify(smokeTask.value.result, null, 2)
                    )}
                  </pre>
                )}
                {smokeTask?.state === "ready" &&
                  smokeTask.value.result &&
                  smokeTask.value.remoteTaskId && (
                    <div className="task-actions">
                      <button
                        className="secondary"
                        onClick={loadTaskArtifacts}
                        disabled={taskArtifacts.state === "loading"}
                      >
                        {taskArtifacts.state === "loading"
                          ? "Consultando…"
                          : "Ver ficheros de la tarea"}
                      </button>
                    </div>
                  )}
                {taskArtifacts.state === "ready" && (
                  <div className="diagnostic">
                    {taskArtifacts.value.length === 0 ? (
                      <span>La tarea no produjo ficheros aparte de su respuesta.</span>
                    ) : (
                      taskArtifacts.value.map((artifact) => (
                        <span key={artifact.artifactId}>
                          {artifact.filename || artifact.artifactId}
                          {" · "}
                          {artifact.final ? "entregable" : artifact.artifactType}
                          {" · "}
                          {artifact.available ? (
                            <button
                              className="secondary"
                              onClick={() => void saveArtifact(artifact.artifactId)}
                            >
                              Guardar
                            </button>
                          ) : (
                            "ya no está en el Broker"
                          )}
                        </span>
                      ))
                    )}
                  </div>
                )}
                {taskArtifacts.state === "error" && (
                  <p className="error">{taskArtifacts.message}</p>
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
