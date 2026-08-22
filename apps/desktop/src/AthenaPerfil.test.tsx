// @vitest-environment jsdom
/**
 * Pruebas del selector de perfil.
 *
 * Lo que se comprueba es que elegir sea una decisión informada: la lista viene de
 * Athena, se dice qué demuestra cada perfil, y no haber podido preguntar no se enseña
 * como «este Athena sólo tiene uno».
 */

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { AthenaPerfilSelector, nombreEvidencia } from "./AthenaPerfil";
import type { AthenaListadoPerfiles } from "./domain";

afterEach(cleanup);

const listado: AthenaListadoPerfiles = {
  default: "software_engineering",
  profiles: [
    {
      name: "software_engineering",
      subject: "a repository",
      evidence: "executed_checks",
      proves: "The project's own checks were executed and passed",
      tools: ["glob", "grep", "read_file", "bash"],
      description: "Trabajo sobre un repositorio"
    },
    {
      name: "documents",
      subject: "a folder of documents",
      evidence: "produced_artifacts",
      proves: "The deliverables exist and are not empty; it does not prove they are correct",
      tools: ["glob", "read_file", "write_file"],
      description: "Trabajo sobre documentos"
    }
  ]
};

describe("selector de perfil", () => {
  it("ofrece los perfiles que dice Athena y señala cuál es el de por defecto", async () => {
    const onListar = vi.fn().mockResolvedValue(listado);
    render(
      <AthenaPerfilSelector valor="" onCambiar={() => {}} onListar={onListar} />
    );

    await waitFor(() => expect(screen.getByLabelText(/Para qué es este trabajo/)).toBeTruthy());
    expect(screen.getByText(/El de por defecto de este Athena \(software_engineering\)/)).toBeTruthy();
    expect(screen.getByRole("option", { name: "documents" })).toBeTruthy();
  });

  it("dice qué demuestra el perfil elegido, incluido lo que no demuestra", async () => {
    // Un perfil puede dar evidencia más débil; lo que no puede es callárselo. Es la
    // mitad del contrato que hace que elegirlo sea una decisión y no una apuesta.
    const onListar = vi.fn().mockResolvedValue(listado);
    render(
      <AthenaPerfilSelector valor="documents" onCambiar={() => {}} onListar={onListar} />
    );

    await waitFor(() => expect(screen.getByText(/does not prove they are correct/)).toBeTruthy());
    expect(screen.getByText(/entregables existen y no están vacíos/)).toBeTruthy();
  });

  it("avisa de que el perfil queda fijado al crear el run", async () => {
    const onListar = vi.fn().mockResolvedValue(listado);
    render(<AthenaPerfilSelector valor="" onCambiar={() => {}} onListar={onListar} />);

    await waitFor(() => expect(screen.getByText(/queda fijado al crear/)).toBeTruthy());
  });

  it("no haber podido preguntar no se enseña como no tener perfiles", async () => {
    // Un desplegable vacío invitaría a creer que este Athena sólo ofrece uno, que es
    // una afirmación distinta de no haberlo podido consultar.
    const onListar = vi.fn().mockRejectedValue(new Error("el servicio no responde"));
    render(<AthenaPerfilSelector valor="" onCambiar={() => {}} onListar={onListar} />);

    await waitFor(() => expect(screen.getByRole("alert")).toBeTruthy());
    expect(screen.queryByLabelText(/Para qué es este trabajo/)).toBeNull();
  });

  it("traduce la clase de evidencia y respeta la que no conoce", () => {
    expect(nombreEvidencia("executed_checks")).toContain("comprobaciones del proyecto");
    expect(nombreEvidencia("produced_artifacts")).toContain("entregables");
    expect(nombreEvidencia("algo_nuevo")).toBe("algo_nuevo");
  });
});
