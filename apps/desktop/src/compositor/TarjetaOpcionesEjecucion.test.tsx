// @vitest-environment jsdom
/**
 * CH-03: la tarjeta de opciones de ejecución extraída de App.tsx se prueba
 * suelta, sin montar la aplicación entera.
 */

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";

import { TarjetaOpcionesEjecucion } from "./TarjetaOpcionesEjecucion";
import type { BrokerDiagnostic, ConversationExecutionPreferences, EffectiveExecutionPolicy, Loadable } from "../domain";

afterEach(cleanup);

const preferences = (patch: Partial<ConversationExecutionPreferences> = {}): ConversationExecutionPreferences => ({
  dataClassification: "internal",
  strategy: "single",
  preset: "fast",
  maxCostUsd: 0,
  longContext: "fail",
  priority: 100,
  ...patch
});

const policyValue = (patch: Partial<EffectiveExecutionPolicy> = {}): EffectiveExecutionPolicy => ({
  dataClassification: "internal",
  maxCostUsd: 0,
  strategy: "single",
  preset: "fast",
  priority: 100,
  longContext: "fail",
  sources: { dataClassification: "chat", maxCostUsd: "chat", routing: "chat" },
  destination: { host: "http://127.0.0.1:8787", localMachine: true, encrypted: false },
  notes: [],
  ...patch
});

const policy = (patch: Partial<EffectiveExecutionPolicy> = {}): Loadable<EffectiveExecutionPolicy> => ({
  state: "ready",
  value: policyValue(patch)
});

const broker = (patch: Partial<BrokerDiagnostic> = {}): Loadable<BrokerDiagnostic> =>
  ({
    state: "ready",
    value: {
      ready: true,
      strategies: ["single", "auto", "mixture_of_agents"],
      longContextMapReduce: true,
      ...patch
    }
  }) as Loadable<BrokerDiagnostic>;

const base = (patch: Partial<Parameters<typeof TarjetaOpcionesEjecucion>[0]> = {}) => ({
  policy: policy(),
  preferences: preferences(),
  appliedPreferences: policyValue(),
  updatePreferences: vi.fn(),
  optionsBusy: false,
  turnBlocks: false,
  broker: broker(),
  ...patch
});

/** El select de una casilla de la rejilla: el label la envuelve y su texto
 * completo incluye las opciones, así que se busca por el span de la etiqueta. */
const selectPorEtiqueta = (etiqueta: string): HTMLSelectElement => {
  const grid = document.querySelector(".execution-settings-grid") as HTMLElement;
  const casilla = Array.from(grid.querySelectorAll(":scope > label")).find(
    (label) => label.querySelector("span")?.textContent === etiqueta
  );
  if (!casilla) throw new Error(`Sin casilla «${etiqueta}»`);
  return casilla.querySelector("select") as HTMLSelectElement;
};

it("opciones: el resumen del desplegable enseña estrategia, privacidad, coste y prioridad", () => {
  render(
    <TarjetaOpcionesEjecucion
      {...base({
        appliedPreferences: policyValue({
          strategy: "mixture_of_agents",
          dataClassification: "confidential",
          maxCostUsd: 0.5,
          priority: 250
        })
      })}
    />
  );
  const resumen = screen.getByTitle("Lo que se aplicará al próximo mensaje");
  expect(resumen.textContent).toContain("Análisis en equipo");
  expect(resumen.textContent).toContain("Confidencial");
  expect(resumen.textContent).toContain("hasta 0.50 USD");
  expect(resumen.textContent).toContain("Prioridad baja");
});

it("opciones: con la ruta fijada por el GPT bloquea estrategia y prioridad con su aviso", () => {
  render(
    <TarjetaOpcionesEjecucion
      {...base({ policy: policy({ sources: { dataClassification: "gpt", maxCostUsd: "gpt", routing: "gpt" }, customGptName: "Analista" }) })}
    />
  );
  const estrategia = selectPorEtiqueta("Forma de responder");
  expect(estrategia.disabled).toBe(true);
  expect(estrategia.getAttribute("title")).toBe("La fija la versión publicada del GPT");
  const prioridad = selectPorEtiqueta("Prioridad en la cola");
  expect(prioridad.disabled).toBe(true);
});

it("opciones: solo se ofrecen las estrategias que el Broker anuncia", () => {
  render(
    <TarjetaOpcionesEjecucion
      {...base({
        broker: broker({ strategies: ["single"] })
      })}
    />
  );
  const opciones = selectPorEtiqueta("Forma de responder").querySelectorAll("option");
  const auto = Array.from(opciones).find((option) => option.value === "auto");
  const moa = Array.from(opciones).find((option) => option.value === "mixture_of_agents");
  expect(auto?.disabled).toBe(true);
  expect(moa?.disabled).toBe(true);
});

it("opciones: cambiar la privacidad pide el cambio con el valor elegido", async () => {
  const user = userEvent.setup();
  const updatePreferences = vi.fn();
  render(<TarjetaOpcionesEjecucion {...base({ updatePreferences })} />);
  await user.selectOptions(selectPorEtiqueta("Privacidad"), "local_only");
  expect(updatePreferences).toHaveBeenCalledWith({ dataClassification: "local_only" });
});

it("opciones: el aviso de destino del Broker se ve en la política y en la privacidad", () => {
  render(
    <TarjetaOpcionesEjecucion
      {...base({
        policy: policy({
          destination: { host: "https://broker.example", localMachine: false, encrypted: true, warning: "Sale de este equipo" }
        })
      })}
    />
  );
  expect(screen.getByText("Sale de este equipo").className).toContain("policy-warning");
  const etiquetaPrivacidad = selectPorEtiqueta("Privacidad").closest("label");
  expect(etiquetaPrivacidad?.textContent).toContain("https://broker.example");
});
