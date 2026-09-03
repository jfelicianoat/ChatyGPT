/** Auditoria, memoria, contexto de un turno, credenciales y carpetas. */
import type { ToolCallView } from "./conversacion";

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
  waiting_for_dependencies: "Esperando a que termine el índice documental",
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

export type ApiCredentialStatus = {
  name: string;
  protected: boolean;
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
  permissions: {
    write?: boolean;
    read?: boolean;
    modify?: boolean;
    athena?: boolean;
    purpose?: string;
  };
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
    case "gpt_read":
      return "Lectura solicitada por GPTs personales";
    case "gpt_modify":
      return "Modificación confirmada por GPTs personales";
    case "athena_workspace":
      return "Espacio de trabajo de Athena";
    default:
      return "Uso no declarado";
  }
};
