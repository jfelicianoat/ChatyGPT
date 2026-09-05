import { afterEach, describe, expect, it, vi } from "vitest";
import { cameraFailureMessage, openCameraStream } from "./cameraCapture";

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("camera permission guidance", () => {
  it("turns camera denials into an actionable Windows message", () => {
    expect(cameraFailureMessage(new DOMException("denied", "NotAllowedError"))).toContain(
      "Privacidad y seguridad > Cámara"
    );
  });

  it("distinguishes a missing or busy camera", () => {
    expect(cameraFailureMessage(new DOMException("missing", "NotFoundError"))).toContain(
      "ninguna cámara"
    );
    expect(cameraFailureMessage(new DOMException("busy", "NotReadableError"))).toContain(
      "otra aplicación"
    );
  });

  it("distingue la camara que no admite el video pedido", () => {
    expect(
      cameraFailureMessage(new DOMException("constraints", "OverconstrainedError"))
    ).toContain("no admite la configuración");
  });

  it("deja pasar cualquier otro fallo tal y como llega", () => {
    expect(cameraFailureMessage(new Error("cable suelto"))).toBe("cable suelto");
    expect(cameraFailureMessage("sin camara")).toBe("sin camara");
  });
});

describe("apertura de la camara", () => {
  it("explica que este WebView2 no puede abrirla", async () => {
    vi.stubGlobal("navigator", { mediaDevices: undefined });

    await expect(openCameraStream()).rejects.toThrow("no está disponible en esta versión");
  });

  it("pide video sin audio y a la resolucion util para un adjunto", async () => {
    const getUserMedia = vi.fn().mockResolvedValue("stream");
    vi.stubGlobal("navigator", { mediaDevices: { getUserMedia } });

    await expect(openCameraStream()).resolves.toBe("stream");
    expect(getUserMedia).toHaveBeenCalledWith(
      expect.objectContaining({ audio: false })
    );
  });
});
