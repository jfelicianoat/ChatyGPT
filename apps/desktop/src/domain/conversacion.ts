/** Confirmaciones, guardas de sondeo, resumenes, proyectos y GPTs. */
import type { AttachmentView } from "./adjuntos";
import type { LocalTaskSnapshot, MemoryItemView, MemorySearchView } from "./memoria";
import type { ConversationExecutionPreferences, ConversationMessage } from "./workflows";

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
      (call.name === "rename_conversation"
        ? "Renombrar la conversación"
        : call.name === "list_authorized_folders"
          ? "Listar una carpeta autorizada"
          : call.name === "read_authorized_file"
            ? "Leer un archivo autorizado"
            : call.name === "replace_authorized_file"
              ? "Reemplazar un archivo autorizado"
              : call.name === "create_scheduled_task"
                ? "Crear una tarea programada"
                : call.name),
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
 * Detecta el breve relevo entre una tarea auxiliar ya terminada y la tarea
 * final del chat. Mientras el mensaje siga pendiente y aún apunte a la tarea
 * terminal, la conversación debe recargarse para descubrir el nuevo vínculo.
 */
export const shouldReconcilePendingTurn = ({
  task,
  messages
}: {
  task: LocalTaskSnapshot;
  messages: Pick<ConversationMessage, "status" | "brokerTaskId">[];
}): boolean =>
  isTerminalTask(task) &&
  messages.some(
    (message) => message.status === "pending" && message.brokerTaskId === task.id
  );

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

/**
 * Ventana progresiva de un historial ya cargado.
 *
 * Conserva siempre el extremo reciente y solo amplía hacia atrás. La función
 * es pura para que el render no pueda perder, reordenar ni duplicar mensajes.
 */
export const progressiveConversationWindow = <T>(
  items: readonly T[],
  visibleLimit: number
): { visibleItems: readonly T[]; hiddenCount: number } => {
  const normalizedLimit = Math.max(1, Math.floor(visibleLimit));
  const hiddenCount = Math.max(0, items.length - normalizedLimit);
  return {
    visibleItems: items.slice(hiddenCount),
    hiddenCount
  };
};

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

export type CustomGptApiAction = {
  name: string;
  description: string;
  url: string;
  queryParameters?: string[];
  parameters: Array<{
    name: string;
    type: "string" | "number" | "boolean";
    required: boolean;
    location: "query" | "path";
    description?: string;
  }>;
  credentialRef?: string;
  authMode: "none" | "bearer" | "api_key";
};

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
    readAuthorizedFolders: "deny" | "confirm";
    modifyAuthorizedFiles: "deny" | "confirm";
    createScheduledTasks: "deny" | "confirm";
    callExternalApis: "deny" | "confirm";
  };
  /** Modelo que el Broker intentará primero; null deja decidir al Broker. */
  preferredModel: string | null;
  /** null conserva las opciones elegidas en cada chat. */
  executionProfile: ConversationExecutionPreferences | null;
  /** Cantidad de historial, recuerdos y fragmentos documentales que puede usar. */
  contextProfile: "focused" | "balanced" | "broad";
  apiActions: CustomGptApiAction[];
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
  contextProfile: CustomGptView["contextProfile"];
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
  contextProfile: CustomGptView["contextProfile"];
  apiActions: CustomGptView["apiActions"];
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
