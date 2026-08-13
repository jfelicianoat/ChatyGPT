import type { WorkflowRunView } from "./domain";

export type WorkflowFailureKind = "credential" | "connection" | "contract" | "node";

export type WorkflowFailureDescription = {
  kind: WorkflowFailureKind;
  title: string;
  guidance: string;
  technicalMessage: string;
  failedNodes: Array<{ id: string; label: string; message: string }>;
};

const messageFrom = (value: Record<string, unknown> | null | undefined) =>
  typeof value?.message === "string" ? value.message.trim() : "";

export function describeWorkflowFailure(run: WorkflowRunView): WorkflowFailureDescription | null {
  const failedNodes = run.nodeRuns
    .filter((node) => node.status === "failed")
    .map((node) => ({
      id: node.nodeId,
      label: node.nodeLabel,
      message: messageFrom(node.error) || "El nodo no pudo completarse."
    }));
  const technicalMessage = messageFrom(run.error) || failedNodes[0]?.message || "";
  if (!technicalMessage && failedNodes.length === 0) return null;

  const normalized = technicalMessage.toLocaleLowerCase("es");
  const nodeLabel = failedNodes[0]?.label;
  const where = nodeLabel ? ` en «${nodeLabel}»` : "";

  if (
    normalized.includes("admin_auth_required") ||
    normalized.includes("http 401") ||
    normalized.includes("http 403")
  ) {
    return {
      kind: "credential",
      title: "El Broker necesita una credencial nueva",
      guidance: `La ejecución se detuvo${where}. Actualiza la credencial de Broker AI y reintenta; los nodos ya completados se conservarán.`,
      technicalMessage,
      failedNodes
    };
  }

  if (
    normalized.includes("no está accesible") ||
    normalized.includes("no esta accesible") ||
    normalized.includes("error sending request") ||
    normalized.includes("connection refused") ||
    normalized.includes("conexión rechazada") ||
    normalized.includes("conexion rechazada") ||
    normalized.includes("tcp connect")
  ) {
    return {
      kind: "connection",
      title: "Broker AI no está disponible",
      guidance: `La ejecución se detuvo${where}. Comprueba que Broker AI esté iniciado y accesible desde este equipo; después podrás reintentar desde el nodo fallido.`,
      technicalMessage,
      failedNodes
    };
  }

  if (
    normalized.includes("contract_validation_failed") ||
    normalized.includes("http 422") ||
    normalized.includes("contrato inesperado") ||
    normalized.includes("no cumple el contrato")
  ) {
    return {
      kind: "contract",
      title: "La configuración no es compatible con Broker AI",
      guidance: `Broker AI rechazó la petición${where}. Revisa la configuración de ese nodo o publica una versión nueva del flujo antes de reintentar.`,
      technicalMessage,
      failedNodes
    };
  }

  return {
    kind: "node",
    title: nodeLabel ? `No se pudo completar «${nodeLabel}»` : "El flujo no pudo completarse",
    guidance: "Revisa el motivo técnico y la configuración del nodo. Puedes reintentar sin repetir los nodos que ya terminaron correctamente.",
    technicalMessage,
    failedNodes
  };
}
