/**
 * Qué debe ocurrir al pulsar Enviar.
 *
 * Extraído de `App.tsx` (fase 3). Antes de que un mensaje salga hacia el
 * Broker, la interfaz encadena varias decisiones: si el GPT seleccionado
 * permite ejecutar código, si conviene proponer Código aislado, y si hay que
 * negarse porque el sandbox no está disponible. Estaba escrito como una
 * secuencia de `return` dentro de una función de 60 líneas con estado y
 * llamadas de red por medio, así que no había forma de comprobar el orden ni
 * los casos límite.
 *
 * Aquí las decisiones son datos: cada función devuelve **qué hacer**, y el
 * componente se limita a obedecer.
 */

import { sandboxUnavailableGuidance, type ComposerErrorGuidance } from "./domain";

/** Lo que la interfaz debe hacer con el turno. */
export type ComposerSendDecision =
  /** Enviar el mensaje tal y como está configurado. */
  | { kind: "send" }
  /** Detenerse y proponer activar Código aislado para este turno. */
  | { kind: "suggest-sandbox" }
  /** No enviar y explicar por qué. */
  | { kind: "blocked"; error: ComposerErrorGuidance };

/**
 * Rechazo por permisos del GPT seleccionado.
 *
 * Se comprueba **antes** de tocar la red: pedir el diagnóstico del Broker para
 * después negarse por un permiso local sería trabajo inútil y una espera que la
 * persona no entendería.
 */
export function sandboxDeniedByCustomGpt({
  useSandbox,
  gptAllowsRunCode
}: {
  useSandbox: boolean;
  gptAllowsRunCode: boolean;
}): ComposerErrorGuidance | null {
  if (!useSandbox || gptAllowsRunCode) return null;
  return {
    title: "Este GPT no tiene permiso para ejecutar código",
    detail: "La versión seleccionada mantiene Código aislado denegado.",
    action:
      "Edita el GPT para permitir solicitudes con confirmación o selecciona otro GPT."
  };
}

/** Error cuando no se puede comprobar si el sandbox está disponible. */
export function sandboxDiagnosticFailure(detail: string): ComposerErrorGuidance {
  return {
    title: "No se pudo comprobar Código aislado",
    detail,
    action:
      "Comprueba que Broker AI está arrancado y vuelve a intentarlo. El mensaje no se ha enviado."
  };
}

/**
 * Decisión final una vez se conoce el estado real del sandbox.
 *
 * El orden importa y es el que estaba implícito en la cadena original:
 *
 * 1. Si el turno **ya lleva** Código aislado activado, se envía: la persona ya
 *    decidió y no se le vuelve a preguntar.
 * 2. Si el mensaje pide ejecutar código y el sandbox está disponible, se
 *    propone activarlo en lugar de enviarlo en silencio sin la herramienta que
 *    hace falta.
 * 3. Si lo pide y **no** está disponible, se rechaza con una explicación, en
 *    vez de enviar algo que va a fallar o a responder sin ejecutar nada.
 * 4. En cualquier otro caso, se envía.
 *
 * `skipSuggestion` es el segundo intento, después de que la persona haya
 * respondido a la propuesta: en ese punto ya no se vuelve a proponer ni a
 * bloquear, porque la decisión ya se tomó.
 */
export function sandboxSendDecision({
  skipSuggestion,
  useSandbox,
  requestsCodeExecution,
  sandboxAvailable,
  sandboxCapabilityKnown,
  attachmentsNeedSandbox,
  diagnosticMessage
}: {
  skipSuggestion: boolean;
  useSandbox: boolean;
  requestsCodeExecution: boolean;
  sandboxAvailable: boolean;
  sandboxCapabilityKnown: boolean;
  attachmentsNeedSandbox: boolean;
  diagnosticMessage?: string;
}): ComposerSendDecision {
  if (skipSuggestion || useSandbox || !requestsCodeExecution) {
    return { kind: "send" };
  }
  // Un fallo al leer capacidades no equivale a que el sandbox no exista. El
  // contrato 2.7 deja que el endpoint de tareas sea autoritativo.
  if (sandboxAvailable || !sandboxCapabilityKnown) {
    return { kind: "suggest-sandbox" };
  }
  return {
    kind: "blocked",
    error: sandboxUnavailableGuidance(attachmentsNeedSandbox, diagnosticMessage)
  };
}
