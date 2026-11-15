// @vitest-environment jsdom
/**
 * CH-03: el hook de diálogos extraído de App.tsx se prueba suelta con
 * `renderHook`; la plataforma (Rust) se simula.
 */
import { act, renderHook, type RenderHookResult } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useDialogos } from "./useDialogos";
import type { DialogState } from "../dialogs";

type Opciones = Parameters<typeof useDialogos>[0];
type Resultado = RenderHookResult<ReturnType<typeof useDialogos>, Opciones>;

const platformMock = vi.hoisted(() => ({
  createProject: vi.fn(async (name: string) => ({ id: "proj-9", name })),
  renameProject: vi.fn(async () => undefined),
  archiveProject: vi.fn(async () => undefined),
  archiveConversation: vi.fn(async () => undefined),
  deleteConversation: vi.fn(async () => undefined)
}));

vi.mock("../platform", () => ({ platform: platformMock }));

afterEach(() => {
  vi.clearAllMocks();
});

const opciones = (patch: Partial<Parameters<typeof useDialogos>[0]> = {}) => ({
  selectedProjectId: "proj-1",
  setSelectedProjectId: vi.fn(),
  setConversation: vi.fn(),
  setActiveTurn: vi.fn(),
  setActiveTurnConversationId: vi.fn(),
  followConversationScrollRef: { current: true },
  reloadNavigation: vi.fn(async () => undefined),
  loadConversation: vi.fn(async () => true),
  openConversation: vi.fn(async () => undefined),
  setNavigationError: vi.fn(),
  ...patch
});

const abre = (resultado: Resultado, dialogo: DialogState) => {
  act(() => {
    resultado.result.current.openDialog(dialogo);
  });
};

describe("useDialogos", () => {
  it("abre el diálogo con el valor inicial y limpia el aviso de navegación", () => {
    const setNavigationError = vi.fn();
    const resultado = renderHook((props: Parameters<typeof useDialogos>[0]) => useDialogos(props), {
      initialProps: opciones({ setNavigationError })
    });
    expect(resultado.result.current.dialog).toBeNull();
    abre(resultado, { kind: "project-create" });
    expect(resultado.result.current.dialog).toEqual({ kind: "project-create" });
    expect(resultado.result.current.dialogValue).toBe("");
    expect(setNavigationError).toHaveBeenCalledWith(null);
  });

  it("no envía nada si no hay diálogo abierto", async () => {
    const opcionesBase = opciones();
    const resultado = renderHook((props: Parameters<typeof useDialogos>[0]) => useDialogos(props), {
      initialProps: opcionesBase
    });
    await act(async () => {
      await resultado.result.current.submitDialog();
    });
    expect(platformMock.createProject).not.toHaveBeenCalled();
    expect(opcionesBase.reloadNavigation).not.toHaveBeenCalled();
  });

  it("exige un nombre para crear un proyecto y al crearlo lo selecciona", async () => {
    const setSelectedProjectId = vi.fn();
    const resultado = renderHook((props: Parameters<typeof useDialogos>[0]) => useDialogos(props), {
      initialProps: opciones({ setSelectedProjectId })
    });
    abre(resultado, { kind: "project-create" });
    // Sin nombre (el diálogo no admite vacío): no envía.
    await act(async () => {
      await resultado.result.current.submitDialog();
    });
    expect(platformMock.createProject).not.toHaveBeenCalled();

    act(() => {
      resultado.result.current.setDialogValue("Cartera 2027");
    });
    await act(async () => {
      await resultado.result.current.submitDialog();
    });
    expect(platformMock.createProject).toHaveBeenCalledWith("Cartera 2027");
    expect(setSelectedProjectId).toHaveBeenCalledWith("proj-9");
    expect(resultado.result.current.dialog).toBeNull();
    expect(resultado.result.current.dialogBusy).toBe(false);
  });

  it("archivar la conversación abierta cierra el chat y recarga la navegación", async () => {
    const setConversation = vi.fn();
    const reloadNavigation = vi.fn(async () => undefined);
    const resultado = renderHook((props: Parameters<typeof useDialogos>[0]) => useDialogos(props), {
      initialProps: opciones({ setConversation, reloadNavigation })
    });
    abre(resultado, { kind: "conversation-archive", conversation: { id: "conv-1" } as never });
    await act(async () => {
      await resultado.result.current.submitDialog();
    });
    expect(platformMock.archiveConversation).toHaveBeenCalledWith("conv-1");
    expect(setConversation).toHaveBeenCalledWith(null);
    expect(reloadNavigation).toHaveBeenCalledTimes(1);
    expect(resultado.result.current.dialog).toBeNull();
  });

  it("si el envío falla, el aviso de navegación recoge el error y el diálogo se libera", async () => {
    const setNavigationError = vi.fn();
    platformMock.archiveProject.mockRejectedValueOnce(new Error("Broker sin respuesta"));
    const resultado = renderHook((props: Parameters<typeof useDialogos>[0]) => useDialogos(props), {
      initialProps: opciones({ setNavigationError })
    });
    abre(resultado, { kind: "project-archive", project: { id: "proj-1" } as never });
    await act(async () => {
      await resultado.result.current.submitDialog();
    });
    expect(setNavigationError).toHaveBeenCalledWith(expect.stringContaining("Broker sin respuesta"));
    expect(resultado.result.current.dialogBusy).toBe(false);
  });
});
