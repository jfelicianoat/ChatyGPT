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

export type LocalTaskSnapshot = {
  id: string;
  remoteTaskId?: string;
  remoteStatus: string;
  localState: string;
  consecutivePollErrors: number;
  result?: Record<string, unknown>;
  error?: Record<string, unknown>;
  updatedAt: string;
};

export const isTerminalTask = (task: LocalTaskSnapshot): boolean =>
  ["completed", "failed", "cancelled"].includes(task.remoteStatus);

export const isTaskPollingComplete = (task: LocalTaskSnapshot): boolean =>
  isTerminalTask(task) ||
  ["waiting_for_tools", "orphaned"].includes(task.localState);

export const isTaskBlockingConversation = (task: LocalTaskSnapshot): boolean =>
  !isTerminalTask(task) && task.localState !== "orphaned";

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
  createdAt: string;
};

export type ConversationView = {
  id: string;
  title: string;
  projectId?: string;
  messages: ConversationMessage[];
};
