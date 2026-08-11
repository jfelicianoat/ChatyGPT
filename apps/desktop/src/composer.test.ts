import { describe, expect, it } from "vitest";
import {
  sandboxDeniedByCustomGpt,
  sandboxDiagnosticFailure,
  sandboxSendDecision
} from "./composer";

describe("permiso del GPT sobre Código aislado", () => {
  it("rechaza solo cuando el turno lo usa y el GPT lo tiene denegado", () => {
    expect(
      sandboxDeniedByCustomGpt({ useSandbox: true, gptAllowsRunCode: false })
    ).not.toBeNull();
    // Si el turno no usa sandbox, el permiso del GPT es irrelevante.
    expect(
      sandboxDeniedByCustomGpt({ useSandbox: false, gptAllowsRunCode: false })
    ).toBeNull();
    expect(
      sandboxDeniedByCustomGpt({ useSandbox: true, gptAllowsRunCode: true })
    ).toBeNull();
  });

  it("explica qué hacer, no solo que no se puede", () => {
    const error = sandboxDeniedByCustomGpt({
      useSandbox: true,
      gptAllowsRunCode: false
    });
    expect(error?.action).toContain("Edita el GPT");
  });

  it("no confunde capacidades no verificadas con sandbox ausente", () => {
    expect(sandboxSendDecision({
      skipSuggestion: false,
      useSandbox: false,
      attachmentsNeedSandbox: false,
      requestsCodeExecution: true,
      sandboxAvailable: false,
      sandboxCapabilityKnown: false
    })).toEqual({ kind: "suggest-sandbox" });
  });
});

describe("fallo al diagnosticar el sandbox", () => {
  it("deja claro que el mensaje no llegó a enviarse", () => {
    const error = sandboxDiagnosticFailure("Broker AI no está accesible");
    expect(error.detail).toBe("Broker AI no está accesible");
    expect(error.action).toContain("no se ha enviado");
  });
});

describe("decisión de envío", () => {
  const base = {
    skipSuggestion: false,
    useSandbox: false,
    requestsCodeExecution: false,
    sandboxAvailable: true,
    sandboxCapabilityKnown: true,
    attachmentsNeedSandbox: false
  };

  it("envía un mensaje corriente sin preguntar nada", () => {
    expect(sandboxSendDecision(base)).toEqual({ kind: "send" });
  });

  it("no vuelve a preguntar si la persona ya activó Código aislado", () => {
    // Ya decidió: proponerlo otra vez sería insistir sobre lo ya resuelto.
    expect(
      sandboxSendDecision({ ...base, useSandbox: true, requestsCodeExecution: true })
    ).toEqual({ kind: "send" });
  });

  it("propone activar el sandbox cuando el mensaje pide ejecutar código", () => {
    expect(
      sandboxSendDecision({ ...base, requestsCodeExecution: true })
    ).toEqual({ kind: "suggest-sandbox" });
  });

  it("se niega en lugar de enviar algo que no podrá ejecutarse", () => {
    const decision = sandboxSendDecision({
      ...base,
      requestsCodeExecution: true,
      sandboxAvailable: false,
      diagnosticMessage: "Broker AI responde, pero no está listo"
    });
    expect(decision.kind).toBe("blocked");
    if (decision.kind !== "blocked") throw new Error("se esperaba un bloqueo");
    expect(decision.error.detail).toContain("Broker AI responde");
  });

  it("adapta el mensaje cuando el bloqueo viene de un adjunto tabular", () => {
    const withAttachment = sandboxSendDecision({
      ...base,
      requestsCodeExecution: true,
      sandboxAvailable: false,
      attachmentsNeedSandbox: true
    });
    const withoutAttachment = sandboxSendDecision({
      ...base,
      requestsCodeExecution: true,
      sandboxAvailable: false
    });
    if (withAttachment.kind !== "blocked" || withoutAttachment.kind !== "blocked") {
      throw new Error("se esperaban dos bloqueos");
    }
    expect(withAttachment.error.title).not.toBe(withoutAttachment.error.title);
    expect(withAttachment.error.title).toContain("archivo");
  });

  it("tras responder a la propuesta, envía sin volver a interrumpir", () => {
    // `skipSuggestion` es el segundo intento: la decisión ya se tomó, así que
    // ni se propone otra vez ni se bloquea, aunque el sandbox no esté.
    expect(
      sandboxSendDecision({
        ...base,
        skipSuggestion: true,
        requestsCodeExecution: true,
        sandboxAvailable: false
      })
    ).toEqual({ kind: "send" });
  });
});
