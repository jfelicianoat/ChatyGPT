/**
 * Tarjeta «Opciones de ejecución» del compositor: privacidad, forma de
 * responder, prioridad, límite de coste y profundidad.
 *
 * Lo que se aplicará de verdad lo calcula Rust (política efectiva); esta
 * tarjeta solo edita las preferencias del chat. Extraída de App.tsx (CH-03)
 * tal cual, sin cambiar el DOM.
 */
import {
  brokerSupportsPreset,
  type BrokerDiagnostic,
  type ConversationExecutionPreferences,
  type EffectiveExecutionPolicy,
  type Loadable
} from "../domain";

const strategyLabel = (strategy: string) =>
  strategy === "auto"
    ? "Automática"
    : strategy === "mixture_of_agents"
      ? "Análisis en equipo"
      : "Respuesta directa";

const classificationShortLabel = (classification: string) =>
  classification === "internal"
    ? "Uso personal"
    : classification === "public"
      ? "Público"
      : classification === "confidential"
        ? "Confidencial"
        : "Solo modelos locales";

/** Los valores que se aplicarán: la política efectiva, o los del chat mientras carga. */
export type PreferenciasAplicadas =
  | EffectiveExecutionPolicy
  | Pick<
      ConversationExecutionPreferences,
      "dataClassification" | "strategy" | "maxCostUsd" | "priority"
    >;

type Props = {
  policy: Loadable<EffectiveExecutionPolicy> | null;
  /** Preferencias del chat: los valores que muestran los controles. */
  preferences: ConversationExecutionPreferences;
  /** Lo que se aplicará de verdad al próximo mensaje (resumen del desplegable). */
  appliedPreferences: PreferenciasAplicadas;
  updatePreferences: (patch: Partial<ConversationExecutionPreferences>) => Promise<void>;
  optionsBusy: boolean;
  turnBlocks: boolean;
  broker: Loadable<BrokerDiagnostic> | null;
};

export function TarjetaOpcionesEjecucion({
  policy,
  preferences,
  appliedPreferences,
  updatePreferences,
  optionsBusy,
  turnBlocks,
  broker
}: Props) {
  const routingFixedByGpt =
    policy?.state === "ready" && policy.value.sources.routing === "gpt";
  return (
    <details className="execution-settings">
                  <summary>
                    <span>Opciones de ejecución</span>
                    <small title="Lo que se aplicará al próximo mensaje">
                      {strategyLabel(appliedPreferences.strategy)}
                      {" · "}
                      {classificationShortLabel(appliedPreferences.dataClassification)}
                      {" · "}
                      hasta {appliedPreferences.maxCostUsd.toFixed(2)} USD
                      {" · "}
                      {/* El chat usa 25/100/250 y los GPT 50/100/200: la etiqueta
                          sirve para ambas escalas. */}
                      {appliedPreferences.priority <= 50
                        ? "Prioridad alta"
                        : appliedPreferences.priority >= 200
                          ? "Prioridad baja"
                          : "Prioridad normal"}
                    </small>
                  </summary>
                  {policy?.state === "ready" && policy.value.destination.warning && (
                    <p className="policy-warning execution-destination-warning">
                      {policy.value.destination.warning}
                    </p>
                  )}
                  <div className="execution-settings-grid">
                    <label>
                      <span>Privacidad</span>
                      <select
                        value={preferences.dataClassification}
                        onChange={(event) => void updatePreferences({
                          dataClassification: event.target.value as ConversationExecutionPreferences["dataClassification"]
                        })}
                        disabled={Boolean(turnBlocks) || optionsBusy}
                      >
                        <option value="internal">Uso personal · local o cloud</option>
                        <option value="public">Contenido público · local o cloud</option>
                        <option value="confidential">Confidencial · solo modelos locales</option>
                        <option value="local_only">Solo modelos locales · máxima restricción</option>
                      </select>
                      <small>
                        Decide si el contenido puede salir a proveedores cloud. «Local» significa
                        modelos locales del Broker, que corre en{" "}
                        {policy?.state === "ready"
                          ? policy.value.destination.host
                          : "el equipo configurado"}
                        .
                      </small>
                    </label>
                    <label>
                      <span>Forma de responder</span>
                      <select
                        value={preferences.strategy}
                        onChange={(event) => void updatePreferences({
                          strategy: event.target.value as ConversationExecutionPreferences["strategy"]
                        })}
                        disabled={
                          Boolean(turnBlocks) ||
                          optionsBusy ||
                          broker?.state !== "ready" ||
                          routingFixedByGpt
                        }
                        title={routingFixedByGpt ? "La fija la versión publicada del GPT" : undefined}
                      >
                        <option value="single">Respuesta directa</option>
                        <option
                          value="auto"
                          disabled={
                            broker?.state === "ready" &&
                            !broker.value.strategies.includes("auto")
                          }
                        >
                          Automática · el Broker decide
                        </option>
                        <option
                          value="mixture_of_agents"
                          disabled={
                            broker?.state === "ready" &&
                            !broker.value.strategies.includes("mixture_of_agents")
                          }
                        >
                          Análisis en equipo
                        </option>
                      </select>
                      <small>Solo aparecen como utilizables las estrategias anunciadas por el Broker.</small>
                    </label>
                    <label>
                      <span>Prioridad en la cola</span>
                      <select
                        value={preferences.priority}
                        onChange={(event) => void updatePreferences({
                          priority: Number(event.target.value)
                        })}
                        disabled={Boolean(turnBlocks) || optionsBusy || routingFixedByGpt}
                        title={routingFixedByGpt ? "La fija la versión publicada del GPT" : undefined}
                      >
                        <option value={25}>Alta</option>
                        <option value={100}>Normal</option>
                        <option value={250}>Baja</option>
                      </select>
                      <small>Las tareas con prioridad alta pasan antes cuando esperan recursos.</small>
                    </label>
                    <label>
                      <span>Límite por petición</span>
                      <select
                        value={preferences.maxCostUsd}
                        onChange={(event) => void updatePreferences({
                          maxCostUsd: Number(event.target.value)
                        })}
                        disabled={Boolean(turnBlocks) || optionsBusy}
                      >
                        <option value={0}>0 USD · sin modelos de pago</option>
                        <option value={0.1}>Hasta 0,10 USD</option>
                        <option value={0.5}>Hasta 0,50 USD</option>
                        <option value={1}>Hasta 1,00 USD</option>
                      </select>
                      <small>Es un corte máximo, no una estimación del coste final.</small>
                    </label>
                    <label>
                      <span>Profundidad</span>
                      <select
                        value={preferences.preset}
                        onChange={(event) => void updatePreferences({
                          preset: event.target.value as ConversationExecutionPreferences["preset"]
                        })}
                        disabled={
                          Boolean(turnBlocks) ||
                          optionsBusy ||
                          preferences.strategy !== "mixture_of_agents"
                        }
                      >
                        <option value="fast">Normal</option>
                        <option
                          value="slow"
                          disabled={
                            broker?.state === "ready" &&
                            !brokerSupportsPreset(broker.value, "mixture_of_agents", "slow")
                          }
                        >
                          Exhaustiva
                        </option>
                      </select>
                      <small>La profundidad exhaustiva solo se aplica al análisis en equipo.</small>
                    </label>
                    <label className="execution-setting-check">
                      <input
                        type="checkbox"
                        checked={preferences.longContext === "map_reduce"}
                        onChange={(event) => void updatePreferences({
                          longContext: event.target.checked ? "map_reduce" : "fail"
                        })}
                        disabled={
                          Boolean(turnBlocks) ||
                          optionsBusy ||
                          broker?.state !== "ready" ||
                          !broker.value.longContextMapReduce ||
                          preferences.strategy === "mixture_of_agents"
                        }
                      />
                      <span>
                        Dividir documentos que no caben
                        <small>
                          Autoriza map-reduce; nunca se activa mediante truncado silencioso.
                        </small>
                      </span>
                    </label>
                    <p className="execution-settings-note">
                      Las herramientas usan temporalmente el modo agente. En “Análisis en equipo”,
                      Código aislado se entrega a los modelos que preparan las propuestas.
                    </p>
                  </div>
                </details>
  );
}
