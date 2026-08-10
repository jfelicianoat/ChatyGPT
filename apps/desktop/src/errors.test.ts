import { describe, expect, it } from "vitest";
import { describeError } from "./errors";

describe("mensajes de error visibles", () => {
  it("usa el mensaje del error, no su representación técnica", () => {
    expect(describeError(new Error("Broker AI no está accesible"))).toBe(
      "Broker AI no está accesible"
    );
  });

  it("conserva el mensaje de los errores derivados", () => {
    class BrokerError extends Error {}
    expect(describeError(new BrokerError("HTTP 503"))).toBe("HTTP 503");
  });

  it("convierte a texto lo que se lanza sin ser un Error", () => {
    // Las órdenes de Tauri rechazan con la cadena que serializa `AppError`,
    // no con una instancia de Error: es el caso más frecuente en esta
    // aplicación, no una rareza.
    expect(describeError("datos no válidos: la métrica no es válida")).toBe(
      "datos no válidos: la métrica no es válida"
    );
    expect(describeError(404)).toBe("404");
  });

  it("no deja la pantalla en blanco ante un fallo sin forma", () => {
    expect(describeError(null)).toBe("null");
    expect(describeError(undefined)).toBe("undefined");
    expect(describeError({ code: "X" })).toBe("[object Object]");
  });
});
