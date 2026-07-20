import { invoke } from "@tauri-apps/api/core";
import type {
  BootstrapReport,
  AttachmentView,
  BrokerDiagnostic,
  ConversationSummary,
  ConversationView,
  LocalTaskSnapshot,
  ProjectSummary
} from "./domain";

export const platform = {
  bootstrap(): Promise<BootstrapReport> {
    return invoke<BootstrapReport>("bootstrap_app");
  },
  diagnoseBroker(): Promise<BrokerDiagnostic> {
    return invoke<BrokerDiagnostic>("diagnose_broker");
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
  sendChatTurn(
    conversationId: string,
    text: string,
    attachmentIds: string[]
  ): Promise<LocalTaskSnapshot> {
    return invoke<LocalTaskSnapshot>("send_chat_turn", { conversationId, text, attachmentIds });
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
