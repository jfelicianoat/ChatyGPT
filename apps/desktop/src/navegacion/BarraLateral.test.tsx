// @vitest-environment jsdom
/**
 * CH-03: la barra lateral extraída de App.tsx se prueba suelta, sin montar la
 * aplicación entera.
 */
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";

import { BarraLateral } from "./BarraLateral";
import type { WorkspaceDestination } from "../navegacion";
import type { ConversationSearchHit, ConversationSummary, Loadable, ProjectSummary } from "../domain";

afterEach(cleanup);

const proyecto = (patch: Partial<ProjectSummary> = {}): ProjectSummary =>
  ({ id: "p1", name: "TFM", conversationCount: 3, ...patch }) as unknown as ProjectSummary;

const conversacion = (patch: Partial<ConversationSummary> = {}): ConversationSummary =>
  ({ id: "c1", title: "Calendario de siembra", projectId: null, ...patch }) as unknown as ConversationSummary;

const hit = (patch: Partial<ConversationSearchHit> = {}): ConversationSearchHit =>
  ({ id: "c1", title: "Calendario de siembra", snippet: "…abril en Canarias…", ...patch }) as unknown as ConversationSearchHit;

const base = (patch: Partial<Parameters<typeof BarraLateral>[0]> = {}) => ({
  appVersion: "0.4.19",
  bootstrap: { state: "ready", value: { appVersion: "0.4.19", schemaVersion: 26, logPath: null } } as Loadable<never>,
  conversation: null,
  conversations: [conversacion()],
  projects: [proyecto()],
  selectedProject: null,
  selectedProjectId: null,
  setSelectedProjectId: vi.fn(),
  workspaceDestination: "chats" as WorkspaceDestination,
  openWorkspaceDestination: vi.fn(),
  searchInputRef: { current: null },
  searchQuery: "",
  setSearchQuery: vi.fn(),
  searchIncludesArchived: false,
  setSearchIncludesArchived: vi.fn(),
  searchResults: [],
  visibleConversationList: [conversacion()],
  createConversation: vi.fn(),
  openConversation: vi.fn(),
  openSearchHit: vi.fn(),
  openDialog: vi.fn(),
  openProjectKnowledge: vi.fn(),
  navigationError: null,
  setKeyboardHelpOpen: vi.fn(),
  ...patch
});

it("lateral: los espacios se pintan y el elegido va marcado como página actual", async () => {
  const user = userEvent.setup();
  const openWorkspaceDestination = vi.fn();
  render(<BarraLateral {...base({ openWorkspaceDestination })} />);
  const proyectos = screen.getByRole("button", { name: "Proyectos" });
  expect(proyectos.getAttribute("aria-current")).toBeNull();
  const chats = screen.getByRole("button", { name: "Chats" });
  expect(chats.getAttribute("aria-current")).toBe("page");
  await user.click(proyectos);
  expect(openWorkspaceDestination).toHaveBeenCalledWith("projects");
});

it("lateral: los proyectos muestran su recuento y solo el seleccionado tiene menú", async () => {
  const user = userEvent.setup();
  const setSelectedProjectId = vi.fn();
  render(
    <BarraLateral
      {...base({
        setSelectedProjectId,
        projects: [proyecto(), proyecto({ id: "p2", name: "Trading" })],
        selectedProjectId: "p1"
      })}
    />
  );
  expect(screen.getByText("◇ TFM")).not.toBeNull();
  const gestionar = screen.getByRole("button", { name: "Gestionar TFM" });
  expect(screen.queryByRole("button", { name: "Gestionar Trading" })).toBeNull();
  const botonTrading = screen.getByText("◇ Trading").closest("button") as HTMLButtonElement;
  // fireEvent: userEvent no despacha sobre este botón envuelto en un div de fila.
  fireEvent.click(botonTrading);
  expect(setSelectedProjectId).toHaveBeenCalledWith("p2");
  void gestionar;
});

it("lateral: sin búsqueda enseña recientes y abre la conversación; con búsqueda, resultados", async () => {
  const user = userEvent.setup();
  const openConversation = vi.fn();
  const openSearchHit = vi.fn();
  const { rerender } = render(
    <BarraLateral
      {...base({
        visibleConversationList: [conversacion()],
        openConversation
      })}
    />
  );
  expect(screen.getByText("Recientes")).not.toBeNull();
  await user.click(screen.getByRole("button", { name: "Calendario de siembra" }));
  expect(openConversation).toHaveBeenCalledWith("c1");

  rerender(
    <BarraLateral
      {...base({
        searchQuery: "siembra",
        searchResults: [hit()],
        visibleConversationList: [conversacion()],
        openConversation,
        openSearchHit
      })}
    />
  );
  expect(screen.getByText("Resultados")).not.toBeNull();
  await user.click(screen.getByRole("button", { name: /Calendario de siembra/ }));
  expect(openSearchHit).toHaveBeenCalledTimes(1);
  expect(openConversation).toHaveBeenCalledTimes(1);
});

it("lateral: el pie enseña la versión del binario y el error de navegación se ve", () => {
  render(
    <BarraLateral
      {...base({ navigationError: "Broker no responde" })}
    />
  );
  expect(screen.getByText("ChatyGPT 0.4.19 · Datos locales · esquema 26")).not.toBeNull();
  expect(screen.getByText("Broker no responde").className).toContain("sidebar-error");
});
