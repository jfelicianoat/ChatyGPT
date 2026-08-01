import { invoke } from "@tauri-apps/api/core";
import type {
  BootstrapReport,
  AttachmentView,
  AuditEventView,
  AuthorizedFolderView,
  BrokerCredentialStatus,
  BrokerDiagnostic,
  ContextSnapshotView,
  ConversationSummary,
  ConversationSummaryOverview,
  ConversationExecutionPreferences,
  ConversationView,
  CustomGptExportReport,
  CustomGptImportReport,
  CustomGptView,
  ExportPathSelection,
  ExportReport,
  LocalTaskSnapshot,
  MemoryItemView,
  MemoryOverview,
  MemorySearchView,
  ProjectKnowledgeOverview,
  ProjectSummary,
  ScheduledHistoryExportReport,
  ScheduledCalendarExportEntry,
  ScheduledCalendarExportReport,
  ScheduledHistoryPeriodFilter,
  ScheduledHistorySort,
  ScheduledHistoryStatusFilter,
  ScheduledRunPageView,
  ScheduledTaskTemplateView,
  ScheduledTaskView,
  WindowsStartupStatus
} from "./domain";

export const platform = {
  bootstrap(): Promise<BootstrapReport> {
    return invoke<BootstrapReport>("bootstrap_app");
  },
  diagnoseBroker(): Promise<BrokerDiagnostic> {
    return invoke<BrokerDiagnostic>("diagnose_broker");
  },
  getWindowsStartupStatus(): Promise<WindowsStartupStatus> {
    return invoke<WindowsStartupStatus>("get_windows_startup_status");
  },
  setWindowsStartupEnabled(enabled: boolean): Promise<WindowsStartupStatus> {
    return invoke<WindowsStartupStatus>("set_windows_startup_enabled", {
      enabled,
      confirmed: enabled
    });
  },
  listAuditEvents(): Promise<AuditEventView[]> {
    return invoke<AuditEventView[]>("list_audit_events");
  },
  getBrokerCredential(): Promise<BrokerCredentialStatus> {
    return invoke<BrokerCredentialStatus>("get_broker_credential");
  },
  setBrokerCredential(token: string): Promise<BrokerCredentialStatus> {
    return invoke<BrokerCredentialStatus>("set_broker_credential", { token });
  },
  clearBrokerCredential(): Promise<BrokerCredentialStatus> {
    return invoke<BrokerCredentialStatus>("clear_broker_credential", {
      confirmed: true
    });
  },
  listAuthorizedFolders(): Promise<AuthorizedFolderView[]> {
    return invoke<AuthorizedFolderView[]>("list_authorized_folders");
  },
  revokeAuthorizedFolder(folderId: string): Promise<AuthorizedFolderView[]> {
    return invoke<AuthorizedFolderView[]>("revoke_authorized_folder", {
      folderId,
      confirmed: true
    });
  },
  getMemoryOverview(): Promise<MemoryOverview> {
    return invoke<MemoryOverview>("get_memory_overview");
  },
  getCustomGptKnowledge(customGptId: string): Promise<MemoryItemView[]> {
    return invoke<MemoryItemView[]>("get_custom_gpt_knowledge", { customGptId });
  },
  listCustomGptFiles(customGptId: string): Promise<AttachmentView[]> {
    return invoke<AttachmentView[]>("list_custom_gpt_files", { customGptId });
  },
  importCustomGptFile(
    customGptId: string,
    sourcePath: string
  ): Promise<AttachmentView> {
    return invoke<AttachmentView>("import_custom_gpt_file", {
      customGptId,
      sourcePath
    });
  },
  removeCustomGptFile(
    customGptId: string,
    attachmentId: string
  ): Promise<AttachmentView[]> {
    return invoke<AttachmentView[]>("remove_custom_gpt_file", {
      customGptId,
      attachmentId,
      confirmed: true
    });
  },
  createCustomGptKnowledgeItem(
    customGptId: string,
    content: string,
    category: "preference" | "instruction" | "fact",
    sensitivity: "normal" | "sensitive"
  ): Promise<MemoryItemView[]> {
    return invoke<MemoryItemView[]>("create_custom_gpt_knowledge_item", {
      customGptId,
      content,
      category,
      sensitivity
    });
  },
  setCustomGptKnowledgeItemEnabled(
    customGptId: string,
    memoryId: string,
    enabled: boolean
  ): Promise<MemoryItemView[]> {
    return invoke<MemoryItemView[]>("set_custom_gpt_knowledge_item_enabled", {
      customGptId,
      memoryId,
      enabled
    });
  },
  deleteCustomGptKnowledgeItem(
    customGptId: string,
    memoryId: string
  ): Promise<MemoryItemView[]> {
    return invoke<MemoryItemView[]>("delete_custom_gpt_knowledge_item", {
      customGptId,
      memoryId,
      confirmed: true
    });
  },
  reindexCustomGptKnowledgeItem(
    customGptId: string,
    memoryId: string
  ): Promise<MemoryItemView[]> {
    return invoke<MemoryItemView[]>("reindex_custom_gpt_knowledge_item", {
      customGptId,
      memoryId
    });
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
  updateMemoryItem(
    memoryId: string,
    content: string,
    category: "preference" | "instruction" | "fact",
    sensitivity: "normal" | "sensitive",
    projectId?: string
  ): Promise<MemoryOverview> {
    return invoke<MemoryOverview>("update_memory_item", {
      memoryId,
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
  listScheduledTasks(): Promise<ScheduledTaskView[]> {
    return invoke<ScheduledTaskView[]>("list_scheduled_tasks");
  },
  listScheduledRuns(
    scheduledTaskId: string,
    statusFilter: ScheduledHistoryStatusFilter,
    periodFilter: ScheduledHistoryPeriodFilter,
    sort: ScheduledHistorySort,
    page: number,
    pageSize: ScheduledRunPageView["pageSize"]
  ): Promise<ScheduledRunPageView> {
    return invoke<ScheduledRunPageView>("list_scheduled_runs", {
      scheduledTaskId,
      statusFilter,
      periodFilter,
      sort,
      page,
      pageSize
    });
  },
  listScheduledTaskTemplates(): Promise<ScheduledTaskTemplateView[]> {
    return invoke<ScheduledTaskTemplateView[]>("list_scheduled_task_templates");
  },
  createScheduledTaskTemplate(
    name: string,
    prompt: string,
    scheduleExpression: ScheduledTaskView["scheduleExpression"]
  ): Promise<ScheduledTaskTemplateView> {
    return invoke<ScheduledTaskTemplateView>("create_scheduled_task_template", {
      name,
      prompt,
      scheduleExpression
    });
  },
  deleteScheduledTaskTemplate(scheduledTaskTemplateId: string): Promise<void> {
    return invoke<void>("delete_scheduled_task_template", {
      scheduledTaskTemplateId,
      confirmed: true
    });
  },
  createScheduledTask(
    name: string,
    conversationId: string,
    prompt: string,
    dueAt: string,
    timezone: string,
    scheduleExpression: ScheduledTaskView["scheduleExpression"]
  ): Promise<ScheduledTaskView> {
    return invoke<ScheduledTaskView>("create_scheduled_task", {
      name,
      conversationId,
      prompt,
      dueAt,
      timezone,
      scheduleExpression,
      confirmed: true
    });
  },
  setScheduledTaskEnabled(
    scheduledTaskId: string,
    enabled: boolean
  ): Promise<ScheduledTaskView> {
    return invoke<ScheduledTaskView>("set_scheduled_task_enabled", {
      scheduledTaskId,
      enabled,
      confirmed: enabled
    });
  },
  updateScheduledTask(
    scheduledTaskId: string,
    name: string,
    conversationId: string,
    prompt: string,
    dueAt: string,
    timezone: string,
    scheduleExpression: ScheduledTaskView["scheduleExpression"]
  ): Promise<ScheduledTaskView> {
    return invoke<ScheduledTaskView>("update_scheduled_task", {
      scheduledTaskId,
      name,
      conversationId,
      prompt,
      dueAt,
      timezone,
      scheduleExpression,
      confirmed: true
    });
  },
  deleteScheduledTask(scheduledTaskId: string): Promise<void> {
    return invoke<void>("delete_scheduled_task", {
      scheduledTaskId,
      confirmed: true
    });
  },
  retryScheduledRun(scheduledRunId: string): Promise<ScheduledTaskView> {
    return invoke<ScheduledTaskView>("retry_scheduled_run", {
      scheduledRunId,
      confirmed: true
    });
  },
  runScheduledTaskNow(scheduledTaskId: string): Promise<ScheduledTaskView> {
    return invoke<ScheduledTaskView>("run_scheduled_task_now", {
      scheduledTaskId,
      confirmed: true
    });
  },
  cancelScheduledRun(scheduledRunId: string): Promise<ScheduledTaskView> {
    return invoke<ScheduledTaskView>("cancel_scheduled_run", {
      scheduledRunId,
      confirmed: true
    });
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
  updateConversationExecutionPreferences(
    conversationId: string,
    preferences: ConversationExecutionPreferences
  ): Promise<ConversationExecutionPreferences> {
    return invoke<ConversationExecutionPreferences>(
      "update_conversation_execution_preferences",
      { conversationId, preferences }
    );
  },
  getConversationSummary(conversationId: string): Promise<ConversationSummaryOverview> {
    return invoke<ConversationSummaryOverview>("get_conversation_summary", { conversationId });
  },
  startConversationSummary(conversationId: string): Promise<ConversationSummaryOverview> {
    return invoke<ConversationSummaryOverview>("start_conversation_summary", { conversationId });
  },
  updateConversationSummary(summaryId: string, text: string): Promise<ConversationSummaryOverview> {
    return invoke<ConversationSummaryOverview>("update_conversation_summary", { summaryId, text });
  },
  approveConversationSummary(summaryId: string): Promise<ConversationSummaryOverview> {
    return invoke<ConversationSummaryOverview>("approve_conversation_summary", { summaryId });
  },
  getTaskContext(localTaskId: string): Promise<ContextSnapshotView> {
    return invoke<ContextSnapshotView>("get_task_context", { localTaskId });
  },
  revealContextSource(localTaskId: string, sourceReference: string): Promise<string> {
    return invoke<string>("reveal_context_source", { localTaskId, sourceReference });
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
  setConversationCustomGpt(
    conversationId: string,
    customGptId?: string
  ): Promise<ConversationView> {
    return invoke<ConversationView>("set_conversation_custom_gpt", {
      conversationId,
      customGptId
    });
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
  listCustomGpts(): Promise<CustomGptView[]> {
    return invoke<CustomGptView[]>("list_custom_gpts");
  },
  createCustomGpt(
    name: string,
    description: string,
    instructions: string,
    conversationStarters: string[],
    toolPermissions: CustomGptView["toolPermissions"]
  ): Promise<CustomGptView> {
    return invoke<CustomGptView>("create_custom_gpt", {
      name,
      description: description.trim() || undefined,
      instructions,
      conversationStarters,
      toolPermissions
    });
  },
  updateCustomGpt(
    customGptId: string,
    name: string,
    description: string,
    instructions: string,
    conversationStarters: string[],
    toolPermissions: CustomGptView["toolPermissions"]
  ): Promise<CustomGptView> {
    return invoke<CustomGptView>("update_custom_gpt", {
      customGptId,
      name,
      description: description.trim() || undefined,
      instructions,
      conversationStarters,
      toolPermissions
    });
  },
  pickCustomGptImportPath(): Promise<string | null> {
    return invoke<string | null>("pick_custom_gpt_import_path");
  },
  pickCustomGptExportPath(suggestedName: string): Promise<string | null> {
    return invoke<string | null>("pick_custom_gpt_export_path", { suggestedName });
  },
  importCustomGpt(sourcePath: string): Promise<CustomGptImportReport> {
    return invoke<CustomGptImportReport>("import_custom_gpt", { sourcePath });
  },
  exportCustomGpt(
    customGptId: string,
    destinationPath: string,
    includeKnowledge: boolean
  ): Promise<CustomGptExportReport> {
    return invoke<CustomGptExportReport>("export_custom_gpt", {
      customGptId,
      destinationPath,
      includeKnowledge
    });
  },
  getProjectKnowledge(projectId: string): Promise<ProjectKnowledgeOverview> {
    return invoke<ProjectKnowledgeOverview>("get_project_knowledge", { projectId });
  },
  removeProjectFile(projectId: string, attachmentId: string): Promise<ProjectKnowledgeOverview> {
    return invoke<ProjectKnowledgeOverview>("remove_project_file", {
      projectId,
      attachmentId,
      confirmed: true
    });
  },
  setProjectMemoryItemEnabled(
    projectId: string,
    memoryId: string,
    enabled: boolean
  ): Promise<ProjectKnowledgeOverview> {
    return invoke<ProjectKnowledgeOverview>("set_project_memory_item_enabled", {
      projectId,
      memoryId,
      enabled
    });
  },
  renameProject(projectId: string, name: string): Promise<ProjectSummary> {
    return invoke<ProjectSummary>("rename_project", { projectId, name });
  },
  updateProjectInstructions(projectId: string, instructions: string): Promise<ProjectSummary> {
    return invoke<ProjectSummary>("update_project_instructions", {
      projectId,
      instructions
    });
  },
  archiveProject(projectId: string): Promise<void> {
    return invoke<void>("archive_project", { projectId, confirmed: true });
  },
  pickExportPath(suggestedName: string): Promise<ExportPathSelection | null> {
    return invoke<ExportPathSelection | null>("pick_export_path", { suggestedName });
  },
  pickScheduledHistoryExportPath(): Promise<ExportPathSelection | null> {
    return invoke<ExportPathSelection | null>("pick_scheduled_history_export_path");
  },
  pickScheduledCalendarExportPath(): Promise<ExportPathSelection | null> {
    return invoke<ExportPathSelection | null>("pick_scheduled_calendar_export_path");
  },
  exportScheduledHistory(
    destinationPath: string,
    statusFilter: ScheduledHistoryStatusFilter,
    periodFilter: ScheduledHistoryPeriodFilter,
    overwriteConfirmed: boolean
  ): Promise<ScheduledHistoryExportReport> {
    return invoke<ScheduledHistoryExportReport>("export_scheduled_history", {
      destinationPath,
      statusFilter,
      periodFilter,
      overwriteConfirmed
    });
  },
  exportScheduledCalendar(
    destinationPath: string,
    entries: ScheduledCalendarExportEntry[],
    rangeDays: 7 | 14 | 30,
    overwriteConfirmed: boolean
  ): Promise<ScheduledCalendarExportReport> {
    return invoke<ScheduledCalendarExportReport>("export_scheduled_calendar", {
      destinationPath,
      entries,
      rangeDays,
      overwriteConfirmed
    });
  },
  pickObsidianVault(): Promise<string | null> {
    return invoke<string | null>("pick_obsidian_vault");
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
  exportConversationToObsidian(
    conversationId: string,
    vaultPath: string,
    overwriteConfirmed: boolean
  ): Promise<ExportReport> {
    return invoke<ExportReport>("export_conversation_to_obsidian", {
      conversationId,
      vaultPath,
      overwriteConfirmed
    });
  },
  sendChatTurn(
    conversationId: string,
    text: string,
    attachmentIds: string[],
    toolsEnabled: boolean,
    sandboxEnabled: boolean,
    semanticMemoryEnabled: boolean,
    researchMode: boolean
  ): Promise<LocalTaskSnapshot> {
    return invoke<LocalTaskSnapshot>("send_chat_turn", {
      conversationId,
      text,
      attachmentIds,
      toolsEnabled,
      sandboxEnabled,
      semanticMemoryEnabled,
      researchMode
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
  importCapturedImage(
    conversationId: string,
    displayName: string,
    bytes: number[]
  ): Promise<AttachmentView> {
    return invoke<AttachmentView>("import_captured_image", {
      conversationId,
      displayName,
      bytes
    });
  },
  listAttachments(conversationId: string): Promise<AttachmentView[]> {
    return invoke<AttachmentView[]>("list_attachments", { conversationId });
  },
  listProjectFiles(conversationId: string): Promise<AttachmentView[]> {
    return invoke<AttachmentView[]>("list_project_files", { conversationId });
  },
  setProjectFile(
    conversationId: string,
    attachmentId: string,
    enabled: boolean
  ): Promise<AttachmentView[]> {
    return invoke<AttachmentView[]>("set_project_file", {
      conversationId,
      attachmentId,
      enabled
    });
  },
  useProjectFile(conversationId: string, attachmentId: string): Promise<AttachmentView[]> {
    return invoke<AttachmentView[]>("use_project_file", { conversationId, attachmentId });
  },
  removeAttachment(conversationId: string, attachmentId: string): Promise<void> {
    return invoke<void>("remove_attachment", { conversationId, attachmentId });
  },
  retryAttachment(attachmentId: string): Promise<AttachmentView> {
    return invoke<AttachmentView>("retry_attachment", { attachmentId });
  },
  retryAttachmentContext(attachmentId: string): Promise<AttachmentView> {
    return invoke<AttachmentView>("retry_attachment_context", { attachmentId });
  },
  retryAttachmentSemanticIndex(attachmentId: string): Promise<AttachmentView> {
    return invoke<AttachmentView>("retry_attachment_semantic_index", { attachmentId });
  }
};
