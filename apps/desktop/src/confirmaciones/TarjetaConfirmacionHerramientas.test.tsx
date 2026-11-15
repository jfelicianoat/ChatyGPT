// @vitest-environment jsdom
/**
 * CH-03: la tarjeta de confirmación de herramientas extraída de App.tsx se
 * prueba suelta, con un objeto con la misma forma que devuelve
 * `useConfirmaciones`.
 */

import { useState } from "react";
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";

import { TarjetaConfirmacionHerramientas } from "./TarjetaConfirmacionHerramientas";
import type { useConfirmaciones } from "./useConfirmaciones";
import type { LocalTaskSnapshot, ToolCallView } from "../domain";

afterEach(cleanup);

const call = (patch: Partial<ToolCallView> = {}): ToolCallView => ({
  toolCallId: "call-1",
  name: "read_authorized_file",
  arguments: {},
  status: "waiting_approval",
  confirmation: {
    toolName: "Leer archivo autorizado",
    resources: { label: "D:\\Notas\\diario.md" },
    consequences: "El contenido del fichero entra en el contexto del turno.",
    disclosure: {
      action_label: "Leer un archivo de una carpeta autorizada",
      destination_label: "Solo en este equipo",
      scope_label: "Permitir una vez",
      data_sent: [{ label: "Ruta", value: "D:\\Notas\\diario.md" }]
    }
  },
  ...patch
} as unknown as ToolCallView);

const turn = (calls: ToolCallView[]): LocalTaskSnapshot =>
  ({ id: "task-1", pendingToolCalls: calls } as unknown as LocalTaskSnapshot);

const baseConfirmaciones = (
  patch: Partial<ReturnType<typeof useConfirmaciones>> = {}
): ReturnType<typeof useConfirmaciones> =>
  ({
    toolDecisions: {},
    setToolDecisions: vi.fn(),
    toolDecisionBusy: false,
    submitToolDecisions: vi.fn(),
    ...patch
  }) as ReturnType<typeof useConfirmaciones>;

/** Puente con decisiones de verdad: las decisiones viven en estado, como en el hook. */
function TarjetaConEstado({
  turn: turno,
  submitToolDecisions,
  toolDecisionBusy = false
}: {
  turn: LocalTaskSnapshot;
  submitToolDecisions?: ReturnType<typeof useConfirmaciones>["submitToolDecisions"];
  toolDecisionBusy?: boolean;
}) {
  const [toolDecisions, setToolDecisions] = useState<Record<string, boolean>>({});
  return (
    <TarjetaConfirmacionHerramientas
      confirmaciones={{
        toolDecisions,
        setToolDecisions,
        toolDecisionBusy,
        submitToolDecisions: submitToolDecisions ?? vi.fn()
      }}
      turn={turno}
    />
  );
}

it("confirmaciones: presenta cada propuesta en lenguaje llano, sin JSON técnico", () => {
  render(
    <TarjetaConfirmacionHerramientas
      confirmaciones={baseConfirmaciones()}
      turn={turn([call()])}
    />
  );
  const seccion = screen.getByRole("region", { name: "Confirmación de herramientas" });
  expect(seccion.textContent).toContain("ChatyGPT quiere realizar una acción");
  expect(seccion.textContent).toContain("Leer un archivo de una carpeta autorizada");
  expect(seccion.textContent).toContain("D:\\Notas\\diario.md");
  expect(seccion.textContent).toContain("Solo en este equipo");
  expect(seccion.textContent).not.toContain("toolCallId");
});

it("confirmaciones: sin decidir todo, el botón de confirmar sigue deshabilitado", () => {
  render(
    <TarjetaConfirmacionHerramientas
      confirmaciones={baseConfirmaciones({ toolDecisions: { "call-1": true } })}
      turn={turn([call(), call({ toolCallId: "call-2" })])}
    />
  );
  const confirmar = screen.getByRole("button", { name: "Confirmar decisiones y continuar" }) as HTMLButtonElement;
  expect(confirmar.disabled).toBe(true);
});

it("confirmaciones: decidir cada propuesta habilita confirmar y envía al reanudar", async () => {
  const user = userEvent.setup();
  const submit = vi.fn();
  render(<TarjetaConEstado turn={turn([call()])} submitToolDecisions={submit} />);
  const confirmar = screen.getByRole("button", { name: "Confirmar decisiones y continuar" }) as HTMLButtonElement;
  expect(confirmar.disabled).toBe(true);

  await user.click(screen.getByRole("button", { name: "Autorizar una vez" }));
  expect(confirmar.disabled).toBe(false);
  const rechazar = screen.getByRole("button", { name: "Rechazar" });
  expect(rechazar.className).not.toContain("selected");
  expect(screen.getByRole("button", { name: "Autorizar una vez" }).className).toContain("selected");

  await user.click(confirmar);
  expect(submit).toHaveBeenCalledTimes(1);
});

it("confirmaciones: mientras se reanuda el turno, el botón lo dice y no repite", async () => {
  const user = userEvent.setup();
  const submit = vi.fn();
  render(
    <TarjetaConEstado
      turn={turn([call()])}
      submitToolDecisions={submit}
      toolDecisionBusy={true}
    />
  );
  const reanudando = screen.getByRole("button", { name: "Reanudando…" }) as HTMLButtonElement;
  expect(reanudando.disabled).toBe(true);
  await user.click(reanudando);
  expect(submit).not.toHaveBeenCalled();
});

it("confirmaciones: rechazar marca la opción rechazada", async () => {
  const user = userEvent.setup();
  const setDecisiones = vi.fn();
  render(
    <TarjetaConfirmacionHerramientas
      confirmaciones={baseConfirmaciones({ setToolDecisions: setDecisiones })}
      turn={turn([call()])}
    />
  );
  await user.click(screen.getByRole("button", { name: "Rechazar" }));
  expect(setDecisiones).toHaveBeenCalledTimes(1);
  const actualizador = setDecisiones.mock.calls[0][0] as (values: Record<string, boolean>) => Record<string, boolean>;
  expect(actualizador({})).toEqual({ "call-1": false });
});
