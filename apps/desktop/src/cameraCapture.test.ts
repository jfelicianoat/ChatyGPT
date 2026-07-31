import { describe, expect, it } from "vitest";
import { cameraFailureMessage } from "./cameraCapture";

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
});
