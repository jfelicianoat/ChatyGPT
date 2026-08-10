/**
 * Proyecciones derivadas extraídas de `App.tsx` en la fase 2.
 *
 * Son cálculos que estaban dentro del componente y decidían qué ve la persona:
 * qué conversaciones aparecen en la barra lateral, qué recuerdos alcanzan a la
 * conversación abierta y cómo se agrupa la agenda de automatizaciones.
 */

import { describe, expect, it } from "vitest";
import {
  activeMemoriesForConversation,
  memoryAppliesToConversation,
  semanticReadyMemoriesForConversation,
  shouldPollMemoryIndex,
  shouldPollMemorySearch,
  shouldReloadConversationAfterTurn,
  taskFailureSummary,
  visibleConversations,
  type ConversationSummary,
  type MemoryItemView,
  type ScheduledCalendarOccurrence
} from "./domain";
import {
  schedulerCalendarConflictCount,
  schedulerCalendarDays
} from "./schedulerView";

const conversation = (
  id: string,
  projectId?: string
): ConversationSummary => ({
  id,
  title: `Conversación ${id}`,
  projectId,
  updatedAt: "2026-08-05T10:00:00Z"
});

const memory = (
  id: string,
  overrides: Partial<MemoryItemView> = {}
): MemoryItemView => ({
  id,
  category: "fact",
  content: `Recuerdo ${id}`,
  sensitivity: "normal",
  enabled: true,
  embeddingStatus: "ready",
  createdAt: "2026-08-01T10:00:00Z",
  updatedAt: "2026-08-01T10:00:00Z",
  ...overrides
});

describe("conversaciones visibles en la barra lateral", () => {
  const conversations = [
    conversation("a", "project-1"),
    conversation("b", "project-2"),
    conversation("c")
  ];

  it("muestra todas cuando no hay proyecto seleccionado", () => {
    expect(
      visibleConversations({
        conversations,
        searchResults: [],
        searchQuery: "",
        selectedProjectId: null
      })
    ).toEqual(conversations);
  });

  it("acota al proyecto seleccionado", () => {
    const visible = visibleConversations({
      conversations,
      searchResults: [],
      searchQuery: "",
      selectedProjectId: "project-1"
    });
    expect(visible.map((item) => item.id)).toEqual(["a"]);
  });

  it("acota a las conversaciones sin proyecto", () => {
    const visible = visibleConversations({
      conversations,
      searchResults: [],
      searchQuery: "",
      selectedProjectId: "unassigned"
    });
    expect(visible.map((item) => item.id)).toEqual(["c"]);
  });

  it("da prioridad a la búsqueda sobre el proyecto seleccionado", () => {
    // La regla que importa: quien busca espera encontrar. Si el filtro de
    // proyecto siguiera aplicándose, la conversación buscada quedaría oculta.
    const results = [conversation("z", "project-9")];
    const visible = visibleConversations({
      conversations,
      searchResults: results,
      searchQuery: "normativa",
      selectedProjectId: "project-1"
    });
    expect(visible.map((item) => item.id)).toEqual(["z"]);
  });

  it("trata una búsqueda en blanco como no buscar", () => {
    const visible = visibleConversations({
      conversations,
      searchResults: [conversation("z")],
      searchQuery: "   ",
      selectedProjectId: "project-2"
    });
    expect(visible.map((item) => item.id)).toEqual(["b"]);
  });
});

describe("alcance de los recuerdos", () => {
  it("un recuerdo global alcanza a cualquier conversación", () => {
    expect(memoryAppliesToConversation({ projectId: undefined }, "project-1")).toBe(true);
    expect(memoryAppliesToConversation({ projectId: undefined }, null)).toBe(true);
  });

  it("un recuerdo de proyecto no se filtra a otro proyecto", () => {
    expect(memoryAppliesToConversation({ projectId: "project-1" }, "project-1")).toBe(true);
    expect(memoryAppliesToConversation({ projectId: "project-1" }, "project-2")).toBe(false);
    // Ni a una conversación sin proyecto.
    expect(memoryAppliesToConversation({ projectId: "project-1" }, null)).toBe(false);
  });

  it("cuenta como activos solo los habilitados que alcanzan la conversación", () => {
    const items = [
      memory("global"),
      memory("mismo-proyecto", { projectId: "project-1" }),
      memory("otro-proyecto", { projectId: "project-2" }),
      memory("deshabilitado", { enabled: false })
    ];
    const active = activeMemoriesForConversation(items, "project-1");
    expect(active.map((item) => item.id)).toEqual(["global", "mismo-proyecto"]);
  });

  it("los indexados son siempre un subconjunto de los activos", () => {
    const items = [
      memory("listo"),
      memory("indexando", { embeddingStatus: "indexing" }),
      memory("fallido", { embeddingStatus: "failed" }),
      // Indexado pero deshabilitado: no puede recuperarse por similitud.
      memory("indexado-pero-apagado", { enabled: false }),
      // Indexado pero de otro proyecto: tampoco.
      memory("indexado-de-otro", { projectId: "project-9" })
    ];
    const active = activeMemoriesForConversation(items, "project-1");
    const ready = semanticReadyMemoriesForConversation(items, "project-1");

    expect(ready.map((item) => item.id)).toEqual(["listo"]);
    const activeIds = new Set(active.map((item) => item.id));
    expect(ready.every((item) => activeIds.has(item.id))).toBe(true);
    expect(ready.length).toBeLessThanOrEqual(active.length);
  });
});

describe("condiciones de parada del sondeo", () => {
  it("sigue sondeando solo mientras algo se está indexando", () => {
    // Equivocarse deja un temporizador vivo para siempre, o corta la
    // actualización antes de que el índice esté listo.
    expect(shouldPollMemoryIndex([{ embeddingStatus: "indexing" }])).toBe(true);
    expect(
      shouldPollMemoryIndex([{ embeddingStatus: "ready" }, { embeddingStatus: "indexing" }])
    ).toBe(true);
    expect(
      shouldPollMemoryIndex([{ embeddingStatus: "ready" }, { embeddingStatus: "failed" }])
    ).toBe(false);
    // Un índice fallido no se reintenta solo: sondear no lo arreglaría.
    expect(shouldPollMemoryIndex([])).toBe(false);
  });

  it("sigue sondeando una búsqueda semántica solo mientras busca", () => {
    expect(shouldPollMemorySearch({ status: "searching" })).toBe(true);
    expect(shouldPollMemorySearch({ status: "completed" })).toBe(false);
    expect(shouldPollMemorySearch({ status: "failed" })).toBe(false);
    expect(shouldPollMemorySearch(null)).toBe(false);
    expect(shouldPollMemorySearch(undefined)).toBe(false);
  });

  it("recarga la conversación solo si es la que está abierta", () => {
    // Recargar otra sobrescribiría lo que la persona está leyendo ahora mismo.
    expect(
      shouldReloadConversationAfterTurn({
        turnConversationId: "conversation-1",
        openConversationId: "conversation-1"
      })
    ).toBe(true);
    expect(
      shouldReloadConversationAfterTurn({
        turnConversationId: "conversation-1",
        openConversationId: "conversation-2"
      })
    ).toBe(false);
    expect(
      shouldReloadConversationAfterTurn({
        turnConversationId: "conversation-1",
        openConversationId: null
      })
    ).toBe(false);
    // Dos ausencias no son una coincidencia.
    expect(
      shouldReloadConversationAfterTurn({
        turnConversationId: null,
        openConversationId: null
      })
    ).toBe(false);
  });
});

describe("agenda de automatizaciones", () => {
  const occurrence = (
    id: string,
    startsAt: string,
    overrides: Partial<ScheduledCalendarOccurrence> = {}
  ): ScheduledCalendarOccurrence => ({
    id,
    taskId: `task-${id}`,
    taskName: `Tarea ${id}`,
    conversationId: "conversation-1",
    conversationTitle: "Chat",
    startsAt,
    scheduleExpression: "daily",
    timezone: "Europe/Madrid",
    projected: false,
    overdue: false,
    conflictingTaskIds: [],
    ...overrides
  });

  it("agrupa por día local y conserva el orden de llegada", () => {
    const days = schedulerCalendarDays([
      occurrence("1", new Date(2026, 7, 6, 9, 0).toISOString()),
      occurrence("2", new Date(2026, 7, 6, 18, 0).toISOString()),
      occurrence("3", new Date(2026, 7, 7, 9, 0).toISOString())
    ]);

    expect(days).toHaveLength(2);
    expect(days[0].items.map((item) => item.id)).toEqual(["1", "2"]);
    expect(days[1].items.map((item) => item.id)).toEqual(["3"]);
    expect(days[0].key).toBe("2026-08-06");
    expect(days[1].key).toBe("2026-08-07");
  });

  it("separa las atrasadas en su propia cesta y no en su fecha", () => {
    const days = schedulerCalendarDays([
      occurrence("vencida", new Date(2026, 7, 1, 9, 0).toISOString(), { overdue: true }),
      occurrence("futura", new Date(2026, 7, 6, 9, 0).toISOString())
    ]);

    expect(days.map((day) => day.key)).toEqual(["overdue", "2026-08-06"]);
    expect(days[0].label).toBe("Pendientes atrasadas");
    // Varias atrasadas de fechas distintas comparten una sola cesta.
    const merged = schedulerCalendarDays([
      occurrence("v1", new Date(2026, 6, 1).toISOString(), { overdue: true }),
      occurrence("v2", new Date(2026, 7, 1).toISOString(), { overdue: true })
    ]);
    expect(merged).toHaveLength(1);
    expect(merged[0].items.map((item) => item.id)).toEqual(["v1", "v2"]);
  });

  it("no inventa días cuando la agenda está vacía", () => {
    expect(schedulerCalendarDays([])).toEqual([]);
  });

  it("cuenta conflictos, no menciones", () => {
    // Las dos automatizaciones implicadas declaran el mismo conflicto.
    const items = [
      occurrence("a", new Date(2026, 7, 6, 9, 0).toISOString(), {
        taskId: "task-a",
        conflictingTaskIds: ["task-b"]
      }),
      occurrence("b", new Date(2026, 7, 6, 9, 10).toISOString(), {
        taskId: "task-b",
        conflictingTaskIds: ["task-a"]
      })
    ];
    expect(schedulerCalendarConflictCount(items)).toBe(1);
    expect(schedulerCalendarConflictCount([])).toBe(0);
  });

  it("no cuenta un conflicto a partir de una sola mención", () => {
    // Una declaración huérfana no describe una pareja: redondear hacia abajo
    // evita informar de un conflicto que no existe.
    const items = [
      occurrence("a", new Date(2026, 7, 6, 9, 0).toISOString(), {
        conflictingTaskIds: ["task-fantasma"]
      })
    ];
    expect(schedulerCalendarConflictCount(items)).toBe(0);
  });
});

describe("fallos que piden una decisión, no un reintento", () => {
  it("distingue una llamada remota ambigua de un fallo corriente", () => {
    // El Broker no reintenta solo para no pagar dos veces la inferencia, así
    // que la interfaz no puede invitar a reenviar sin avisar.
    const ambiguous = taskFailureSummary({
      code: "RECOVERY_AMBIGUOUS_REMOTE_CALL",
      message: "La llamada remota quedó en estado desconocido",
      retryable: false
    });
    expect(ambiguous?.title).toBe("No se sabe si la respuesta llegó a generarse");
    expect(ambiguous?.retryable).toBe(false);
    expect(ambiguous?.guidance).toContain("no lo reintenta solo");
    expect(ambiguous?.guidance).toContain("Revisa si la respuesta llegó");
  });

  it("mantiene el título genérico salvo donde ya estaba decidido otro", () => {
    // Solo se personalizan los dos códigos que lo tenían decidido: reescribir
    // el resto sería un cambio de comportamiento que nadie pidió.
    const unknown = taskFailureSummary({ code: "ALGO_NUEVO", message: "detalle" });
    expect(unknown?.title).toBe("La tarea no pudo completarse");
    expect(unknown?.guidance).toBeUndefined();
    expect(taskFailureSummary({ code: "PROVIDER_UNAVAILABLE" })?.title).toBe(
      "La tarea no pudo completarse"
    );
    expect(taskFailureSummary({ code: "CONTEXT_LIMIT_EXCEEDED" })?.title).toBe(
      "El contenido no cabe en el modelo seleccionado"
    );
  });

  it("obedece al Broker sobre si algo es reintentable", () => {
    // Aunque el código nos resulte familiar, manda la bandera del Broker.
    expect(
      taskFailureSummary({ code: "PROVIDER_UNAVAILABLE", retryable: true })?.retryable
    ).toBe(true);
    expect(
      taskFailureSummary({ code: "PROVIDER_UNAVAILABLE", retryable: false })?.retryable
    ).toBe(false);
  });

  it("explica qué hacer cuando la salida no es reintentar", () => {
    expect(
      taskFailureSummary({ code: "MODEL_CAPABILITY_MISMATCH" })?.guidance
    ).toContain("Elige otro modelo");
    expect(taskFailureSummary({ code: "BUDGET_EXCEEDED" })?.guidance).toContain(
      "Sube el presupuesto"
    );
  });

  it("no inventa detalle cuando el Broker no lo da", () => {
    expect(taskFailureSummary({ code: "TASK_TIMEOUT" })?.detail).toContain(
      "no proporcionó más detalles"
    );
    expect(taskFailureSummary(undefined)).toBeUndefined();
  });
});
