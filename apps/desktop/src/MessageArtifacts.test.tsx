// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import type { TaskArtifact } from "./domain";
import { MessageArtifacts } from "./MessageArtifacts";

const remote: TaskArtifact = {
  artifactId: "art-1",
  artifactType: "image_output",
  filename: "imagen.png",
  sizeBytes: 2048,
  available: true,
  final: true
};

const apiWith = (artifacts: TaskArtifact[][]) => {
  const list = vi.fn();
  for (const items of artifacts) list.mockResolvedValueOnce(items);
  list.mockResolvedValue(artifacts[artifacts.length - 1]);
  return {
    getLocalTask: vi.fn().mockResolvedValue({ remoteTaskId: "remote-chat-1" }),
    listTaskArtifacts: list,
    saveTaskArtifact: vi.fn().mockResolvedValue("C:/ChatyGPT/attachments/abc/imagen.png"),
    saveTaskArtifactAs: vi.fn().mockResolvedValue("D:/Descargas/imagen.png"),
    revealTaskArtifact: vi.fn().mockResolvedValue("imagen.png")
  };
};

describe("ficheros generados en una respuesta", () => {
  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
  });

  it("conserva el artefacto en el equipo y lo asocia al mensaje, sin ventanas de alerta", async () => {
    const alert = vi.spyOn(window, "alert").mockImplementation(() => undefined);
    const api = apiWith([
      [remote],
      [{ ...remote, localPath: "C:/ChatyGPT/attachments/abc/imagen.png", savedAt: "2026-09-28" }]
    ]);
    render(<MessageArtifacts localTaskId="local-chat-1" api={api} />);

    fireEvent.click(screen.getByRole("button", { name: "Ver ficheros generados" }));
    await waitFor(() => expect(screen.getByText("imagen.png")).toBeTruthy());
    expect(api.getLocalTask).toHaveBeenCalledWith("local-chat-1");
    expect(api.listTaskArtifacts).toHaveBeenCalledWith("remote-chat-1");

    fireEvent.click(screen.getByRole("button", { name: "Conservar en el equipo" }));
    await screen.findByText(/se conserva en este equipo/);
    expect(api.saveTaskArtifact).toHaveBeenCalledWith("remote-chat-1", "art-1");
    expect(screen.getByRole("button", { name: "Mostrar en el Explorador" })).toBeTruthy();
    expect(alert).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "Mostrar en el Explorador" }));
    await waitFor(() =>
      expect(api.revealTaskArtifact).toHaveBeenCalledWith("remote-chat-1", "art-1")
    );
  });

  it("«Guardar como…» deja elegir destino e informa de dónde quedó", async () => {
    const api = apiWith([[remote]]);
    render(<MessageArtifacts localTaskId="local-chat-1" api={api} />);
    fireEvent.click(screen.getByRole("button", { name: "Ver ficheros generados" }));
    fireEvent.click(await screen.findByRole("button", { name: "Guardar como…" }));
    await screen.findByText("Guardado en D:/Descargas/imagen.png");
    expect(api.saveTaskArtifactAs).toHaveBeenCalledWith("remote-chat-1", "art-1");
  });

  it("una copia conservada sigue accesible cuando el Broker ya podó el artefacto", async () => {
    const api = apiWith([
      [
        {
          ...remote,
          artifactType: "local_copy",
          available: false,
          localPath: "C:/ChatyGPT/attachments/abc/imagen.png",
          savedAt: "2026-09-28"
        }
      ]
    ]);
    render(<MessageArtifacts localTaskId="local-chat-1" api={api} />);
    fireEvent.click(screen.getByRole("button", { name: "Ver ficheros generados" }));
    await screen.findByText(/conservado en este equipo/);
    expect(screen.queryByRole("button", { name: "Conservar en el equipo" })).toBeNull();
    expect(screen.getByRole("button", { name: "Mostrar en el Explorador" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Guardar como…" })).toBeTruthy();
  });
});
