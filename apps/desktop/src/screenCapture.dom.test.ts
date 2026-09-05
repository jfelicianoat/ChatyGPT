// @vitest-environment jsdom
/**
 * Pruebas de la captura de pantalla contra un DOM real.
 *
 * Lo que aqui se comprueba no son las cuentas —eso ya lo cubre
 * `screenCapture.test.ts`— sino los caminos que el usuario nota cuando algo va
 * mal: la pantalla que nunca entrega imagen, el lienzo que no se puede crear y
 * el WebView2 sin captura. Son justo los tramos donde un fallo silencioso deja
 * la aplicacion sin decir nada.
 */
import { afterEach, describe, expect, it, vi } from "vitest";

import {
  captureScreenFrame,
  captureVideoFrame,
  cropCapturedFrame
} from "./screenCapture";

type Contexto = { drawImage: ReturnType<typeof vi.fn> };

/** Deja el lienzo de jsdom devolviendo un contexto de mentira y un JPEG vacio. */
function prepararLienzo(opciones: { contexto?: Contexto | null; blob?: Blob | null } = {}) {
  const contexto =
    opciones.contexto === undefined ? { drawImage: vi.fn() } : opciones.contexto;
  const blob = opciones.blob === undefined ? new Blob(["jpeg"], { type: "image/jpeg" }) : opciones.blob;
  const getContext = vi
    .spyOn(HTMLCanvasElement.prototype, "getContext")
    .mockReturnValue(contexto as unknown as CanvasRenderingContext2D);
  const toBlob = vi
    .spyOn(HTMLCanvasElement.prototype, "toBlob")
    .mockImplementation((callback) => callback(blob));
  return { contexto, getContext, toBlob };
}

/** Video con las dimensiones y el estado de carga que pida la prueba. */
function prepararVideo(readyState: number, ancho = 1_920, alto = 1_080) {
  const video = document.createElement("video");
  Object.defineProperty(video, "readyState", { value: readyState, configurable: true });
  Object.defineProperty(video, "videoWidth", { value: ancho, configurable: true });
  Object.defineProperty(video, "videoHeight", { value: alto, configurable: true });
  return video;
}

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("captura de un fotograma de video", () => {
  it("recorta la pantalla al maximo admitido y devuelve el JPEG", async () => {
    const { contexto } = prepararLienzo();
    const video = prepararVideo(HTMLMediaElement.HAVE_CURRENT_DATA, 7_680, 4_320);

    const frame = await captureVideoFrame(video, "captura.jpg");

    expect(frame.displayName).toBe("captura.jpg");
    expect(frame.width).toBeLessThanOrEqual(2_560);
    expect(frame.blob.type).toBe("image/jpeg");
    expect(contexto?.drawImage).toHaveBeenCalledOnce();
  });

  it("espera a que la pantalla entregue la primera imagen", async () => {
    prepararLienzo();
    const video = prepararVideo(HTMLMediaElement.HAVE_NOTHING, 1_280, 720);

    const pendiente = captureVideoFrame(video, "tardia.jpg");
    video.dispatchEvent(new Event("loadeddata"));

    await expect(pendiente).resolves.toMatchObject({ width: 1_280, height: 720 });
  });

  it("avisa cuando la pantalla elegida falla al abrirse", async () => {
    prepararLienzo();
    const video = prepararVideo(HTMLMediaElement.HAVE_NOTHING);

    const pendiente = captureVideoFrame(video, "rota.jpg");
    video.dispatchEvent(new Event("error"));

    await expect(pendiente).rejects.toThrow("No se pudo leer la pantalla seleccionada");
  });

  it("desiste si la pantalla no entrega imagen en diez segundos", async () => {
    vi.useFakeTimers();
    try {
      prepararLienzo();
      const video = prepararVideo(HTMLMediaElement.HAVE_NOTHING);
      const pendiente = captureVideoFrame(video, "muda.jpg");
      const comprobacion = expect(pendiente).rejects.toThrow("no entregó ninguna imagen");
      vi.advanceTimersByTime(10_000);
      await comprobacion;
    } finally {
      vi.useRealTimers();
    }
  });

  it("avisa si el WebView2 no da lienzo en el que dibujar", async () => {
    prepararLienzo({ contexto: null });
    const video = prepararVideo(HTMLMediaElement.HAVE_CURRENT_DATA);

    await expect(captureVideoFrame(video, "sin-lienzo.jpg")).rejects.toThrow(
      "lienzo de la captura"
    );
  });

  it("avisa si el lienzo no llega a producir el fichero", async () => {
    prepararLienzo({ blob: null });
    const video = prepararVideo(HTMLMediaElement.HAVE_CURRENT_DATA);

    await expect(captureVideoFrame(video, "sin-fichero.jpg")).rejects.toThrow(
      "No se pudo preparar la captura"
    );
  });
});

describe("captura de pantalla completa", () => {
  it("explica que el WebView2 instalado no sabe capturar", async () => {
    vi.stubGlobal("navigator", { mediaDevices: undefined });

    await expect(captureScreenFrame()).rejects.toThrow("no está disponible en esta versión");
  });

  it("cierra la pista de video aunque la captura termine bien", async () => {
    prepararLienzo();
    const stop = vi.fn();
    const stream = { getTracks: () => [{ stop }] };
    vi.stubGlobal("navigator", {
      mediaDevices: { getDisplayMedia: vi.fn().mockResolvedValue(stream) }
    });
    vi.spyOn(HTMLMediaElement.prototype, "play").mockResolvedValue(undefined);
    // jsdom no implementa `pause()`: sin esto la prueba pasa pero ensucia la salida.
    vi.spyOn(HTMLMediaElement.prototype, "pause").mockImplementation(() => {});
    Object.defineProperty(HTMLMediaElement.prototype, "readyState", {
      value: HTMLMediaElement.HAVE_CURRENT_DATA,
      configurable: true
    });
    Object.defineProperty(HTMLVideoElement.prototype, "videoWidth", {
      value: 1_600,
      configurable: true
    });
    Object.defineProperty(HTMLVideoElement.prototype, "videoHeight", {
      value: 900,
      configurable: true
    });

    const frame = await captureScreenFrame();

    expect(frame.displayName).toMatch(/^captura-.*\.jpg$/);
    expect(stop).toHaveBeenCalledOnce();
  });
});

describe("recorte de una captura", () => {
  /** `Image` de mentira que resuelve o falla en cuanto le asignan `src`. */
  function prepararImagen(exito: boolean) {
    class ImagenFalsa {
      onload: (() => void) | null = null;
      onerror: (() => void) | null = null;
      naturalWidth = 1_000;
      naturalHeight = 500;
      set src(_valor: string) {
        queueMicrotask(() => (exito ? this.onload?.() : this.onerror?.()));
      }
    }
    vi.stubGlobal("Image", ImagenFalsa);
    vi.stubGlobal("URL", {
      createObjectURL: vi.fn().mockReturnValue("blob:captura"),
      revokeObjectURL: vi.fn()
    });
  }

  it("devuelve solo la region marcada, en pixeles reales de la imagen", async () => {
    prepararLienzo();
    prepararImagen(true);
    const original = {
      blob: new Blob(["jpeg"], { type: "image/jpeg" }),
      width: 1_000,
      height: 500,
      displayName: "captura.jpg"
    };

    const recorte = await cropCapturedFrame(original, {
      x: 0.1,
      y: 0.2,
      width: 0.5,
      height: 0.25
    });

    expect(recorte).toMatchObject({ width: 500, height: 125, displayName: "captura.jpg" });
  });

  it("avisa si la captura guardada ya no se puede leer", async () => {
    prepararLienzo();
    prepararImagen(false);

    await expect(
      cropCapturedFrame(
        {
          blob: new Blob(["roto"]),
          width: 10,
          height: 10,
          displayName: "captura.jpg"
        },
        { x: 0, y: 0, width: 1, height: 1 }
      )
    ).rejects.toThrow("No se pudo leer la imagen");
  });
});
