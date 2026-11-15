// @vitest-environment jsdom
/**
 * CH-03: las tarjetas de ajustes extraídas de App.tsx se prueban sueltas, sin
 * montar la aplicación entera.
 */

import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";

import { TarjetaActividad } from "./TarjetaActividad";
import { TarjetaApariencia } from "./TarjetaApariencia";
import { TarjetaEstadoYBroker } from "./TarjetaEstadoYBroker";
import { TarjetaImagenesDocumentos } from "./TarjetaImagenesDocumentos";
import { TarjetaProyectos } from "./TarjetaProyectos";
import { TarjetaCarpetasAutorizadas } from "./TarjetaCarpetasAutorizadas";
import { TarjetaCredencialBroker } from "./TarjetaCredencialBroker";
import { TarjetaRendimiento } from "./TarjetaRendimiento";

afterEach(cleanup);

it("rendimiento: sin muestras no deja vaciar y con error lo anuncia", () => {
  const vaciar = vi.fn();
  const { rerender } = render(
    <TarjetaRendimiento
      performanceReport={{ state: "ready", value: { metrics: [], sampleLimit: 200, totalSamples: 0 } }}
      performanceBusy={false}
      clearPerformanceSamples={vaciar}
    />
  );
  expect((screen.getByRole("button", { name: "Vaciar mediciones" }) as HTMLButtonElement).disabled).toBe(true);
  rerender(
    <TarjetaRendimiento
      performanceReport={{ state: "error", message: "sin acceso" }}
      performanceBusy={false}
      clearPerformanceSamples={vaciar}
    />
  );
  expect(screen.getByRole("alert").textContent).toContain("sin acceso");
});

it("credencial: no guarda un token vacío y retira solo la protegida", async () => {
  const guardar = vi.fn();
  const retirar = vi.fn();
  const escribir = vi.fn();
  render(
    <TarjetaCredencialBroker
      brokerCredential={{
        state: "ready",
        value: { source: "protected", protected: true, environmentPresent: false, message: "Cifrada." }
      }}
      credentialDraft=""
      setCredentialDraft={escribir}
      credentialBusy={false}
      credentialNotice={null}
      saveBrokerCredential={guardar}
      removeBrokerCredential={retirar}
    />
  );
  expect((screen.getByRole("button", { name: "Guardar credencial" }) as HTMLButtonElement).disabled).toBe(true);
  await userEvent.type(screen.getByLabelText("Token administrativo"), "x");
  expect(escribir).toHaveBeenCalledWith("x");
  await userEvent.click(screen.getByRole("button", { name: "Retirar" }));
  expect(retirar).toHaveBeenCalledOnce();
});

it("carpetas: una revocada no ofrece revocar y una vigente revoca la suya", async () => {
  const revocar = vi.fn();
  const base = { path: "C:\\x", permissions: { read: true }, grantedAt: "2026-11-01" };
  render(
    <TarjetaCarpetasAutorizadas
      authorizedFolders={{
        state: "ready",
        value: [
          { ...base, id: "a", displayName: "Vigente", revokedAt: null },
          { ...base, id: "b", displayName: "Antigua", revokedAt: "2026-11-02" }
        ]
      }}
      folderBusy={null}
      authorizeGptReadFolder={vi.fn()}
      authorizeGptModifyFolder={vi.fn()}
      revokeFolder={revocar}
    />
  );
  const botones = screen.getAllByRole("button", { name: "Revocar" });
  expect(botones).toHaveLength(1);
  await userEvent.click(botones[0]);
  expect(revocar).toHaveBeenCalledWith("a");
});

it("actividad: lista vacía avisa y el botón actualiza", async () => {
  const actualizar = vi.fn();
  render(<TarjetaActividad auditEvents={{ state: "ready", value: [] }} refreshAuditEvents={actualizar} />);
  expect(screen.queryByText("Todavía no hay actividad registrada.")).not.toBeNull();
  await userEvent.click(screen.getByRole("button", { name: "Actualizar" }));
  expect(actualizar).toHaveBeenCalledOnce();
});

it("apariencia: marca la opción elegida y avisa del tema visible", async () => {
  const elegir = vi.fn();
  render(<TarjetaApariencia appearancePreference="system" setAppearancePreference={elegir} resolvedAppearance="dark" />);
  expect(screen.getByRole("radio", { name: /Windows/ }).getAttribute("aria-checked")).toBe("true");
  expect(screen.queryByText(/Tema visible: oscuro/)).not.toBeNull();
  await userEvent.click(screen.getByRole("radio", { name: /Claro/ }));
  expect(elegir).toHaveBeenCalledWith("light");
});

it("imágenes en documentos: cambia la preferencia", async () => {
  const elegir = vi.fn();
  render(<TarjetaImagenesDocumentos imageDescriptionPreference="describe" setImageDescriptionPreference={elegir} />);
  await userEvent.click(screen.getByRole("radio", { name: /Sin imágenes/ }));
  expect(elegir).toHaveBeenCalledWith("ignore");
});

it("proyectos: «Ver chats» selecciona el proyecto y va a los chats", async () => {
  const seleccionar = vi.fn();
  const ir = vi.fn();
  const proyecto = { id: "p1", name: "TFM", conversationCount: 3 } as never;
  render(
    <TarjetaProyectos
      projects={[proyecto]}
      openDialog={vi.fn()}
      setSelectedProjectId={seleccionar}
      setWorkspaceDestination={ir}
      openProjectKnowledge={vi.fn()}
    />
  );
  await userEvent.click(screen.getByRole("button", { name: "Ver chats" }));
  expect(seleccionar).toHaveBeenCalledWith("p1");
  expect(ir).toHaveBeenCalledWith("chats");
});

it("estado y Broker: comprobar conexión llama al diagnóstico y muestra el error", async () => {
  const comprobar = vi.fn();
  render(
    <TarjetaEstadoYBroker
      bootstrap={{ state: "error", message: "base bloqueada" }}
      conversations={[]}
      projects={[]}
      broker={{ state: "error", message: "sin respuesta" }}
      checkBroker={comprobar}
    />
  );
  expect(screen.queryByText("base bloqueada")).not.toBeNull();
  expect(screen.queryByText("sin respuesta")).not.toBeNull();
  await userEvent.click(screen.getByRole("button", { name: "Comprobar conexión" }));
  expect(comprobar).toHaveBeenCalledOnce();
});
