// @vitest-environment jsdom
/**
 * CH-03: la lista de mensajes extraída de App.tsx se prueba suelta, con un
 * objeto con la misma forma que devuelve `useListaMensajes`.
 * MessageArtifacts se simula: consulta al Broker y aquí no toca.
 */

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";

vi.mock("../MessageArtifacts", () => ({
  MessageArtifacts: () => <aside aria-label="Artefactos de la tarea" />
}));

import { ListaMensajes } from "./ListaMensajes";
import type { useListaMensajes } from "./useListaMensajes";
import type {
  ConversationMessage,
  ConversationView,
  CustomGptView,
  Loadable,
  LocalTaskSnapshot
} from "../domain";

afterEach(cleanup);

const mensaje = (patch: Partial<ConversationMessage> = {}): ConversationMessage =>
  ({
    id: "m1",
    role: "user",
    status: "complete",
    text: "¿Qué tiempo hace?",
    sequenceNo: 1,
    sources: [],
    ...patch
  }) as unknown as ConversationMessage;

const conversation = (patch: Partial<ConversationView> = {}): ConversationView =>
  ({
    id: "conv-1",
    messages: [mensaje()],
    researchRuns: [],
    ...patch
  }) as unknown as ConversationView;

const lista = (patch: Partial<ReturnType<typeof useListaMensajes>> = {}): ReturnType<typeof useListaMensajes> =>
  ({
    messageListRef: { current: null },
    messageWindow: { conversationId: "conv-1", limit: 80 },
    earlierMessagesBusy: false,
    effectiveMessageLimit: 80,
    progressiveMessages: { visibleItems: [], hiddenCount: 0 },
    hasEarlierMessages: false,
    revealEarlierMessages: vi.fn(),
    ...patch
  }) as ReturnType<typeof useListaMensajes>;

const base = (patch: Partial<Parameters<typeof ListaMensajes>[0]> = {}) => ({
  lista: lista(),
  conversation: conversation(),
  currentTurn: null,
  currentProgress: null,
  selectedCustomGpt: undefined,
  currentTurnBlocks: false,
  setDraft: vi.fn(),
  followConversationScrollRef: { current: true },
  contextPanel: null,
  toggleTaskContext: vi.fn(),
  contextSourceAction: null,
  revealContextSource: vi.fn(),
  ...patch
});

it("lista: sin mensajes enseña el estado vacío con los iniciadores del GPT", async () => {
  const user = userEvent.setup();
  const setDraft = vi.fn();
  render(
    <ListaMensajes
      {...base({
        setDraft,
        conversation: conversation({ messages: [], projectId: "p1" }),
        selectedCustomGpt: {
          name: "Analista",
          conversationStarters: ["Resume el informe", "Compara cifras"]
        } as CustomGptView
      })}
    />
  );
  const vacio = screen.getByText("¿En qué quieres trabajar?").closest(".chat-empty");
  expect(vacio?.textContent).toContain("Conversación de proyecto");
  await user.click(screen.getByRole("button", { name: "Resume el informe" }));
  expect(setDraft).toHaveBeenCalledWith("Resume el informe");
});

it("lista: con páginas anteriores ofrece cargarlas y cuenta las que faltan", async () => {
  const user = userEvent.setup();
  const reveal = vi.fn();
  const { rerender } = render(
    <ListaMensajes
      {...base({
        lista: lista({
          hasEarlierMessages: true,
          revealEarlierMessages: reveal
        }),
        conversation: conversation({ totalMessageCount: 130 })
      })}
    />
  );
  const boton = screen.getByRole("button", { name: "Mostrar 50 mensajes anteriores" }) as HTMLButtonElement;
  expect(boton.disabled).toBe(false);
  expect(screen.getByText(/mensajes anteriores sin cargar/).textContent).toContain("129");

  await user.click(boton);
  expect(reveal).toHaveBeenCalledTimes(1);

  rerender(
    <ListaMensajes
      {...base({
        lista: lista({ hasEarlierMessages: true, earlierMessagesBusy: true, revealEarlierMessages: reveal })
      })}
    />
  );
  expect((screen.getByRole("button", { name: "Cargando mensajes anteriores…" }) as HTMLButtonElement).disabled).toBe(true);
});

it("lista: pinta los mensajes con su rol y ancla para el salto de búsqueda (H22)", () => {
  render(
    <ListaMensajes
      {...base({
        conversation: conversation({
          messages: [
            mensaje(),
            mensaje({ id: "m2", role: "assistant", text: "Hoy **llueve** en Canarias", sequenceNo: 2 })
          ]
        }),
        lista: lista({
          progressiveMessages: {
            visibleItems: [
              mensaje(),
              mensaje({ id: "m2", role: "assistant", text: "Hoy **llueve** en Canarias", sequenceNo: 2 })
            ],
            hiddenCount: 0
          }
        })
      })}
    />
  );
  expect(document.getElementById("mensaje-m1")).not.toBeNull();
  expect(document.getElementById("mensaje-m2")).not.toBeNull();
  expect(screen.getByText("Tú")).not.toBeNull();
  expect(screen.getByText("ChatyGPT")).not.toBeNull();
  expect(screen.getByText("¿Qué tiempo hace?").className).toContain("message-text");
});

it("lista: con un mensaje pendiente muestra el progreso real del turno", () => {
  const turno = {
    state: "ready",
    value: {
      id: "task-1",
      progress: { phase: "planning" },
      remoteStatus: "planning"
    }
  } as unknown as Loadable<LocalTaskSnapshot>;
  render(
    <ListaMensajes
      {...base({
        conversation: conversation({
          messages: [mensaje({ id: "m1", status: "pending", brokerTaskId: "task-1" })]
        }),
        lista: lista({
          progressiveMessages: {
            visibleItems: [mensaje({ id: "m1", status: "pending", brokerTaskId: "task-1" })],
            hiddenCount: 0
          }
        }),
        currentTurn: turno,
        currentProgress: { label: "Planificando", completed: 1, total: 4 }
      })}
    />
  );
  const progreso = document.querySelector(".real-progress");
  expect(progreso?.textContent).toContain("Planificando · 1/4");
  expect(document.querySelector("progress")).not.toBeNull();
});

it("lista: una respuesta cancelada se presenta como decisión de la persona", () => {
  // La respuesta cancelada no llega con texto: por eso se muestra su aviso.
  const cancelada = mensaje({ id: "m1", role: "assistant", status: "cancelled", text: undefined as unknown as string });
  render(
    <ListaMensajes
      {...base({
        conversation: conversation({ messages: [cancelada] }),
        lista: lista({
          progressiveMessages: { visibleItems: [cancelada], hiddenCount: 0 }
        })
      })}
    />
  );
  expect(screen.getByText("Respuesta cancelada")).not.toBeNull();
  expect(document.querySelector(".real-progress")).toBeNull();
});
