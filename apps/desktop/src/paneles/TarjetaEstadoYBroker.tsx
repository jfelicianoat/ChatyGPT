import type {
  BootstrapReport,
  BrokerDiagnostic,
  ConversationSummary,
  Loadable,
  ProjectSummary,
} from "../domain";

/** Estado de la base local y diagnóstico del Broker (ajustes). Extraída de App.tsx (CH-03): recibe el estado y los manejadores tal cual. */
export type TarjetaEstadoYBrokerProps = {
  bootstrap: Loadable<BootstrapReport>;
  conversations: ConversationSummary[];
  projects: ProjectSummary[];
  broker: Loadable<BrokerDiagnostic> | null;
  checkBroker: () => Promise<void> | void;
};

export function TarjetaEstadoYBroker({ bootstrap, conversations, projects, broker, checkBroker }: TarjetaEstadoYBrokerProps) {
  return (
    <div className="grid">
      <article className="panel">
        <div className="panel-heading">
          <div><span className="kicker">Persistencia</span><h3>Estado local</h3></div>
          <span className={`badge ${bootstrap.state === "ready" ? "success" : ""}`}>
            {bootstrap.state === "loading"
              ? "Inicializando"
              : bootstrap.state === "ready"
                ? "Operativa"
                : "Error"}
          </span>
        </div>
        {bootstrap.state === "ready" && (
          <dl className="facts">
            <div><dt>Esquema</dt><dd>{bootstrap.value.schemaVersion}</dd></div>
            <div><dt>Conversaciones</dt><dd>{conversations.length}</dd></div>
            <div><dt>Proyectos</dt><dd>{projects.length}</dd></div>
          </dl>
        )}
        {bootstrap.state === "error" && <p className="error">{bootstrap.message}</p>}
      </article>

      <article className="panel">
        <div className="panel-heading">
          <div><span className="kicker">Inferencia</span><h3>Broker AI</h3></div>
          {broker?.state === "ready" && (
            <span className={`badge ${broker.value.ready ? "success" : "warning"}`}>
              {broker.value.ready ? "Listo" : "No disponible"}
            </span>
          )}
        </div>
        <p className="muted">
          Comprueba salud y capacidades reales sin crear una inferencia.
        </p>
        <button
          className="primary"
          onClick={checkBroker}
          disabled={broker?.state === "loading"}
        >
          {broker?.state === "loading" ? "Comprobando…" : "Comprobar conexión"}
        </button>
        {broker?.state === "ready" && (
          <div className="diagnostic">
            <strong>{broker.value.message}</strong>
            <span>
              {broker.value.contractVersion
                ? `Contrato ${broker.value.contractVersion}`
                : broker.value.baseUrl}
            </span>
            <span>{broker.value.latencyMs} ms</span>
            <span>
              Código aislado: {broker.value.sandboxRunCode ? "disponible" : "no disponible"}
            </span>
            <span>
              Carriles: {broker.value.workLanes.length > 0
                ? broker.value.workLanes.join(", ")
                : "no declarados"}
            </span>
            <span>
              Frontera de datos: {broker.value.derivedDataBoundary
                ? "derivada por clasificación"
                : "compatibilidad explícita"}
            </span>
            <span>
              Dependencias entre tareas: {broker.value.taskDependencies
                ? "disponibles"
                : "no anunciadas"}
            </span>
            {(broker.value.agentSkillsEgress?.length ?? 0) > 0 && (
              <span>
                Herramientas con salida a Internet: {broker.value.agentSkillsEgress?.join(", ")}
              </span>
            )}
            <span>
              Documentos largos: {broker.value.longContextMapReduce
                ? "map-reduce disponible"
                : "sin map-reduce"}
            </span>
            <span>
              Exclusividad de contenido: {broker.value.contentExclusivity === undefined
                ? "no consta"
                : broker.value.contentExclusivity
                  ? "solo la ve el modelo que responde"
                  : "el Broker puede sondear otro modelo local"}
            </span>
            <span>
              Credencial: {broker.value.credential === "valid"
                ? "aceptada por el Broker"
                : broker.value.credential === "rejected"
                  ? "rechazada: introduce el token actual"
                  : broker.value.credential === "missing"
                    ? "sin credencial: introdúcela en Ajustes"
                  : broker.value.credential === "not_required"
                    ? "este Broker no exige credencial"
                    : broker.value.credential === "backend_unavailable"
                      ? "el Broker no puede comprobarla ahora"
                      : "no comprobada"}
            </span>
            {broker.value.destination && (
              <span>
                Destino: {broker.value.destination.host}
                {broker.value.destination.localMachine
                  ? " · este equipo"
                  : broker.value.destination.encrypted
                    ? " · conexión cifrada"
                    : " · conexión sin cifrar"}
              </span>
            )}
            {broker.value.destination?.warning && (
              <span className="policy-warning">{broker.value.destination.warning}</span>
            )}
            <span>
              Ejecución demostrable: {broker.value.demonstrableExecution === undefined
                ? "no consta"
                : broker.value.demonstrableExecution
                  ? "contrato 2.10"
                  : "no la declara"}
            </span>
          </div>
        )}
        {broker?.state === "error" && <p className="error">{broker.message}</p>}
      </article>
    </div>
  );
}
