// @vitest-environment jsdom
/**
 * Pruebas de interfaz sobre las acciones sensibles.
 *
 * El 5 de agosto de 2026 se descubrió que cinco acciones enviaban
 * `confirmed: true` a Rust sin haber preguntado a nadie: la comprobación del
 * backend existía, pero el frontend la satisfacía por su cuenta. La prueba de
 * contrato en Python impide que vuelva a ocurrir leyendo el código fuente; esta
 * lo comprueba desde el otro lado, **ejecutando la interfaz**: monta la
 * aplicación, pulsa el botón real y verifica que cancelar no ejecuta nada.
 *
 * Es la diferencia entre «la confirmación está escrita» y «la confirmación
 * funciona». Ambas comprobaciones se complementan: un análisis estático no
 * detecta que la pregunta se ignore, y esta no detecta una ruta que nadie use.
 */

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/** Respuestas por defecto que permiten montar la aplicación sin Broker real. */
const DEFAULTS: Record<string, unknown> = {
  bootstrap: {
    appVersion: "0.1.0",
    databasePath: "C:/pruebas/chatygpt.db",
    logPath: null,
    schemaVersion: 18,
    recoveredTasks: 0,
    recoveredAttachments: 0,
    recoveryItems: []
  },
  diagnoseBroker: {
    reachable: true,
    ready: true,
    baseUrl: "http://127.0.0.1:8765",
    contractVersion: "2.7",
    strategies: ["single"],
    presets: {},
    workLanes: ["inference"],
    agentSkills: [],
    latencyMs: 4,
    message: "Broker AI está listo"
  },
  getWindowsStartupStatus: {
    supported: true,
    enabled: false,
    credentialProtected: true,
    message: "Disponible"
  },
  getBrokerCredential: {
    source: "protected",
    protected: true,
    environmentPresent: false,
    message: "Credencial cifrada para tu cuenta de Windows."
  },
  listAuthorizedFolders: [
    {
      id: "folder-1",
      path: "D:/Exportaciones",
      displayName: "D:/Exportaciones",
      permissions: { write: true, purpose: "export" },
      grantedAt: "2026-08-01T10:00:00Z",
      revokedAt: null
    }
  ],
  getMemoryOverview: { enabled: false, items: [] },
  getLatestMemorySearch: null,
  getPerformanceReport: {
    sampleLimit: 200,
    totalSamples: 12,
    metrics: [
      {
        metric: "app_start",
        label: "Arranque de la aplicación",
        description: "Desde que la vista web empieza a cargar.",
        budgetMs: 2000,
        samples: 12,
        p50Ms: 900,
        p95Ms: 1400,
        maxMs: 1800,
        meetsBudget: true,
        lastRecordedAt: "2026-08-05T09:00:00Z"
      }
    ]
  }
};

/**
 * Doble de `platform` que registra cada llamada.
 *
 * Se usa un proxy para no tener que declarar las más de cien órdenes: las que
 * la prueba no necesita devuelven una lista vacía, que es lo que la interfaz
 * espera de casi todas ellas.
 */
const callLog = new Map<string, ReturnType<typeof vi.fn>>();

function platformMethod(name: string) {
  let mock = callLog.get(name);
  if (!mock) {
    mock = vi.fn(async () =>
      Object.prototype.hasOwnProperty.call(DEFAULTS, name) ? DEFAULTS[name] : []
    );
    callLog.set(name, mock);
  }
  return mock;
}

vi.mock("./platform", () => ({
  platform: new Proxy(
    {},
    {
      get: (_target, property: string) => platformMethod(property)
    }
  )
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    onDragDropEvent: async () => () => undefined
  })
}));

import { App } from "./App";

/** Espera a que el arranque termine y la pantalla de Inicio esté montada. */
async function mountHome() {
  render(<App />);
  await waitFor(() => expect(platformMethod("bootstrap")).toHaveBeenCalled());
  await screen.findByRole("heading", { name: "Credencial de Broker AI" });
}

describe("navegación principal simplificada", () => {
  beforeEach(() => {
    callLog.clear();
    vi.restoreAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("cambia de área sin mezclar los paneles de cada destino", async () => {
    await mountHome();

    expect(screen.getByRole("button", { name: "Chats" }).getAttribute("aria-current"))
      .toBe("page");
    expect(document.querySelector(".home-chats")).not.toBeNull();

    await userEvent.click(screen.getByRole("button", { name: "Proyectos" }));

    expect(screen.getByRole("button", { name: "Proyectos" }).getAttribute("aria-current"))
      .toBe("page");
    expect(document.querySelector(".home-projects")).not.toBeNull();
    expect(screen.getByRole("heading", { name: "Proyectos" })).toBeDefined();
  });
});

describe("arranque de los paneles de seguridad", () => {
  beforeEach(() => {
    callLog.clear();
    vi.restoreAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  /**
   * Defecto encontrado por esta prueba el 5 de agosto de 2026.
   *
   * La credencial y las carpetas autorizadas solo se cargaban desde
   * `reloadNavigation`, que se ejecuta después de una acción de la persona.
   * Al abrir la aplicación y no hacer nada, ambos paneles se quedaban
   * cargando indefinidamente, de modo que quien solo quisiera revisar su
   * credencial o revocar una carpeta no llegaba a verlas nunca.
   */
  it("carga credencial y carpetas autorizadas sin necesidad de actuar antes", async () => {
    await mountHome();

    await waitFor(() =>
      expect(platformMethod("getBrokerCredential")).toHaveBeenCalled()
    );
    expect(platformMethod("listAuthorizedFolders")).toHaveBeenCalled();

    // No basta con haberlas pedido: deben estar visibles.
    expect(await screen.findByRole("button", { name: "Retirar" })).toBeDefined();
    expect(await screen.findByRole("button", { name: "Revocar" })).toBeDefined();
    expect(screen.queryByText("Comprobando credencial…")).toBeNull();
    expect(screen.queryByText("Cargando permisos…")).toBeNull();
  });
});

describe("acciones sensibles en la interfaz", () => {
  beforeEach(() => {
    callLog.clear();
    vi.restoreAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("no retira la credencial si la persona cancela la confirmación", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    await mountHome();

    await userEvent.click(screen.getByRole("button", { name: "Retirar" }));

    expect(confirm).toHaveBeenCalledOnce();
    expect(platformMethod("clearBrokerCredential")).not.toHaveBeenCalled();
  });

  it("retira la credencial solo después de aceptar", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    await mountHome();

    await userEvent.click(screen.getByRole("button", { name: "Retirar" }));

    expect(confirm).toHaveBeenCalledOnce();
    await waitFor(() =>
      expect(platformMethod("clearBrokerCredential")).toHaveBeenCalledOnce()
    );
    // La confirmación precede a la llamada, no al revés.
    expect(confirm.mock.invocationCallOrder[0]).toBeLessThan(
      platformMethod("clearBrokerCredential").mock.invocationCallOrder[0]
    );
  });

  it("no revoca una carpeta autorizada si la persona cancela", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    await mountHome();

    await userEvent.click(screen.getByRole("button", { name: "Revocar" }));

    expect(confirm).toHaveBeenCalledOnce();
    expect(platformMethod("revokeAuthorizedFolder")).not.toHaveBeenCalled();
  });

  it("no vacía las mediciones de rendimiento si la persona cancela", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    await mountHome();

    await userEvent.click(screen.getByRole("button", { name: "Vaciar mediciones" }));

    expect(confirm).toHaveBeenCalledOnce();
    expect(platformMethod("clearPerformanceSamples")).not.toHaveBeenCalled();
  });
});
