// @vitest-environment jsdom

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { MessageArtifacts } from "./MessageArtifacts";

describe("ficheros generados en una respuesta", () => {
  it("resuelve la tarea local, lista el artefacto remoto y permite guardarlo", async () => {
    const onSaved = vi.fn();
    const api = {
      getLocalTask: vi.fn().mockResolvedValue({ remoteTaskId: "remote-chat-1" }),
      listTaskArtifacts: vi.fn().mockResolvedValue([
        {
          artifactId: "art-1",
          artifactType: "image_output",
          filename: "imagen.png",
          available: true,
          final: true
        }
      ]),
      saveTaskArtifact: vi.fn().mockResolvedValue("C:/ChatyGPT/imagen.png")
    };
    render(<MessageArtifacts localTaskId="local-chat-1" api={api} onSaved={onSaved} />);

    fireEvent.click(screen.getByRole("button", { name: "Ver ficheros generados" }));
    await waitFor(() => expect(screen.getByText("imagen.png")).toBeTruthy());
    expect(api.getLocalTask).toHaveBeenCalledWith("local-chat-1");
    expect(api.listTaskArtifacts).toHaveBeenCalledWith("remote-chat-1");

    fireEvent.click(screen.getByRole("button", { name: "Guardar" }));
    await waitFor(() => expect(onSaved).toHaveBeenCalledWith("C:/ChatyGPT/imagen.png"));
    expect(api.saveTaskArtifact).toHaveBeenCalledWith("remote-chat-1", "art-1");
  });
});
