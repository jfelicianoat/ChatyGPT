// @vitest-environment jsdom
/**
 * Pruebas del tema aplicado sobre un documento real.
 *
 * El calculo puro ya lo cubre `appearance.test.ts`. Aqui interesa lo otro: que
 * la preferencia sobreviva al cierre, que un WebView2 con el almacenamiento
 * bloqueado no tire la aplicacion, y que la barra de titulo de Windows cambie
 * de color con el tema en lugar de quedarse clara sobre fondo oscuro.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import {
  APPEARANCE_STORAGE_KEY,
  applyAppearancePreference,
  loadAppearancePreference,
  persistAppearancePreference,
  subscribeToSystemAppearance,
  systemPrefersDark
} from "./appearance";

/** `matchMedia` de mentira que responde lo que pida la prueba. */
function prepararMatchMedia(prefiereOscuro: boolean) {
  const listeners = new Set<() => void>();
  const media = {
    matches: prefiereOscuro,
    addEventListener: (_evento: string, oyente: () => void) => listeners.add(oyente),
    removeEventListener: (_evento: string, oyente: () => void) => listeners.delete(oyente)
  };
  vi.stubGlobal("matchMedia", vi.fn().mockReturnValue(media));
  return { listeners, media };
}

beforeEach(() => {
  window.localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  document.head.innerHTML = "";
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("preferencia guardada entre sesiones", () => {
  it("recupera la eleccion anterior y descarta un valor corrupto", () => {
    persistAppearancePreference("dark");
    expect(window.localStorage.getItem(APPEARANCE_STORAGE_KEY)).toBe("dark");
    expect(loadAppearancePreference()).toBe("dark");

    window.localStorage.setItem(APPEARANCE_STORAGE_KEY, "sepia");
    expect(loadAppearancePreference()).toBe("system");
  });

  it("sigue funcionando si WebView2 bloquea el almacenamiento", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("bloqueado", "SecurityError");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("bloqueado", "SecurityError");
    });

    expect(() => persistAppearancePreference("light")).not.toThrow();
    expect(loadAppearancePreference()).toBe("system");
  });
});

describe("tema aplicado al documento", () => {
  it("marca el documento y tine la barra de titulo en oscuro", () => {
    prepararMatchMedia(true);
    const meta = document.createElement("meta");
    meta.name = "theme-color";
    document.head.append(meta);

    expect(applyAppearancePreference("system")).toBe("dark");
    expect(document.documentElement.dataset.appearance).toBe("system");
    expect(document.documentElement.dataset.theme).toBe("dark");
    expect(document.documentElement.style.colorScheme).toBe("dark");
    expect(meta.getAttribute("content")).toBe("#171512");
  });

  it("respeta la eleccion explicita por encima de Windows", () => {
    prepararMatchMedia(true);
    const meta = document.createElement("meta");
    meta.name = "theme-color";
    document.head.append(meta);

    expect(applyAppearancePreference("light")).toBe("light");
    expect(meta.getAttribute("content")).toBe("#f3f0e9");
  });

  it("no falla en una pagina sin la etiqueta de color de barra", () => {
    prepararMatchMedia(false);

    expect(applyAppearancePreference("system")).toBe("light");
  });
});

describe("seguimiento del tema de Windows", () => {
  it("avisa cuando Windows cambia de tema y deja de hacerlo al soltar", () => {
    const { listeners } = prepararMatchMedia(true);
    const alCambiar = vi.fn();

    const soltar = subscribeToSystemAppearance(alCambiar);
    expect(listeners.size).toBe(1);
    listeners.forEach((oyente) => oyente());
    expect(alCambiar).toHaveBeenCalledOnce();

    soltar();
    expect(listeners.size).toBe(0);
  });

  it("da por claro el tema y no se suscribe si no hay `matchMedia`", () => {
    vi.stubGlobal("matchMedia", undefined);

    expect(systemPrefersDark()).toBe(false);
    expect(() => subscribeToSystemAppearance(vi.fn())()).not.toThrow();
  });
});
