import { useEffect, useState } from "react";

import type { KnowledgeOrchestratorStatus } from "../domain";
import { describeError } from "../errors";
import { platform } from "../platform";
import { confirmAction } from "../confirmacion";

/**
 * Conexión con Knowledge Orchestrator.
 *
 * El token es el de un consumidor creado en Knowledge Orchestrator → Servicios
 * con permiso «read». Se cifra con Windows igual que el del Broker y nunca se
 * vuelve a mostrar.
 */
export function ConexionKnowledgeOrchestrator() {
  const [status, setStatus] = useState<KnowledgeOrchestratorStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [draft, setDraft] = useState("");
  const [busy, setBusy] = useState(false);

  const run = async (action: () => Promise<KnowledgeOrchestratorStatus>) => {
    setBusy(true);
    setError(null);
    try {
      setStatus(await action());
    } catch (failure) {
      setError(describeError(failure));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    void run(() => platform.getKnowledgeOrchestratorStatus());
  }, []);

  const save = async () => {
    await run(() => platform.setKnowledgeOrchestratorCredential(draft));
    setDraft("");
  };

  const remove = async () => {
    if (!await confirmAction("¿Retirar la credencial de Knowledge Orchestrator?")) return;
    await run(() => platform.clearKnowledgeOrchestratorCredential(true));
  };

  return (
    <section className="broker-credential" aria-labelledby="ko-credential-heading">
      <div>
        <span className="kicker">Conocimiento</span>
        <h3 id="ko-credential-heading">Knowledge Orchestrator</h3>
        <p>
          Permite que un chat o la mochila de un GPT consulten tu base de conocimiento. Crea un
          consumidor con permiso «read» en Knowledge Orchestrator → Servicios, inicia allí la API
          y pega aquí su token.
        </p>
      </div>
      {error && <small role="alert">{error}</small>}
      {status === null && !error && <small>Comprobando conexión…</small>}
      {status && (
        <>
          <div className="credential-state">
            <span className={`badge ${status.reachable ? "ok" : ""}`}>
              {status.reachable ? "Conectado" : status.credentialStored ? "Sin conexión" : "Sin credencial"}
            </span>
            <small>{status.baseUrl} · {status.message}</small>
          </div>
          <div className="credential-form">
            <label htmlFor="ko-token">Token de consumidor</label>
            <input
              id="ko-token"
              type="password"
              autoComplete="off"
              spellCheck={false}
              placeholder="Pega aquí el token de Knowledge Orchestrator"
              value={draft}
              onChange={(event) => setDraft(event.target.value)}
            />
            <div className="credential-actions">
              <button
                className="primary"
                disabled={busy || draft.trim().length === 0}
                onClick={() => void save()}
              >
                {busy ? "Guardando…" : "Guardar credencial"}
              </button>
              <button
                className="secondary"
                disabled={busy}
                onClick={() => void run(() => platform.getKnowledgeOrchestratorStatus())}
              >
                Comprobar
              </button>
              {status.credentialStored && (
                <button className="secondary" disabled={busy} onClick={() => void remove()}>
                  Retirar
                </button>
              )}
            </div>
          </div>
        </>
      )}
    </section>
  );
}
