import { describe, expect, it } from "vitest";
import {
  captureDisplayName,
  constrainedCaptureSize,
  normalizeCropSelection
} from "./screenCapture";

describe("screen capture preparation", () => {
  it("keeps normal displays unchanged and bounds very large captures", () => {
    expect(constrainedCaptureSize(1_920, 1_080)).toEqual({
      width: 1_920,
      height: 1_080
    });
    const large = constrainedCaptureSize(7_680, 4_320);
    expect(large.width).toBeLessThanOrEqual(2_560);
    expect(large.width * large.height).toBeLessThanOrEqual(4_000_000);
    expect(large.width / large.height).toBeCloseTo(16 / 9, 2);
  });

  it("creates a local, readable and deterministic capture name", () => {
    expect(captureDisplayName(new Date(2026, 6, 30, 9, 5, 7))).toBe(
      "captura-2026-07-30-090507.jpg"
    );
  });

  it("rejects invalid dimensions before allocating a canvas", () => {
    expect(() => constrainedCaptureSize(0, 1_080)).toThrow("dimensiones válidas");
    expect(() => constrainedCaptureSize(Number.NaN, 1_080)).toThrow("dimensiones válidas");
  });

  it("normalizes crop selections drawn in either direction", () => {
    const selection = normalizeCropSelection(0.8, 0.7, 0.2, 0.1);
    expect(selection?.x).toBeCloseTo(0.2);
    expect(selection?.y).toBeCloseTo(0.1);
    expect(selection?.width).toBeCloseTo(0.6);
    expect(selection?.height).toBeCloseTo(0.6);
  });

  it("bounds crop selections to the image and rejects accidental clicks", () => {
    expect(normalizeCropSelection(-0.2, 0.25, 1.4, 0.75)).toEqual({
      x: 0,
      y: 0.25,
      width: 1,
      height: 0.5
    });
    expect(normalizeCropSelection(0.3, 0.3, 0.305, 0.31)).toBeNull();
  });
});
