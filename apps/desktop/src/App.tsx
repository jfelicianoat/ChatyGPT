import { useEffect, useState } from "react";
import type { BootstrapReport, BrokerDiagnostic } from "./domain";
import { platform } from "./platform";

type Loadable<T> =
  | { state: "loading" }
  | { state: "ready"; value: T }
  | { state: "error"; message: string };

function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function App() {
  const [bootstrap, setBootstrap] = useState<Loadable<BootstrapReport>>({ state: "loading" });
  const [broker, setBroker] = useState<Loadable<BrokerDiagnostic> | null>(null);

  useEffect(() => {
    platform.bootstrap()
      .then((value) => setBootstrap({ state: "ready", value }))
      .catch((error) => setBootstrap({ state: "error", message: describeError(error) }));
  }, []);

  const checkBroker = async () => {
    setBroker({ state: "loading" });
    try {
      setBroker({ state: "ready", value: await platform.diagnoseBroker() });
    } catch (error) {
      setBroker({ state: "error", message: describeError(error) });
    }
  };

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand">
          <span className="brand-mark">C</span>
          <div><strong>ChatyGPT</strong><small>Espacio personal</small></div>
        </div>
        <button className="new-chat" disabled>＋ Nueva conversación</button>
        <nav aria-label="Navegación principal">
          <p className="nav-label">Espacio</p>
          <button className="nav-item active">◌ Inicio</button>
          <button className="nav-item" disabled>⌕ Buscar</button>
          <button className="nav-item" disabled>◇ Proyectos</button>
          <p className="nav-label">Recientes</p>
          <div className="empty-nav">Las conversaciones aparecerán aquí.</div>
        </nav>
        <div className="sidebar-footer">
          <span className={`status-dot ${bootstrap.state === "ready" ? "ok" : ""}`} />
          {bootstrap.state === "ready" ? `Datos locales · esquema ${bootstrap.value.schemaVersion}` : "Preparando datos locales"}
        </div>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <div>
            <span className="eyebrow">Fase 0 · Fundamentos</span>
            <h1>Tu IA, con estado durable.</h1>
          </div>
          <span className="version">v0.1.0</span>
        </header>

        <div className="content">
          <section className="hero-card">
            <div>
              <span className="pill">Local-first</span>
              <h2>La base ya distingue tus datos de las tareas remotas.</h2>
              <p>SQLite conserva el estado transaccional. Broker AI queda detrás de un adaptador y nunca recibe secretos desde la interfaz.</p>
            </div>
            <div className="orb" aria-hidden="true"><span /></div>
          </section>

          <div className="grid">
            <article className="panel">
              <div className="panel-heading">
                <div>
                  <span className="kicker">Persistencia</span>
                  <h3>Estado local</h3>
                </div>
                <span className={`badge ${bootstrap.state === "ready" ? "success" : ""}`}>
                  {bootstrap.state === "loading" ? "Inicializando" : bootstrap.state === "ready" ? "Operativa" : "Error"}
                </span>
              </div>
              {bootstrap.state === "ready" && (
                <dl className="facts">
                  <div><dt>Esquema</dt><dd>{bootstrap.value.schemaVersion}</dd></div>
                  <div><dt>Tareas recuperadas</dt><dd>{bootstrap.value.recoveredTasks}</dd></div>
                  <div><dt>Versión</dt><dd>{bootstrap.value.appVersion}</dd></div>
                </dl>
              )}
              {bootstrap.state === "error" && <p className="error">{bootstrap.message}</p>}
            </article>

            <article className="panel">
              <div className="panel-heading">
                <div>
                  <span className="kicker">Inferencia</span>
                  <h3>Broker AI</h3>
                </div>
                {broker?.state === "ready" && (
                  <span className={`badge ${broker.value.ready ? "success" : "warning"}`}>
                    {broker.value.ready ? "Listo" : "No disponible"}
                  </span>
                )}
              </div>
              <p className="muted">
                Comprueba salud y capacidades reales. Esta acción no crea tareas ni consume inferencia.
              </p>
              <button className="primary" onClick={checkBroker} disabled={broker?.state === "loading"}>
                {broker?.state === "loading" ? "Comprobando…" : "Comprobar conexión"}
              </button>
              {broker?.state === "ready" && (
                <div className="diagnostic">
                  <strong>{broker.value.message}</strong>
                  <span>{broker.value.contractVersion ? `Contrato ${broker.value.contractVersion}` : broker.value.baseUrl}</span>
                  <span>{broker.value.latencyMs} ms</span>
                </div>
              )}
              {broker?.state === "error" && <p className="error">{broker.message}</p>}
            </article>
          </div>

          <section className="next-step">
            <div className="step-number">01</div>
            <div>
              <span className="kicker">Siguiente slice</span>
              <h3>Crear una conversación y persistir una tarea antes de enviarla.</h3>
              <p>La interfaz de chat se habilitará cuando el recorrido idempotente crear → persistir → sondear → recuperar esté cubierto por pruebas.</p>
            </div>
          </section>
        </div>
      </section>
    </main>
  );
}

