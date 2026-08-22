// @vitest-environment jsdom
/**
 * Pruebas del panel de memoria.
 *
 * La regla que se vigila es una: **la interfaz no convierte propuestas en hechos**.
 * Todo lo demás —tipos, fechas, procedencia— está para que quien decide pueda decidir.
 */

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  AthenaMemoria,
  nombreClaseRecuerdo,
  nombreEstadoRecuerdo,
  nombreVerificacion
} from "./AthenaMemoria";
import type { AthenaRecuerdo } from "./domain";

afterEach(cleanup);

function recuerdo(parcial: Partial<AthenaRecuerdo> = {}): AthenaRecuerdo {
  return {
    id: "mem-1",
    projectId: "ws-1",
    kind: "verified_command",
    content: "pytest -q",
    source: "run:ws-1",
    sourceReference: null,
    confidence: 0.9,
    verificationState: "verified",
    scope: "project",
    status: "active",
    supersedes: null,
    createdAt: "2026-08-20T10:00:00+00:00",
    updatedAt: "2026-08-20T10:00:00+00:00",
    stale: false,
    ...parcial
  };
}

describe("panel de memoria del proyecto", () => {
  it("enseña de dónde salió cada recuerdo y quién responde por él", async () => {
    // Un recuerdo sin origen no se puede juzgar, y un hint que no se puede juzgar es
    // un rumor. Y «propuesto» no es un grado de confianza: es otro autor.
    const onListar = vi.fn().mockResolvedValue([recuerdo({ verificationState: "proposed" })]);
    render(
      <AthenaMemoria
        workspaceId="ws-1"
        onListar={onListar}
        onConfirmar={vi.fn()}
        onOlvidar={vi.fn()}
      />
    );

    await waitFor(() => expect(screen.getByText("pytest -q")).toBeTruthy());
    expect(screen.getByText(/Lo dijo el modelo; nadie lo ha comprobado/)).toBeTruthy();
    expect(screen.getByText(/run:ws-1/)).toBeTruthy();
    expect(screen.getByText("Comando que funcionó")).toBeTruthy();
  });

  it("una propuesta no se convierte en hecho sola: hay que pulsarlo", async () => {
    const onConfirmar = vi.fn().mockResolvedValue(recuerdo({ verificationState: "user_confirmed" }));
    const onListar = vi.fn().mockResolvedValue([recuerdo({ verificationState: "proposed" })]);
    render(
      <AthenaMemoria
        workspaceId="ws-1"
        onListar={onListar}
        onConfirmar={onConfirmar}
        onOlvidar={vi.fn()}
      />
    );

    await waitFor(() => expect(screen.getByText("pytest -q")).toBeTruthy());
    expect(onConfirmar).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: /Respondo por esto/ }));
    await waitFor(() => expect(onConfirmar).toHaveBeenCalledWith("mem-1"));
  });

  it("no ofrece respaldar lo que una persona ya respaldó", async () => {
    const onListar = vi
      .fn()
      .mockResolvedValue([recuerdo({ verificationState: "user_confirmed" })]);
    render(
      <AthenaMemoria
        workspaceId="ws-1"
        onListar={onListar}
        onConfirmar={vi.fn()}
        onOlvidar={vi.fn()}
      />
    );

    await waitFor(() => expect(screen.getByText("pytest -q")).toBeTruthy());
    expect(screen.queryByRole("button", { name: /Respondo por esto/ })).toBeNull();
  });

  it("marca lo caduco en vez de esconderlo", async () => {
    // Lo viejo se etiqueta, no se tira: sigue diciendo qué se creía.
    const onListar = vi.fn().mockResolvedValue([recuerdo({ stale: true })]);
    render(
      <AthenaMemoria
        workspaceId="ws-1"
        onListar={onListar}
        onConfirmar={vi.fn()}
        onOlvidar={vi.fn()}
      />
    );

    await waitFor(() => expect(screen.getByText(/ha pasado su plazo/)).toBeTruthy());
  });

  it("dice a qué recuerdo sustituye, para que sobreviva el «antes creíamos X»", async () => {
    const onListar = vi.fn().mockResolvedValue([recuerdo({ supersedes: "mem-0" })]);
    render(
      <AthenaMemoria
        workspaceId="ws-1"
        onListar={onListar}
        onConfirmar={vi.fn()}
        onOlvidar={vi.fn()}
      />
    );

    await waitFor(() => expect(screen.getByText("mem-0")).toBeTruthy());
  });

  it("distingue quién responde de si el recuerdo sigue vigente", () => {
    // Un solo campo haría indistinguible «nadie lo ha comprobado» de «ya no vale».
    expect(nombreVerificacion("verified")).toContain("Algo lo comprobó");
    expect(nombreEstadoRecuerdo("superseded")).toContain("Sustituido");
    expect(nombreClaseRecuerdo("algo_nuevo")).toBe("algo_nuevo");
  });
});
