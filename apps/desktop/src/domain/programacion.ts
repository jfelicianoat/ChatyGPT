/** Estados de una tarea del Broker, arranque, rendimiento y programacion. */
import type { RecoveryItemView } from "./adjuntos";
import type { MemoryItemView } from "./memoria";
import type { ConversationSummary } from "./conversacion";

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
  "waiting_for_dependencies",
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
  metric:
    | "app_start"
    | "conversation_open"
    | "conversation_search"
    | "remote_operation_start"
    | "ui_response";
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
