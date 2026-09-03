/** Adjuntos, sus fallos y lo que el Broker dice poder aceptar. */

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
  agentSkillsEgress?: string[];
  taskDependencies?: boolean;
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
