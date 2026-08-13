import { describe, expect, it } from "vitest";
import {
  attachmentFailureGuidance,
  attachmentContextSummary,
  attachmentNeedsSandbox,
  attachmentSelectionOnConversationOpen,
  attachmentStatusLabel,
  authorizedFolderPurpose,
  brokerCredentialLabel,
  brokerAttachmentExtensions,
  brokerSupportsPreset,
  canSendMessage,
  canStartMemoryEdit,
  canUseSemanticMemory,
  canRevealContextSource,
  confirmationSummary,
  customGptIconGlyph,
  customGptVersionSummary,
  formatResponseDuration,
  formatResponseUsage,
  filterProjectKnowledge,
  filterScheduledRuns,
  filterScheduledTasks,
  isTaskBlockingConversation,
  isTaskPollingComplete,
  isTerminalTask,
  memoryUpdateNotice,
  projectFilesAvailableToConversation,
  shouldApplyContextLoad,
  shouldFollowConversationScroll,
  shouldOfferSandboxForPrompt,
  shouldRefreshSandboxDiagnostic,
  sandboxUnavailableGuidance,
  scheduledCalendarOccurrences,
  scheduledNotifications,
  scheduledRunDetail,
  scheduledTaskDuplicateDraft,
  taskFailureSummary,
  taskProgressSummary,
  type CustomGptVersionView,
  type ToolCallView,
  type LocalTaskSnapshot,
  type ProjectKnowledgeOverview,
  type ScheduledTaskView
} from "./domain";

describe("contrato 2.7 del Broker", () => {
  it("normaliza y limita el selector a los formatos de ingesta anunciados", () => {
    expect(brokerAttachmentExtensions({
      reachable: true,
      ready: true,
      baseUrl: "http://broker",
      capabilitiesVerified: true,
      ingestionFormats: { documents: [".PDF", "docx"], tabular: ["csv", "../exe"] },
      strategies: [],
      presets: {},
      workLanes: [],
      agentSkills: [],
      latencyMs: 1,
      message: "listo"
    })).toEqual(["csv", "docx", "pdf"]);
  });

  it("presenta el uso nuevo sin depender de un único estilo de claves", () => {
    expect(formatResponseUsage({ total_tokens: 1234, cost_usd: 0.0123 }))
      .toBe("1234 tokens · 0,0123 USD");
  });
});

describe("response duration", () => {
  it("keeps short and long response times readable", () => {
    expect(formatResponseDuration()).toBeNull();
    expect(formatResponseDuration(12_500)).toBe("12,5 s");
    expect(formatResponseDuration(64_000)).toBe("1 min 4 s");
  });
});

describe("scheduled notifications", () => {
  it("builds a readable agenda from durable next dates and recurring projections", () => {
    const base: ScheduledTaskView = {
      id: "daily",
      name: "Informe diario",
      conversationId: "conversation-1",
      conversationTitle: "Seguimiento",
      prompt: "Resume las novedades.",
      scheduleExpression: "daily",
      timezone: "Atlantic/Canary",
      enabled: true,
      nextRunAt: "2026-08-01T10:00:00.000Z",
      createdAt: "2026-07-31T09:00:00.000Z",
      updatedAt: "2026-07-31T09:00:00.000Z",
      runs: []
    };
    const paused = { ...base, id: "paused", enabled: false };

    const agenda = scheduledCalendarOccurrences(
      [base, paused],
      new Date("2026-07-31T12:00:00.000Z"),
      3
    );

    expect(agenda).toHaveLength(3);
    expect(agenda.map((item) => item.taskId)).toEqual(["daily", "daily", "daily"]);
    expect(agenda.map((item) => item.projected)).toEqual([false, true, true]);
    expect(agenda.every((item) => !item.overdue)).toBe(true);
  });

  it("keeps one overdue occurrence and flags different tasks within fifteen minutes", () => {
    const base: ScheduledTaskView = {
      id: "overdue",
      name: "Pendiente",
      conversationId: "conversation-1",
      conversationTitle: "Seguimiento",
      prompt: "Revisa el estado.",
      scheduleExpression: "once",
      timezone: "Atlantic/Canary",
      enabled: true,
      nextRunAt: "2026-07-31T09:00:00.000Z",
      createdAt: "2026-07-30T09:00:00.000Z",
      updatedAt: "2026-07-30T09:00:00.000Z",
      runs: []
    };
    const first = {
      ...base,
      id: "first",
      name: "Primera",
      nextRunAt: "2026-08-01T10:00:00.000Z"
    };
    const second = {
      ...base,
      id: "second",
      name: "Segunda",
      nextRunAt: "2026-08-01T10:12:00.000Z"
    };

    const agenda = scheduledCalendarOccurrences(
      [base, first, second],
      new Date("2026-07-31T12:00:00.000Z"),
      3
    );

    expect(agenda[0]).toEqual(expect.objectContaining({ taskId: "overdue", overdue: true }));
    expect(agenda.find((item) => item.taskId === "first")?.conflictingTaskIds)
      .toEqual(["second"]);
    expect(agenda.find((item) => item.taskId === "second")?.conflictingTaskIds)
      .toEqual(["first"]);
  });

  it("duplicates a schedule only as an unconfirmed draft", () => {
    const task: ScheduledTaskView = {
      id: "schedule-original",
      name: "Informe semanal",
      conversationId: "conversation-1",
      conversationTitle: "Seguimiento",
      prompt: "Resume los avances.",
      scheduleExpression: "weekly",
      timezone: "Atlantic/Canary",
      enabled: true,
      confirmedAt: "2026-07-31T09:00:00.000Z",
      nextRunAt: "2026-08-07T09:00:00.000Z",
      createdAt: "2026-07-31T09:00:00.000Z",
      updatedAt: "2026-07-31T09:00:00.000Z",
      runs: []
    };

    expect(scheduledTaskDuplicateDraft(task)).toEqual({
      name: "Copia de Informe semanal",
      conversationId: "conversation-1",
      prompt: "Resume los avances.",
      scheduleExpression: "weekly",
      confirmed: false
    });
  });

  it("searches scheduled tasks by name, conversation and instruction without accents", () => {
    const base: ScheduledTaskView = {
      id: "schedule-1",
      name: "Resumen técnico",
      conversationId: "conversation-1",
      conversationTitle: "Proyecto Ágora",
      prompt: "Revisa los bloqueos de integración.",
      scheduleExpression: "weekly",
      timezone: "Atlantic/Canary",
      enabled: true,
      nextRunAt: "2026-08-01T09:00:00.000Z",
      createdAt: "2026-07-31T09:00:00.000Z",
      updatedAt: "2026-07-31T09:00:00.000Z",
      runs: []
    };
    const other = {
      ...base,
      id: "schedule-2",
      name: "Mercados",
      conversationTitle: "Finanzas",
      prompt: "Resume el cierre diario."
    };

    expect(filterScheduledTasks([base, other], "tecnico")).toEqual([base]);
    expect(filterScheduledTasks([base, other], "agora")).toEqual([base]);
    expect(filterScheduledTasks([base, other], "integracion")).toEqual([base]);
    expect(filterScheduledTasks([base, other], "  ")).toEqual([base, other]);
  });

  it("keeps only terminal runs and orders the newest notice first", () => {
    const task: ScheduledTaskView = {
      id: "schedule-1",
      name: "Informe",
      conversationId: "conversation-1",
      conversationTitle: "Seguimiento",
      prompt: "Resume las novedades.",
      scheduleExpression: "daily",
      timezone: "Atlantic/Canary",
      enabled: true,
      nextRunAt: "2026-07-31T09:00:00.000Z",
      createdAt: "2026-07-29T09:00:00.000Z",
      updatedAt: "2026-07-30T09:00:00.000Z",
      runs: [
        {
          id: "running",
          dueAt: "2026-07-30T09:00:00.000Z",
          status: "running",
          attempt: 1,
          createdAt: "2026-07-30T09:00:00.000Z",
          updatedAt: "2026-07-30T09:00:01.000Z"
        },
        {
          id: "failed",
          dueAt: "2026-07-29T09:00:00.000Z",
          status: "failed",
          attempt: 1,
          createdAt: "2026-07-29T09:00:00.000Z",
          updatedAt: "2026-07-29T09:00:10.000Z"
        },
        {
          id: "completed-retry",
          dueAt: "2026-07-29T09:01:00.000Z",
          status: "completed",
          attempt: 2,
          createdAt: "2026-07-29T09:01:00.000Z",
          updatedAt: "2026-07-29T09:01:20.000Z"
        }
      ]
    };

    expect(scheduledNotifications([task])).toEqual([
      expect.objectContaining({ id: "completed-retry", attempt: 2 }),
      expect.objectContaining({ id: "failed", status: "failed" })
    ]);
  });

  it("filters scheduler history by terminal state and local date window", () => {
    const runs = [
      {
        id: "today-failed",
        dueAt: "2026-07-31T09:00:00.000Z",
        status: "failed" as const,
        attempt: 1,
        createdAt: "2026-07-31T09:00:00.000Z",
        updatedAt: "2026-07-31T09:05:00.000Z"
      },
      {
        id: "week-completed",
        dueAt: "2026-07-27T09:00:00.000Z",
        status: "completed" as const,
        attempt: 1,
        createdAt: "2026-07-27T09:00:00.000Z",
        updatedAt: "2026-07-27T09:05:00.000Z"
      },
      {
        id: "old-running",
        dueAt: "2026-06-01T09:00:00.000Z",
        status: "running" as const,
        attempt: 1,
        createdAt: "2026-06-01T09:00:00.000Z",
        updatedAt: "2026-06-01T09:05:00.000Z"
      }
    ];
    const now = new Date("2026-07-31T12:00:00.000Z");

    expect(filterScheduledRuns(runs, "failed", "today", now).map((run) => run.id))
      .toEqual(["today-failed"]);
    expect(filterScheduledRuns(runs, "completed", "7d", now).map((run) => run.id))
      .toEqual(["week-completed"]);
    expect(filterScheduledRuns(runs, "active", "all", now).map((run) => run.id))
      .toEqual(["old-running"]);
  });

  it("turns persisted scheduler results into readable details", () => {
    expect(scheduledRunDetail({
      id: "failed",
      dueAt: "2026-07-31T09:00:00.000Z",
      status: "failed",
      attempt: 1,
      result: { code: "BROKER_DOWN", message: "No se pudo conectar con el Broker." },
      createdAt: "2026-07-31T09:00:00.000Z",
      updatedAt: "2026-07-31T09:01:00.000Z"
    })).toEqual({
      label: "Motivo del fallo",
      text: "No se pudo conectar con el Broker."
    });
    expect(scheduledRunDetail({
      id: "completed",
      dueAt: "2026-07-31T10:00:00.000Z",
      status: "completed",
      attempt: 1,
      createdAt: "2026-07-31T10:00:00.000Z",
      updatedAt: "2026-07-31T10:01:00.000Z"
    })?.text).toContain("guardada en la conversación");
  });
});

describe("conversation attachment selection", () => {
  it("reactivates only the books that remain linked when a conversation opens", () => {
    expect(
      attachmentSelectionOnConversationOpen([
        { id: "math-deep" }
      ])
    ).toEqual(["math-deep"]);
  });
});

describe("project file library", () => {
  it("offers only files that are not already linked to the conversation", () => {
    expect(
      projectFilesAvailableToConversation(
        [{ id: "already-here" }],
        [{ id: "already-here", name: "A" }, { id: "reusable", name: "B" }]
      )
    ).toEqual([{ id: "reusable", name: "B" }]);
  });
});

describe("project knowledge filters", () => {
  const overview: ProjectKnowledgeOverview = {
    project: {
      id: "project-1",
      name: "Plan anual",
      conversationCount: 2,
      updatedAt: "2026-07-29"
    },
    files: [{
      id: "file-1",
      displayName: "Planificación 2026.pdf",
      mediaType: "application/pdf",
      sizeBytes: 100,
      sha256: "sha",
      ingestionStatus: "ready",
      contextStatus: "ready",
      chunkCount: 2,
      indexedCharacters: 200,
      semanticIndexedChunks: 2,
      semanticIndexStatus: "ready",
      updatedAt: "2026-07-29"
    }],
    fileUsages: [],
    memories: [{
      id: "memory-1",
      projectId: "project-1",
      category: "fact",
      content: "El cierre es mensual.",
      sensitivity: "normal",
      enabled: true,
      embeddingStatus: "ready",
      createdAt: "2026-07-29",
      updatedAt: "2026-07-29"
    }],
    memoryEnabled: true
  };

  it("matches file names without requiring accents or exact casing", () => {
    const filtered = filterProjectKnowledge(overview, "PLANIFICACION", "all");
    expect(filtered.files.map((file) => file.id)).toEqual(["file-1"]);
    expect(filtered.memories).toEqual([]);
    expect(filtered.total).toBe(1);
  });

  it("can restrict results to files or memories", () => {
    expect(filterProjectKnowledge(overview, "", "files").total).toBe(1);
    expect(filterProjectKnowledge(overview, "", "files").memories).toEqual([]);
    expect(filterProjectKnowledge(overview, "mensual", "memories").memories)
      .toHaveLength(1);
    expect(filterProjectKnowledge(overview, "mensual", "memories").files)
      .toEqual([]);
  });

  it("returns a clear empty result for unknown content", () => {
    expect(filterProjectKnowledge(overview, "inexistente", "all").total).toBe(0);
  });
});

describe("tabular attachment execution", () => {
  it("requires the sandbox for CSV, TSV and Excel files", () => {
    expect(attachmentNeedsSandbox({ displayName: "prices.csv", mediaType: "text/csv" })).toBe(true);
    expect(attachmentNeedsSandbox({
      displayName: "prices.xlsx",
      mediaType: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    })).toBe(true);
    expect(attachmentNeedsSandbox({
      displayName: "manual.pdf",
      mediaType: "application/pdf"
    })).toBe(false);
  });
});

describe("conversation scroll following", () => {
  it("follows new answers while the reader remains near the end", () => {
    expect(
      shouldFollowConversationScroll({
        scrollHeight: 1200,
        scrollTop: 560,
        clientHeight: 600
      })
    ).toBe(true);
  });

  it("preserves the reader position after they scroll up", () => {
    expect(
      shouldFollowConversationScroll({
        scrollHeight: 1200,
        scrollTop: 200,
        clientHeight: 600
      })
    ).toBe(false);
  });
});

describe("memory editor interaction", () => {
  it("protects the active draft from being replaced by another editor", () => {
    expect(canStartMemoryEdit(null)).toBe(true);
    expect(canStartMemoryEdit("memory-active")).toBe(false);
  });

  it("announces whether saving started a replacement index", () => {
    expect(memoryUpdateNotice(false)).toBe("Recuerdo actualizado.");
    expect(memoryUpdateNotice(true)).toContain("preparando un índice nuevo");
  });
});

describe("document source traceability", () => {
  const documentSource = {
    kind: "attachment_chunk",
    label: "informe.pdf · fragmento 2",
    reason: "Coincidencia con la pregunta",
    estimatedTokens: 120,
    excerpt: "Contenido",
    sourceReference: "opaque-source",
    sourceAvailable: true
  };

  it("only offers reveal for an available traced document fragment", () => {
    expect(canRevealContextSource(documentSource)).toBe(true);
    expect(canRevealContextSource({ ...documentSource, sourceAvailable: false })).toBe(false);
    expect(canRevealContextSource({ ...documentSource, kind: "memory" })).toBe(false);
    expect(canRevealContextSource({ ...documentSource, sourceReference: undefined })).toBe(false);
  });

  it("ignores a context load that resolves after the active panel changed", () => {
    expect(shouldApplyContextLoad("task-a", "task-a")).toBe(true);
    expect(shouldApplyContextLoad("task-b", "task-a")).toBe(false);
    expect(shouldApplyContextLoad(undefined, "task-a")).toBe(false);
  });
});

const task = (remoteStatus: string, localState = "polling"): LocalTaskSnapshot => ({
  id: "local-test",
  remoteStatus,
  localState,
  consecutivePollErrors: 0,
  progress: {},
  pendingToolCalls: [],
  updatedAt: "2026-07-20T00:00:00Z"
});

describe("Broker 2.6 task presentation", () => {
  it("honours presets declared per strategy while remaining compatible with old diagnostics", () => {
    const diagnostic = {
      reachable: true,
      ready: true,
      baseUrl: "http://broker",
      strategies: ["single", "mixture_of_agents"],
      presets: {
        single: ["fast"],
        mixture_of_agents: ["fast"]
      },
      workLanes: ["inference"],
      agentSkills: [],
      latencyMs: 1,
      message: "ok"
    };

    expect(brokerSupportsPreset(diagnostic, "mixture_of_agents", "fast")).toBe(true);
    expect(brokerSupportsPreset(diagnostic, "mixture_of_agents", "slow")).toBe(false);
    expect(brokerSupportsPreset({ ...diagnostic, presets: null }, "mixture_of_agents", "slow")).toBe(true);
  });

  it("uses structured invocation progress instead of exposing technical states", () => {
    expect(taskProgressSummary({
      ...task("proposing"),
      activity: "Generando respuesta",
      progress: {
        phase: "proposing",
        invocationsCompleted: 2,
        invocationsTotal: 3
      }
    })).toEqual({
      label: "Consultando modelos",
      completed: 2,
      total: 3
    });
  });

  it("explains the non-terminal memory wait introduced by contract 2.7", () => {
    expect(taskProgressSummary({
      ...task("waiting_for_memory"),
      progress: { phase: "waiting_for_memory" }
    })).toEqual({ label: "Esperando memoria disponible" });
  });

  it("explains whether a Broker failure is worth retrying", () => {
    expect(taskFailureSummary({
      code: "PROVIDER_UNAVAILABLE",
      message: "Proveedor temporalmente apagado",
      retryable: true
    })).toEqual({
      title: "La tarea no pudo completarse",
      detail: "Proveedor temporalmente apagado",
      retryable: true
    });
  });
});

describe("message submission eligibility", () => {
  it("does not depend on running the optional broker diagnostic", () => {
    expect(canSendMessage({
      hasConversation: true,
      hasText: true,
      attachmentsReady: true,
      attachmentBusy: false,
      turnBlocking: false
    })).toBe(true);
  });

  it("blocks both click and keyboard submission while local prerequisites are pending", () => {
    expect(canSendMessage({
      hasConversation: true,
      hasText: true,
      attachmentsReady: false,
      attachmentBusy: false,
      turnBlocking: false
    })).toBe(false);
  });
});

describe("attachment failure guidance", () => {
  it("explains a broker page limit with the actual and allowed page counts", () => {
    const guidance = attachmentFailureGuidance({
      id: "attachment-large-pdf",
      displayName: "math-deep.pdf",
      mediaType: "application/pdf",
      sizeBytes: 24_629_575,
      sha256: "hash",
      brokerFileId: "file-large-pdf",
      ingestionStatus: "failed",
      ingestionError: {
        code: "CONVERSION_FAILED",
        message:
          "Conversion failed for: original.pdf with status: failure. Errors: Document has 2204 pages, exceeding the max_num_pages limit of 2000."
      },
      contextStatus: "pending",
      chunkCount: 0,
      indexedCharacters: 0,
      semanticIndexedChunks: 0,
      semanticIndexStatus: "unavailable",
      updatedAt: "2026-07-25T19:46:01Z"
    });

    expect(guidance).toEqual({
      title: "El PDF supera el límite de páginas",
      detail: "Tiene 2.204 páginas y el Broker admite 2.000 por conversión.",
      action: "Divide el PDF en archivos más pequeños o aumenta el límite de páginas del Broker.",
      retryLabel: "Reintentar tras corregir"
    });
  });

  it("presents ingestion states in user-facing language", () => {
    expect(attachmentStatusLabel("converting")).toBe("Convirtiendo");
    expect(attachmentStatusLabel("ready")).toBe("Preparado");
    expect(attachmentStatusLabel("failed")).toBe("No preparado");
  });
});

describe("document context visibility", () => {
  it("shows the durable number of prepared fragments", () => {
    expect(attachmentContextSummary({
      id: "attachment-ready",
      displayName: "manual.pdf",
      mediaType: "application/pdf",
      sizeBytes: 240_000,
      sha256: "manual-hash",
      brokerFileId: "broker-manual",
      ingestionStatus: "ready",
      contextStatus: "ready",
      chunkCount: 42,
      indexedCharacters: 128_000,
      semanticIndexedChunks: 12,
      semanticIndexStatus: "indexing",
      updatedAt: "2026-07-26T00:00:00Z"
    })).toEqual({
      label: "Contexto preparado · 42 fragmentos",
      detail:
        "Cobertura: 128.000 caracteres consultables (~32.000 tokens estimados). " +
        "Preparando índice semántico (12/42). " +
        "Se recuperan los fragmentos relevantes y su contexto próximo.",
      tone: "ready",
      retryable: false
    });
  });

  it("keeps lexical retrieval available when the semantic index is partial", () => {
    const summary = attachmentContextSummary({
      id: "attachment-partial",
      displayName: "manual.pdf",
      mediaType: "application/pdf",
      sizeBytes: 240_000,
      sha256: "partial-hash",
      brokerFileId: "broker-partial",
      ingestionStatus: "ready",
      contextStatus: "ready",
      chunkCount: 10,
      indexedCharacters: 30_000,
      semanticIndexedChunks: 6,
      semanticIndexStatus: "partial",
      updatedAt: "2026-07-26T00:00:00Z"
    });

    expect(summary?.tone).toBe("warning");
    expect(summary?.detail).toContain("Índice semántico parcial (6/10)");
    expect(summary?.retryTarget).toBe("semantic");
    expect(summary?.retryLabel).toBe("Reintentar índice");
  });

  it("offers a specific retry when local context preparation fails", () => {
    expect(attachmentContextSummary({
      id: "attachment-context-failed",
      displayName: "manual.pdf",
      mediaType: "application/pdf",
      sizeBytes: 240_000,
      sha256: "manual-hash",
      brokerFileId: "broker-manual",
      ingestionStatus: "ready",
      contextStatus: "failed",
      contextError: { message: "No se pudo descargar el Markdown." },
      chunkCount: 0,
      indexedCharacters: 0,
      semanticIndexedChunks: 0,
      semanticIndexStatus: "unavailable",
      updatedAt: "2026-07-26T00:00:00Z"
    })).toEqual({
      label: "Contexto local no preparado",
      detail: "No se pudo descargar el Markdown.",
      tone: "error",
      retryable: true
    });
  });

  it("distinguishes context preparation from file upload", () => {
    expect(attachmentContextSummary({
      id: "attachment-context-preparing",
      displayName: "manual.pdf",
      mediaType: "application/pdf",
      sizeBytes: 240_000,
      sha256: "manual-hash",
      brokerFileId: "broker-manual",
      ingestionStatus: "ready",
      contextStatus: "preparing",
      chunkCount: 0,
      indexedCharacters: 0,
      semanticIndexedChunks: 0,
      semanticIndexStatus: "unavailable",
      updatedAt: "2026-07-26T00:00:00Z"
    })).toEqual({
      label: "Preparando contexto local",
      detail: "El archivo ya está en el Broker; ChatyGPT está preparando sus fragmentos.",
      tone: "pending",
      retryable: false
    });
  });

  it("keeps the queued context state visible after upload completes", () => {
    expect(attachmentContextSummary({
      id: "attachment-context-pending",
      displayName: "manual.pdf",
      mediaType: "application/pdf",
      sizeBytes: 240_000,
      sha256: "manual-hash",
      brokerFileId: "broker-manual",
      ingestionStatus: "ready",
      contextStatus: "pending",
      chunkCount: 0,
      indexedCharacters: 0,
      semanticIndexedChunks: 0,
      semanticIndexStatus: "unavailable",
      updatedAt: "2026-07-26T00:00:00Z"
    })).toEqual({
      label: "Contexto local pendiente",
      detail: "El archivo ya está disponible; su contenido se preparará a continuación.",
      tone: "pending",
      retryable: false
    });
  });

  it("explains when the broker did not provide converted text", () => {
    expect(attachmentContextSummary({
      id: "attachment-context-unavailable",
      displayName: "scan.pdf",
      mediaType: "application/pdf",
      sizeBytes: 240_000,
      sha256: "scan-hash",
      brokerFileId: "broker-scan",
      ingestionStatus: "ready",
      contextStatus: "unavailable",
      chunkCount: 0,
      indexedCharacters: 0,
      semanticIndexedChunks: 0,
      semanticIndexStatus: "unavailable",
      updatedAt: "2026-07-26T00:00:00Z"
    })).toEqual({
      label: "Sin fragmentos locales",
      detail: "El Broker no ofreció texto convertido; se usará el archivo completo.",
      tone: "warning",
      retryable: true
    });
  });
});

describe("semantic memory eligibility", () => {
  it("requires the feature and an indexed memory compatible with the conversation", () => {
    expect(canUseSemanticMemory({
      memoryEnabled: true,
      hasConversation: true,
      readyEligibleMemories: 1
    })).toBe(true);
    expect(canUseSemanticMemory({
      memoryEnabled: true,
      hasConversation: true,
      readyEligibleMemories: 0
    })).toBe(false);
    expect(canUseSemanticMemory({
      memoryEnabled: false,
      hasConversation: true,
      readyEligibleMemories: 3
    })).toBe(false);
  });
});

describe("broker task state helpers", () => {
  it("keeps a generating task blocking and pollable", () => {
    expect(isTerminalTask(task("generating"))).toBe(false);
    expect(isTaskPollingComplete(task("generating"))).toBe(false);
    expect(isTaskBlockingConversation(task("generating"))).toBe(true);
  });

  it("recognizes terminal and orphaned tasks", () => {
    expect(isTerminalTask(task("completed", "terminal"))).toBe(true);
    expect(isTaskPollingComplete(task("failed", "terminal"))).toBe(true);
    expect(isTaskBlockingConversation(task("not_submitted", "orphaned"))).toBe(false);
  });
});

describe("sandbox intent", () => {
  it("refreshes a stale negative diagnostic before blocking code execution", () => {
    expect(shouldRefreshSandboxDiagnostic({
      requiresCodeExecution: true,
      sandboxEnabledForTurn: false,
      sandboxAvailable: false,
      skipSuggestion: false
    })).toBe(true);
    expect(shouldRefreshSandboxDiagnostic({
      requiresCodeExecution: true,
      sandboxEnabledForTurn: false,
      sandboxAvailable: true,
      skipSuggestion: false
    })).toBe(false);
  });

  it("gives tabular failures a local, actionable recovery message", () => {
    expect(sandboxUnavailableGuidance(true)).toEqual({
      title: "No se puede analizar el archivo todavía",
      detail: "El CSV o la hoja de cálculo necesita Código aislado, pero Broker AI no lo anuncia como disponible.",
      action: "Comprueba la conexión y vuelve a intentarlo. El mensaje no se ha enviado."
    });
  });

  it("detects explicit requests to execute or test code", () => {
    expect(shouldOfferSandboxForPrompt("Crea el programa, ejecútalo y pruébalo")).toBe(true);
    expect(shouldOfferSandboxForPrompt("Run the tests for this Python script")).toBe(true);
  });

  it("does not interrupt ordinary programming questions", () => {
    expect(shouldOfferSandboxForPrompt("Explícame qué hace este código")).toBe(false);
    expect(shouldOfferSandboxForPrompt("¿Qué es una prueba de concepto?")).toBe(false);
  });
});

describe("historial de versiones de un GPT", () => {
  const version = (
    overrides: Partial<CustomGptVersionView> = {}
  ): CustomGptVersionView => ({
    id: "version-1",
    versionNo: 1,
    iconRef: "spark",
    instructions: "Explica con ejemplos.",
    conversationStarters: [],
    preferredModel: null,
    executionProfile: null,
    createdAt: "2026-08-01T09:00:00Z",
    active: false,
    toolPermissions: { runCode: "deny", renameConversation: "deny" },
    taskCount: 0,
    ...overrides
  });

  it("resuelve los iconos conocidos y protege datos antiguos o desconocidos", () => {
    expect(customGptIconGlyph("research")).toBe("⌕");
    expect(customGptIconGlyph(undefined)).toBe("✦");
    expect(customGptIconGlyph("icono-no-admitido")).toBe("✦");
  });

  it("distingue la versión en uso de las revisiones anteriores", () => {
    expect(customGptVersionSummary(version({ active: true, taskCount: 4 }))).toBe(
      "Versión en uso · 4 respuesta(s)"
    );
    expect(customGptVersionSummary(version({ active: true }))).toBe(
      "Versión en uso · todavía sin respuestas"
    );
  });

  it("avisa de que una revisión sigue respaldando respuestas ya emitidas", () => {
    expect(customGptVersionSummary(version({ taskCount: 2 }))).toBe(
      "Revisión anterior · 2 respuesta(s) conservan esta versión"
    );
    expect(customGptVersionSummary(version())).toBe(
      "Revisión anterior · no llegó a usarse"
    );
  });
});

describe("credencial del Broker", () => {
  it("nombra el origen real de la credencial en uso", () => {
    expect(
      brokerCredentialLabel({
        source: "protected",
        protected: true,
        environmentPresent: false,
        message: ""
      })
    ).toBe("Guardada y cifrada");
    expect(
      brokerCredentialLabel({
        source: "environment",
        protected: false,
        environmentPresent: true,
        message: ""
      })
    ).toBe("Heredada del entorno");
    expect(
      brokerCredentialLabel({
        source: "missing",
        protected: false,
        environmentPresent: false,
        message: ""
      })
    ).toBe("Sin credencial");
  });
});

describe("carpetas autorizadas", () => {
  const folder = (purpose?: string) => ({
    id: "folder-1",
    path: "c:\\users\\ana\\documentos",
    displayName: "C:\\Users\\Ana\\Documentos",
    permissions: purpose ? { write: true, purpose } : {},
    grantedAt: "2026-08-01T09:00:00Z",
    revokedAt: null
  });

  it("traduce el uso concedido a lenguaje comprensible", () => {
    expect(authorizedFolderPurpose(folder("obsidian_vault"))).toBe("Bóveda de Obsidian");
    expect(authorizedFolderPurpose(folder("conversation_markdown"))).toBe(
      "Exportar conversaciones a Markdown"
    );
  });

  it("no inventa un uso cuando la concesión no lo declara", () => {
    expect(authorizedFolderPurpose(folder())).toBe("Uso no declarado");
  });
});

describe("confirmación de herramientas", () => {
  const call = (confirmation: ToolCallView["confirmation"]): ToolCallView => ({
    toolCallId: "call-1",
    name: "rename_conversation",
    arguments: { title: "Presupuesto de obra" },
    status: "confirmation_required",
    confirmation
  });

  it("muestra los siete elementos del expediente sin JSON técnico", () => {
    const detail = confirmationSummary(
      call({
        id: "confirm-1",
        actionType: "conversation.rename",
        toolName: "rename_conversation",
        resources: { kind: "conversation", label: "La conversación abierta" },
        disclosure: {
          action_label: "Renombrar la conversación",
          data_sent: [{ label: "Título propuesto", value: "Presupuesto de obra" }],
          destination: "local",
          destination_label: "Solo esta aplicación; nada sale del equipo",
          scope: "one_time",
          scope_label: "Permitir una vez, solo para esta propuesta"
        },
        consequences: "El título se sustituirá. Es reversible.",
        status: "pending",
        requestedAt: "2026-08-01T09:00:00Z",
        resolvedAt: null
      })
    );

    expect(detail.action).toBe("Renombrar la conversación");
    expect(detail.tool).toBe("rename_conversation");
    expect(detail.resource).toBe("La conversación abierta");
    expect(detail.data).toEqual([
      { label: "Título propuesto", value: "Presupuesto de obra" }
    ]);
    expect(detail.destination).toBe("Solo esta aplicación; nada sale del equipo");
    expect(detail.scope).toBe("Permitir una vez, solo para esta propuesta");
    expect(detail.consequences).toBe("El título se sustituirá. Es reversible.");
  });

  it("no tranquiliza cuando falta el expediente", () => {
    const detail = confirmationSummary(call(null));

    expect(detail.resource).toBe("Recursos no declarados");
    expect(detail.destination).toBe("Destino no declarado");
    expect(detail.consequences).toContain("no puede anticipar");
    expect(detail.data).toEqual([]);
  });
});
