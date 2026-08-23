// @vitest-environment jsdom
/**
 * El aviso de recepción de un permiso, que es lo que arranca el reloj de pensar.
 *
 * Athena mide con dos relojes (ADR-017): uno corto de entrega —«¿ha llegado esto a una
 * pantalla?»— y uno largo de decisión que **sólo empieza cuando el cliente avisa**. Este
 * cliente avisaba al *responder* en vez de al *mostrar*, así que el largo no arrancaba
 * nunca y todo permiso tenía en la práctica treinta segundos.
 *
 * No es teórico: en el run del 22-ago-2026 cinco permisos murieron exactamente a los
 * 30,0 s mientras la persona los estaba leyendo, y el run acabó gastando su presupuesto
 * en negarse a sí mismo.
 */

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { AthenaArea } from "./AthenaArea";
import type { AthenaPermiso, AthenaRun } from "./domain";

const acknowledgeAthenaPermission = vi.fn().mockResolvedValue(undefined);
const getAthenaRun = vi.fn();
const startAthenaRun = vi.fn().mockResolvedValue("run-1");

vi.mock("./platform", () => ({
  platform: {
    getAthenaStatus: vi.fn().mockResolvedValue({
      estado: "conectado",
      urlBase: "http://127.0.0.1:8770",
      credencialConfigurada: true,
      runsActivos: 0
    }),
    listAthenaProfiles: vi.fn().mockResolvedValue({ default: "software_engineering", profiles: [] }),
    listAthenaModels: vi.fn().mockResolvedValue({ default: "", models: [] }),
    listAthenaRuns: vi.fn().mockResolvedValue([]),
    listAthenaRecoveryRuns: vi.fn().mockResolvedValue([]),
    listAthenaTrackedRuns: vi.fn().mockResolvedValue([]),
    listAthenaMemory: vi.fn().mockResolvedValue([]),
    getAthenaRunHistory: vi.fn().mockResolvedValue({ runId: "run-1", events: [] }),
    getAthenaGoal: vi.fn().mockResolvedValue({ text: "", revision: 1 }),
    startAthenaRun: (...args: unknown[]) => startAthenaRun(...args),
    getAthenaRun: (...args: unknown[]) => getAthenaRun(...args),
    acknowledgeAthenaPermission: (...args: unknown[]) => acknowledgeAthenaPermission(...args),
    resolveAthenaPermission: vi.fn().mockResolvedValue(undefined),
    cancelAthenaRun: vi.fn().mockResolvedValue(undefined),
    resumeAthenaRun: vi.fn().mockResolvedValue(undefined),
    fetchAthenaArtifact: vi.fn().mockResolvedValue(""),
    setAthenaCredential: vi.fn().mockResolvedValue(undefined),
    confirmAthenaMemory: vi.fn().mockResolvedValue(undefined),
    forgetAthenaMemory: vi.fn().mockResolvedValue(undefined),
    reviseAthenaGoal: vi.fn().mockResolvedValue({ aplicada: false, objetivo: null })
  }
}));

function permiso(parcial: Partial<AthenaPermiso> = {}): AthenaPermiso {
  return {
    requestId: "req-1",
    herramienta: "edit_file",
    operacion: "write",
    accion: "replace 1 occurrence(s) in calc.py",
    riesgo: "medium",
    nivel: "r1_workspace_write",
    motivo: "quiere escribir",
    efectos: ["Modifica calc.py"],
    recursos: ["calc.py"],
    workspace: "D:/repo",
    argumentos: [],
    soloLectura: false,
    destructivo: false,
    confirmado: false,
    segundosRestantes: 300,
    caducado: false,
    ...parcial
  };
}

function run(permisos: AthenaPermiso[]): AthenaRun {
  return {
    runId: "run-1",
    objetivo: "Arreglar calc.media",
    objetivoRevision: 1,
    perfilSolicitado: "",
    workspaceId: "ws-1",
    fase: "waiting_permission",
    carpeta: "D:/repo",
    degradado: false,
    reanudable: false,
    conectado: true,
    suscriptor: "sus-1",
    controla: true,
    tareas: [],
    delegados: [],
    herramientas: [],
    permisos,
    comprobaciones: [],
    ficherosModificados: [],
    artefactos: [],
    errores: [],
    actividad: [],
    evidencia: [],
    ciclosReparacion: 0
  };
}

const carpetas = [
  {
    id: "folder-1",
    displayName: "repo",
    path: "D:/repo",
    revokedAt: null,
    permissions: { athena: true }
  }
];

async function lanzar() {
  render(
    <AthenaArea
      carpetas={carpetas as never}
      carpetasCargando={false}
      carpetasError={null}
      onAutorizarCarpeta={vi.fn()}
    />
  );
  await waitFor(() => expect(screen.getByLabelText(/Carpeta autorizada/i)).toBeTruthy());
  await userEvent.type(screen.getByLabelText(/Objetivo/i), "arregla el bug");
  await userEvent.selectOptions(screen.getByLabelText(/Carpeta autorizada/i), "folder-1");
  await userEvent.click(screen.getByRole("button", { name: /Lanzar trabajo/i }));
  await waitFor(() => expect(startAthenaRun).toHaveBeenCalled());
}

beforeEach(() => {
  acknowledgeAthenaPermission.mockClear();
  startAthenaRun.mockClear();
  getAthenaRun.mockReset();
});

afterEach(cleanup);

describe("aviso de recepción de un permiso", () => {
  it("avisa a Athena en cuanto la pregunta se enseña, sin esperar a la respuesta", async () => {
    getAthenaRun.mockResolvedValue(run([permiso()]));

    await lanzar();

    await waitFor(() =>
      expect(acknowledgeAthenaPermission).toHaveBeenCalledWith("run-1", "req-1")
    );
    // Nadie ha contestado todavía: el aviso es de recepción, no de decisión, y la
    // pregunta sigue delante esperando a que la persona la lea.
    // Nadie ha contestado todavía: el aviso es de recepción, no de decisión, y la
    // pregunta sigue delante esperando a que la persona la lea.
    await waitFor(() =>
      expect(document.querySelector("section.athena-permiso")).not.toBeNull()
    );
    expect(screen.getByText("Athena necesita tu autorización")).toBeTruthy();
  });

  it("no repite el aviso mientras siga siendo la misma pregunta", async () => {
    // El sondeo devuelve la misma petición una y otra vez. Un segundo aviso no le cuenta
    // a Athena nada que el primero no dijera, y gastaría una petición por vuelta.
    getAthenaRun.mockResolvedValue(run([permiso()]));

    await lanzar();

    await waitFor(() => expect(acknowledgeAthenaPermission).toHaveBeenCalledTimes(1));
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(acknowledgeAthenaPermission).toHaveBeenCalledTimes(1);
  });

  it("no avisa cuando no hay ninguna pregunta pendiente", async () => {
    getAthenaRun.mockResolvedValue(run([]));

    await lanzar();

    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(acknowledgeAthenaPermission).not.toHaveBeenCalled();
  });
});
