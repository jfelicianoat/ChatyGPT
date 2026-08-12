import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import {
  attachmentFailureGuidance,
  attachmentContextSummary,
  attachmentNeedsSandbox,
  attachmentSelectionOnConversationOpen,
  attachmentStatusLabel,
  brokerSupportsPreset,
  brokerAttachmentExtensions,
  canSendMessage,
  canStartMemoryEdit,
  canUseSemanticMemory,
  shouldPollMemoryIndex,
  shouldPollMemorySearch,
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
  customGptVersionSummary,
  type BootstrapReport,
  type BrokerCredentialStatus,
  type AttachmentView,
  type AuditEventView,
  type AuthorizedFolderView,
  type BrokerDiagnostic,
  type ContextSnapshotView,
  type ConversationSummary,
  type ConversationSummaryOverview,
  type ConversationExecutionPreferences,
  type ComposerErrorGuidance,
  type ConversationView,
  type CustomGptPreview,
  type CustomGptVersionView,
  type CustomGptView,
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
import { isEditableKeyboardTarget, keyboardShortcutAction } from "./keyboard";
import { dialogCopy, type DialogState } from "./dialogs";
import { describeError } from "./errors";
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

type Loadable<T> =
  | { state: "loading" }
  | { state: "ready"; value: T }
  | { state: "error"; message: string };

type MemoryEditDraft = {
  content: string;
  category: MemoryItemView["category"];
  projectId: string;
  sensitive: boolean;
};

type ScreenCapturePreview = CapturedScreenFrame & {
  conversationId: string;
  previewUrl: string;
  source: "screen" | "camera";
};

type WorkspaceDestination = "chats" | "projects" | "gpts" | "automations" | "settings";

export function App() {
  const messageListRef = useRef<HTMLDivElement>(null);
  const searchInputRef = useRef<HTMLInputElement>(null);
  const composerRef = useRef<HTMLTextAreaElement>(null);
  const activeModalRef = useRef<HTMLElement>(null);
  const dialogBusyRef = useRef(false);
  const followConversationScrollRef = useRef(true);
  const [bootstrap, setBootstrap] = useState<Loadable<BootstrapReport>>({ state: "loading" });
  const [appearancePreference, setAppearancePreference] =
    useState<AppearancePreference>(loadAppearancePreference);
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
  const [memory, setMemory] = useState<Loadable<MemoryOverview>>({ state: "loading" });
  const [authorizedFolders, setAuthorizedFolders] =
    useState<Loadable<AuthorizedFolderView[]>>({ state: "loading" });
  const [folderBusy, setFolderBusy] = useState<string | null>(null);
  const [brokerCredential, setBrokerCredential] =
    useState<Loadable<BrokerCredentialStatus>>({ state: "loading" });
  const [credentialDraft, setCredentialDraft] = useState("");
  const [credentialBusy, setCredentialBusy] = useState(false);
  const [credentialNotice, setCredentialNotice] = useState<string | null>(null);
  const [scheduledTasks, setScheduledTasks] =
    useState<Loadable<ScheduledTaskView[]>>({ state: "loading" });
  const [scheduledTaskTemplates, setScheduledTaskTemplates] =
    useState<Loadable<ScheduledTaskTemplateView[]>>({ state: "loading" });
  const [scheduleSearchQuery, setScheduleSearchQuery] = useState("");
  const [scheduleName, setScheduleName] = useState("");
  const [scheduleConversationId, setScheduleConversationId] = useState("");
  const [schedulePrompt, setSchedulePrompt] = useState("");
  const [scheduleAt, setScheduleAt] = useState(defaultScheduledLocalTime);
  const [scheduleExpression, setScheduleExpression] =
    useState<ScheduledTaskView["scheduleExpression"]>("once");
  const [scheduleConfirmed, setScheduleConfirmed] = useState(false);
  const [scheduleEditingId, setScheduleEditingId] = useState<string | null>(null);
  const [scheduleBusyId, setScheduleBusyId] = useState<string | null>(null);
  const [scheduleError, setScheduleError] = useState<string | null>(null);
  const [scheduleNotice, setScheduleNotice] = useState<string | null>(null);
  const [scheduledHistoryStatus, setScheduledHistoryStatus] =
    useState<ScheduledHistoryStatusFilter>("all");
  const [scheduledHistoryPeriod, setScheduledHistoryPeriod] =
    useState<ScheduledHistoryPeriodFilter>("all");
  const [scheduledHistoryTaskId, setScheduledHistoryTaskId] = useState<string | null>(null);
  const [scheduledHistoryPageNumber, setScheduledHistoryPageNumber] = useState(1);
  const [scheduledHistoryPageSize, setScheduledHistoryPageSize] =
    useState<ScheduledRunPageView["pageSize"]>(10);
  const [scheduledHistorySort, setScheduledHistorySort] =
    useState<ScheduledHistorySort>("newest");
  const [scheduledHistoryPage, setScheduledHistoryPage] =
    useState<Loadable<ScheduledRunPageView> | null>(null);
  const [scheduledHistoryRefreshVersion, setScheduledHistoryRefreshVersion] = useState(0);
  const [schedulerCenterOpen, setSchedulerCenterOpen] = useState(false);
  const [schedulerCalendarOpen, setSchedulerCalendarOpen] = useState(false);
  const [schedulerCalendarRange, setSchedulerCalendarRange] =
    useState<7 | 14 | 30>(14);
  const [schedulerCalendarExportMessage, setSchedulerCalendarExportMessage] = useState<
    { kind: "success" | "error"; text: string } | null
  >(null);
  const [windowsStartup, setWindowsStartup] = useState<Loadable<WindowsStartupStatus>>({
    state: "loading"
  });

  useLayoutEffect(() => {
    persistAppearancePreference(appearancePreference);
    const synchronize = () =>
      setResolvedAppearance(applyAppearancePreference(appearancePreference));
    synchronize();
    return appearancePreference === "system"
      ? subscribeToSystemAppearance(synchronize)
      : undefined;
  }, [appearancePreference]);
  const [schedulerReadIds, setSchedulerReadIds] =
    useState<Set<string>>(loadSchedulerReadNotifications);
  const [schedulerNotifications, setSchedulerNotifications] = useState<
    NotificationPermission | "unsupported"
  >(() => {
    if (!("Notification" in window)) return "unsupported";
    return window.Notification.permission;
  });
  const scheduledRunStatesRef = useRef<Map<string, string>>(new Map());
  const schedulerHistoryInitializedRef = useRef(false);
  const schedulerReadStateExistedRef = useRef(schedulerReadNotificationsExist());
  const [customGpts, setCustomGpts] =
    useState<Loadable<CustomGptView[]>>({ state: "loading" });
  const [smokeTask, setSmokeTask] = useState<Loadable<LocalTaskSnapshot> | null>(null);
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
  const [conversation, setConversation] = useState<Loadable<ConversationView> | null>(null);
  const [contextInspectorOpen, setContextInspectorOpen] = useState(true);
  const [activeTurn, setActiveTurn] = useState<Loadable<LocalTaskSnapshot> | null>(null);
  const [activeTurnConversationId, setActiveTurnConversationId] = useState<string | null>(null);
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
  const [memoryDraft, setMemoryDraft] = useState("");
  const [memoryCategory, setMemoryCategory] = useState<"preference" | "instruction" | "fact">("preference");
  const [memoryProjectId, setMemoryProjectId] = useState("global");
  const [memorySensitive, setMemorySensitive] = useState(false);
  const [memoryBusy, setMemoryBusy] = useState(false);
  const [memoryEditingId, setMemoryEditingId] = useState<string | null>(null);
  const [memoryEditDraft, setMemoryEditDraft] = useState<MemoryEditDraft | null>(null);
  const [memoryEditError, setMemoryEditError] = useState<string | null>(null);
  const [memoryNotice, setMemoryNotice] = useState<string | null>(null);
  const [memorySearchQuery, setMemorySearchQuery] = useState("");
  const [memorySearchProjectId, setMemorySearchProjectId] = useState("global");
  const [memorySearch, setMemorySearch] = useState<Loadable<MemorySearchView> | null>(null);
  const [customGptEditingId, setCustomGptEditingId] = useState<string | null>(null);
  const [customGptName, setCustomGptName] = useState("");
  const [customGptDescription, setCustomGptDescription] = useState("");
  const [customGptInstructions, setCustomGptInstructions] = useState("");
  const [customGptStartersText, setCustomGptStartersText] = useState("");
  const [customGptRunCodePermission, setCustomGptRunCodePermission] = useState(false);
  const [customGptRenamePermission, setCustomGptRenamePermission] = useState(false);
  const [customGptPreferredModel, setCustomGptPreferredModel] = useState("");
  const [customGptDefaultProject, setCustomGptDefaultProject] = useState("");
  const [customGptHistoryId, setCustomGptHistoryId] = useState<string | null>(null);
  const [customGptPreview, setCustomGptPreview] =
    useState<Loadable<CustomGptPreview> | null>(null);
  const [customGptVersions, setCustomGptVersions] =
    useState<Loadable<CustomGptVersionView[]>>({ state: "loading" });
  const [customGptBusy, setCustomGptBusy] = useState(false);
  const [customGptError, setCustomGptError] = useState<string | null>(null);
  const [customGptNotice, setCustomGptNotice] = useState<string | null>(null);
  const [customGptKnowledge, setCustomGptKnowledge] = useState<{
    customGptId: string;
    data: Loadable<MemoryItemView[]>;
  } | null>(null);
  const [customGptFiles, setCustomGptFiles] = useState<{
    customGptId: string;
    data: Loadable<AttachmentView[]>;
  } | null>(null);
  const [customGptKnowledgeDraft, setCustomGptKnowledgeDraft] = useState("");
  const [customGptKnowledgeCategory, setCustomGptKnowledgeCategory] =
    useState<MemoryItemView["category"]>("fact");
  const [customGptKnowledgeSensitive, setCustomGptKnowledgeSensitive] = useState(false);
  const [customGptKnowledgeBusy, setCustomGptKnowledgeBusy] = useState(false);
  const [customGptKnowledgeNotice, setCustomGptKnowledgeNotice] =
    useState<string | null>(null);
  const [activeCustomGptKnowledge, setActiveCustomGptKnowledge] =
    useState<Loadable<MemoryItemView[]> | null>(null);
  const [activeCustomGptFiles, setActiveCustomGptFiles] =
    useState<Loadable<AttachmentView[]> | null>(null);
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
    const messageList = messageListRef.current;
    if (!messageList || !followConversationScrollRef.current) return;
    messageList.scrollTop = messageList.scrollHeight;
  }, [conversationScrollSignal]);

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
        "¿Revocar la autorización de escritura de esta carpeta? Las próximas exportaciones volverán a pedir permiso."
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
    platform.getWindowsStartupStatus()
      .then((value) => setWindowsStartup({ state: "ready", value }))
      .catch((error) =>
        setWindowsStartup({ state: "error", message: describeError(error) })
      );
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

  useEffect(() => {
    if (bootstrap.state !== "ready") return;
    const refresh = () => {
      platform.listScheduledTasks()
        .then((value) => {
          const { notifications, nextStates } = pendingScheduledRunNotifications({
            tasks: value,
            knownStates: scheduledRunStatesRef.current,
            historyInitialized: schedulerHistoryInitializedRef.current,
            permissionGranted: schedulerNotifications === "granted"
          });
          for (const notification of notifications) {
            try {
              new window.Notification(notification.title, {
                body: notification.body,
                tag: notification.tag
              });
            } catch {
              // El historial visible sigue siendo la fuente durable si Windows
              // rechaza el aviso.
            }
          }
          scheduledRunStatesRef.current = nextStates;
          schedulerHistoryInitializedRef.current = true;
          setScheduledTasks({ state: "ready", value });
        })
        .catch((error) =>
          setScheduledTasks({ state: "error", message: describeError(error) })
        );
    };
    const interval = window.setInterval(refresh, 10_000);
    return () => window.clearInterval(interval);
  }, [bootstrap.state, schedulerNotifications]);

  useEffect(() => {
    if (!scheduledHistoryTaskId) {
      setScheduledHistoryPage(null);
      return;
    }
    let disposed = false;
    const load = (showLoading: boolean) => {
      if (showLoading) setScheduledHistoryPage({ state: "loading" });
      platform.listScheduledRuns(
        scheduledHistoryTaskId,
        scheduledHistoryStatus,
        scheduledHistoryPeriod,
        scheduledHistorySort,
        scheduledHistoryPageNumber,
        scheduledHistoryPageSize
      )
        .then((value) => {
          if (disposed) return;
          setScheduledHistoryPage({ state: "ready", value });
          if (value.page !== scheduledHistoryPageNumber) {
            setScheduledHistoryPageNumber(value.page);
          }
        })
        .catch((error) => {
          if (!disposed) {
            setScheduledHistoryPage({ state: "error", message: describeError(error) });
          }
        });
    };
    load(true);
    const interval = window.setInterval(() => load(false), 10_000);
    return () => {
      disposed = true;
      window.clearInterval(interval);
    };
  }, [
    scheduledHistoryTaskId,
    scheduledHistoryStatus,
    scheduledHistoryPeriod,
    scheduledHistorySort,
    scheduledHistoryPageNumber,
    scheduledHistoryPageSize,
    scheduledHistoryRefreshVersion
  ]);

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
    conversation?.state === "ready" ? conversation.value.id : null
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

  const refreshAuditEvents = async () => {
    setAuditEvents({ state: "loading" });
    try {
      setAuditEvents({ state: "ready", value: await platform.listAuditEvents() });
    } catch (error) {
      setAuditEvents({ state: "error", message: describeError(error) });
    }
  };

  const toggleMemory = async () => {
    if (memory.state !== "ready") return;
    setMemoryBusy(true);
    try {
      setMemory({ state: "ready", value: await platform.setMemoryEnabled(!memory.value.enabled) });
      await refreshAuditEvents();
    } catch (error) {
      setMemory({ state: "error", message: describeError(error) });
    } finally {
      setMemoryBusy(false);
    }
  };

  const createMemory = async () => {
    if (!memoryDraft.trim()) return;
    setMemoryBusy(true);
    try {
      const overview = await platform.createMemoryItem(
        memoryDraft,
        memoryCategory,
        memorySensitive ? "sensitive" : "normal",
        memoryProjectId === "global" ? undefined : memoryProjectId
      );
      setMemory({ state: "ready", value: overview });
      setMemoryDraft("");
      setMemorySensitive(false);
      await refreshAuditEvents();
    } catch (error) {
      setMemory({ state: "error", message: describeError(error) });
    } finally {
      setMemoryBusy(false);
    }
  };

  const toggleMemoryItem = async (memoryId: string, enabled: boolean) => {
    setMemoryBusy(true);
    try {
      setMemory({ state: "ready", value: await platform.setMemoryItemEnabled(memoryId, enabled) });
      await refreshAuditEvents();
    } catch (error) {
      setMemory({ state: "error", message: describeError(error) });
    } finally {
      setMemoryBusy(false);
    }
  };

  const beginMemoryEdit = (item: MemoryItemView) => {
    setMemoryEditingId(item.id);
    setMemoryEditDraft({
      content: item.content,
      category: item.category,
      projectId: item.projectId ?? "global",
      sensitive: item.sensitivity === "sensitive"
    });
    setMemoryEditError(null);
    setMemoryNotice(null);
  };

  const cancelMemoryEdit = () => {
    setMemoryEditingId(null);
    setMemoryEditDraft(null);
    setMemoryEditError(null);
  };

  const saveMemoryEdit = async () => {
    if (!memoryEditingId || !memoryEditDraft?.content.trim()) return;
    const editedId = memoryEditingId;
    const previous = memory.state === "ready"
      ? memory.value.items.find((item) => item.id === editedId)
      : undefined;
    const contentChanged = previous?.content !== memoryEditDraft.content.trim();
    setMemoryBusy(true);
    setMemoryEditError(null);
    try {
      const overview = await platform.updateMemoryItem(
        editedId,
        memoryEditDraft.content,
        memoryEditDraft.category,
        memoryEditDraft.sensitive ? "sensitive" : "normal",
        memoryEditDraft.projectId === "global" ? undefined : memoryEditDraft.projectId
      );
      setMemory({ state: "ready", value: overview });
      cancelMemoryEdit();
      setMemoryNotice(memoryUpdateNotice(contentChanged));
      requestAnimationFrame(() => {
        document
          .querySelector<HTMLButtonElement>(
            `[data-memory-id="${editedId}"] .memory-edit-button`
          )
          ?.focus();
      });
      await refreshAuditEvents();
    } catch (error) {
      setMemoryEditError(describeError(error));
    } finally {
      setMemoryBusy(false);
    }
  };

  const removeMemoryItem = async (memoryId: string) => {
    if (!window.confirm("¿Eliminar este recuerdo de forma permanente?")) return;
    setMemoryBusy(true);
    try {
      setMemory({ state: "ready", value: await platform.deleteMemoryItem(memoryId) });
      await refreshAuditEvents();
    } catch (error) {
      setMemory({ state: "error", message: describeError(error) });
    } finally {
      setMemoryBusy(false);
    }
  };

  const reindexMemoryItem = async (memoryId: string) => {
    setMemoryBusy(true);
    try {
      setMemory({ state: "ready", value: await platform.reindexMemoryItem(memoryId) });
    } catch (error) {
      setMemory({ state: "error", message: describeError(error) });
    } finally {
      setMemoryBusy(false);
    }
  };

  const resetCustomGptForm = () => {
    setCustomGptEditingId(null);
    setCustomGptName("");
    setCustomGptDescription("");
    setCustomGptInstructions("");
    setCustomGptStartersText("");
    setCustomGptRunCodePermission(false);
    setCustomGptRenamePermission(false);
    setCustomGptPreferredModel("");
    setCustomGptDefaultProject("");
    setCustomGptError(null);
  };

  const loadCustomGptVersions = async (customGptId: string) => {
    if (customGptHistoryId === customGptId) {
      setCustomGptHistoryId(null);
      return;
    }
    setCustomGptHistoryId(customGptId);
    setCustomGptVersions({ state: "loading" });
    try {
      setCustomGptVersions({
        state: "ready",
        value: await platform.listCustomGptVersions(customGptId)
      });
    } catch (error) {
      setCustomGptVersions({ state: "error", message: describeError(error) });
    }
  };

  const restoreCustomGptVersion = async (customGptId: string, versionId: string) => {
    // Restaurar reemplaza la configuración vigente del GPT. Las versiones
    // anteriores se conservan, pero la decisión sigue siendo de la persona.
    if (
      !window.confirm(
        "¿Restaurar esta versión? Reemplazará la configuración actual del GPT; las versiones anteriores se conservan."
      )
    ) {
      return;
    }
    setCustomGptBusy(true);
    setCustomGptError(null);
    try {
      const restored = await platform.restoreCustomGptVersion(customGptId, versionId);
      setCustomGpts({ state: "ready", value: await platform.listCustomGpts() });
      setCustomGptVersions({
        state: "ready",
        value: await platform.listCustomGptVersions(customGptId)
      });
      setCustomGptNotice(
        `${restored.name}: restaurado como versión ${restored.versionNo}. Las anteriores se conservan.`
      );
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptError(describeError(error));
    } finally {
      setCustomGptBusy(false);
    }
  };

  const openCustomGptPreview = async (customGptId: string) => {
    // La vista previa no envía nada al Broker ni genera coste: solo compone
    // localmente lo que recibiría el modelo si se usara este GPT.
    setCustomGptPreview({ state: "loading" });
    try {
      setCustomGptPreview({
        state: "ready",
        value: await platform.previewCustomGpt(customGptId)
      });
    } catch (error) {
      setCustomGptPreview({ state: "error", message: describeError(error) });
    }
  };

  const duplicateCustomGpt = async (customGptId: string) => {
    setCustomGptBusy(true);
    setCustomGptError(null);
    try {
      const copy = await platform.duplicateCustomGpt(customGptId);
      setCustomGpts({ state: "ready", value: await platform.listCustomGpts() });
      setCustomGptNotice(
        `${copy.name}: copia creada sin permisos ni conocimiento del original.`
      );
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptError(describeError(error));
    } finally {
      setCustomGptBusy(false);
    }
  };

  const beginCustomGptEdit = (item: CustomGptView) => {
    setCustomGptEditingId(item.id);
    setCustomGptName(item.name);
    setCustomGptDescription(item.description ?? "");
    setCustomGptInstructions(item.instructions);
    setCustomGptStartersText(item.conversationStarters.join("\n"));
    setCustomGptRunCodePermission(item.toolPermissions.runCode === "confirm");
    setCustomGptRenamePermission(item.toolPermissions.renameConversation === "confirm");
    setCustomGptPreferredModel(item.preferredModel ?? "");
    setCustomGptDefaultProject(item.defaultProjectId ?? "");
    setCustomGptError(null);
    setCustomGptNotice(null);
  };

  const saveCustomGpt = async () => {
    if (!customGptName.trim() || !customGptInstructions.trim()) return;
    setCustomGptBusy(true);
    setCustomGptError(null);
    setCustomGptNotice(null);
    try {
      const conversationStarters = customGptStartersText
        .split(/\r?\n/)
        .map((starter) => starter.trim())
        .filter(Boolean);
      const saved = customGptEditingId
        ? await platform.updateCustomGpt(
            customGptEditingId,
            customGptName,
            customGptDescription,
            customGptInstructions,
            conversationStarters,
            {
              runCode: customGptRunCodePermission ? "confirm" : "deny",
              renameConversation: customGptRenamePermission ? "confirm" : "deny"
            },
            customGptPreferredModel.trim() || null,
            customGptDefaultProject || null
          )
        : await platform.createCustomGpt(
            customGptName,
            customGptDescription,
            customGptInstructions,
            conversationStarters,
            {
              runCode: customGptRunCodePermission ? "confirm" : "deny",
              renameConversation: customGptRenamePermission ? "confirm" : "deny"
            },
            customGptPreferredModel.trim() || null,
            customGptDefaultProject || null
          );
      setCustomGpts({ state: "ready", value: await platform.listCustomGpts() });
      setCustomGptNotice(
        customGptEditingId
          ? `${saved.name}: versión ${saved.versionNo} guardada.`
          : `${saved.name}: GPT creado con su versión 1.`
      );
      resetCustomGptForm();
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptError(describeError(error));
    } finally {
      setCustomGptBusy(false);
    }
  };

  const importCustomGpt = async () => {
    setCustomGptBusy(true);
    setCustomGptError(null);
    setCustomGptNotice(null);
    try {
      const sourcePath = await platform.pickCustomGptImportPath();
      if (!sourcePath) return;
      const report = await platform.importCustomGpt(sourcePath);
      const imported = report.customGpt;
      setCustomGpts({ state: "ready", value: await platform.listCustomGpts() });
      setCustomGptNotice(
        report.knowledgeRequiresReview
          ? `${imported.name}: GPT importado con ${report.importedKnowledge} elemento(s) de conocimiento pendientes de revisión. Abre Conocimiento y pulsa Usar en los que quieras activar.`
          : `${imported.name}: GPT importado como versión 1. Sus permisos están denegados por seguridad.`
      );
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptError(describeError(error));
    } finally {
      setCustomGptBusy(false);
    }
  };

  const exportCustomGpt = async (item: CustomGptView, includeKnowledge = false) => {
    setCustomGptBusy(true);
    setCustomGptError(null);
    setCustomGptNotice(null);
    try {
      const destinationPath = await platform.pickCustomGptExportPath(
        includeKnowledge ? `${item.name}-con-conocimiento` : item.name
      );
      if (!destinationPath) return;
      const report = await platform.exportCustomGpt(
        item.id,
        destinationPath,
        includeKnowledge
      );
      setCustomGptNotice(
        includeKnowledge
          ? `${item.name}: paquete exportado con ${report.includedKnowledge} elemento(s) de conocimiento. Se excluyeron ${report.excludedSensitive} sensibles, ${report.excludedDisabled} desactivados y ${report.excludedFiles} archivos.`
          : `${item.name}: configuración exportada sin conocimiento ni archivos.`
      );
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptError(describeError(error));
    } finally {
      setCustomGptBusy(false);
    }
  };

  const openCustomGptKnowledge = async (customGptId: string) => {
    if (customGptKnowledge?.customGptId === customGptId) {
      setCustomGptKnowledge(null);
      setCustomGptFiles(null);
      setCustomGptKnowledgeNotice(null);
      return;
    }
    setCustomGptKnowledge({ customGptId, data: { state: "loading" } });
    setCustomGptFiles({ customGptId, data: { state: "loading" } });
    setCustomGptKnowledgeNotice(null);
    try {
      const [items, files] = await Promise.all([
        platform.getCustomGptKnowledge(customGptId),
        platform.listCustomGptFiles(customGptId)
      ]);
      setCustomGptKnowledge({
        customGptId,
        data: { state: "ready", value: items }
      });
      setCustomGptFiles({
        customGptId,
        data: { state: "ready", value: files }
      });
    } catch (error) {
      const message = describeError(error);
      setCustomGptKnowledge({
        customGptId,
        data: { state: "error", message }
      });
      setCustomGptFiles({
        customGptId,
        data: { state: "error", message }
      });
    }
  };

  const importCustomGptFiles = async () => {
    if (!customGptKnowledge) return;
    const customGptId = customGptKnowledge.customGptId;
    setCustomGptKnowledgeBusy(true);
    setCustomGptKnowledgeNotice(null);
    try {
      const paths = await platform.pickAttachmentPaths(
        broker?.state === "ready" ? brokerAttachmentExtensions(broker.value) : []
      );
      if (paths.length === 0) return;
      const currentCount =
        customGptFiles?.customGptId === customGptId &&
        customGptFiles.data.state === "ready"
          ? customGptFiles.data.value.length
          : 0;
      if (currentCount + paths.length > 20) {
        throw new Error(
          `Este GPT admite hasta 20 archivos. Ya tiene ${currentCount} y has elegido ${paths.length}.`
        );
      }
      for (const sourcePath of paths) {
        await platform.importCustomGptFile(customGptId, sourcePath);
      }
      const files = await platform.listCustomGptFiles(customGptId);
      setCustomGptFiles({ customGptId, data: { state: "ready", value: files } });
      if (
        conversation?.state === "ready" &&
        conversation.value.customGptId === customGptId
      ) {
        setActiveCustomGptFiles({ state: "ready", value: files });
      }
      setCustomGptKnowledgeNotice(
        `${paths.length} archivo(s) añadido(s). Se usarán cuando su estado sea Preparado.`
      );
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptFiles({
        customGptId,
        data: { state: "error", message: describeError(error) }
      });
    } finally {
      setCustomGptKnowledgeBusy(false);
    }
  };

  const removeCustomGptFile = async (attachmentId: string) => {
    if (!customGptKnowledge) return;
    if (!window.confirm("¿Retirar este archivo del conocimiento de este GPT?")) return;
    const customGptId = customGptKnowledge.customGptId;
    setCustomGptKnowledgeBusy(true);
    try {
      const files = await platform.removeCustomGptFile(customGptId, attachmentId);
      setCustomGptFiles({ customGptId, data: { state: "ready", value: files } });
      if (
        conversation?.state === "ready" &&
        conversation.value.customGptId === customGptId
      ) {
        setActiveCustomGptFiles({ state: "ready", value: files });
      }
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptFiles({
        customGptId,
        data: { state: "error", message: describeError(error) }
      });
    } finally {
      setCustomGptKnowledgeBusy(false);
    }
  };

  const createCustomGptKnowledge = async () => {
    if (
      !customGptKnowledge ||
      customGptKnowledge.data.state !== "ready" ||
      !customGptKnowledgeDraft.trim()
    ) return;
    const customGptId = customGptKnowledge.customGptId;
    setCustomGptKnowledgeBusy(true);
    setCustomGptKnowledgeNotice(null);
    try {
      const items = await platform.createCustomGptKnowledgeItem(
        customGptId,
        customGptKnowledgeDraft,
        customGptKnowledgeCategory,
        customGptKnowledgeSensitive ? "sensitive" : "normal"
      );
      setCustomGptKnowledge({ customGptId, data: { state: "ready", value: items } });
      if (
        conversation?.state === "ready" &&
        conversation.value.customGptId === customGptId
      ) {
        setActiveCustomGptKnowledge({ state: "ready", value: items });
      }
      setCustomGptKnowledgeDraft("");
      setCustomGptKnowledgeSensitive(false);
      setCustomGptKnowledgeNotice(
        "Conocimiento guardado. Ya se aplicará cuando selecciones este GPT en un chat."
      );
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptKnowledge({
        customGptId,
        data: { state: "error", message: describeError(error) }
      });
    } finally {
      setCustomGptKnowledgeBusy(false);
    }
  };

  const toggleCustomGptKnowledgeItem = async (
    memoryId: string,
    enabled: boolean
  ) => {
    if (!customGptKnowledge) return;
    const customGptId = customGptKnowledge.customGptId;
    setCustomGptKnowledgeBusy(true);
    try {
      const items = await platform.setCustomGptKnowledgeItemEnabled(
        customGptId,
        memoryId,
        enabled
      );
      setCustomGptKnowledge({ customGptId, data: { state: "ready", value: items } });
      if (
        conversation?.state === "ready" &&
        conversation.value.customGptId === customGptId
      ) {
        setActiveCustomGptKnowledge({ state: "ready", value: items });
      }
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptKnowledge({
        customGptId,
        data: { state: "error", message: describeError(error) }
      });
    } finally {
      setCustomGptKnowledgeBusy(false);
    }
  };

  const removeCustomGptKnowledgeItem = async (memoryId: string) => {
    if (!customGptKnowledge) return;
    if (!window.confirm("¿Eliminar este conocimiento solo de este GPT personal?")) return;
    const customGptId = customGptKnowledge.customGptId;
    setCustomGptKnowledgeBusy(true);
    try {
      const items = await platform.deleteCustomGptKnowledgeItem(customGptId, memoryId);
      setCustomGptKnowledge({ customGptId, data: { state: "ready", value: items } });
      if (
        conversation?.state === "ready" &&
        conversation.value.customGptId === customGptId
      ) {
        setActiveCustomGptKnowledge({ state: "ready", value: items });
      }
      await refreshAuditEvents();
    } catch (error) {
      setCustomGptKnowledge({
        customGptId,
        data: { state: "error", message: describeError(error) }
      });
    } finally {
      setCustomGptKnowledgeBusy(false);
    }
  };

  const reindexCustomGptKnowledgeItem = async (memoryId: string) => {
    if (!customGptKnowledge) return;
    const customGptId = customGptKnowledge.customGptId;
    setCustomGptKnowledgeBusy(true);
    try {
      const items = await platform.reindexCustomGptKnowledgeItem(customGptId, memoryId);
      setCustomGptKnowledge({ customGptId, data: { state: "ready", value: items } });
      if (
        conversation?.state === "ready" &&
        conversation.value.customGptId === customGptId
      ) {
        setActiveCustomGptKnowledge({ state: "ready", value: items });
      }
    } catch (error) {
      setCustomGptKnowledge({
        customGptId,
        data: { state: "error", message: describeError(error) }
      });
    } finally {
      setCustomGptKnowledgeBusy(false);
    }
  };

  const runMemorySearch = async () => {
    if (!memorySearchQuery.trim()) return;
    setMemorySearch({ state: "loading" });
    try {
      const result = await platform.startMemorySearch(
        memorySearchQuery,
        memorySearchProjectId === "global" ? undefined : memorySearchProjectId
      );
      setMemorySearch({ state: "ready", value: result });
    } catch (error) {
      setMemorySearch({ state: "error", message: describeError(error) });
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

  const openWorkspaceDestination = (destination: WorkspaceDestination) => {
    setWorkspaceDestination(destination);
    setConversation(null);
    setNavigationError(null);
    if (destination !== "projects") setSelectedProjectId(null);
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
  const schedulerCenterItems = useMemo(
    () => scheduledTasks.state === "ready"
      ? scheduledNotifications(scheduledTasks.value)
      : [],
    [scheduledTasks]
  );
  const schedulerUnreadCount = schedulerCenterItems.filter(
    (item) => !schedulerReadIds.has(item.id)
  ).length;
  const schedulerCalendarItems = useMemo(
    () => scheduledTasks.state === "ready"
      ? scheduledCalendarOccurrences(
          scheduledTasks.value,
          new Date(),
          schedulerCalendarRange
        )
      : [],
    [scheduledTasks, schedulerCalendarRange]
  );
  const schedulerCalendarGroupedDays = useMemo(
    () => schedulerCalendarDays(schedulerCalendarItems),
    [schedulerCalendarItems]
  );
  const schedulerCalendarConflicts = schedulerCalendarConflictCount(
    schedulerCalendarItems
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
      }
      await reloadNavigation();
      setDialog(null);
    } catch (error) {
      setNavigationError(describeError(error));
    } finally {
      setDialogBusy(false);
    }
  };

  const createSchedule = async () => {
    const validation = validateScheduleDraft({
      name: scheduleName,
      conversationId: scheduleConversationId,
      prompt: schedulePrompt,
      at: scheduleAt,
      confirmed: scheduleConfirmed
    });
    if (validation.status === "incomplete") return;
    setScheduleBusyId("create");
    setScheduleError(null);
    setScheduleNotice(null);
    try {
      if (validation.status === "invalid-date") {
        throw new Error(validation.message);
      }
      const dueAt = new Date(validation.dueAtIso);
      const timezone = resolvedSchedulerTimezone();
      if (scheduleEditingId) {
        await platform.updateScheduledTask(
          scheduleEditingId,
          scheduleName.trim(),
          scheduleConversationId,
          schedulePrompt.trim(),
          dueAt.toISOString(),
          timezone,
          scheduleExpression
        );
      } else {
        await platform.createScheduledTask(
          scheduleName.trim(),
          scheduleConversationId,
          schedulePrompt.trim(),
          dueAt.toISOString(),
          timezone,
          scheduleExpression
        );
      }
      setScheduledTasks({
        state: "ready",
        value: await platform.listScheduledTasks()
      });
      setScheduleName("");
      setSchedulePrompt("");
      setScheduleAt(defaultScheduledLocalTime());
      setScheduleExpression("once");
      setScheduleConfirmed(false);
      setScheduleEditingId(null);
      setScheduleNotice(
        scheduleEditingId
          ? "Cambios guardados. La programación vuelve a estar activa con la nueva fecha."
          : scheduleExpression === "once"
          ? "Programación guardada y activa. ChatyGPT la ejecutará una sola vez a la hora indicada."
          : `Programación guardada y activa. Se repetirá ${
              scheduleExpression === "daily" ? "cada día" : "cada semana"
            } a la hora indicada.`
      );
    } catch (error) {
      setScheduleError(describeError(error));
    } finally {
      setScheduleBusyId(null);
    }
  };

  const beginScheduleEdit = (task: ScheduledTaskView) => {
    setScheduleEditingId(task.id);
    setScheduleName(task.name);
    setScheduleConversationId(task.conversationId);
    setSchedulePrompt(task.prompt);
    setScheduleExpression(task.scheduleExpression);
    setScheduleAt(
      task.nextRunAt
        ? scheduledLocalTimeValue(new Date(task.nextRunAt))
        : defaultScheduledLocalTime()
    );
    setScheduleConfirmed(false);
    setScheduleError(null);
    setScheduleNotice(
      "Revisa los cambios y vuelve a marcar la confirmación para guardar."
    );
    document.querySelector(".scheduler-card")?.scrollIntoView({
      behavior: "smooth",
      block: "start"
    });
  };

  const duplicateSchedule = (task: ScheduledTaskView) => {
    const duplicate = scheduledTaskDuplicateDraft(task);
    setScheduleEditingId(null);
    setScheduleName(duplicate.name);
    setScheduleConversationId(duplicate.conversationId);
    setSchedulePrompt(duplicate.prompt);
    setScheduleExpression(duplicate.scheduleExpression);
    setScheduleAt(defaultScheduledLocalTime());
    setScheduleConfirmed(duplicate.confirmed);
    setScheduleError(null);
    setScheduleNotice(
      "Copia preparada como borrador. Revisa la fecha y vuelve a confirmar antes de activarla."
    );
    document.querySelector(".scheduler-card")?.scrollIntoView({
      behavior: "smooth",
      block: "start"
    });
  };

  const cancelScheduleEdit = () => {
    setScheduleEditingId(null);
    setScheduleName("");
    setSchedulePrompt("");
    setScheduleAt(defaultScheduledLocalTime());
    setScheduleExpression("once");
    setScheduleConfirmed(false);
    setScheduleNotice(null);
  };

  const saveScheduledTaskTemplate = async () => {
    if (!canSaveScheduleTemplate({ name: scheduleName, prompt: schedulePrompt })) return;
    setScheduleBusyId("template-create");
    setScheduleError(null);
    setScheduleNotice(null);
    try {
      await platform.createScheduledTaskTemplate(
        scheduleName.trim(),
        schedulePrompt.trim(),
        scheduleExpression
      );
      setScheduledTaskTemplates({
        state: "ready",
        value: await platform.listScheduledTaskTemplates()
      });
      setScheduleNotice(
        "Plantilla guardada. No se ha programado ni activado ninguna ejecución."
      );
    } catch (error) {
      setScheduleError(describeError(error));
    } finally {
      setScheduleBusyId(null);
    }
  };

  const applyScheduledTaskTemplate = (template: ScheduledTaskTemplateView) => {
    setScheduleEditingId(null);
    setScheduleName(template.name);
    setSchedulePrompt(template.prompt);
    setScheduleExpression(template.scheduleExpression);
    setScheduleConfirmed(false);
    setScheduleError(null);
    setScheduleNotice(
      "Plantilla aplicada. Elige conversación y fecha, revisa el contenido y confirma para activarla."
    );
  };

  const removeScheduledTaskTemplate = async (
    template: ScheduledTaskTemplateView
  ) => {
    if (!window.confirm(`¿Eliminar la plantilla “${template.name}”?`)) return;
    setScheduleBusyId(template.id);
    setScheduleError(null);
    setScheduleNotice(null);
    try {
      await platform.deleteScheduledTaskTemplate(template.id);
      setScheduledTaskTemplates({
        state: "ready",
        value: await platform.listScheduledTaskTemplates()
      });
      setScheduleNotice(`Se ha eliminado la plantilla “${template.name}”.`);
    } catch (error) {
      setScheduleError(describeError(error));
    } finally {
      setScheduleBusyId(null);
    }
  };

  const enableSchedulerNotifications = async () => {
    setScheduleError(null);
    if (!("Notification" in window)) {
      setSchedulerNotifications("unsupported");
      setScheduleError(
        "Esta versión de WebView2 no permite avisos de Windows. El historial seguirá actualizándose."
      );
      return;
    }
    try {
      const permission = await window.Notification.requestPermission();
      setSchedulerNotifications(permission);
      if (permission === "granted") {
        new window.Notification("Avisos de ChatyGPT activados", {
          body: "Te avisaré cuando termine una tarea programada.",
          tag: "chatygpt-scheduler-permission"
        });
      } else {
        setScheduleError(
          "Windows no concedió permiso para mostrar avisos. Puedes seguir usando el historial."
        );
      }
    } catch (error) {
      setScheduleError(describeError(error));
    }
  };

  const markSchedulerNotificationRead = (item: ScheduledNotificationView) => {
    setSchedulerReadIds((current) => {
      const updated = new Set(current);
      updated.add(item.id);
      persistSchedulerReadNotifications(updated);
      return updated;
    });
  };

  const markAllSchedulerNotificationsRead = () => {
    setSchedulerReadIds((current) => {
      const updated = new Set(current);
      for (const item of schedulerCenterItems) updated.add(item.id);
      persistSchedulerReadNotifications(updated);
      return updated;
    });
  };

  const toggleScheduledHistory = (task: ScheduledTaskView) => {
    if (scheduledHistoryTaskId === task.id) {
      setScheduledHistoryTaskId(null);
      return;
    }
    setScheduledHistoryTaskId(task.id);
    setScheduledHistoryPageNumber(1);
    setScheduledHistoryPage(null);
  };

  const retryScheduledRun = async (
    task: ScheduledTaskView,
    run: ScheduledTaskView["runs"][number]
  ) => {
    if (
      !window.confirm(
        `¿Reintentar “${task.name}”? Se conservará el intento fallido en el historial.`
      )
    ) return;
    setScheduleBusyId(run.id);
    setScheduleError(null);
    setScheduleNotice(null);
    try {
      await platform.retryScheduledRun(run.id);
      setScheduledTasks({
        state: "ready",
        value: await platform.listScheduledTasks()
      });
      setScheduleNotice(
        `Se ha iniciado un nuevo intento de “${task.name}”. El fallo anterior se conserva.`
      );
      setScheduledHistoryRefreshVersion((current) => current + 1);
    } catch (error) {
      setScheduleError(describeError(error));
      try {
        setScheduledTasks({
          state: "ready",
          value: await platform.listScheduledTasks()
        });
      } catch {
        // Se mantiene el error original si también falla la actualización visual.
      }
    } finally {
      setScheduleBusyId(null);
    }
  };

  const runScheduledTaskNow = async (task: ScheduledTaskView) => {
    const recurringNote = task.nextRunAt
      ? " La próxima fecha programada no cambiará."
      : " Esta ejecución no reactivará una programación finalizada o pausada.";
    if (!window.confirm(`¿Ejecutar ahora “${task.name}”?${recurringNote}`)) return;
    const busyId = `run-now:${task.id}`;
    setScheduleBusyId(busyId);
    setScheduleError(null);
    setScheduleNotice(null);
    try {
      await platform.runScheduledTaskNow(task.id);
      setScheduledTasks({
        state: "ready",
        value: await platform.listScheduledTasks()
      });
      setScheduleNotice(
        `Se ha iniciado “${task.name}” ahora. Su programación futura no ha cambiado.`
      );
      setScheduledHistoryRefreshVersion((current) => current + 1);
    } catch (error) {
      setScheduleError(describeError(error));
      try {
        setScheduledTasks({
          state: "ready",
          value: await platform.listScheduledTasks()
        });
      } catch {
        // Se mantiene el error original si también falla la actualización visual.
      }
    } finally {
      setScheduleBusyId(null);
    }
  };

  const cancelScheduledRun = async (
    task: ScheduledTaskView,
    run: ScheduledTaskView["runs"][number]
  ) => {
    const recurringNote = task.scheduleExpression === "once"
      ? ""
      : " La próxima repetición seguirá programada.";
    if (
      !window.confirm(
        `¿Cancelar la ejecución activa de “${task.name}”?${recurringNote}`
      )
    ) return;
    setScheduleBusyId(run.id);
    setScheduleError(null);
    setScheduleNotice(null);
    try {
      await platform.cancelScheduledRun(run.id);
      setScheduledTasks({
        state: "ready",
        value: await platform.listScheduledTasks()
      });
      setScheduleNotice(
        `Se ha cancelado la ejecución activa de “${task.name}”.` + recurringNote
      );
      setScheduledHistoryRefreshVersion((current) => current + 1);
    } catch (error) {
      setScheduleError(describeError(error));
      try {
        setScheduledTasks({
          state: "ready",
          value: await platform.listScheduledTasks()
        });
      } catch {
        // Se mantiene el error de cancelación si también falla la actualización visual.
      }
    } finally {
      setScheduleBusyId(null);
    }
  };

  const exportScheduledHistory = async () => {
    setScheduleBusyId("export");
    setScheduleError(null);
    setScheduleNotice(null);
    try {
      const selection = await platform.pickScheduledHistoryExportPath();
      if (!selection) return;
      const report = await platform.exportScheduledHistory(
        selection.path,
        scheduledHistoryStatus,
        scheduledHistoryPeriod,
        selection.existed
      );
      setScheduleNotice(
        `Historial exportado: ${report.runCount} ejecución(es) en ${report.destinationPath}`
      );
    } catch (error) {
      setScheduleError(describeError(error));
    } finally {
      setScheduleBusyId(null);
    }
  };

  const exportScheduledCalendar = async () => {
    if (schedulerCalendarItems.length === 0) {
      setSchedulerCalendarExportMessage({
        kind: "error",
        text: "No hay fechas visibles para exportar en este periodo."
      });
      return;
    }
    setScheduleBusyId("calendar-export");
    setSchedulerCalendarExportMessage(null);
    try {
      const selection = await platform.pickScheduledCalendarExportPath();
      if (!selection) return;
      const report = await platform.exportScheduledCalendar(
        selection.path,
        schedulerCalendarItems.map((item) => ({
          occurrenceId: item.id,
          taskName: item.taskName,
          conversationTitle: item.conversationTitle,
          startsAt: item.startsAt,
          projected: item.projected,
          overdue: item.overdue
        })),
        schedulerCalendarRange,
        selection.existed
      );
      setSchedulerCalendarExportMessage({
        kind: "success",
        text: `Calendario exportado: ${report.eventCount} evento(s) en ${report.destinationPath}`
      });
    } catch (error) {
      setSchedulerCalendarExportMessage({ kind: "error", text: describeError(error) });
    } finally {
      setScheduleBusyId(null);
    }
  };

  const toggleWindowsStartup = async () => {
    if (windowsStartup.state !== "ready") return;
    const enabled = !windowsStartup.value.enabled;
    if (
      enabled &&
      !window.confirm(
        "¿Activar el inicio automático con Windows? El token actual del Broker se guardará cifrado para esta cuenta de Windows y ChatyGPT esperará a que el Broker esté disponible antes de abrirse."
      )
    ) return;
    setScheduleBusyId("windows-startup");
    try {
      const value = await platform.setWindowsStartupEnabled(enabled);
      setWindowsStartup({ state: "ready", value });
    } catch (error) {
      setWindowsStartup({ state: "error", message: describeError(error) });
    } finally {
      setScheduleBusyId(null);
    }
  };

  const reloadWindowsStartupStatus = async () => {
    setWindowsStartup({ state: "loading" });
    try {
      setWindowsStartup({
        state: "ready",
        value: await platform.getWindowsStartupStatus()
      });
    } catch (error) {
      setWindowsStartup({ state: "error", message: describeError(error) });
    }
  };

  const toggleSchedule = async (task: ScheduledTaskView) => {
    // Rust solo exige confirmación al reactivar, y con razón: pausar no ejecuta
    // nada, mientras que reactivar devuelve a la tarea la capacidad de lanzar
    // trabajos contra el Broker sin que nadie esté delante. Se pregunta en el
    // mismo caso, para que la comprobación del backend responda a una decisión.
    if (
      !task.enabled &&
      !window.confirm(
        `¿Reactivar «${task.name}»? Volverá a ejecutarse sola en la fecha prevista.`
      )
    ) {
      return;
    }
    setScheduleBusyId(task.id);
    setScheduleError(null);
    try {
      await platform.setScheduledTaskEnabled(task.id, !task.enabled);
      setScheduledTasks({
        state: "ready",
        value: await platform.listScheduledTasks()
      });
    } catch (error) {
      setScheduleError(describeError(error));
    } finally {
      setScheduleBusyId(null);
    }
  };

  const removeSchedule = async (task: ScheduledTaskView) => {
    if (!window.confirm(`¿Eliminar la programación “${task.name}”?`)) return;
    setScheduleBusyId(task.id);
    setScheduleError(null);
    try {
      await platform.deleteScheduledTask(task.id);
      setScheduledTasks({
        state: "ready",
        value: await platform.listScheduledTasks()
      });
    } catch (error) {
      setScheduleError(describeError(error));
    } finally {
      setScheduleBusyId(null);
    }
  };

  const visibleScheduledTasks = scheduledTasks.state === "ready"
    ? filterScheduledTasks(scheduledTasks.value, scheduleSearchQuery)
    : [];

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
          <div><strong>ChatyGPT</strong><small>Espacio personal</small></div>
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
                      GPT · {customGpt.name} · v{customGpt.versionNo}
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
            <span className="version">v0.1.0</span>
          )}
        </header>

        <main
          className={`content ${conversation?.state === "ready" ? "conversation-content" : "home-content"}`}
          id="main-content"
          tabIndex={-1}
        >
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
                {conversation.value.messages.map((message) => (
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
                            <small>{attachmentStatusLabel(file.ingestionStatus)}</small>
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
                        Documentos largos: {broker.value.longContextMapReduce
                          ? "map-reduce disponible"
                          : "sin map-reduce"}
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
                    ChatyGPT solo escribe en carpetas que hayas elegido en un selector de
                    Windows. Al revocar una, la siguiente exportación volverá a pedirte el
                    destino. Los archivos ya guardados no se tocan.
                  </p>
                </div>
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

              <section className="scheduler-card">
                <div className="panel-heading">
                  <div>
                    <span className="kicker">Fase 4 · Automatización local</span>
                    <h3>Tareas programadas</h3>
                  </div>
                  <div className="scheduler-heading-actions">
                    {scheduledTasks.state === "ready" && (
                      <span className="badge">
                        {scheduledTasks.value.filter((task) => task.enabled).length} activa(s)
                      </span>
                    )}
                    <button
                      className={schedulerCalendarOpen ? "primary" : "secondary"}
                      onClick={() => setSchedulerCalendarOpen((current) => !current)}
                      aria-expanded={schedulerCalendarOpen}
                    >
                      Calendario
                    </button>
                    <button
                      className={schedulerCenterOpen ? "primary" : "secondary"}
                      onClick={() => setSchedulerCenterOpen((current) => !current)}
                      aria-expanded={schedulerCenterOpen}
                    >
                      Avisos{schedulerUnreadCount > 0 ? ` (${schedulerUnreadCount})` : ""}
                    </button>
                    <button
                      className={schedulerNotifications === "granted" ? "secondary" : "primary"}
                      onClick={() => void enableSchedulerNotifications()}
                      disabled={
                        schedulerNotifications === "granted" ||
                        schedulerNotifications === "unsupported"
                      }
                    >
                      {schedulerNotifications === "granted"
                        ? "Avisos de Windows activos"
                        : schedulerNotifications === "unsupported"
                          ? "Avisos no disponibles"
                          : "Activar avisos de Windows"}
                    </button>
                  </div>
                </div>
                <p className="muted">
                  Programa una instrucción para una conversación existente. La hora se guarda
                  con tu zona horaria y cada ejecución queda registrada. Si ChatyGPT está
                  cerrado a esa hora, la tarea se iniciará al volver a abrirlo.
                </p>
                <div className="scheduler-safety">
                  La programación no modifica archivos ni concede herramientas. Crear, editar,
                  reactivar o reintentar requiere tu confirmación.
                </div>
                <div className="scheduler-startup-panel">
                  <div>
                    <strong>Inicio con Windows</strong>
                    {windowsStartup.state === "loading" && (
                      <span>Comprobando la configuración…</span>
                    )}
                    {windowsStartup.state === "ready" && (
                      <>
                        <span>{windowsStartup.value.message}</span>
                        <small>
                          {windowsStartup.value.enabled
                            ? "La credencial está protegida con DPAPI para tu cuenta de Windows. Si cambia el token del Broker, abre una vez ChatyGPT con el BAT para actualizarla."
                            : "No instala servicios ni requiere permisos de administrador."}
                        </small>
                      </>
                    )}
                    {windowsStartup.state === "error" && (
                      <span className="error" role="alert">{windowsStartup.message}</span>
                    )}
                  </div>
                  <div>
                    {windowsStartup.state === "ready" && windowsStartup.value.supported && (
                      <button
                        className={windowsStartup.value.enabled ? "secondary" : "primary"}
                        onClick={() => void toggleWindowsStartup()}
                        disabled={scheduleBusyId !== null}
                      >
                        {scheduleBusyId === "windows-startup"
                          ? "Aplicando…"
                          : windowsStartup.value.enabled
                            ? "Desactivar"
                            : "Activar"}
                      </button>
                    )}
                    {windowsStartup.state === "error" && (
                      <button
                        className="secondary"
                        onClick={() => void reloadWindowsStartupStatus()}
                      >
                        Volver a comprobar
                      </button>
                    )}
                  </div>
                </div>
                {schedulerCenterOpen && (
                  <div className="scheduler-notification-center">
                    <div className="scheduler-notification-heading">
                      <div>
                        <strong>Centro de avisos</strong>
                        <span>
                          Finalizaciones recientes de las tareas programadas.
                        </span>
                      </div>
                      {schedulerUnreadCount > 0 && (
                        <button
                          className="secondary"
                          onClick={markAllSchedulerNotificationsRead}
                        >
                          Marcar todo como leído
                        </button>
                      )}
                    </div>
                    {schedulerCenterItems.length === 0 ? (
                      <p className="activity-empty">
                        Los avisos aparecerán cuando finalice una tarea.
                      </p>
                    ) : (
                      <div className="scheduler-notification-list">
                        {schedulerCenterItems.map((item) => {
                          const unread = !schedulerReadIds.has(item.id);
                          return (
                            <article
                              key={item.id}
                              className={`scheduler-notification ${unread ? "unread" : ""}`}
                            >
                              <div>
                                <strong>{item.taskName}</strong>
                                <span>
                                  {scheduledRunLabel(item.status)}
                                  {item.attempt > 1 ? ` · intento ${item.attempt}` : ""}
                                  {" · "}
                                  {new Date(item.updatedAt).toLocaleString("es-ES", {
                                    dateStyle: "short",
                                    timeStyle: "short"
                                  })}
                                </span>
                                <small>{item.conversationTitle}</small>
                              </div>
                              <button
                                className="secondary"
                                onClick={() => {
                                  markSchedulerNotificationRead(item);
                                  void openConversation(item.conversationId);
                                }}
                              >
                                Abrir
                              </button>
                            </article>
                          );
                        })}
                      </div>
                    )}
                  </div>
                )}
                {schedulerCalendarOpen && (
                  <div className="scheduler-calendar-panel">
                    <div className="scheduler-calendar-heading">
                      <div>
                        <strong>Próximas automatizaciones</strong>
                        <span>
                          La primera fecha es la guardada. Las repeticiones posteriores son
                          una proyección informativa y no ejecutan ni modifican tareas.
                        </span>
                      </div>
                      <div className="scheduler-calendar-actions">
                        <label>
                          <span>Periodo</span>
                          <select
                            value={schedulerCalendarRange}
                            onChange={(event) => {
                              setSchedulerCalendarRange(
                                Number(event.target.value) as 7 | 14 | 30
                              );
                              setSchedulerCalendarExportMessage(null);
                            }}
                          >
                            <option value={7}>7 días</option>
                            <option value={14}>14 días</option>
                            <option value={30}>30 días</option>
                          </select>
                        </label>
                        <button
                          className="secondary"
                          onClick={() => void exportScheduledCalendar()}
                          disabled={
                            scheduleBusyId !== null || schedulerCalendarItems.length === 0
                          }
                        >
                          {scheduleBusyId === "calendar-export"
                            ? "Exportando…"
                            : "Exportar .ics"}
                        </button>
                      </div>
                    </div>
                    {schedulerCalendarExportMessage && (
                      <div
                        className={`scheduler-calendar-export-message ${
                          schedulerCalendarExportMessage.kind
                        }`}
                        role={
                          schedulerCalendarExportMessage.kind === "error" ? "alert" : "status"
                        }
                      >
                        {schedulerCalendarExportMessage.text}
                      </div>
                    )}
                    {schedulerCalendarConflicts > 0 && (
                      <div className="scheduler-calendar-warning" role="status">
                        {schedulerCalendarConflicts} coincidencia(s): hay tareas distintas
                        separadas por 15 minutos o menos. Revisa si quieres evitar que compitan
                        por los mismos recursos.
                      </div>
                    )}
                    {schedulerCalendarGroupedDays.length === 0 ? (
                      <p className="activity-empty">
                        No hay tareas activas dentro de este periodo.
                      </p>
                    ) : (
                      <div className="scheduler-calendar-days">
                        {schedulerCalendarGroupedDays.map((day) => (
                          <section key={day.key} className="scheduler-calendar-day">
                            <h4>{day.label}</h4>
                            <div>
                              {day.items.map((item) => (
                                <article
                                  key={item.id}
                                  className={`scheduler-calendar-occurrence ${
                                    item.conflictingTaskIds.length > 0 ? "conflict" : ""
                                  } ${item.overdue ? "overdue" : ""}`}
                                >
                                  <time>
                                    {new Date(item.startsAt).toLocaleTimeString("es-ES", {
                                      hour: "2-digit",
                                      minute: "2-digit"
                                    })}
                                  </time>
                                  <div>
                                    <strong>{item.taskName}</strong>
                                    <span>{item.conversationTitle}</span>
                                    <small>
                                      {item.overdue
                                        ? "Atrasada"
                                        : item.projected
                                          ? "Proyección"
                                          : "Próxima guardada"}
                                      {item.conflictingTaskIds.length > 0
                                        ? ` · Coincide con ${item.conflictingTaskIds.length}`
                                        : ""}
                                      {` · ${item.timezone}`}
                                    </small>
                                  </div>
                                  <button
                                    className="secondary compact"
                                    onClick={() => void openConversation(item.conversationId)}
                                  >
                                    Abrir chat
                                  </button>
                                </article>
                              ))}
                            </div>
                          </section>
                        ))}
                      </div>
                    )}
                  </div>
                )}
                <div className="scheduler-template-panel">
                  <div className="scheduler-template-heading">
                    <div>
                      <strong>Plantillas reutilizables</strong>
                      <span>
                        Guardan nombre, instrucción y repetición; nunca la conversación,
                        la fecha ni la autorización.
                      </span>
                    </div>
                    {scheduledTaskTemplates.state === "ready" && (
                      <span className="badge">
                        {scheduledTaskTemplates.value.length} guardada(s)
                      </span>
                    )}
                  </div>
                  {scheduledTaskTemplates.state === "loading" && (
                    <p className="muted">Cargando plantillas…</p>
                  )}
                  {scheduledTaskTemplates.state === "error" && (
                    <p className="error">{scheduledTaskTemplates.message}</p>
                  )}
                  {scheduledTaskTemplates.state === "ready" &&
                    scheduledTaskTemplates.value.length === 0 && (
                      <p className="activity-empty">
                        Completa nombre e instrucción para guardar tu primera plantilla.
                      </p>
                    )}
                  {scheduledTaskTemplates.state === "ready" &&
                    scheduledTaskTemplates.value.length > 0 && (
                      <div className="scheduler-template-list">
                        {scheduledTaskTemplates.value.map((template) => (
                          <article key={template.id}>
                            <div>
                              <strong>{template.name}</strong>
                              <span>
                                {template.scheduleExpression === "daily"
                                  ? "Cada día"
                                  : template.scheduleExpression === "weekly"
                                    ? "Cada semana"
                                    : "Una vez"}
                              </span>
                              <p>{template.prompt}</p>
                            </div>
                            <div>
                              <button
                                className="secondary"
                                onClick={() => applyScheduledTaskTemplate(template)}
                                disabled={scheduleBusyId !== null}
                              >
                                Usar
                              </button>
                              <button
                                className="danger-link"
                                onClick={() => void removeScheduledTaskTemplate(template)}
                                disabled={scheduleBusyId !== null}
                              >
                                {scheduleBusyId === template.id ? "Eliminando…" : "Eliminar"}
                              </button>
                            </div>
                          </article>
                        ))}
                      </div>
                    )}
                </div>
                <div className="scheduler-form">
                  <div className="scheduler-form-heading">
                    <strong>
                      {scheduleEditingId ? "Editar programación" : "Nueva programación"}
                    </strong>
                    {scheduleEditingId && (
                      <button
                        className="secondary"
                        onClick={cancelScheduleEdit}
                        disabled={scheduleBusyId !== null}
                      >
                        Cancelar edición
                      </button>
                    )}
                  </div>
                  <label>
                    <span>Nombre de la tarea</span>
                    <input
                      value={scheduleName}
                      onChange={(event) => setScheduleName(event.target.value)}
                      placeholder="Ejemplo: Resumen del viernes"
                      maxLength={120}
                      disabled={scheduleBusyId !== null}
                    />
                  </label>
                  <label>
                    <span>Conversación donde aparecerá la respuesta</span>
                    <select
                      value={scheduleConversationId}
                      onChange={(event) => setScheduleConversationId(event.target.value)}
                      disabled={scheduleBusyId !== null}
                    >
                      <option value="">Elige una conversación</option>
                      {conversations.map((item) => (
                        <option key={item.id} value={item.id}>{item.title}</option>
                      ))}
                    </select>
                  </label>
                  <label>
                    <span>Fecha y hora</span>
                    <input
                      type="datetime-local"
                      value={scheduleAt}
                      onChange={(event) => setScheduleAt(event.target.value)}
                      min={defaultScheduledLocalTime(new Date(Date.now() - 55 * 60 * 1000))}
                      disabled={scheduleBusyId !== null}
                    />
                    <small>
                      Zona horaria: {Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC"}
                    </small>
                  </label>
                  <label>
                    <span>Repetición</span>
                    <select
                      value={scheduleExpression}
                      onChange={(event) =>
                        setScheduleExpression(
                          event.target.value as ScheduledTaskView["scheduleExpression"]
                        )}
                      disabled={scheduleBusyId !== null}
                    >
                      <option value="once">Una sola vez</option>
                      <option value="daily">Cada día</option>
                      <option value="weekly">Cada semana</option>
                    </select>
                    <small>
                      {scheduleExpression === "once"
                        ? "No se repetirá."
                        : "Mantendrá esta hora según la zona local de Windows."}
                    </small>
                  </label>
                  <label className="scheduler-prompt">
                    <span>Instrucción</span>
                    <textarea
                      value={schedulePrompt}
                      onChange={(event) => setSchedulePrompt(event.target.value)}
                      placeholder="Escribe exactamente lo que ChatyGPT deberá pedir al Broker."
                      rows={4}
                      maxLength={20_000}
                      disabled={scheduleBusyId !== null}
                    />
                  </label>
                  <label className="scheduler-confirmation">
                    <input
                      type="checkbox"
                      checked={scheduleConfirmed}
                      onChange={(event) => setScheduleConfirmed(event.target.checked)}
                      disabled={scheduleBusyId !== null}
                    />
                    <span>
                      Confirmo que quiero activar esta ejecución automática en la conversación
                      seleccionada.
                    </span>
                  </label>
                  <div className="scheduler-form-actions">
                    <button
                      className="secondary"
                      onClick={() => void saveScheduledTaskTemplate()}
                      disabled={
                        scheduleBusyId !== null ||
                        !scheduleName.trim() ||
                        !schedulePrompt.trim()
                      }
                    >
                      {scheduleBusyId === "template-create"
                        ? "Guardando plantilla…"
                        : "Guardar como plantilla"}
                    </button>
                    <button
                      className="primary"
                      onClick={() => void createSchedule()}
                      disabled={
                        scheduleBusyId !== null ||
                        !scheduleName.trim() ||
                        !scheduleConversationId ||
                        !schedulePrompt.trim() ||
                        !scheduleAt ||
                        !scheduleConfirmed
                      }
                    >
                      {scheduleBusyId === "create"
                        ? "Guardando…"
                        : scheduleEditingId
                          ? "Guardar cambios y activar"
                          : "Guardar y activar"}
                    </button>
                  </div>
                </div>
                {scheduleNotice && (
                  <p className="scheduler-notice" role="status">{scheduleNotice}</p>
                )}
                {scheduleError && (
                  <p className="error" role="alert">{scheduleError}</p>
                )}
                {scheduledTasks.state === "ready" &&
                  scheduledTasks.value.some((task) => task.runs.length > 0) && (
                    <div className="scheduler-history-filters">
                      <strong>Filtrar historiales</strong>
                      <label>
                        <span>Estado</span>
                        <select
                          value={scheduledHistoryStatus}
                          onChange={(event) => {
                            setScheduledHistoryStatus(
                              event.target.value as ScheduledHistoryStatusFilter
                            );
                            setScheduledHistoryPageNumber(1);
                          }}
                        >
                          <option value="all">Todos</option>
                          <option value="active">En curso</option>
                          <option value="completed">Completadas</option>
                          <option value="failed">Fallidas</option>
                          <option value="cancelled">Canceladas</option>
                        </select>
                      </label>
                      <label>
                        <span>Fecha</span>
                        <select
                          value={scheduledHistoryPeriod}
                          onChange={(event) => {
                            setScheduledHistoryPeriod(
                              event.target.value as ScheduledHistoryPeriodFilter
                            );
                            setScheduledHistoryPageNumber(1);
                          }}
                        >
                          <option value="all">Cualquier fecha</option>
                          <option value="today">Hoy</option>
                          <option value="7d">Últimos 7 días</option>
                          <option value="30d">Últimos 30 días</option>
                        </select>
                      </label>
                      <button
                        className="secondary"
                        onClick={() => void exportScheduledHistory()}
                        disabled={scheduleBusyId !== null}
                      >
                        {scheduleBusyId === "export"
                          ? "Exportando…"
                          : "Exportar historial visible"}
                      </button>
                    </div>
                  )}
                {scheduledTasks.state === "loading" && (
                  <p className="muted">Cargando programaciones…</p>
                )}
                {scheduledTasks.state === "error" && (
                  <p className="error">{scheduledTasks.message}</p>
                )}
                {scheduledTasks.state === "ready" &&
                  scheduledTasks.value.length > 0 && (
                    <label className="scheduler-search">
                      <span>Buscar tareas</span>
                      <input
                        type="search"
                        value={scheduleSearchQuery}
                        onChange={(event) => setScheduleSearchQuery(event.target.value)}
                        placeholder="Nombre, conversación o texto de la instrucción"
                      />
                      <small>
                        {visibleScheduledTasks.length} de {scheduledTasks.value.length} visible(s)
                      </small>
                    </label>
                  )}
                {scheduledTasks.state === "ready" && (
                  scheduledTasks.value.length === 0 ? (
                    <p className="activity-empty">
                      Todavía no has creado ninguna tarea programada.
                    </p>
                  ) : visibleScheduledTasks.length === 0 ? (
                    <p className="activity-empty">
                      No hay tareas que coincidan con la búsqueda.
                    </p>
                  ) : (
                    <div className="scheduler-list">
                      {visibleScheduledTasks.map((task) => (
                        <article
                          key={task.id}
                          className={`scheduler-item ${task.enabled ? "enabled" : ""}`}
                        >
                          <div className="scheduler-item-heading">
                            <div>
                              <strong>{task.name}</strong>
                              <span>{task.conversationTitle}</span>
                            </div>
                            <span className={`badge ${task.enabled ? "success" : ""}`}>
                              {task.enabled
                                ? "Activa"
                                : task.runs.length > 0
                                ? scheduledRunLabel(task.runs[0].status)
                                : "Pausada"}
                            </span>
                          </div>
                          <p>{task.prompt}</p>
                          <small>
                            {task.nextRunAt
                              ? new Date(task.nextRunAt).toLocaleString("es-ES", {
                                  dateStyle: "medium",
                                  timeStyle: "short"
                                })
                              : "Sin próxima ejecución"}{" "}
                            · {task.timezone} ·{" "}
                            {task.scheduleExpression === "daily"
                              ? "Diaria"
                              : task.scheduleExpression === "weekly"
                                ? "Semanal"
                                : "Una vez"}
                          </small>
                          {task.runs.length > 0 && (
                            <div className="scheduler-history">
                              <div className="scheduler-history-heading">
                                <strong>Actividad reciente</strong>
                                <button
                                  className="secondary compact"
                                  onClick={() => toggleScheduledHistory(task)}
                                  aria-expanded={scheduledHistoryTaskId === task.id}
                                >
                                  {scheduledHistoryTaskId === task.id
                                    ? "Ocultar historial completo"
                                    : "Ver historial completo"}
                                </button>
                              </div>
                              {filterScheduledRuns(
                                task.runs,
                                scheduledHistoryStatus,
                                scheduledHistoryPeriod
                              ).length === 0 && (
                                <span className="scheduler-history-empty">
                                  No hay ejecuciones que coincidan con los filtros.
                                </span>
                              )}
                              {filterScheduledRuns(
                                task.runs,
                                scheduledHistoryStatus,
                                scheduledHistoryPeriod
                              ).map((run) => {
                                const detail = scheduledRunDetail(run);
                                return (
                                  <div key={run.id} className="scheduler-history-entry">
                                    <div className="scheduler-history-row">
                                      <span>
                                        {scheduledRunLabel(run.status)}
                                        {run.attempt > 1 ? ` · intento ${run.attempt}` : ""}
                                      </span>
                                      <div>
                                        <time>
                                          {new Date(run.updatedAt).toLocaleString("es-ES", {
                                            dateStyle: "short",
                                            timeStyle: "medium"
                                          })}
                                        </time>
                                        {run.status === "failed" && (
                                          <button
                                            className="secondary compact"
                                            onClick={() => void retryScheduledRun(task, run)}
                                            disabled={
                                              scheduleBusyId !== null ||
                                              task.runs.some((item) =>
                                                ["claimed", "running"].includes(item.status)
                                              )
                                            }
                                          >
                                            {scheduleBusyId === run.id
                                              ? "Reintentando…"
                                              : "Reintentar"}
                                          </button>
                                        )}
                                        {run.status === "running" && run.brokerTaskId && (
                                          <button
                                            className="danger-link compact"
                                            onClick={() => void cancelScheduledRun(task, run)}
                                            disabled={scheduleBusyId !== null}
                                          >
                                            {scheduleBusyId === run.id
                                              ? "Cancelando…"
                                              : "Cancelar ejecución"}
                                          </button>
                                        )}
                                      </div>
                                    </div>
                                    {detail && (
                                      <details className="scheduler-run-detail">
                                        <summary>Ver detalle</summary>
                                        <strong>{detail.label}</strong>
                                        <p>{detail.text}</p>
                                      </details>
                                    )}
                                  </div>
                                );
                              })}
                            </div>
                          )}
                          {scheduledHistoryTaskId === task.id && (
                            <div className="scheduler-full-history">
                              <div className="scheduler-full-history-heading">
                                <div>
                                  <strong>Historial completo</strong>
                                  <span>Los filtros superiores también se aplican aquí.</span>
                                </div>
                                <div>
                                  <label>
                                    <span>Orden</span>
                                    <select
                                      value={scheduledHistorySort}
                                      onChange={(event) => {
                                        setScheduledHistorySort(
                                          event.target.value as ScheduledHistorySort
                                        );
                                        setScheduledHistoryPageNumber(1);
                                      }}
                                    >
                                      <option value="newest">Más recientes primero</option>
                                      <option value="oldest">Más antiguas primero</option>
                                    </select>
                                  </label>
                                  <label>
                                    <span>Por página</span>
                                    <select
                                      value={scheduledHistoryPageSize}
                                      onChange={(event) => {
                                        setScheduledHistoryPageSize(
                                          Number(event.target.value) as ScheduledRunPageView["pageSize"]
                                        );
                                        setScheduledHistoryPageNumber(1);
                                      }}
                                    >
                                      <option value={10}>10</option>
                                      <option value={25}>25</option>
                                      <option value={50}>50</option>
                                    </select>
                                  </label>
                                </div>
                              </div>
                              {scheduledHistoryPage?.state === "loading" && (
                                <p className="muted">Cargando historial completo…</p>
                              )}
                              {scheduledHistoryPage?.state === "error" && (
                                <p className="error">{scheduledHistoryPage.message}</p>
                              )}
                              {scheduledHistoryPage?.state === "ready" && (
                                <>
                                  <div className="scheduler-full-history-summary">
                                    {scheduledHistoryPage.value.total === 0
                                      ? "No hay ejecuciones que coincidan con los filtros."
                                      : `${scheduledHistoryPage.value.total} ejecución(es) · página ${scheduledHistoryPage.value.page} de ${Math.max(
                                          1,
                                          Math.ceil(
                                            scheduledHistoryPage.value.total /
                                              scheduledHistoryPage.value.pageSize
                                          )
                                        )}`}
                                  </div>
                                  <div className="scheduler-full-history-list">
                                    {scheduledHistoryPage.value.items.map((run) => {
                                      const detail = scheduledRunDetail(run);
                                      return (
                                        <div key={run.id} className="scheduler-history-entry">
                                          <div className="scheduler-history-row">
                                            <span>
                                              {scheduledRunLabel(run.status)}
                                              {run.attempt > 1
                                                ? ` · intento ${run.attempt}`
                                                : ""}
                                            </span>
                                            <div>
                                              <time>
                                                {new Date(run.updatedAt).toLocaleString("es-ES", {
                                                  dateStyle: "short",
                                                  timeStyle: "medium"
                                                })}
                                              </time>
                                              {run.status === "failed" && (
                                                <button
                                                  className="secondary compact"
                                                  onClick={() => void retryScheduledRun(task, run)}
                                                  disabled={scheduleBusyId !== null}
                                                >
                                                  {scheduleBusyId === run.id
                                                    ? "Reintentando…"
                                                    : "Reintentar"}
                                                </button>
                                              )}
                                              {run.status === "running" && run.brokerTaskId && (
                                                <button
                                                  className="danger-link compact"
                                                  onClick={() => void cancelScheduledRun(task, run)}
                                                  disabled={scheduleBusyId !== null}
                                                >
                                                  {scheduleBusyId === run.id
                                                    ? "Cancelando…"
                                                    : "Cancelar ejecución"}
                                                </button>
                                              )}
                                            </div>
                                          </div>
                                          {detail && (
                                            <details className="scheduler-run-detail">
                                              <summary>Ver detalle</summary>
                                              <strong>{detail.label}</strong>
                                              <p>{detail.text}</p>
                                            </details>
                                          )}
                                        </div>
                                      );
                                    })}
                                  </div>
                                  {scheduledHistoryPage.value.total >
                                    scheduledHistoryPage.value.pageSize && (
                                      <div className="scheduler-pagination">
                                        <button
                                          className="secondary"
                                          onClick={() =>
                                            setScheduledHistoryPageNumber((current) =>
                                              Math.max(1, current - 1)
                                            )
                                          }
                                          disabled={scheduledHistoryPage.value.page <= 1}
                                        >
                                          Anterior
                                        </button>
                                        <span>Página {scheduledHistoryPage.value.page}</span>
                                        <button
                                          className="secondary"
                                          onClick={() =>
                                            setScheduledHistoryPageNumber((current) => current + 1)
                                          }
                                          disabled={
                                            scheduledHistoryPage.value.page >=
                                            Math.ceil(
                                              scheduledHistoryPage.value.total /
                                                scheduledHistoryPage.value.pageSize
                                            )
                                          }
                                        >
                                          Siguiente
                                        </button>
                                      </div>
                                    )}
                                </>
                              )}
                            </div>
                          )}
                          <div className="scheduler-item-actions">
                            <button
                              className="secondary"
                              onClick={() => openConversation(task.conversationId)}
                            >
                              Abrir conversación
                            </button>
                            {(task.runs.length === 0 ||
                              task.scheduleExpression !== "once") && (
                              <button
                                className="secondary"
                                onClick={() => void toggleSchedule(task)}
                                disabled={scheduleBusyId !== null}
                              >
                                {task.enabled ? "Pausar" : "Reactivar"}
                              </button>
                            )}
                            <button
                              className="primary"
                              onClick={() => void runScheduledTaskNow(task)}
                              disabled={
                                scheduleBusyId !== null ||
                                task.runs.some((run) =>
                                  ["claimed", "running"].includes(run.status)
                                )
                              }
                            >
                              {scheduleBusyId === `run-now:${task.id}`
                                ? "Iniciando…"
                                : "Ejecutar ahora"}
                            </button>
                            <button
                              className="secondary"
                              onClick={() => duplicateSchedule(task)}
                              disabled={scheduleBusyId !== null}
                            >
                              Duplicar
                            </button>
                            <button
                              className="secondary"
                              onClick={() => beginScheduleEdit(task)}
                              disabled={
                                scheduleBusyId !== null ||
                                task.runs.some((run) =>
                                  ["claimed", "running"].includes(run.status)
                                )
                              }
                            >
                              Editar
                            </button>
                            <button
                              className="danger-link"
                              onClick={() => void removeSchedule(task)}
                              disabled={
                                scheduleBusyId !== null ||
                                task.runs.some((run) =>
                                  ["claimed", "running"].includes(run.status)
                                )
                              }
                            >
                              Eliminar
                            </button>
                          </div>
                        </article>
                      ))}
                    </div>
                  )
                )}
              </section>

              <section className="custom-gpt-card">
                <div className="panel-heading">
                  <div>
                    <span className="kicker">Fase 3 · Asistentes personales</span>
                    <h3>Mis GPTs</h3>
                  </div>
                  <div className="custom-gpt-panel-actions">
                    {customGpts.state === "ready" && (
                      <span className="badge">
                        {customGpts.value.length} guardado(s)
                      </span>
                    )}
                    <button
                      className="secondary"
                      onClick={() => void importCustomGpt()}
                      disabled={customGptBusy}
                    >
                      Importar GPT
                    </button>
                  </div>
                </div>
                <p className="muted">
                  Define asistentes reutilizables con propuestas para empezar. Cada cambio crea
                  una versión nueva y conserva localmente las anteriores.
                </p>
                <div className="custom-gpt-safety">
                  Elegir un GPT aplica sus instrucciones a los mensajes de ese chat. No concede
                  herramientas ni ejecuta acciones por sí mismo.
                </div>
                <div className="custom-gpt-form">
                  <div className="custom-gpt-form-heading">
                    <strong>
                      {customGptEditingId ? "Editar GPT personal" : "Crear GPT personal"}
                    </strong>
                    {customGptEditingId && <span>Se guardará como una versión nueva</span>}
                  </div>
                  <label>
                    <span>Nombre</span>
                    <input
                      value={customGptName}
                      onChange={(event) => setCustomGptName(event.target.value)}
                      placeholder="Ejemplo: Tutor de arquitectura"
                      maxLength={80}
                      disabled={customGptBusy}
                    />
                  </label>
                  <label>
                    <span>Descripción breve (opcional)</span>
                    <textarea
                      value={customGptDescription}
                      onChange={(event) => setCustomGptDescription(event.target.value)}
                      placeholder="Explica para qué sirve este GPT."
                      rows={2}
                      maxLength={500}
                      disabled={customGptBusy}
                    />
                  </label>
                  <label>
                    <span>Instrucciones</span>
                    <textarea
                      value={customGptInstructions}
                      onChange={(event) => setCustomGptInstructions(event.target.value)}
                      placeholder="Describe cómo debe responder, qué debe priorizar y qué límites debe respetar."
                      rows={5}
                      maxLength={12000}
                      disabled={customGptBusy}
                    />
                  </label>
                  <label>
                    <span>Iniciadores de conversación (opcional)</span>
                    <textarea
                      value={customGptStartersText}
                      onChange={(event) => setCustomGptStartersText(event.target.value)}
                      placeholder={"Una propuesta por línea, hasta 6.\nEjemplo: Explícame este tema paso a paso"}
                      rows={4}
                      maxLength={1805}
                      disabled={customGptBusy}
                    />
                    <small>
                      Aparecerán como botones al abrir un chat vacío que use este GPT.
                    </small>
                  </label>
                  <fieldset className="custom-gpt-permissions">
                    <legend>Permisos de herramientas</legend>
                    <p>
                      Todo está denegado por defecto. Activar una capacidad solo permite
                      solicitarla: ChatyGPT seguirá pidiendo tu confirmación.
                    </p>
                    <label>
                      <input
                        type="checkbox"
                        checked={customGptRunCodePermission}
                        onChange={(event) =>
                          setCustomGptRunCodePermission(event.target.checked)}
                        disabled={customGptBusy}
                      />
                      <span>
                        Código aislado
                        <small>Puede solicitar Python para un turno; nunca se activa solo.</small>
                      </span>
                    </label>
                    <label>
                      <input
                        type="checkbox"
                        checked={customGptRenamePermission}
                        onChange={(event) =>
                          setCustomGptRenamePermission(event.target.checked)}
                        disabled={customGptBusy}
                      />
                      <span>
                        Renombrar conversación
                        <small>Puede proponer un título; tendrás que aprobarlo.</small>
                      </span>
                    </label>
                  </fieldset>
                  <fieldset className="custom-gpt-preferences">
                    <legend>Preferencias de ejecución</legend>
                    <p>
                      Son preferencias, no imposiciones: si el modelo no está
                      disponible, el Broker elegirá otro y la respuesta seguirá llegando.
                    </p>
                    <label htmlFor="custom-gpt-model">Modelo preferido</label>
                    <input
                      id="custom-gpt-model"
                      value={customGptPreferredModel}
                      onChange={(event) => setCustomGptPreferredModel(event.target.value)}
                      placeholder="Por ejemplo, qwen2.5:14b (vacío = decide el Broker)"
                      spellCheck={false}
                      disabled={customGptBusy}
                    />
                    <label htmlFor="custom-gpt-project">Proyecto predeterminado</label>
                    <select
                      id="custom-gpt-project"
                      value={customGptDefaultProject}
                      onChange={(event) => setCustomGptDefaultProject(event.target.value)}
                      disabled={customGptBusy}
                    >
                      <option value="">Sin proyecto predeterminado</option>
                      {projects.map((project) => (
                        <option key={project.id} value={project.id}>
                          {project.name}
                        </option>
                      ))}
                    </select>
                    <small>
                      Solo se aplica a los chats que aún no pertenecen a ningún
                      proyecto; nunca mueve una conversación ya clasificada.
                    </small>
                  </fieldset>
                  {customGptError && (
                    <p className="error" role="alert">{customGptError}</p>
                  )}
                  <div className="custom-gpt-form-actions">
                    {customGptEditingId && (
                      <button
                        className="secondary"
                        onClick={resetCustomGptForm}
                        disabled={customGptBusy}
                      >
                        Cancelar edición
                      </button>
                    )}
                    <button
                      className="primary"
                      onClick={() => void saveCustomGpt()}
                      disabled={
                        customGptBusy ||
                        !customGptName.trim() ||
                        !customGptInstructions.trim()
                      }
                    >
                      {customGptBusy
                        ? "Guardando…"
                        : customGptEditingId
                          ? "Guardar versión nueva"
                          : "Crear GPT"}
                    </button>
                  </div>
                </div>
                {customGptNotice && (
                  <p className="custom-gpt-notice" role="status" aria-live="polite">
                    {customGptNotice}
                  </p>
                )}
                {customGpts.state === "loading" && (
                  <p className="muted">Cargando GPTs personales…</p>
                )}
                {customGpts.state === "error" && (
                  <p className="error">{customGpts.message}</p>
                )}
                {customGpts.state === "ready" && (
                  customGpts.value.length === 0 ? (
                    <p className="activity-empty">Todavía no has creado ningún GPT personal.</p>
                  ) : (
                    <div className="custom-gpt-list">
                      {customGpts.value.map((item) => (
                        <article key={item.id} className="custom-gpt-item">
                          <div>
                            <div className="custom-gpt-item-heading">
                              <strong>{item.name}</strong>
                              <span>Versión {item.versionNo}</span>
                            </div>
                            {item.description && <p>{item.description}</p>}
                            <small>{item.instructions}</small>
                            {item.conversationStarters.length > 0 && (
                              <div className="custom-gpt-starter-summary">
                                {item.conversationStarters.length} iniciador(es)
                              </div>
                            )}
                            <div className="custom-gpt-permission-summary">
                              <span>
                                Código: {item.toolPermissions.runCode === "confirm"
                                  ? "confirmar"
                                  : "denegado"}
                              </span>
                              <span>
                                Renombrar: {item.toolPermissions.renameConversation === "confirm"
                                  ? "confirmar"
                                  : "denegado"}
                              </span>
                            </div>
                          </div>
                          <div className="custom-gpt-item-actions">
                            <button
                              className={
                                customGptKnowledge?.customGptId === item.id
                                  ? "primary"
                                  : "secondary"
                              }
                              onClick={() => void openCustomGptKnowledge(item.id)}
                              disabled={customGptBusy || customGptKnowledgeBusy}
                              aria-expanded={customGptKnowledge?.customGptId === item.id}
                            >
                              Conocimiento
                            </button>
                            <button
                              className="secondary"
                              onClick={() => void exportCustomGpt(item)}
                              disabled={customGptBusy}
                              title="Exporta solo la configuración. No incluye conocimiento, archivos ni permisos."
                            >
                              Exportar
                            </button>
                            <button
                              className="secondary"
                              onClick={() => void exportCustomGpt(item, true)}
                              disabled={customGptBusy}
                              title="Incluye únicamente conocimiento textual activo y no sensible. Los archivos y permisos nunca se exportan."
                            >
                              Exportar con conocimiento
                            </button>
                            <button
                              className="secondary"
                              onClick={() => void openCustomGptPreview(item.id)}
                              disabled={customGptBusy}
                              title="Muestra lo que recibiría el modelo. No envía nada ni genera coste."
                            >
                              Vista previa
                            </button>
                            <button
                              className="secondary"
                              onClick={() => void duplicateCustomGpt(item.id)}
                              disabled={customGptBusy}
                              title="Crea una copia con la misma configuración, sin permisos ni conocimiento."
                            >
                              Duplicar
                            </button>
                            <button
                              className={customGptHistoryId === item.id ? "primary" : "secondary"}
                              onClick={() => void loadCustomGptVersions(item.id)}
                              disabled={customGptBusy}
                              aria-expanded={customGptHistoryId === item.id}
                            >
                              Historial
                            </button>
                            <button
                              className="secondary"
                              onClick={() => beginCustomGptEdit(item)}
                              disabled={customGptBusy}
                            >
                              Editar
                            </button>
                          </div>
                          {customGptHistoryId === item.id && (
                            <div className="custom-gpt-history" aria-live="polite">
                              {customGptVersions.state === "loading" && (
                                <small>Cargando historial…</small>
                              )}
                              {customGptVersions.state === "error" && (
                                <small role="alert">{customGptVersions.message}</small>
                              )}
                              {customGptVersions.state === "ready" && (
                                <ul>
                                  {customGptVersions.value.map((version) => (
                                    <li key={version.id}>
                                      <div>
                                        <strong>Versión {version.versionNo}</strong>
                                        <span>{customGptVersionSummary(version)}</span>
                                        <small>{version.instructions}</small>
                                      </div>
                                      {!version.active && (
                                        <button
                                          className="secondary"
                                          onClick={() =>
                                            void restoreCustomGptVersion(item.id, version.id)}
                                          disabled={customGptBusy}
                                          title="Crea una versión nueva con este contenido. No borra ninguna revisión."
                                        >
                                          Restaurar
                                        </button>
                                      )}
                                    </li>
                                  ))}
                                </ul>
                              )}
                            </div>
                          )}
                        </article>
                      ))}
                    </div>
                  )
                )}
                {customGptKnowledge && (
                  <section className="custom-gpt-knowledge" aria-live="polite">
                    <div className="custom-gpt-knowledge-heading">
                      <div>
                        <span className="kicker">Conocimiento privado</span>
                        <h4>
                          {customGpts.state === "ready"
                            ? customGpts.value.find(
                                (item) => item.id === customGptKnowledge.customGptId
                              )?.name ?? "GPT personal"
                            : "GPT personal"}
                        </h4>
                      </div>
                      <button
                        className="secondary"
                        onClick={() => {
                          setCustomGptKnowledge(null);
                          setCustomGptFiles(null);
                        }}
                        disabled={customGptKnowledgeBusy}
                      >
                        Cerrar
                      </button>
                    </div>
                    <p className="muted">
                      Estos datos y archivos solo se añaden a los chats que usan este GPT. No
                      aparecen en la memoria general ni en otros GPTs.
                    </p>
                    <div className="custom-gpt-files">
                      <div className="custom-gpt-files-heading">
                        <div>
                          <strong>Archivos de conocimiento</strong>
                          <span>Hasta 20; se aplican automáticamente al seleccionar el GPT.</span>
                        </div>
                        <button
                          className="secondary"
                          onClick={() => void importCustomGptFiles()}
                          disabled={customGptKnowledgeBusy}
                        >
                          Añadir archivos
                        </button>
                      </div>
                      {customGptFiles?.data.state === "loading" && (
                        <p className="muted">Cargando archivos…</p>
                      )}
                      {customGptFiles?.data.state === "error" && (
                        <p className="error" role="alert">
                          {customGptFiles.data.message}
                        </p>
                      )}
                      {customGptFiles?.data.state === "ready" &&
                        (customGptFiles.data.value.length === 0 ? (
                          <p className="activity-empty">
                            Este GPT todavía no tiene archivos propios.
                          </p>
                        ) : (
                          <div className="custom-gpt-files-list">
                            {customGptFiles.data.value.map((file) => (
                              <article key={file.id} className="custom-gpt-file-item">
                                <div>
                                  <strong>{file.displayName}</strong>
                                  <span>
                                    {attachmentStatusLabel(file.ingestionStatus)}
                                    {file.ingestionStatus === "ready" &&
                                      ` · ${
                                        attachmentContextSummary(file)?.label ??
                                        "Contexto pendiente"
                                      }`}
                                  </span>
                                </div>
                                <button
                                  className="danger-link"
                                  onClick={() => void removeCustomGptFile(file.id)}
                                  disabled={customGptKnowledgeBusy}
                                >
                                  Retirar
                                </button>
                              </article>
                            ))}
                          </div>
                        ))}
                    </div>
                    {customGptKnowledge.data.state === "loading" && (
                      <p className="muted">Cargando conocimiento…</p>
                    )}
                    {customGptKnowledge.data.state === "error" && (
                      <p className="error" role="alert">
                        {customGptKnowledge.data.message}
                      </p>
                    )}
                    {customGptKnowledge.data.state === "ready" && (
                      <>
                        <div className="custom-gpt-knowledge-form">
                          <textarea
                            value={customGptKnowledgeDraft}
                            onChange={(event) =>
                              setCustomGptKnowledgeDraft(event.target.value)}
                            placeholder="Ejemplo: El producto usa contratos versionados y prioriza compatibilidad hacia atrás."
                            rows={3}
                            maxLength={2000}
                            disabled={customGptKnowledgeBusy}
                          />
                          <div className="custom-gpt-knowledge-controls">
                            <select
                              value={customGptKnowledgeCategory}
                              onChange={(event) =>
                                setCustomGptKnowledgeCategory(
                                  event.target.value as MemoryItemView["category"]
                                )}
                              disabled={customGptKnowledgeBusy}
                              aria-label="Tipo de conocimiento"
                            >
                              <option value="fact">Dato</option>
                              <option value="instruction">Instrucción</option>
                              <option value="preference">Preferencia</option>
                            </select>
                            <label>
                              <input
                                type="checkbox"
                                checked={customGptKnowledgeSensitive}
                                onChange={(event) =>
                                  setCustomGptKnowledgeSensitive(event.target.checked)}
                                disabled={customGptKnowledgeBusy}
                              />
                              Sensible: mantener en modelos locales
                            </label>
                            <button
                              className="primary"
                              onClick={() => void createCustomGptKnowledge()}
                              disabled={
                                customGptKnowledgeBusy ||
                                !customGptKnowledgeDraft.trim()
                              }
                            >
                              {customGptKnowledgeBusy ? "Guardando…" : "Añadir conocimiento"}
                            </button>
                          </div>
                        </div>
                        {customGptKnowledgeNotice && (
                          <p className="custom-gpt-notice" role="status">
                            {customGptKnowledgeNotice}
                          </p>
                        )}
                        {customGptKnowledge.data.value.length === 0 ? (
                          <p className="activity-empty">
                            Este GPT todavía no tiene conocimiento propio.
                          </p>
                        ) : (
                          <div className="custom-gpt-knowledge-list">
                            {customGptKnowledge.data.value.map((item) => (
                              <article
                                key={item.id}
                                className={`custom-gpt-knowledge-item ${
                                  item.enabled ? "" : "disabled"
                                }`}
                              >
                                <div>
                                  <div className="memory-badges">
                                    <span>
                                      {item.category === "preference"
                                        ? "Preferencia"
                                        : item.category === "instruction"
                                          ? "Instrucción"
                                          : "Dato"}
                                    </span>
                                    {item.sensitivity === "sensitive" && (
                                      <span className="sensitive">Sensible</span>
                                    )}
                                    <span className={`embedding ${item.embeddingStatus}`}>
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
                                  {item.embeddingError && (
                                    <small className="error">{item.embeddingError}</small>
                                  )}
                                </div>
                                <div className="custom-gpt-knowledge-item-actions">
                                  <button
                                    className="secondary"
                                    onClick={() =>
                                      void toggleCustomGptKnowledgeItem(
                                        item.id,
                                        !item.enabled
                                      )}
                                    disabled={customGptKnowledgeBusy}
                                  >
                                    {item.enabled ? "No usar" : "Usar"}
                                  </button>
                                  {item.embeddingStatus !== "indexing" && (
                                    <button
                                      className="secondary"
                                      onClick={() =>
                                        void reindexCustomGptKnowledgeItem(item.id)}
                                      disabled={customGptKnowledgeBusy}
                                    >
                                      Preparar índice
                                    </button>
                                  )}
                                  <button
                                    className="danger-link"
                                    onClick={() =>
                                      void removeCustomGptKnowledgeItem(item.id)}
                                    disabled={customGptKnowledgeBusy}
                                  >
                                    Eliminar
                                  </button>
                                </div>
                              </article>
                            ))}
                          </div>
                        )}
                      </>
                    )}
                  </section>
                )}
              </section>

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

      {customGptPreview && (
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
                ? `${customGptPreview.value.name} · versión ${customGptPreview.value.versionNo}`
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
      )}
      {keyboardHelpOpen && (
        <div className="modal-backdrop" role="presentation">
          <section
            ref={activeModalRef}
            className="modal keyboard-help-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="keyboard-help-title"
            aria-describedby="keyboard-help-description"
            tabIndex={-1}
          >
            <span className="kicker">Navegación accesible</span>
            <h2 id="keyboard-help-title">Atajos de teclado</h2>
            <p id="keyboard-help-description">
              Funcionan en toda la aplicación, pero las teclas sin modificadores no interrumpen
              la escritura ni se ejecutan encima de otra ventana.
            </p>
            <dl className="keyboard-shortcut-list">
              <div><dt><kbd>Ctrl</kbd> + <kbd>N</kbd></dt><dd>Nueva conversación</dd></div>
              <div><dt><kbd>Ctrl</kbd> + <kbd>F</kbd></dt><dd>Buscar conversaciones</dd></div>
              <div><dt><kbd>/</kbd></dt><dd>Buscar cuando no estás escribiendo</dd></div>
              <div><dt><kbd>Ctrl</kbd> + <kbd>Mayús</kbd> + <kbd>M</kbd></dt><dd>Ir al mensaje</dd></div>
              <div><dt><kbd>Alt</kbd> + <kbd>1</kbd></dt><dd>Volver a Inicio</dd></div>
              <div><dt><kbd>?</kbd></dt><dd>Abrir esta ayuda</dd></div>
              <div><dt><kbd>Esc</kbd></dt><dd>Cerrar una ventana abierta</dd></div>
            </dl>
            <div className="modal-actions">
              <button className="primary" autoFocus onClick={() => setKeyboardHelpOpen(false)}>
                Cerrar
              </button>
            </div>
          </section>
        </div>
      )}

      {dialog && (
        <div className="modal-backdrop" role="presentation">
          <section
            ref={activeModalRef}
            className="modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="dialog-title"
            tabIndex={-1}
          >
            <span className="kicker">Gestión local</span>
            <h2 id="dialog-title">{dialogCopy(dialog).title}</h2>
            <p>{dialogCopy(dialog).description}</p>
            {dialogCopy(dialog).fieldLabel && (
              <label>
                <span>{dialogCopy(dialog).fieldLabel}</span>
                {dialogCopy(dialog).multiline ? (
                  <textarea
                    autoFocus
                    value={dialogValue}
                    onChange={(event) => setDialogValue(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Escape") setDialog(null);
                    }}
                    maxLength={dialogCopy(dialog).maxLength}
                    rows={9}
                    placeholder="Ejemplo: responde en español, cita siempre las fuentes y separa claramente hechos de hipótesis."
                  />
                ) : (
                  <input
                    autoFocus
                    value={dialogValue}
                    onChange={(event) => setDialogValue(event.target.value)}
                    onKeyDown={(event) => {
                      if (event.key === "Enter") void submitDialog();
                      if (event.key === "Escape") setDialog(null);
                    }}
                    maxLength={dialogCopy(dialog).maxLength ?? 120}
                  />
                )}
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
                  Boolean(
                    dialogCopy(dialog).fieldLabel
                    && !dialogCopy(dialog).allowEmpty
                    && !dialogValue.trim()
                  )
                }
              >
                {dialogBusy ? "Guardando…" : dialogCopy(dialog).action}
              </button>
            </div>
          </section>
        </div>
      )}

      {projectKnowledge && (
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
      )}

      {summaryPanel && (
        <div className="modal-backdrop" role="presentation">
          <section
            ref={activeModalRef}
            className="modal summary-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="summary-title"
            tabIndex={-1}
          >
            <span className="kicker">Contexto controlado</span>
            <h2 id="summary-title">Resumen de la conversación</h2>
            <p>
              El historial original siempre se conserva. Solo un resumen que edites y apruebes
              se utilizará para representar los mensajes anteriores que cubre.
            </p>
            {summaryPanel.state === "loading" && <p className="muted">Cargando resumen…</p>}
            {summaryPanel.state === "error" && <p className="error">{summaryPanel.message}</p>}
            {summaryPanel.state !== "ready" && (
              <div className="modal-actions">
                <button className="secondary" onClick={() => setSummaryPanel(null)}>
                  Cerrar
                </button>
              </div>
            )}
            {summaryPanel.state === "ready" && (
              <>
                {summaryPanel.value.active && (
                  <div className="summary-active">
                    <strong>Resumen activo</strong>
                    <p>{summaryPanel.value.active.approvedText}</p>
                    <small>
                      Cubre {summaryPanel.value.activeCoveredMessageCount} de{" "}
                      {summaryPanel.value.totalMessageCount} mensajes · quedan{" "}
                      {summaryPanel.value.remainingMessageCount}
                    </small>
                  </div>
                )}
                {summaryPanel.value.candidate?.status === "generating" && (
                  <div className="summary-progress">
                    <span className="spinner" aria-hidden="true" />
                    <div>
                      <strong>Preparando borrador…</strong>
                      <p>Puedes cerrar esta ventana; la tarea continuará y se recuperará al reiniciar.</p>
                      {summaryPanel.value.candidateCoveredMessageCount !== undefined && (
                        <small>
                          Este lote avanzará hasta{" "}
                          {summaryPanel.value.candidateCoveredMessageCount} de{" "}
                          {summaryPanel.value.totalMessageCount} mensajes.
                        </small>
                      )}
                    </div>
                  </div>
                )}
                {summaryPanel.value.candidate?.status === "draft" && (
                  <label className="summary-editor">
                    <span>Borrador pendiente de aprobación</span>
                    <textarea
                      autoFocus
                      value={summaryDraft}
                      onChange={(event) => setSummaryDraft(event.target.value)}
                      maxLength={10_000}
                    />
                    <small>{summaryDraft.length.toLocaleString("es-ES")} / 10.000 caracteres</small>
                    {summaryPanel.value.candidateCoveredMessageCount !== undefined && (
                      <small className="summary-coverage">
                        Al aprobarlo cubrirá{" "}
                        {summaryPanel.value.candidateCoveredMessageCount} de{" "}
                        {summaryPanel.value.totalMessageCount} mensajes y conservará{" "}
                        {summaryPanel.value.totalMessageCount -
                          summaryPanel.value.candidateCoveredMessageCount} recientes.
                      </small>
                    )}
                  </label>
                )}
                {!summaryPanel.value.candidate && (
                  <p className="muted">
                    {summaryPanel.value.totalMessageCount === 0
                      ? "Todavía no hay mensajes que resumir."
                      : summaryPanel.value.active && summaryPanel.value.remainingMessageCount === 0
                        ? "El resumen está al día y ya cubre todos los mensajes disponibles."
                        : summaryPanel.value.active
                      ? "Puedes generar un nuevo borrador sin desactivar el resumen actual."
                      : "Todavía no hay ningún resumen. La generación crea un borrador, nunca uno activo."}
                  </p>
                )}
                <div className="modal-actions">
                  <button
                    className="secondary"
                    onClick={() => setSummaryPanel(null)}
                    disabled={summaryBusy}
                  >
                    Cerrar
                  </button>
                  {!summaryPanel.value.candidate &&
                    summaryPanel.value.totalMessageCount > 0 &&
                    summaryPanel.value.remainingMessageCount > 0 && (
                    <button className="primary" onClick={generateSummary} disabled={summaryBusy}>
                      {summaryBusy
                        ? "Preparando…"
                        : summaryPanel.value.active
                          ? "Actualizar borrador"
                          : "Generar borrador"}
                    </button>
                  )}
                  {summaryPanel.value.candidate?.status === "draft" && (
                    <>
                      <button
                        className="secondary"
                        onClick={saveSummaryDraft}
                        disabled={summaryBusy || !summaryDraft.trim()}
                      >
                        Guardar borrador
                      </button>
                      <button
                        className="primary"
                        onClick={approveSummaryDraft}
                        disabled={summaryBusy || !summaryDraft.trim()}
                      >
                        {summaryBusy ? "Guardando…" : "Guardar y aprobar"}
                      </button>
                    </>
                  )}
                </div>
              </>
            )}
          </section>
        </div>
      )}
    </div>
  );
}
