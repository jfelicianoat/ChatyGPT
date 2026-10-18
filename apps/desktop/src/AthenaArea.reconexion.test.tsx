// @vitest-environment jsdom
/**
 * Auditoría QA del 14-oct (INC-08): con Athena caído, perfiles, modelos e
 * historial enseñan su error; al volver a comprobar con el servicio ya en
 * marcha, el estado decía «disponible» pero los errores seguían pintados hasta
 * salir del área y volver.
 */

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";

import { AthenaArea } from "./AthenaArea";

let servicioArriba = false;
const caido = () => Promise.reject(new Error("no se pudo conectar con 127.0.0.1:8770"));

vi.mock("./platform", () => ({
  platform: {
    getAthenaStatus: vi.fn(async () => ({
      estado: servicioArriba ? "conectado" : "no_disponible",
      urlBase: "http://127.0.0.1:8770",
      credencialConfigurada: true,
      runsActivos: 0
    })),
    listAthenaProfiles: vi.fn(async () =>
      servicioArriba
        ? {
            default: "software_engineering",
            profiles: [
              { name: "software_engineering", description: "Ingeniería", subject: "repositorio", tools: [] }
            ]
          }
        : caido()
    ),
    listAthenaModels: vi.fn(async () =>
      servicioArriba ? { default: "", models: [] } : caido()
    ),
    listAthenaRuns: vi.fn(async () => (servicioArriba ? [] : caido())),
    listAthenaRecoveryRuns: vi.fn().mockResolvedValue([]),
    listAthenaTrackedRuns: vi.fn().mockResolvedValue([])
  }
}));

afterEach(() => {
  cleanup();
  servicioArriba = false;
});

it("al volver a comprobar con el servicio arrancado desaparecen los errores obsoletos", async () => {
  render(
    <AthenaArea carpetas={[]} carpetasCargando={false} carpetasError={null} onAutorizarCarpeta={vi.fn()} />
  );
  expect(await screen.findByText(/No se pudo consultar los perfiles/)).toBeTruthy();

  servicioArriba = true;
  await userEvent.click(screen.getByRole("button", { name: "Volver a comprobar" }));

  await waitFor(() => expect(screen.queryByText(/No se pudo consultar/)).toBeNull());
  expect(await screen.findByLabelText("Para qué es este trabajo")).toBeTruthy();
});
