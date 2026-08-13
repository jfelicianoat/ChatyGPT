export const TASK_STATUSES = [
  "queued",
  "routing",
  "planning",
  "resource_planning",
  "converting",
  "chunking",
  "generating",
  "proposing",
  "evaluating",
  "debating",
  "synthesizing",
  "verifying",
  "waiting_for_memory",
  "waiting_for_tools",
  "completed",
  "failed",
  "cancelled"
] as const;

export type BrokerTaskStatus = (typeof TASK_STATUSES)[number];

export type BootstrapReport = {
  appVersion: string;
  databasePath: string;
  /** Ruta del registro estructurado; ausente si todavía no pudo prepararse. */
  logPath: string | null;
  schemaVersion: number;
  recoveredTasks: number;
  recoveredAttachments: number;
  recoveredWorkflows: number;
  recoveryItems: RecoveryItemView[];
};

/**
 * Resumen de una métrica de rendimiento sobre las muestras conservadas.
 *
 * `meetsBudget` es `null` mientras no exista ninguna muestra: el objetivo no se
 * declara cumplido ni incumplido sin una ejecución real que lo respalde.
 */
export type PerformanceMetricSummary = {
  metric: "app_start" | "conversation_open" | "conversation_search" | "ui_response";
  label: string;
  description: string;
  budgetMs: number;
  samples: number;
  p50Ms: number | null;
  p95Ms: number | null;
  maxMs: number | null;
  meetsBudget: boolean | null;
  lastRecordedAt: string | null;
};

export type PerformanceReportView = {
  metrics: PerformanceMetricSummary[];
  sampleLimit: number;
  totalSamples: number;
};

export type WindowsStartupStatus = {
  supported: boolean;
  enabled: boolean;
  credentialProtected: boolean;
  message: string;
};

export type ScheduledRunView = {
  id: string;
  dueAt: string;
  status: "claimed" | "running" | "completed" | "failed" | "cancelled" | "skipped";
  brokerTaskId?: string;
  workflowRunId?: string;
  attempt: number;
  result?: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
};

export type ScheduledTaskView = {
  id: string;
  name: string;
  targetKind?: "conversation" | "workflow";
  conversationId?: string;
  conversationTitle?: string;
  workflowId?: string;
  workflowName?: string;
  workflowVersionNo?: number;
  prompt: string;
  scheduleExpression: "once" | "daily" | "weekly";
  timezone: string;
  enabled: boolean;
  confirmedAt?: string;
  nextRunAt?: string;
  createdAt: string;
  updatedAt: string;
  runs: ScheduledRunView[];
};

export type ScheduledHistorySort = "newest" | "oldest";

export type ScheduledRunPageView = {
  items: ScheduledRunView[];
  total: number;
  page: number;
  pageSize: 10 | 25 | 50;
  sort: ScheduledHistorySort;
};

export type ScheduledTaskTemplateView = {
  id: string;
  name: string;
  prompt: string;
  scheduleExpression: ScheduledTaskView["scheduleExpression"];
  createdAt: string;
  updatedAt: string;
};

export type ScheduledCalendarOccurrence = {
  id: string;
  taskId: string;
  taskName: string;
  conversationId: string;
  conversationTitle: string;
  startsAt: string;
  scheduleExpression: ScheduledTaskView["scheduleExpression"];
  timezone: string;
  projected: boolean;
  overdue: boolean;
  conflictingTaskIds: string[];
};

const nextScheduledOccurrence = (
  current: Date,
  scheduleExpression: ScheduledTaskView["scheduleExpression"]
): Date | null => {
  if (scheduleExpression === "once") return null;
  const next = new Date(current);
  next.setDate(next.getDate() + (scheduleExpression === "daily" ? 1 : 7));
  return next;
};

export const scheduledCalendarOccurrences = (
  tasks: ScheduledTaskView[],
  now = new Date(),
  rangeDays = 14,
  conflictWindowMinutes = 15
): ScheduledCalendarOccurrence[] => {
  const start = now.getTime();
  if (!Number.isFinite(start)) return [];
  const endDate = new Date(now);
  endDate.setDate(endDate.getDate() + Math.min(90, Math.max(1, rangeDays)));
  const end = endDate.getTime();
  const occurrences: ScheduledCalendarOccurrence[] = [];

  for (const task of tasks) {
    if (!task.enabled || !task.nextRunAt) continue;
    let startsAt = new Date(task.nextRunAt);
    if (!Number.isFinite(startsAt.getTime())) continue;
    let projected = false;
    let includedOverdue = false;

    for (let guard = 0; guard < 400 && startsAt.getTime() < end; guard += 1) {
      const timestamp = startsAt.getTime();
      const overdue = timestamp < start;
      if (!overdue || !includedOverdue) {
        occurrences.push({
          id: `${task.id}:${startsAt.toISOString()}`,
          taskId: task.id,
          taskName: task.name,
          conversationId: task.conversationId ?? task.workflowId ?? task.id,
          conversationTitle: task.conversationTitle ?? task.workflowName ?? task.name,
          startsAt: startsAt.toISOString(),
          scheduleExpression: task.scheduleExpression,
          timezone: task.timezone,
          projected,
          overdue,
          conflictingTaskIds: []
        });
        if (overdue) includedOverdue = true;
      }
      const next = nextScheduledOccurrence(startsAt, task.scheduleExpression);
      if (!next) break;
      startsAt = next;
      projected = true;
    }
  }

  occurrences.sort((left, right) => left.startsAt.localeCompare(right.startsAt));
  const conflictWindow = Math.max(0, conflictWindowMinutes) * 60 * 1000;
  for (let leftIndex = 0; leftIndex < occurrences.length; leftIndex += 1) {
    const left = occurrences[leftIndex];
    if (left.overdue) continue;
    const leftTime = new Date(left.startsAt).getTime();
    for (let rightIndex = leftIndex + 1; rightIndex < occurrences.length; rightIndex += 1) {
      const right = occurrences[rightIndex];
      if (right.overdue) continue;
      const distance = new Date(right.startsAt).getTime() - leftTime;
      if (distance > conflictWindow) break;
      if (left.taskId === right.taskId) continue;
      if (!left.conflictingTaskIds.includes(right.taskId)) {
        left.conflictingTaskIds.push(right.taskId);
      }
      if (!right.conflictingTaskIds.includes(left.taskId)) {
        right.conflictingTaskIds.push(left.taskId);
      }
    }
  }
  return occurrences;
};

/**
 * Conversaciones que debe mostrar la barra lateral.
 *
 * Extraído de `App.tsx` (fase 2). Hay una regla que no es evidente y conviene
 * fijar: **buscar tiene prioridad sobre el ámbito de proyecto**. Los resultados
 * de una búsqueda se muestran completos aunque haya un proyecto seleccionado,
 * porque quien busca espera encontrar, no que el filtro activo le esconda la
 * conversación que estaba buscando.
 */
export const visibleConversations = ({
  conversations,
  searchResults,
  searchQuery,
  selectedProjectId
}: {
  conversations: ConversationSummary[];
  searchResults: ConversationSummary[];
  searchQuery: string;
  /** `null` es «todos los chats» y `"unassigned"`, los que no tienen proyecto. */
  selectedProjectId: string | null;
}): ConversationSummary[] => {
  const searching = searchQuery.trim().length > 0;
  const source = searching ? searchResults : conversations;
  if (searching || selectedProjectId === null) {
    return source;
  }
  if (selectedProjectId === "unassigned") {
    return source.filter((item) => !item.projectId);
  }
  return source.filter((item) => item.projectId === selectedProjectId);
};

/**
 * Si un recuerdo alcanza a la conversación abierta.
 *
 * Un recuerdo sin proyecto es global y alcanza a cualquier conversación; uno
 * acotado a un proyecto solo alcanza a las de ese proyecto. La regla estaba
 * escrita dos veces en `App.tsx` —para contar los activos y para contar los que
 * tienen índice— y esa duplicación es justo la forma en que estas reglas se
 * desincronizan al cambiarlas.
 */
export const memoryAppliesToConversation = (
  item: Pick<MemoryItemView, "projectId">,
  conversationProjectId: string | null | undefined
): boolean => !item.projectId || item.projectId === conversationProjectId;

/** Recuerdos habilitados que alcanzan a la conversación abierta. */
export const activeMemoriesForConversation = (
  items: MemoryItemView[],
  conversationProjectId: string | null | undefined
): MemoryItemView[] =>
  items.filter(
    (item) => item.enabled && memoryAppliesToConversation(item, conversationProjectId)
  );

/**
 * Recuerdos activos que además tienen índice semántico utilizable.
 *
 * Es deliberadamente un subconjunto de los activos: un recuerdo indexado pero
 * deshabilitado, o acotado a otro proyecto, no puede recuperarse por similitud.
 */
export const semanticReadyMemoriesForConversation = (
  items: MemoryItemView[],
  conversationProjectId: string | null | undefined
): MemoryItemView[] =>
  activeMemoriesForConversation(items, conversationProjectId).filter(
    (item) => item.embeddingStatus === "ready"
  );

const normalizeScheduledSearch = (value: string): string =>
  value
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLocaleLowerCase("es");

export const filterScheduledTasks = (
  tasks: ScheduledTaskView[],
  query: string
): ScheduledTaskView[] => {
  const needle = normalizeScheduledSearch(query.trim());
  if (!needle) return tasks;
  return tasks.filter((task) =>
    normalizeScheduledSearch(
      `${task.name} ${task.conversationTitle ?? ""} ${task.workflowName ?? ""} ${task.prompt}`
    ).includes(needle)
  );
};

export type ScheduledTaskDuplicateDraft = {
  name: string;
  conversationId: string;
  prompt: string;
  scheduleExpression: ScheduledTaskView["scheduleExpression"];
  confirmed: false;
};

export const scheduledTaskDuplicateDraft = (
  task: ScheduledTaskView
): ScheduledTaskDuplicateDraft => ({
  name: `Copia de ${task.name}`.slice(0, 120),
  conversationId: task.conversationId ?? "",
  prompt: task.prompt,
  scheduleExpression: task.scheduleExpression,
  confirmed: false
});

export type ScheduledNotificationView = {
  id: string;
  taskId: string;
  taskName: string;
  conversationId: string;
  conversationTitle: string;
  status: "completed" | "failed" | "cancelled";
  attempt: number;
  updatedAt: string;
};

export const scheduledNotifications = (
  tasks: ScheduledTaskView[],
  limit = 30
): ScheduledNotificationView[] =>
  tasks
    .flatMap((task) =>
      task.runs
        .filter(
          (run): run is typeof run & {
            status: ScheduledNotificationView["status"];
          } => ["completed", "failed", "cancelled"].includes(run.status)
        )
        .map((run) => ({
          id: run.id,
          taskId: task.id,
          taskName: task.name,
          conversationId: task.conversationId ?? task.workflowId ?? task.id,
          conversationTitle: task.conversationTitle ?? task.workflowName ?? task.name,
          status: run.status,
          attempt: run.attempt,
          updatedAt: run.updatedAt
        }))
    )
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    .slice(0, Math.max(0, limit));

export type ScheduledHistoryStatusFilter =
  "all" | "active" | "completed" | "failed" | "cancelled";
export type ScheduledHistoryPeriodFilter = "all" | "today" | "7d" | "30d";

export const filterScheduledRuns = (
  runs: ScheduledRunView[],
  statusFilter: ScheduledHistoryStatusFilter,
  periodFilter: ScheduledHistoryPeriodFilter,
  now = new Date()
): ScheduledRunView[] => {
  const activeStatuses = new Set<ScheduledRunView["status"]>(["claimed", "running"]);
  const maximumAge = periodFilter === "7d"
    ? 7 * 24 * 60 * 60 * 1000
    : periodFilter === "30d"
      ? 30 * 24 * 60 * 60 * 1000
      : null;
  return runs.filter((run) => {
    const statusMatches =
      statusFilter === "all" ||
      (statusFilter === "active"
        ? activeStatuses.has(run.status)
        : run.status === statusFilter);
    if (!statusMatches || periodFilter === "all") return statusMatches;
    const updatedAt = new Date(run.updatedAt);
    if (!Number.isFinite(updatedAt.getTime())) return false;
    if (periodFilter === "today") {
      return updatedAt.toDateString() === now.toDateString();
    }
    return maximumAge !== null &&
      updatedAt.getTime() >= now.getTime() - maximumAge &&
      updatedAt.getTime() <= now.getTime();
  });
};

export const scheduledRunDetail = (
  run: ScheduledRunView
): { label: string; text: string } | undefined => {
  const result = run.result;
  const nestedError = result?.error;
  const candidates = [
    result?.message,
    result?.assistant_content,
    result?.result_markdown,
    result?.text,
    result?.detail,
    typeof nestedError === "object" && nestedError !== null
      ? (nestedError as Record<string, unknown>).message
      : undefined
  ];
  const detail = candidates.find(
    (value): value is string => typeof value === "string" && value.trim().length > 0
  );
  if (detail) {
    return {
      label: run.status === "failed" ? "Motivo del fallo" : "Resultado",
      text: detail.trim().slice(0, 4_000)
    };
  }
  if (result?.outputs && typeof result.outputs === "object") {
    const outputs = Object.entries(result.outputs as Record<string, unknown>)
      .filter((entry): entry is [string, string] => typeof entry[1] === "string")
      .map(([label, value]) => `${label}: ${value.trim()}`)
      .join("\n\n")
      .trim();
    if (outputs) return { label: "Resultado del flujo", text: outputs.slice(0, 4_000) };
  }
  if (run.status === "completed") {
    return {
      label: "Resultado",
      text: "La respuesta completa está guardada en la conversación."
    };
  }
  if (run.status === "cancelled") {
    return { label: "Detalle", text: "Cancelada por el usuario." };
  }
  if (run.status === "failed") {
    return {
      label: "Motivo del fallo",
      text: "El Broker no proporcionó más detalles."
    };
  }
  return undefined;
};

export type ScheduledHistoryExportReport = {
  destinationPath: string;
  destinationHash: string;
  overwritten: boolean;
  runCount: number;
};

export type ScheduledCalendarExportEntry = {
  occurrenceId: string;
  taskName: string;
  conversationTitle: string;
  startsAt: string;
  projected: boolean;
  overdue: boolean;
};

export type ScheduledCalendarExportReport = {
  destinationPath: string;
  destinationHash: string;
  overwritten: boolean;
  eventCount: number;
};

export type RecoveryItemView = {
  kind: "task" | "embedding";
  label: string;
  status: string;
  conversationId?: string;
  conversationTitle?: string;
  updatedAt: string;
};

export type AttachmentView = {
  id: string;
  displayName: string;
  mediaType?: string;
  sizeBytes: number;
  sha256: string;
  brokerFileId?: string;
  ingestionStatus: "local" | "uploading" | "received" | "converting" | "ready" | "failed";
  ingestionError?: Record<string, unknown>;
  contextStatus: "pending" | "preparing" | "ready" | "unavailable" | "failed";
  contextError?: Record<string, unknown>;
  chunkCount: number;
  indexedCharacters: number;
  semanticIndexedChunks: number;
  semanticIndexStatus: "pending" | "indexing" | "ready" | "partial" | "failed" | "unavailable";
  semanticIndexModel?: string;
  describeImages?: boolean | null;
  updatedAt: string;
};

export const attachmentImagePolicyLabel = (
  attachment: Pick<AttachmentView, "describeImages">
): string | null =>
  attachment.describeImages === true
    ? "con imágenes"
    : attachment.describeImages === false
      ? "sin imágenes"
      : null;

export const attachmentSelectionOnConversationOpen = (
  attachments: Array<Pick<AttachmentView, "id">>
): string[] => attachments.map((attachment) => attachment.id);

export const projectFilesAvailableToConversation = <T extends Pick<AttachmentView, "id">>(
  attachments: Array<Pick<AttachmentView, "id">>,
  projectFiles: T[]
): T[] => {
  const attachedIds = new Set(attachments.map((attachment) => attachment.id));
  return projectFiles.filter((file) => !attachedIds.has(file.id));
};

export const attachmentNeedsSandbox = (
  attachment: Pick<AttachmentView, "displayName" | "mediaType">
): boolean => {
  const mediaType = attachment.mediaType?.toLowerCase() ?? "";
  const displayName = attachment.displayName.toLowerCase();
  return [
    "text/csv",
    "text/tab-separated-values",
    "application/vnd.ms-excel",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
  ].includes(mediaType) || [".csv", ".tsv", ".xls", ".xlsx"].some(
    (extension) => displayName.endsWith(extension)
  );
};

export const shouldRefreshSandboxDiagnostic = ({
  requiresCodeExecution,
  sandboxEnabledForTurn,
  sandboxAvailable,
  skipSuggestion
}: {
  requiresCodeExecution: boolean;
  sandboxEnabledForTurn: boolean;
  sandboxAvailable: boolean;
  skipSuggestion: boolean;
}): boolean =>
  !skipSuggestion &&
  !sandboxEnabledForTurn &&
  requiresCodeExecution &&
  !sandboxAvailable;

export type ComposerErrorGuidance = {
  title: string;
  detail: string;
  action: string;
};

export const sandboxUnavailableGuidance = (
  tabularAttachment: boolean,
  diagnosticMessage?: string
): ComposerErrorGuidance => ({
  title: tabularAttachment
    ? "No se puede analizar el archivo todavía"
    : "Código aislado no está disponible todavía",
  detail: diagnosticMessage?.trim() || (
    tabularAttachment
      ? "El CSV o la hoja de cálculo necesita Código aislado, pero Broker AI no lo anuncia como disponible."
      : "La petición necesita ejecutar código, pero Broker AI no anuncia un contenedor aislado disponible."
  ),
  action: "Comprueba la conexión y vuelve a intentarlo. El mensaje no se ha enviado."
});

export type AttachmentFailureGuidance = {
  title: string;
  detail: string;
  action: string;
  retryLabel: string;
};

export type AttachmentContextSummary = {
  label: string;
  detail: string;
  tone: "pending" | "ready" | "warning" | "error";
  retryable: boolean;
  retryTarget?: "context" | "semantic";
  retryLabel?: string;
};

export const attachmentContextSummary = (
  attachment: AttachmentView
): AttachmentContextSummary | undefined => {
  if (attachment.ingestionStatus !== "ready") {
    return undefined;
  }
  if (attachment.contextStatus === "failed") {
    const message =
      typeof attachment.contextError?.message === "string"
        ? attachment.contextError.message.slice(0, 300)
        : "No se pudo descargar o dividir el contenido convertido.";
    return {
      label: "Contexto local no preparado",
      detail: message,
      tone: "error",
      retryable: true
    };
  }
  if (attachment.contextStatus === "preparing") {
    return {
      label: "Preparando contexto local",
      detail: "El archivo ya está en el Broker; ChatyGPT está preparando sus fragmentos.",
      tone: "pending",
      retryable: false
    };
  }
  if (attachment.contextStatus === "pending") {
    return {
      label: "Contexto local pendiente",
      detail: "El archivo ya está disponible; su contenido se preparará a continuación.",
      tone: "pending",
      retryable: false
    };
  }
  if (attachment.contextStatus === "unavailable") {
    return {
      label: "Sin fragmentos locales",
      detail: "El Broker no ofreció texto convertido; se usará el archivo completo.",
      tone: "warning",
      retryable: true
    };
  }
  if (attachment.contextStatus !== "ready") return undefined;
  const unit = attachment.chunkCount === 1 ? "fragmento" : "fragmentos";
  const estimatedTokens = Math.ceil(attachment.indexedCharacters / 4);
  const semanticModel = attachment.semanticIndexModel?.split("/").at(-1);
  const semanticDetail = (() => {
    switch (attachment.semanticIndexStatus) {
      case "ready":
        return (
          `Índice semántico preparado (${attachment.semanticIndexedChunks}/${attachment.chunkCount})` +
          `${semanticModel ? ` con ${semanticModel}` : ""}.`
        );
      case "partial":
        return `Índice semántico parcial (${attachment.semanticIndexedChunks}/${attachment.chunkCount}).`;
      case "failed":
        return "No se pudo preparar el índice semántico; la búsqueda por texto sigue disponible.";
      case "indexing":
      case "pending":
        return `Preparando índice semántico (${attachment.semanticIndexedChunks}/${attachment.chunkCount}).`;
      default:
        return "Búsqueda por texto disponible.";
    }
  })();
  const semanticRetryable = ["partial", "failed"].includes(attachment.semanticIndexStatus);
  return {
    label: `Contexto preparado · ${attachment.chunkCount} ${unit}`,
    detail:
      `Cobertura: ${attachment.indexedCharacters.toLocaleString("es-ES")} caracteres consultables ` +
      `(~${estimatedTokens.toLocaleString("es-ES")} tokens estimados). ` +
      `${semanticDetail} Se recuperan los fragmentos relevantes y su contexto próximo.`,
    tone: semanticRetryable ? "warning" : "ready",
    retryable: semanticRetryable,
    ...(semanticRetryable
      ? { retryTarget: "semantic" as const, retryLabel: "Reintentar índice" }
      : {})
  };
};

export const attachmentStatusLabel = (
  status: AttachmentView["ingestionStatus"]
): string => ({
  local: "Pendiente",
  uploading: "Subiendo",
  received: "Recibido",
  converting: "Convirtiendo",
  ready: "Preparado",
  failed: "No preparado"
})[status];

export const attachmentFailureGuidance = (
  attachment: AttachmentView
): AttachmentFailureGuidance | undefined => {
  if (attachment.ingestionStatus !== "failed") return undefined;

  const message =
    typeof attachment.ingestionError?.message === "string"
      ? attachment.ingestionError.message
      : "";
  const pageLimit = message.match(
    /Document has\s+(\d+)\s+pages,\s+exceeding the max_num_pages limit of\s+(\d+)/i
  );
  if (pageLimit) {
    const format = (value: string): string =>
      value.replace(/\B(?=(\d{3})+(?!\d))/g, ".");
    return {
      title: "El PDF supera el límite de páginas",
      detail: `Tiene ${format(pageLimit[1])} páginas y el Broker admite ${format(pageLimit[2])} por conversión.`,
      action: "Divide el PDF en archivos más pequeños o aumenta el límite de páginas del Broker.",
      retryLabel: "Reintentar tras corregir"
    };
  }

  return {
    title: "No se pudo preparar el archivo",
    detail: message.slice(0, 300) || "El Broker no proporcionó más detalles sobre el fallo.",
    action: "Comprueba el Broker y vuelve a intentarlo.",
    retryLabel: "Reintentar"
  };
};

export type BrokerDiagnostic = {
  reachable: boolean;
  ready: boolean;
  baseUrl: string;
  contractVersion?: string;
  capabilitiesVerified?: boolean;
  ingestionFormats?: Record<string, string[]>;
  strategies: string[];
  presets: Record<string, string[]> | unknown;
  derivedDataBoundary?: boolean;
  workLanes: string[];
  agentSkills: string[];
  sandboxRunCode?: boolean;
  fileIngestion?: boolean;
  longContextMapReduce?: boolean;
  maxActiveWorkflows?: number;
  latencyMs: number;
  message: string;
};

export const brokerSupportsPreset = (
  broker: BrokerDiagnostic,
  strategy: string,
  preset: string
): boolean => {
  if (!broker.presets || typeof broker.presets !== "object" || Array.isArray(broker.presets)) {
    return true;
  }
  const declared = (broker.presets as Record<string, unknown>)[strategy];
  return !Array.isArray(declared) || declared.includes(preset);
};

export type AuditEventView = {
  id: number;
  category: "project" | "conversation" | "attachment" | "task" | "tool" | "export" | "memory" | "gpt" | "system";
  summary: string;
  severity: "info" | "warning" | "error";
  actor: string;
  conversationTitle?: string;
  occurredAt: string;
};

export type MemoryItemView = {
  id: string;
  projectId?: string;
  projectName?: string;
  customGptId?: string;
  customGptName?: string;
  category: "preference" | "instruction" | "fact";
  content: string;
  sensitivity: "normal" | "sensitive";
  enabled: boolean;
  embeddingStatus: "missing" | "indexing" | "ready" | "failed";
  embeddingModel?: string;
  embeddingError?: string;
  createdAt: string;
  updatedAt: string;
};

export type MemoryOverview = {
  enabled: boolean;
  items: MemoryItemView[];
};

export const canStartMemoryEdit = (activeMemoryId: string | null): boolean =>
  activeMemoryId === null;

export const memoryUpdateNotice = (contentChanged: boolean): string =>
  contentChanged
    ? "Recuerdo actualizado. ChatyGPT está preparando un índice nuevo."
    : "Recuerdo actualizado.";

export type MemorySearchResultView = {
  memoryId: string;
  content: string;
  category: "preference" | "instruction" | "fact";
  projectName?: string;
  sensitivity: "normal" | "sensitive";
  score: number;
  reason: string;
};

export type MemorySearchView = {
  id: string;
  query: string;
  projectId?: string;
  status: "searching" | "completed" | "failed";
  model?: string;
  error?: string;
  results: MemorySearchResultView[];
  createdAt: string;
};

export type ContextSourceView = {
  kind: "message" | "memory" | string;
  label: string;
  reason: string;
  score?: number;
  estimatedTokens: number;
  excerpt: string;
  sourceReference?: string;
  sourceAvailable: boolean;
};

export const canRevealContextSource = (source: ContextSourceView): boolean =>
  source.kind === "attachment_chunk" &&
  Boolean(source.sourceReference) &&
  source.sourceAvailable;

export const shouldApplyContextLoad = (
  activeTaskId: string | undefined,
  resolvedTaskId: string
): boolean => activeTaskId === resolvedTaskId;

export type ContextSnapshotView = {
  strategy: string;
  estimatedTokens: number;
  sources: ContextSourceView[];
};

export type LocalTaskSnapshot = {
  id: string;
  activity?: string;
  remoteTaskId?: string;
  remoteStatus: string;
  localState: string;
  consecutivePollErrors: number;
  result?: Record<string, unknown>;
  error?: Record<string, unknown>;
  progress: {
    phase?: string;
    invocationsCompleted?: number;
    invocationsTotal?: number;
  };
  pendingToolCalls: ToolCallView[];
  updatedAt: string;
};

const TASK_PHASE_LABELS: Record<string, string> = {
  queued: "En cola",
  routing: "Eligiendo el mejor modelo",
  planning: "Planificando",
  resource_planning: "Preparando recursos",
  converting: "Convirtiendo el archivo",
  chunking: "Dividiendo el documento",
  generating: "Generando respuesta",
  proposing: "Consultando modelos",
  evaluating: "Comparando propuestas",
  debating: "Contrastando respuestas",
  synthesizing: "Preparando la respuesta final",
  verifying: "Verificando el resultado",
  waiting_for_memory: "Esperando memoria disponible",
  waiting_for_tools: "Esperando tu confirmación",
  completed: "Completado"
};

export const taskProgressSummary = (task: LocalTaskSnapshot): {
  label: string;
  completed?: number;
  total?: number;
} => {
  const phase = task.progress.phase ?? task.remoteStatus;
  const label = TASK_PHASE_LABELS[phase] ?? task.activity ?? "Procesando";
  const completed = task.progress.invocationsCompleted;
  const total = task.progress.invocationsTotal;
  return total && total > 0 && completed !== undefined
    ? { label, completed: Math.min(completed, total), total }
    : { label };
};

/**
 * Fallos que piden una decisión de la persona, no un reintento.
 *
 * `RECOVERY_AMBIGUOUS_REMOTE_CALL` es el caso que obliga a distinguirlos: si
 * ChatyGPT se reinicia con una llamada a un modelo remoto en vuelo, el Broker
 * **no** reintenta por su cuenta para no pagar dos veces la misma inferencia.
 * Presentarlo como «la tarea no pudo completarse» invitaría a reenviar, que es
 * justo lo que se está evitando: quien decide si vale la pena volver a pagarla
 * es la persona, y necesita saber que quizá ya se ejecutó.
 */
const TASK_FAILURE_TITLES: Record<string, string> = {
  CONTEXT_LIMIT_EXCEEDED: "El contenido no cabe en el modelo seleccionado",
  RECOVERY_AMBIGUOUS_REMOTE_CALL: "No se sabe si la respuesta llegó a generarse",
  PROMPT_ECHOED: "El modelo repitió la petición en vez de responder",
  DEGENERATE_OUTPUT: "El modelo generó una respuesta repetitiva",
  VRAM_MODEL_TOO_LARGE: "El modelo no cabe en la memoria configurada"
};

/** Qué hacer a continuación, cuando no es simplemente reintentar. */
const TASK_FAILURE_GUIDANCE: Record<string, string> = {
  RECOVERY_AMBIGUOUS_REMOTE_CALL:
    "ChatyGPT se reinició mientras el modelo estaba respondiendo. El Broker no lo reintenta solo para no cobrar dos veces la misma inferencia. Revisa si la respuesta llegó antes de volver a enviarla.",
  MODEL_CAPABILITY_MISMATCH:
    "Elige otro modelo en las opciones de ejecución de la conversación.",
  BUDGET_EXCEEDED: "Sube el presupuesto de la conversación y vuelve a enviarlo.",
  PROMPT_ECHOED: "Prueba con otro modelo o permite que el Broker seleccione uno diferente.",
  DEGENERATE_OUTPUT: "Prueba con otro modelo o reduce la longitud solicitada.",
  VRAM_MODEL_TOO_LARGE:
    "Elige un modelo más pequeño o permite proveedores cloud si la clasificación de los datos lo admite."
};

export const taskFailureSummary = (
  error?: Record<string, unknown>
): {
  title: string;
  detail: string;
  retryable: boolean;
  /** Presente cuando el fallo pide una decisión y no un reintento. */
  guidance?: string;
} | undefined => {
  if (!error) return undefined;
  const code = typeof error.code === "string" ? error.code : "TASK_FAILED";
  const detail =
    typeof error.message === "string" && error.message.trim()
      ? error.message.slice(0, 500)
      : "El Broker no proporcionó más detalles sobre el fallo.";
  const guidance = TASK_FAILURE_GUIDANCE[code];
  return {
    title: TASK_FAILURE_TITLES[code] ?? "La tarea no pudo completarse",
    detail,
    // El Broker manda: un fallo marcado como no reintentable no se ofrece como
    // reintentable aunque su código nos resulte familiar.
    retryable: error.retryable === true,
    ...(guidance ? { guidance } : {})
  };
};

/** Estado de la credencial del Broker, sin exponer nunca su valor. */
export type BrokerCredentialStatus = {
  source: "protected" | "environment" | "missing";
  protected: boolean;
  environmentPresent: boolean;
  message: string;
};

/** Etiqueta corta del origen de la credencial en uso. */
export const brokerCredentialLabel = (status: BrokerCredentialStatus): string => {
  switch (status.source) {
    case "protected":
      return "Guardada y cifrada";
    case "environment":
      return "Heredada del entorno";
    default:
      return "Sin credencial";
  }
};

/** Carpeta que la persona autorizó explícitamente para escribir en ella. */
export type AuthorizedFolderView = {
  id: string;
  path: string;
  displayName: string;
  permissions: { write?: boolean; purpose?: string };
  grantedAt: string;
  revokedAt: string | null;
};

/** Nombre legible del uso que motivó la concesión de una carpeta. */
export const authorizedFolderPurpose = (folder: AuthorizedFolderView): string => {
  switch (folder.permissions?.purpose) {
    case "conversation_markdown":
      return "Exportar conversaciones a Markdown";
    case "obsidian_vault":
      return "Bóveda de Obsidian";
    case "scheduled_history":
      return "Historial de automatizaciones";
    case "scheduled_calendar":
      return "Calendario de automatizaciones";
    case "custom_gpt_export":
      return "Exportar GPTs personales";
    default:
      return "Uso no declarado";
  }
};

/** Dato concreto que se enviará si la acción se autoriza. */
export type ConfirmationDisclosedDatum = {
  label: string;
  value: string;
};

/**
 * Expediente durable de una confirmación: lo que la persona debe poder leer
 * antes de decidir y lo que queda registrado después de decidir.
 */
export type ConfirmationRequestView = {
  id: string;
  actionType: string;
  toolName: string | null;
  // `resources` y `disclosure` viajan como JSON tal cual se guardó en SQLite:
  // sus claves internas son las del expediente, no las de la proyección.
  resources: { label?: string; kind?: string; conversation_id?: string | null };
  disclosure: {
    action_label?: string;
    data_sent?: ConfirmationDisclosedDatum[];
    destination?: string;
    destination_label?: string;
    scope?: string;
    scope_label?: string;
  };
  consequences: string;
  status: string;
  requestedAt: string;
  resolvedAt: string | null;
};

export type ToolCallView = {
  toolCallId: string;
  name: string;
  arguments: Record<string, unknown>;
  status: string;
  confirmation: ConfirmationRequestView | null;
};

/** Resumen legible de un expediente, sin exponer JSON técnico. */
export const confirmationSummary = (
  call: ToolCallView
): {
  action: string;
  tool: string;
  resource: string;
  data: ConfirmationDisclosedDatum[];
  destination: string;
  scope: string;
  consequences: string;
} => {
  const confirmation = call.confirmation;
  const disclosure = confirmation?.disclosure ?? {};
  return {
    action:
      disclosure.action_label ??
      (call.name === "rename_conversation" ? "Renombrar la conversación" : call.name),
    tool: confirmation?.toolName ?? call.name,
    resource: confirmation?.resources?.label ?? "Recursos no declarados",
    data: disclosure.data_sent ?? [],
    destination: disclosure.destination_label ?? "Destino no declarado",
    scope: disclosure.scope_label ?? "Permitir una vez",
    consequences:
      confirmation?.consequences ??
      "ChatyGPT no puede anticipar las consecuencias de esta acción."
  };
};

export const isTerminalTask = (task: LocalTaskSnapshot): boolean =>
  ["completed", "failed", "cancelled"].includes(task.remoteStatus);

export const isTaskPollingComplete = (task: LocalTaskSnapshot): boolean =>
  isTerminalTask(task) ||
  ["waiting_for_tools", "orphaned"].includes(task.localState);

/**
 * Si queda algún recuerdo indexándose y por tanto hay que seguir sondeando.
 *
 * Extraído de `App.tsx` (fase 4). La condición de parada de un sondeo es una
 * decisión: equivocarse deja un temporizador vivo para siempre o corta la
 * actualización antes de que el índice esté listo.
 */
export const shouldPollMemoryIndex = (
  items: Pick<MemoryItemView, "embeddingStatus">[]
): boolean => items.some((item) => item.embeddingStatus === "indexing");

/** Si una búsqueda semántica sigue en curso. */
export const shouldPollMemorySearch = (
  search: Pick<MemorySearchView, "status"> | null | undefined
): boolean => search?.status === "searching";

/**
 * Si hay que recargar la conversación al terminar un turno.
 *
 * Solo cuando la conversación abierta es la misma en la que se envió: recargar
 * otra sobreescribiría lo que la persona está leyendo ahora mismo, y el turno
 * terminado ya quedó persistido en la suya.
 */
export const shouldReloadConversationAfterTurn = ({
  turnConversationId,
  openConversationId
}: {
  turnConversationId: string | null;
  openConversationId: string | null;
}): boolean =>
  Boolean(turnConversationId) && turnConversationId === openConversationId;

export const isTaskBlockingConversation = (task: LocalTaskSnapshot): boolean =>
  !isTerminalTask(task) && task.localState !== "orphaned";

export const shouldFollowConversationScroll = ({
  scrollHeight,
  scrollTop,
  clientHeight,
  threshold = 96
}: {
  scrollHeight: number;
  scrollTop: number;
  clientHeight: number;
  threshold?: number;
}): boolean =>
  Math.max(0, scrollHeight - scrollTop - clientHeight) <= threshold;

export const canSendMessage = ({
  hasConversation,
  hasText,
  attachmentsReady,
  attachmentBusy,
  turnBlocking
}: {
  hasConversation: boolean;
  hasText: boolean;
  attachmentsReady: boolean;
  attachmentBusy: boolean;
  turnBlocking: boolean;
}): boolean =>
  hasConversation && hasText && attachmentsReady && !attachmentBusy && !turnBlocking;

export const canUseSemanticMemory = ({
  memoryEnabled,
  hasConversation,
  readyEligibleMemories
}: {
  memoryEnabled: boolean;
  hasConversation: boolean;
  readyEligibleMemories: number;
}): boolean =>
  memoryEnabled && hasConversation && readyEligibleMemories > 0;

export const shouldOfferSandboxForPrompt = (text: string): boolean => {
  const normalized = text
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLowerCase();
  return /\b(ejecuta|ejecutar|ejecutalo|corre|correr|compila|compilar|pruebalo|testea|testear|run|execute|compile)\b/.test(normalized) ||
    /\b(run|execute)\s+(the\s+)?tests?\b/.test(normalized);
};

export type ConversationSummary = {
  id: string;
  title: string;
  projectId?: string;
  updatedAt: string;
};

export type ConversationSummaryRevision = {
  id: string;
  status: "generating" | "draft" | "approved" | "failed" | "cancelled" | "superseded";
  draftText?: string;
  approvedText?: string;
  sourceThroughSequence: number;
  brokerTaskId?: string;
  updatedAt: string;
};

export type ConversationSummaryOverview = {
  candidate?: ConversationSummaryRevision;
  active?: ConversationSummaryRevision;
  totalMessageCount: number;
  activeCoveredMessageCount: number;
  remainingMessageCount: number;
  candidateCoveredMessageCount?: number;
};

export type ProjectSummary = {
  id: string;
  name: string;
  description?: string;
  instructions?: string;
  conversationCount: number;
  updatedAt: string;
};

export type CustomGptIcon = "spark" | "research" | "writing" | "code" | "data" | "teacher" | "briefcase";

export const customGptIconOptions: Array<{ id: CustomGptIcon; glyph: string; label: string }> = [
  { id: "spark", glyph: "✦", label: "General" },
  { id: "research", glyph: "⌕", label: "Investigación" },
  { id: "writing", glyph: "¶", label: "Escritura" },
  { id: "code", glyph: "</>", label: "Código" },
  { id: "data", glyph: "▦", label: "Datos" },
  { id: "teacher", glyph: "A", label: "Tutor" },
  { id: "briefcase", glyph: "◆", label: "Trabajo" }
];

export const customGptIconGlyph = (icon: string | null | undefined): string =>
  customGptIconOptions.find((option) => option.id === icon)?.glyph ?? "✦";

export type CustomGptView = {
  id: string;
  name: string;
  description?: string;
  iconRef: CustomGptIcon;
  instructions: string;
  conversationStarters: string[];
  toolPermissions: {
    runCode: "deny" | "confirm";
    renameConversation: "deny" | "confirm";
  };
  /** Modelo que el Broker intentará primero; null deja decidir al Broker. */
  preferredModel: string | null;
  /** null conserva las opciones elegidas en cada chat. */
  executionProfile: ConversationExecutionPreferences | null;
  /** Proyecto al que van los chats nuevos que eligen este GPT. */
  defaultProjectId: string | null;
  versionNo: number;
  createdAt: string;
  updatedAt: string;
};

/** Lo que recibiría el modelo con este GPT, calculado sin enviar nada. */
export type CustomGptPreview = {
  customGptId: string;
  name: string;
  iconRef: CustomGptIcon;
  versionNo: number;
  /** Texto exacto que se antepone al mensaje en la petición real. */
  promptBlock: string;
  preferredModel: string | null;
  executionProfile: ConversationExecutionPreferences | null;
  defaultProjectName: string | null;
  conversationStarters: string[];
  toolPermissions: CustomGptView["toolPermissions"];
  activeKnowledgeCount: number;
  disabledKnowledgeCount: number;
  sensitiveKnowledgeCount: number;
  unindexedKnowledgeCount: number;
  readyFileCount: number;
  pendingFileCount: number;
  warnings: string[];
};

/** Revisión guardada de un GPT personal. */
export type CustomGptVersionView = {
  id: string;
  versionNo: number;
  iconRef: CustomGptIcon;
  instructions: string;
  conversationStarters: string[];
  preferredModel: string | null;
  executionProfile: ConversationExecutionPreferences | null;
  createdAt: string;
  active: boolean;
  toolPermissions: CustomGptView["toolPermissions"];
  /** Respuestas que quedaron congeladas con esta versión exacta. */
  taskCount: number;
};

/**
 * Explica en una línea qué representa una revisión dentro del historial.
 *
 * Se apoya en `taskCount` para distinguir una revisión que llegó a usarse de
 * otra que se sustituyó antes de enviar nada.
 */
export const customGptVersionSummary = (version: CustomGptVersionView): string => {
  if (version.active) {
    return version.taskCount === 0
      ? "Versión en uso · todavía sin respuestas"
      : `Versión en uso · ${version.taskCount} respuesta(s)`;
  }
  return version.taskCount === 0
    ? "Revisión anterior · no llegó a usarse"
    : `Revisión anterior · ${version.taskCount} respuesta(s) conservan esta versión`;
};

export type CustomGptExportReport = {
  path: string;
  includedKnowledge: number;
  excludedSensitive: number;
  excludedDisabled: number;
  excludedFiles: number;
};

export type CustomGptImportReport = {
  customGpt: CustomGptView;
  importedKnowledge: number;
  knowledgeRequiresReview: boolean;
};

export type ProjectKnowledgeOverview = {
  project: ProjectSummary;
  files: AttachmentView[];
  fileUsages: ProjectFileUsage[];
  memories: MemoryItemView[];
  memoryEnabled: boolean;
};

export type ProjectFileUsage = {
  attachmentId: string;
  conversations: ConversationSummary[];
};

export type ProjectKnowledgeFilter = "all" | "files" | "memories";

export type FilteredProjectKnowledge = {
  files: AttachmentView[];
  memories: MemoryItemView[];
  total: number;
};

const normalizeProjectKnowledgeSearch = (value: string): string =>
  value
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "")
    .toLocaleLowerCase("es");

export const filterProjectKnowledge = (
  overview: ProjectKnowledgeOverview,
  query: string,
  filter: ProjectKnowledgeFilter
): FilteredProjectKnowledge => {
  const needle = normalizeProjectKnowledgeSearch(query.trim());
  const matches = (values: Array<string | undefined>): boolean =>
    !needle || normalizeProjectKnowledgeSearch(values.filter(Boolean).join(" ")).includes(needle);
  const files = filter === "memories"
    ? []
    : overview.files.filter((file) => matches([
        file.displayName,
        file.mediaType,
        file.ingestionStatus,
        file.semanticIndexStatus
      ]));
  const memories = filter === "files"
    ? []
    : overview.memories.filter((item) => matches([
        item.content,
        item.category,
        item.sensitivity,
        item.enabled ? "activo activado" : "desactivado",
        item.embeddingStatus
      ]));
  return {
    files,
    memories,
    total: files.length + memories.length
  };
};

export type ConversationMessage = {
  id: string;
  role: "system" | "user" | "assistant" | "tool" | "error";
  status: "draft" | "pending" | "complete" | "failed" | "cancelled";
  sequenceNo: number;
  brokerTaskId?: string;
  taskRemoteStatus?: string;
  taskLocalState?: string;
  text?: string;
  error?: Record<string, unknown>;
  modelUsed?: {
    provider: string;
    deployment: string;
    model: string;
  };
  responseDurationMs?: number;
  usage?: Record<string, unknown>;
  fallbackUsed?: boolean;
  longContext?: Record<string, unknown>;
  consensusSynthesized?: boolean;
  consensusWarnings?: string[];
  arbiterFailureCount?: number;
  sources: ConversationSource[];
  createdAt: string;
};

export type WorkflowNodeKind = "input" | "custom_gpt" | "prompt" | "approval" | "result";

export type WorkflowNode = {
  id: string;
  kind: WorkflowNodeKind;
  label: string;
  x: number;
  y: number;
  customGptId?: string | null;
  customGptVersionId?: string | null;
  customGptName?: string | null;
  customGptIconRef?: CustomGptIcon | null;
  customGptInstructions?: string | null;
  preferredModel?: string | null;
  executionProfile?: ConversationExecutionPreferences | null;
  /** Conocimiento del GPT seleccionado al publicar; una revocación posterior se respeta. */
  customGptMemoryIds?: string[];
  /** Archivos propios del GPT preparados al publicar. */
  customGptAttachmentIds?: string[];
  instruction?: string | null;
  attachmentIds: string[];
};

export type WorkflowEdge = {
  id: string;
  source: string;
  target: string;
};

export type WorkflowDefinition = {
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
  /** Contexto autorizado del proyecto fijado en la versión publicada. */
  projectContext?: {
    projectId: string;
    projectName: string;
    instructions?: string | null;
    memoryIds: string[];
  } | null;
};

export type WorkflowSummary = {
  id: string;
  name: string;
  description?: string | null;
  projectId?: string | null;
  publishedVersionNo?: number | null;
  nodeCount: number;
  updatedAt: string;
};

export type WorkflowView = WorkflowSummary & {
  definition: WorkflowDefinition;
};

export type WorkflowNodeRunView = {
  id: string;
  nodeId: string;
  nodeKind: WorkflowNodeKind;
  nodeLabel: string;
  status: "pending" | "running" | "waiting_approval" | "completed" | "failed" | "skipped" | "cancelled";
  inputText?: string | null;
  outputText?: string | null;
  brokerTaskId?: string | null;
  error?: Record<string, unknown> | null;
  updatedAt: string;
};

export type WorkflowRunView = {
  id: string;
  workflowId: string;
  workflowVersionId: string;
  versionNo: number;
  status: "queued" | "running" | "waiting_approval" | "completed" | "partial_failed" | "failed" | "cancelled";
  inputText: string;
  outputs: Record<string, string>;
  error?: Record<string, unknown> | null;
  nodeRuns: WorkflowNodeRunView[];
  startedAt?: string | null;
  completedAt?: string | null;
  updatedAt: string;
};

export const brokerAttachmentExtensions = (broker?: BrokerDiagnostic): string[] => {
  if (!broker?.capabilitiesVerified) return [];
  return [...new Set(Object.values(broker.ingestionFormats ?? {})
    .flat()
    .map((extension) => extension.trim().replace(/^\./, "").toLowerCase())
    .filter((extension) => /^[a-z0-9]{1,16}$/.test(extension)))]
    .sort();
};

export const formatResponseUsage = (usage?: Record<string, unknown>): string | null => {
  if (!usage) return null;
  const total = [usage.total_tokens, usage.totalTokens, usage.tokens]
    .find((value) => typeof value === "number" && Number.isFinite(value)) as number | undefined;
  const cost = [usage.cost_usd, usage.costUsd, usage.estimated_cost_usd]
    .find((value) => typeof value === "number" && Number.isFinite(value)) as number | undefined;
  const parts: string[] = [];
  if (total !== undefined) parts.push(`${total.toLocaleString("es-ES")} tokens`);
  if (cost !== undefined) parts.push(`${cost.toLocaleString("es-ES", { maximumFractionDigits: 4 })} USD`);
  return parts.length > 0 ? parts.join(" · ") : null;
};

export const formatResponseDuration = (milliseconds?: number): string | null => {
  if (milliseconds === undefined || !Number.isFinite(milliseconds) || milliseconds < 0) {
    return null;
  }
  if (milliseconds < 60_000) {
    const seconds = Math.max(0.1, Math.round(milliseconds / 100) / 10);
    return `${seconds.toLocaleString("es-ES", { maximumFractionDigits: 1 })} s`;
  }
  const totalSeconds = Math.round(milliseconds / 1000);
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  return `${minutes} min ${seconds} s`;
};

export type ConversationSource = {
  id: string;
  title: string;
  sourceAttachmentId?: string;
  mediaType?: string;
  sizeBytes?: number;
  url?: string;
  quoteText?: string;
  claimText?: string;
};

export type ConversationView = {
  id: string;
  title: string;
  projectId?: string;
  customGptId?: string;
  executionPreferences: ConversationExecutionPreferences;
  messages: ConversationMessage[];
  researchRuns: ResearchRunView[];
};

export type ResearchStepView = {
  id: string;
  kind: "plan" | "research" | "synthesis";
  title: string;
  status: "pending" | "running" | "completed" | "failed" | "cancelled";
};

export type ResearchRunView = {
  id: string;
  brokerTaskId: string;
  objective: string;
  status: "planning" | "researching" | "synthesizing" | "completed" | "failed" | "cancelled";
  steps: ResearchStepView[];
  sourceCount: number;
  createdAt: string;
  updatedAt: string;
};

export type ConversationExecutionPreferences = {
  dataClassification: "public" | "internal" | "confidential" | "local_only";
  strategy: "single" | "auto" | "mixture_of_agents";
  preset: "fast" | "slow";
  maxCostUsd: number;
  longContext: "fail" | "map_reduce";
  priority: number;
};

export type ExportPathSelection = {
  path: string;
  existed: boolean;
};

export type ExportReport = {
  destinationPath: string;
  sourceHash: string;
  destinationHash: string;
  overwritten: boolean;
  format: "markdown" | "obsidian";
  attachmentCount: number;
  reusedAttachmentCount: number;
  projectIndexUpdated: boolean;
  approvedMemoryCount: number;
};
