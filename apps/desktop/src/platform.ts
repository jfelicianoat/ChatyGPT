import { invoke } from "@tauri-apps/api/core";
import type {
  BootstrapReport,
  AttachmentView,
  AuditEventView,
  BrokerDiagnostic,
  ConversationSummary,
  ConversationView,
  ExportPathSelection,
  ExportReport,
  LocalTaskSnapshot,
  MemoryOverview,
  MemorySearchView,
  ProjectSummary
} from "./domain";

export const platform = {
  bootstrap(): Promise<BootstrapReport> {
    return invoke<BootstrapReport>("bootstrap_app");
  },
  diagnoseBroker(): Promise<BrokerDiagnostic> {
    return invoke<BrokerDiagnostic>("diagnose_broker");
  },
  listAuditEvents(): Promise<AuditEventView[]> {
    return invoke<AuditEventView[]>("list_audit_events");
  },
  getMemoryOverview(): Promise<MemoryOverview> {
    return invoke<MemoryOverview>("get_memory_overview");
  },
  setMemoryEnabled(enabled: boolean): Promise<MemoryOverview> {
    return invoke<MemoryOverview>("set_memory_enabled", { enabled });
  },
  createMemoryItem(
    content: string,
    category: "preference" | "instruction" | "fact",
    sensitivity: "normal" | "sensitive",
    projectId?: string
  ): Promise<MemoryOverview> {
    return invoke<MemoryOverview>("create_memory_item", {
      content,
      category,
      sensitivity,
      projectId
    });
  },
  setMemoryItemEnabled(memoryId: string, enabled: boolean): Promise<MemoryOverview> {
    return invoke<MemoryOverview>("set_memory_item_enabled", { memoryId, enabled });
  },
  deleteMemoryItem(memoryId: string): Promise<MemoryOverview> {
    return invoke<MemoryOverview>("delete_memory_item", { memoryId, confirmed: true });
  },
  reindexMemoryItem(memoryId: string): Promise<MemoryOverview> {
    return invoke<MemoryOverview>("reindex_memory_item", { memoryId });
  },
  startMemorySearch(query: string, projectId?: string): Promise<MemorySearchView> {
    return invoke<MemorySearchView>("start_memory_search", { query, projectId });
  },
  getMemorySearch(searchId: string): Promise<MemorySearchView> {
    return invoke<MemorySearchView>("get_memory_search", { searchId });
  },
  getLatestMemorySearch(): Promise<MemorySearchView | null> {
    return invoke<MemorySearchView | null>("get_latest_memory_search");
  },
  startSmokeTask(): Promise<LocalTaskSnapshot> {
    return invoke<LocalTaskSnapshot>("start_smoke_task");
  },
  getLocalTask(localTaskId: string): Promise<LocalTaskSnapshot> {
    return invoke<LocalTaskSnapshot>("get_local_task", { localTaskId });
  },
  cancelLocalTask(localTaskId: string): Promise<LocalTaskSnapshot> {
    return invoke<LocalTaskSnapshot>("cancel_local_task", { localTaskId });
  },
  createConversation(title?: string, projectId?: string): Promise<ConversationSummary> {
    return invoke<ConversationSummary>("create_conversation", { title, projectId });
  },
  listConversations(): Promise<ConversationSummary[]> {
    return invoke<ConversationSummary[]>("list_conversations");
  },
  getConversation(conversationId: string): Promise<ConversationView> {
    return invoke<ConversationView>("get_conversation", { conversationId });
  },
  searchConversations(query: string): Promise<ConversationSummary[]> {
    return invoke<ConversationSummary[]>("search_conversations", { query });
  },
  renameConversation(conversationId: string, title: string): Promise<ConversationSummary> {
    return invoke<ConversationSummary>("rename_conversation", { conversationId, title });
  },
  moveConversation(conversationId: string, projectId?: string): Promise<ConversationSummary> {
    return invoke<ConversationSummary>("move_conversation", { conversationId, projectId });
  },
  archiveConversation(conversationId: string): Promise<void> {
    return invoke<void>("archive_conversation", { conversationId, confirmed: true });
  },
  deleteConversation(conversationId: string): Promise<void> {
    return invoke<void>("delete_conversation", { conversationId, confirmed: true });
  },
  createProject(name: string, description?: string): Promise<ProjectSummary> {
    return invoke<ProjectSummary>("create_project", { name, description });
  },
  listProjects(): Promise<ProjectSummary[]> {
    return invoke<ProjectSummary[]>("list_projects");
  },
  renameProject(projectId: string, name: string): Promise<ProjectSummary> {
    return invoke<ProjectSummary>("rename_project", { projectId, name });
  },
  archiveProject(projectId: string): Promise<void> {
    return invoke<void>("archive_project", { projectId, confirmed: true });
  },
  pickExportPath(suggestedName: string): Promise<ExportPathSelection | null> {
    return invoke<ExportPathSelection | null>("pick_export_path", { suggestedName });
  },
  exportConversation(
    conversationId: string,
    destinationPath: string,
    overwriteConfirmed: boolean
  ): Promise<ExportReport> {
    return invoke<ExportReport>("export_conversation", {
      conversationId,
      destinationPath,
      overwriteConfirmed
    });
  },
  sendChatTurn(
    conversationId: string,
    text: string,
    attachmentIds: string[],
    toolsEnabled: boolean,
    sandboxEnabled: boolean
  ): Promise<LocalTaskSnapshot> {
    return invoke<LocalTaskSnapshot>("send_chat_turn", {
      conversationId,
      text,
      attachmentIds,
      toolsEnabled,
      sandboxEnabled
    });
  },
  resolveToolCalls(
    localTaskId: string,
    decisions: Array<{ toolCallId: string; approved: boolean }>
  ): Promise<LocalTaskSnapshot> {
    return invoke<LocalTaskSnapshot>("resolve_tool_calls", { localTaskId, decisions });
  },
  pickAttachmentPaths(): Promise<string[]> {
    return invoke<string[]>("pick_attachment_paths");
  },
  importAttachment(conversationId: string, sourcePath: string): Promise<AttachmentView> {
    return invoke<AttachmentView>("import_attachment", { conversationId, sourcePath });
  },
  listAttachments(conversationId: string): Promise<AttachmentView[]> {
    return invoke<AttachmentView[]>("list_attachments", { conversationId });
  },
  removeAttachment(conversationId: string, attachmentId: string): Promise<void> {
    return invoke<void>("remove_attachment", { conversationId, attachmentId });
  },
  retryAttachment(attachmentId: string): Promise<AttachmentView> {
    return invoke<AttachmentView>("retry_attachment", { attachmentId });
  }
};
