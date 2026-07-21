export const TASK_STATUSES = [
  "queued",
  "routing",
  "planning",
  "resource_planning",
  "chunking",
  "generating",
  "proposing",
  "evaluating",
  "debating",
  "synthesizing",
  "verifying",
  "waiting_for_tools",
  "completed",
  "failed",
  "cancelled"
] as const;

export type BrokerTaskStatus = (typeof TASK_STATUSES)[number];

export type BootstrapReport = {
  appVersion: string;
  databasePath: string;
  schemaVersion: number;
  recoveredTasks: number;
  recoveredAttachments: number;
  recoveryItems: RecoveryItemView[];
};

export type RecoveryItemView = {
  kind: "task";
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
  updatedAt: string;
};

export type BrokerDiagnostic = {
  reachable: boolean;
  ready: boolean;
  baseUrl: string;
  contractVersion?: string;
  strategies: string[];
  sandboxRunCode?: boolean;
  fileIngestion?: boolean;
  latencyMs: number;
  message: string;
};

export type AuditEventView = {
  id: number;
  category: "project" | "conversation" | "attachment" | "task" | "tool" | "export" | "system";
  summary: string;
  severity: "info" | "warning" | "error";
  actor: string;
  conversationTitle?: string;
  occurredAt: string;
};

export type LocalTaskSnapshot = {
  id: string;
  remoteTaskId?: string;
  remoteStatus: string;
  localState: string;
  consecutivePollErrors: number;
  result?: Record<string, unknown>;
  error?: Record<string, unknown>;
  pendingToolCalls: ToolCallView[];
  updatedAt: string;
};

export type ToolCallView = {
  toolCallId: string;
  name: string;
  arguments: Record<string, unknown>;
  status: string;
};

export const isTerminalTask = (task: LocalTaskSnapshot): boolean =>
  ["completed", "failed", "cancelled"].includes(task.remoteStatus);

export const isTaskPollingComplete = (task: LocalTaskSnapshot): boolean =>
  isTerminalTask(task) ||
  ["waiting_for_tools", "orphaned"].includes(task.localState);

export const isTaskBlockingConversation = (task: LocalTaskSnapshot): boolean =>
  !isTerminalTask(task) && task.localState !== "orphaned";

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

export type ConversationSummary = {
  id: string;
  title: string;
  projectId?: string;
  updatedAt: string;
};

export type ProjectSummary = {
  id: string;
  name: string;
  description?: string;
  conversationCount: number;
  updatedAt: string;
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
  sources: ConversationSource[];
  createdAt: string;
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
  messages: ConversationMessage[];
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
};
