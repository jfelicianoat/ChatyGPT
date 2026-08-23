// @vitest-environment jsdom
/**
 * Pruebas del selector de modelo.
 *
 * Lo que se comprueba es que la elección sea real y honesta: la lista viene de Athena, un
 * despliegue sin elección no enseña un desplegable vacío, y no haber podido preguntar se
 * dice en vez de disfrazarse de «aquí no se elige».
 */

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AthenaModeloSelector } from "./AthenaModelo";
import type { AthenaListadoModelos } from "./domain";

afterEach(cleanup);

const listado: AthenaListadoModelos = {
  default: "qwen3.8:27b",
  models: [
    { name: "qwen3.8:27b", default: true },
    { name: "qwen3.6:35b", default: false },
    { name: "DeepSeek-V4-Pro", default: false }
  ]
};

describe("selector de modelo", () => {
  it("ofrece los modelos que dice Athena y señala cuál es el de por defecto", async () => {
    const onListar = vi.fn().mockResolvedValue(listado);
    render(<AthenaModeloSelector valor="" onCambiar={() => {}} onListar={onListar} />);

    await waitFor(() => expect(screen.getByLabelText(/Con qué modelo/)).toBeTruthy());
    expect(screen.getByText(/El de por defecto de este Athena \(qwen3\.8:27b\)/)).toBeTruthy();
    expect(screen.getByRole("option", { name: /DeepSeek-V4-Pro/ })).toBeTruthy();
    expect(screen.getByRole("option", { name: /qwen3\.8:27b \(por defecto\)/ })).toBeTruthy();
  });

  it("avisa de que un modelo elegido no se sustituye por otro", async () => {
    // Es la mitad del contrato que hace que elegir signifique algo. Sin decirlo, alguien
    // podría suponer que el broker enruta a lo que tenga libre, que es lo que hace cuando
    // NO se elige.
    const onListar = vi.fn().mockResolvedValue(listado);
    render(<AthenaModeloSelector valor="" onCambiar={() => {}} onListar={onListar} />);

    await waitFor(() => expect(screen.getByLabelText(/Con qué modelo/)).toBeTruthy());
    expect(screen.getByText(/no lo sustituya/)).toBeTruthy();
  });

  it("devuelve el nombre elegido tal cual", async () => {
    const onCambiar = vi.fn();
    const onListar = vi.fn().mockResolvedValue(listado);
    render(<AthenaModeloSelector valor="" onCambiar={onCambiar} onListar={onListar} />);

    await waitFor(() => expect(screen.getByLabelText(/Con qué modelo/)).toBeTruthy());
    await userEvent.selectOptions(screen.getByLabelText(/Con qué modelo/), "qwen3.6:35b");

    expect(onCambiar).toHaveBeenCalledWith("qwen3.6:35b");
  });

  it("no enseña selector cuando el despliegue no ofrece elección", async () => {
    // Athena contesta 404 y el cliente lo traduce a lista vacía. Un desplegable vacío
    // pediría una decisión que no existe.
    const onListar = vi.fn().mockResolvedValue({ default: "", models: [] });
    const { container } = render(
      <AthenaModeloSelector valor="" onCambiar={() => {}} onListar={onListar} />
    );

    await waitFor(() => expect(onListar).toHaveBeenCalled());
    expect(container.querySelector("select")).toBeNull();
  });

  it("tampoco lo enseña cuando sólo hay un modelo", async () => {
    const onListar = vi.fn().mockResolvedValue({
      default: "qwen3.8:27b",
      models: [{ name: "qwen3.8:27b", default: true }]
    });
    const { container } = render(
      <AthenaModeloSelector valor="" onCambiar={() => {}} onListar={onListar} />
    );

    await waitFor(() => expect(onListar).toHaveBeenCalled());
    expect(container.querySelector("select")).toBeNull();
  });

  it("dice que no pudo preguntar en vez de callarse", async () => {
    // No haber podido consultar y no haber elección son dos cosas distintas, y la
    // interfaz no puede enseñar la segunda cuando ha pasado la primera.
    const onListar = vi.fn().mockRejectedValue(new Error("Athena no responde"));
    render(<AthenaModeloSelector valor="" onCambiar={() => {}} onListar={onListar} />);

    await waitFor(() => expect(screen.getByRole("alert")).toBeTruthy());
    expect(screen.getByRole("alert").textContent).toContain("Athena no responde");
  });
});
