/** Mensajes, acciones de API, workflows y vistas de conversacion. */
import type { BrokerDiagnostic } from "./adjuntos";
import type { CustomGptIcon, CustomGptView } from "./conversacion";

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
  executionWarnings?: string[];
  unsupportedCitationUrls?: string[];
  sources: ConversationSource[];
  createdAt: string;
};

export type CustomGptApiActionPreview = {
  finalUrl: string;
  destination: string;
  method: "GET";
  dataSent: Array<{ name: string; value: unknown }>;
};

export type CustomGptApiActionTestResult = {
  finalUrl: string;
  destination: string;
  status: number;
  contentType?: string;
  body: string;
  truncated: boolean;
  durationMs: number;
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
  contextProfile?: CustomGptView["contextProfile"];
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
