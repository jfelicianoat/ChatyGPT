// @vitest-environment jsdom
/**
 * Recorridos de recuperación (auditoría 28-sep-2026, H14 y H16): lo archivado
 * y lo eliminado se recupera sin tocar SQLite, borrar para siempre exige una
 * confirmación aparte, y una copia de seguridad se verifica antes de ofrecer
 * restaurarla.
 */

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

const calls = new Map<string, ReturnType<typeof vi.fn>>();
const method = (name: string) => {
  let mock = calls.get(name);
  if (!mock) {
    mock = vi.fn(async () => undefined);
    calls.set(name, mock);
  }
  return mock;
};

vi.mock("../platform", () => ({
  platform: new Proxy({}, { get: (_target, property: string) => method(property) })
}));

import { ArchivoPanel } from "./ArchivoPanel";
import { CopiaSeguridad } from "./CopiaSeguridad";

const overview = {
  conversations: [
    {
      id: "c-arch",
      title: "Viaje a Tenerife",
      projectName: "Vacaciones",
      messageCount: 12,
      archivedAt: "2026-09-20 10:00:00",
      deletedAt: null
    }
  ],
  trash: [
    {
      id: "c-trash",
      title: "Borrador descartado",
      projectName: null,
      messageCount: 3,
      archivedAt: null,
      deletedAt: "2026-09-21 10:00:00"
    }
  ],
  projects: [
    { id: "p-1", name: "Reforma", archivedAt: "2026-09-22 10:00:00", conversationCount: 4 }
  ]
};

const report = {
  folder: "D:/Copias/ChatyGPT-copia-20260928-120000",
  createdAt: "2026-09-28T12:00:00Z",
  appVersion: "0.3.0",
  schemaVersion: 25,
  conversationCount: 40,
  attachmentCount: 7,
  totalBytes: 5_242_880,
  notIncluded: ["Credenciales: habrá que volver a introducirlas."]
};

beforeEach(() => {
  calls.clear();
  method("getArchiveOverview").mockResolvedValue(overview);
});

afterEach(() => {
  vi.restoreAllMocks();
  cleanup();
});

it("lista lo archivado y lo eliminado y lo restaura con su agrupación", async () => {
  const onChanged = vi.fn();
  render(<ArchivoPanel onChanged={onChanged} onOpenConversation={() => {}} />);
  expect(await screen.findByText("Viaje a Tenerife")).toBeDefined();
  expect(screen.getByText("Borrador descartado")).toBeDefined();
  expect(screen.getByText(/al restaurarlo vuelven 4 conversación/)).toBeDefined();

  const [restoreArchived] = screen.getAllByRole("button", { name: "Restaurar" });
  await userEvent.click(restoreArchived);
  await waitFor(() => expect(method("restoreConversation")).toHaveBeenCalledWith("c-arch"));
  expect(await screen.findByText("«Viaje a Tenerife» vuelve a tus chats.")).toBeDefined();
  expect(onChanged).toHaveBeenCalled();

  await userEvent.click(screen.getByRole("button", { name: "Restaurar proyecto" }));
  await waitFor(() => expect(method("restoreProject")).toHaveBeenCalledWith("p-1"));
});

it("borrar para siempre solo existe en la papelera y exige confirmación", async () => {
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
  render(<ArchivoPanel onChanged={() => {}} onOpenConversation={() => {}} />);
  await screen.findByText("Borrador descartado");
  const purge = screen.getAllByRole("button", { name: "Borrar para siempre" });
  expect(purge).toHaveLength(1);

  await userEvent.click(purge[0]);
  expect(confirm).toHaveBeenCalled();
  expect(method("purgeConversation")).not.toHaveBeenCalled();

  confirm.mockReturnValue(true);
  await userEvent.click(purge[0]);
  await waitFor(() => expect(method("purgeConversation")).toHaveBeenCalledWith("c-trash"));
});

it("crea una copia verificada y explica lo que no viaja en ella", async () => {
  method("pickBackupFolder").mockResolvedValue("D:/Copias");
  method("createBackup").mockResolvedValue(report);
  render(<CopiaSeguridad />);
  await userEvent.click(screen.getByRole("button", { name: "Crear copia…" }));
  expect(await screen.findByText("Copia creada y verificada.")).toBeDefined();
  expect(method("createBackup")).toHaveBeenCalledWith("D:/Copias");
  expect(screen.getByText(/Credenciales: habrá que volver a introducirlas/)).toBeDefined();
});

it("restaurar primero verifica la copia y luego pide confirmación", async () => {
  method("pickBackupFolder").mockResolvedValue(report.folder);
  method("inspectBackup").mockResolvedValue(report);
  method("scheduleBackupRestore").mockResolvedValue(report);
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
  render(<CopiaSeguridad />);

  await userEvent.click(screen.getByRole("button", { name: "Restaurar una copia…" }));
  expect(await screen.findByText("La copia está completa y abre correctamente.")).toBeDefined();
  expect(method("scheduleBackupRestore")).not.toHaveBeenCalled();

  await userEvent.click(screen.getByRole("button", { name: "Restaurar esta copia al reiniciar" }));
  expect(confirm).toHaveBeenCalled();
  expect(method("scheduleBackupRestore")).not.toHaveBeenCalled();

  confirm.mockReturnValue(true);
  await userEvent.click(screen.getByRole("button", { name: "Restaurar esta copia al reiniciar" }));
  expect(await screen.findByText("Restauración programada.")).toBeDefined();
  expect(method("scheduleBackupRestore")).toHaveBeenCalledWith(report.folder);
});

it("una copia dañada se explica y no se ofrece restaurarla", async () => {
  method("pickBackupFolder").mockResolvedValue("D:/Copias/rota");
  method("inspectBackup").mockRejectedValue(
    new Error("el fichero chatygpt.db no coincide con su huella: la copia está dañada")
  );
  render(<CopiaSeguridad />);
  await userEvent.click(screen.getByRole("button", { name: "Restaurar una copia…" }));
  expect(await screen.findByRole("alert")).toBeDefined();
  expect(screen.queryByRole("button", { name: "Restaurar esta copia al reiniciar" })).toBeNull();
});
