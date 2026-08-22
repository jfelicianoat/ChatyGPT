// @vitest-environment jsdom
/**
 * Pruebas del historial.
 *
 * Lo que se vigila: que se enseñe lo que Athena dice de un run pasado, que no se
 * invente lo que Athena no mide, y que «no consta historia» no se presente como «no
 * pasó nada».
 */

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AthenaHistorial } from "./AthenaHistorial";
import type { AthenaHistoria, AthenaResumenRun, AthenaRun } from "./domain";

afterEach(cleanup);

function resumenRun(parcial: Partial<AthenaResumenRun> = {}): AthenaResumenRun {
  return {
    runId: "run-1",
    workspaceId: "ws-1",
    status: "completed",
    resumable: false,
    degraded: false,
    objective: "Arreglar calc.add",
    filesModified: [],
    updatedAt: "2026-08-22T10:01:00+00:00",
    ...parcial
  };
}

function proyeccion(parcial: Partial<AthenaRun> = {}): AthenaRun {
  return {
    runId: "run-1",
    objetivo: "Arreglar calc.add",
    objetivoRevision: 2,
    perfilSolicitado: "software_engineering",
    workspaceId: "ws-1",
    fase: "completed",
    carpeta: "D:/repo",
    degradado: false,
    reanudable: false,
    conectado: false,
    controla: false,
    tareas: [],
    delegados: [],
    herramientas: [],
    permisos: [],
    comprobaciones: [],
    ficherosModificados: [],
    artefactos: [],
    errores: [],
    actividad: [],
    evidencia: [],
    ciclosReparacion: 0,
    ...parcial
  };
}

function historia(parcial: Partial<AthenaHistoria> = {}): AthenaHistoria {
  return {
    proyeccion: proyeccion(),
    resumen: {
      status: "completed",
      executedAs: "hierarchical",
      tasks: { T01: "completed" },
      delegates: { "sub-1": "explorer" },
      verification: "passed",
      permissionRequests: 1
    },
    hechos: [
      {
        secuencia: 1,
        nombre: "agent.started",
        cuando: "2026-08-22T10:00:00+00:00",
        actor: "root",
        delegado: false
      },
      {
        secuencia: 2,
        nombre: "file.changed",
        cuando: "2026-08-22T10:00:09+00:00",
        actor: "explorer",
        tarea: "T01",
        delegado: true
      }
    ],
    ...parcial
  };
}

describe("historial de runs", () => {
  it("lista los runs que recuerda Athena, no los que lanzó esta aplicación", async () => {
    // Un run lanzado desde Telegram aparece igual: el run es del runtime, no de quien
    // lo pidió.
    const onListar = vi
      .fn()
      .mockResolvedValue([resumenRun(), resumenRun({ runId: "run-2", objective: "Lo de Telegram" })]);
    render(<AthenaHistorial onListar={onListar} onAbrir={vi.fn()} />);

    await waitFor(() => expect(screen.getByText("Lo de Telegram")).toBeTruthy());
  });

  it("al abrir un run enseña estrategia, tareas, verificación y permisos", async () => {
    const onAbrir = vi.fn().mockResolvedValue(historia());
    render(<AthenaHistorial onListar={vi.fn().mockResolvedValue([resumenRun()])} onAbrir={onAbrir} />);

    await waitFor(() => expect(screen.getByRole("button", { name: /Ver qué pasó/ })).toBeTruthy());
    fireEvent.click(screen.getByRole("button", { name: /Ver qué pasó/ }));

    await waitFor(() => expect(screen.getByLabelText("Run anterior")).toBeTruthy());
    expect(screen.getByText("Repartido en tareas")).toBeTruthy();
    expect(screen.getByText(/T01 — completed/)).toBeTruthy();
    expect(screen.getByText(/Perfil: software_engineering/)).toBeTruthy();
  });

  it("atribuye a su delegado lo que hizo un delegado", async () => {
    // Sin esto, un run con delegados se leería como si todo lo hubiera hecho el padre.
    const onAbrir = vi.fn().mockResolvedValue(historia());
    render(<AthenaHistorial onListar={vi.fn().mockResolvedValue([resumenRun()])} onAbrir={onAbrir} />);

    fireEvent.click(await screen.findByRole("button", { name: /Ver qué pasó/ }));
    await waitFor(() => expect(screen.getByText(/Los hechos, en orden/)).toBeTruthy());
    expect(screen.getByText(/lo hizo explorer/)).toBeTruthy();
  });

  it("no inventa métricas de un run que Athena mide en agregado", async () => {
    const onAbrir = vi.fn().mockResolvedValue(historia());
    render(<AthenaHistorial onListar={vi.fn().mockResolvedValue([resumenRun()])} onAbrir={onAbrir} />);

    fireEvent.click(await screen.findByRole("button", { name: /Ver qué pasó/ }));
    await waitFor(() =>
      expect(screen.getByText(/no hay métricas de este trabajo en concreto/)).toBeTruthy()
    );
  });

  it("«no consta historia» no se enseña como «no pasó nada»", async () => {
    // Athena responde 404 a propósito: o el run no existió, o es anterior al registro.
    const onAbrir = vi.fn().mockRejectedValue(new Error("No durable history for run-1"));
    render(<AthenaHistorial onListar={vi.fn().mockResolvedValue([resumenRun()])} onAbrir={onAbrir} />);

    fireEvent.click(await screen.findByRole("button", { name: /Ver qué pasó/ }));
    await waitFor(() => expect(screen.getByText(/No durable history/)).toBeTruthy());
    expect(screen.queryByLabelText("Run anterior")).toBeNull();
  });

  it("dice que un run releído ya no se pronunció sobre la verificación cuando no consta", async () => {
    const sinVeredicto = historia({
      resumen: { ...historia().resumen, verification: "" }
    });
    const onAbrir = vi.fn().mockResolvedValue(sinVeredicto);
    render(<AthenaHistorial onListar={vi.fn().mockResolvedValue([resumenRun()])} onAbrir={onAbrir} />);

    fireEvent.click(await screen.findByRole("button", { name: /Ver qué pasó/ }));
    await waitFor(() =>
      expect(screen.getByText(/No consta que se pronunciara/)).toBeTruthy()
    );
  });
});
