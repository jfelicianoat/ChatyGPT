// @vitest-environment jsdom
/**
 * Pruebas del cambio de encargo.
 *
 * Lo que se comprueba no es que el formulario pinte, sino las dos reglas que ADR-029
 * pone del lado del cliente: que un conflicto no se reintente solo, y que «escrito» no
 * se enseñe como «aplicado».
 */

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AthenaEncargo } from "./AthenaEncargo";
import type { AthenaObjetivo, AthenaRevisionObjetivo, AthenaRun } from "./domain";

afterEach(cleanup);

function run(parcial: Partial<AthenaRun> = {}): AthenaRun {
  return {
    runId: "run-1",
    objetivo: "Arreglar calc.add",
    objetivoRevision: 1,
    perfilSolicitado: "",
    workspaceId: "ws-1",
    fase: "running",
    carpeta: "D:/repo",
    degradado: false,
    reanudable: false,
    conectado: true,
    controla: true,
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

function objetivo(parcial: Partial<AthenaObjetivo> = {}): AthenaObjetivo {
  return {
    text: "Arreglar calc.add",
    revision: 1,
    reason: "",
    revisedAt: "2026-08-22T10:00:00+00:00",
    ...parcial
  };
}

async function abrir(
  onLeer: () => Promise<AthenaObjetivo>,
  onRevisar: (objetivo: string, motivo: string) => Promise<AthenaRevisionObjetivo>
) {
  render(<AthenaEncargo run={run()} onLeer={onLeer} onRevisar={onRevisar} />);
  fireEvent.click(screen.getByText(/Encargo/));
  await waitFor(() => expect(onLeer).toHaveBeenCalled());
}

describe("cambiar el encargo de un run", () => {
  it("relee el encargo al abrir en vez de fiarse de lo que hubiera en pantalla", async () => {
    // Entre que se pintó el run y alguien decide cambiarlo caben minutos, y en ese
    // hueco cabe otra persona escribiendo desde Telegram.
    const onLeer = vi.fn().mockResolvedValue(objetivo({ text: "Lo de Telegram", revision: 4 }));
    const onRevisar = vi.fn();

    await abrir(onLeer, onRevisar);

    await waitFor(() => expect(screen.getByText(/revisión 4/)).toBeTruthy());
    const campo = screen.getByLabelText(/Nuevo encargo/) as HTMLTextAreaElement;
    expect(campo.value).toBe("Lo de Telegram");
  });

  it("dice que se escribió, no que se esté aplicando", async () => {
    // Athena responde `applied: false` a propósito: recoge la revisión entre
    // iteraciones. Decir «ya está trabajando en ello» sería cómodo y falso.
    const onLeer = vi.fn().mockResolvedValue(objetivo());
    const onRevisar = vi.fn().mockResolvedValue({
      resultado: "aceptada",
      objetivo: objetivo({ text: "Otra cosa", revision: 2 })
    } satisfies AthenaRevisionObjetivo);

    await abrir(onLeer, onRevisar);
    fireEvent.change(screen.getByLabelText(/Nuevo encargo/), {
      target: { value: "Otra cosa" }
    });
    fireEvent.click(screen.getByRole("button", { name: /Cambiar el encargo/ }));

    await waitFor(() =>
      expect(screen.getByText(/Athena lo recogerá al terminar la iteración/)).toBeTruthy()
    );
  });

  it("ante un conflicto enseña el encargo vigente y no reintenta", async () => {
    // La regla del cliente: nada se reintenta solo. El encargo de otro puede ser
    // incompatible con el que se estaba escribiendo, y repetir sin mirarlo lo pisaría.
    const onLeer = vi.fn().mockResolvedValue(objetivo());
    const onRevisar = vi.fn().mockResolvedValue({
      resultado: "conflicto",
      vigente: objetivo({
        text: "Lo que pidió Telegram",
        revision: 3,
        reason: "cambio de alcance"
      })
    } satisfies AthenaRevisionObjetivo);

    await abrir(onLeer, onRevisar);
    fireEvent.change(screen.getByLabelText(/Nuevo encargo/), {
      target: { value: "Lo que quiero yo" }
    });
    fireEvent.click(screen.getByRole("button", { name: /Cambiar el encargo/ }));

    await waitFor(() => expect(screen.getByRole("alert")).toBeTruthy());
    expect(screen.getByText(/va por la revisión 3/)).toBeTruthy();
    // Dos veces: como encargo vigente del run y dentro del aviso. Que la cabecera se
    // actualice es la mitad de la recuperación — quien mira ya está viendo lo que hay.
    expect(screen.getAllByText(/Lo que pidió Telegram/).length).toBeGreaterThan(0);
    expect(screen.getByText(/cambio de alcance/)).toBeTruthy();
    // Una sola escritura: el conflicto no dispara un segundo intento por su cuenta.
    expect(onRevisar).toHaveBeenCalledTimes(1);
  });

  it("repetir sobre la revisión nueva es una decisión que alguien toma", async () => {
    const onLeer = vi.fn().mockResolvedValue(objetivo());
    const onRevisar = vi
      .fn()
      .mockResolvedValueOnce({
        resultado: "conflicto",
        vigente: objetivo({ text: "Lo de Telegram", revision: 3 })
      } satisfies AthenaRevisionObjetivo)
      .mockResolvedValueOnce({
        resultado: "aceptada",
        objetivo: objetivo({ text: "Lo que quiero yo", revision: 4 })
      } satisfies AthenaRevisionObjetivo);

    await abrir(onLeer, onRevisar);
    fireEvent.change(screen.getByLabelText(/Nuevo encargo/), {
      target: { value: "Lo que quiero yo" }
    });
    fireEvent.click(screen.getByRole("button", { name: /Cambiar el encargo/ }));
    await waitFor(() => expect(screen.getByRole("alert")).toBeTruthy());

    fireEvent.click(screen.getByRole("button", { name: /Escribir igualmente/ }));

    await waitFor(() => expect(onRevisar).toHaveBeenCalledTimes(2));
    await waitFor(() =>
      expect(screen.getByText(/Athena lo recogerá al terminar la iteración/)).toBeTruthy()
    );
  });

  it("permite partir del encargo nuevo en vez de pisarlo", async () => {
    const onLeer = vi.fn().mockResolvedValue(objetivo());
    const onRevisar = vi.fn().mockResolvedValue({
      resultado: "conflicto",
      vigente: objetivo({ text: "Lo de Telegram", revision: 3 })
    } satisfies AthenaRevisionObjetivo);

    await abrir(onLeer, onRevisar);
    fireEvent.change(screen.getByLabelText(/Nuevo encargo/), {
      target: { value: "Lo que quiero yo" }
    });
    fireEvent.click(screen.getByRole("button", { name: /Cambiar el encargo/ }));
    await waitFor(() => expect(screen.getByRole("alert")).toBeTruthy());

    fireEvent.click(screen.getByRole("button", { name: /Partir del encargo nuevo/ }));

    const campo = screen.getByLabelText(/Nuevo encargo/) as HTMLTextAreaElement;
    expect(campo.value).toBe("Lo de Telegram");
    expect(onRevisar).toHaveBeenCalledTimes(1);
  });

  it("no manda un encargo vacío", async () => {
    const onLeer = vi.fn().mockResolvedValue(objetivo());
    const onRevisar = vi.fn();

    await abrir(onLeer, onRevisar);
    fireEvent.change(screen.getByLabelText(/Nuevo encargo/), { target: { value: "   " } });
    fireEvent.click(screen.getByRole("button", { name: /Cambiar el encargo/ }));

    expect(screen.getByText(/no puede quedarse vacío/)).toBeTruthy();
    expect(onRevisar).not.toHaveBeenCalled();
  });
});
