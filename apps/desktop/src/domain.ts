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

