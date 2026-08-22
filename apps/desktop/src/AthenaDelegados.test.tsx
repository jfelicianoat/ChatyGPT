// @vitest-environment jsdom
/**
 * Pruebas del panel de delegados.
 *
 * Lo que se comprueba es qué se dice y qué no: que se vea de quién es cada delegado y
 * si se le puede volver a preguntar, y que no aparezca nada que Athena no publicó.
 */

import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";

import { AthenaDelegados } from "./AthenaDelegados";
import type { AthenaDelegado } from "./domain";

afterEach(cleanup);

function delegado(parcial: Partial<AthenaDelegado> = {}): AthenaDelegado {
  return {
    sesion: "sub-1",
    padre: "run-1",
    rol: "explorer",
    proveedor: "native",
    estado: "running",
    encargo: "Encontrar por qué falla calc.add",
    continuable: true,
    seguimientos: 0,
    actividad: [],
    ficheros: [],
    bloqueos: [],
    ...parcial
  };
}

describe("panel de delegados", () => {
  it("no se dibuja cuando el run no delegó nada", () => {
    const { container } = render(<AthenaDelegados delegados={[]} />);
    expect(container.firstChild).toBeNull();
  });

  it("dice quién ejecuta cada delegado", () => {
    // Athena admite proveedores que no son Athena desde la fase 2. Callar el proveedor
    // presentaría como propios delegados que no lo son.
    render(<AthenaDelegados delegados={[delegado({ proveedor: "native" })]} />);
    expect(screen.getByText(/lo ejecuta native/)).toBeTruthy();
  });

  it("distingue un delegado continuable de uno de un solo encargo", () => {
    render(
      <AthenaDelegados
        delegados={[
          delegado({ sesion: "sub-1", continuable: true, seguimientosRestantes: 2 }),
          delegado({ sesion: "sub-2", continuable: false })
        ]}
      />
    );
    expect(screen.getByText(/se le puede volver a preguntar \(2\)/)).toBeTruthy();
    expect(screen.getByText(/un solo encargo/)).toBeTruthy();
  });

  it("enseña el informe del delegado, que es un resumen y no su conversación", () => {
    render(
      <AthenaDelegados
        delegados={[delegado({ resumen: "El operador está invertido en calc.py:14" })]}
      />
    );
    expect(screen.getByText(/operador está invertido/)).toBeTruthy();
  });

  it("dice que una tarea está bloqueada sin llamarla fallida", () => {
    render(
      <AthenaDelegados
        delegados={[delegado({ tarea: "T02", bloqueos: ["Esperando a T01"], estado: "pending" })]}
      />
    );
    expect(screen.getByText(/Esperando a T01/)).toBeTruthy();
    expect(screen.queryByText(/Fallida/)).toBeNull();
  });

  it("no enseña proveedor cuando Athena no lo publicó", () => {
    render(<AthenaDelegados delegados={[delegado({ proveedor: "" })]} />);
    expect(screen.queryByText(/lo ejecuta/)).toBeNull();
  });
});
