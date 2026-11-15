import { brokerCredentialLabel, type BrokerCredentialStatus, type Loadable } from "../domain";

/** Credencial del Broker cifrada con DPAPI (ajustes). Extraída de App.tsx (CH-03): recibe el estado y los manejadores tal cual. */
export type TarjetaCredencialBrokerProps = {
  brokerCredential: Loadable<BrokerCredentialStatus>;
  credentialDraft: string;
  setCredentialDraft: (value: string) => void;
  credentialBusy: boolean;
  credentialNotice: string | null;
  saveBrokerCredential: () => Promise<void> | void;
  removeBrokerCredential: () => Promise<void> | void;
};

export function TarjetaCredencialBroker({ brokerCredential, credentialDraft, setCredentialDraft, credentialBusy, credentialNotice, saveBrokerCredential, removeBrokerCredential }: TarjetaCredencialBrokerProps) {
  return (
    <section className="broker-credential" aria-labelledby="credential-heading">
      <div>
        <span className="kicker">Seguridad local</span>
        <h3 id="credential-heading">Credencial de Broker AI</h3>
        <p>
          El token se cifra con Windows para tu cuenta de usuario. No se guarda
          en la base de datos, ni en los registros, ni en el script de inicio, y
          nunca se vuelve a mostrar.
        </p>
      </div>
      {brokerCredential.state === "loading" && <small>Comprobando credencial…</small>}
      {brokerCredential.state === "error" && (
        <small role="alert">{brokerCredential.message}</small>
      )}
      {brokerCredential.state === "ready" && (
        <>
          <div className="credential-state">
            <span className={`badge ${brokerCredential.value.protected ? "ok" : ""}`}>
              {brokerCredentialLabel(brokerCredential.value)}
            </span>
            <small>{brokerCredential.value.message}</small>
          </div>
          <div className="credential-form">
            <label htmlFor="broker-token">Token administrativo</label>
            <input
              id="broker-token"
              type="password"
              autoComplete="off"
              spellCheck={false}
              placeholder="Pega aquí el token de Broker AI"
              value={credentialDraft}
              onChange={(event) => setCredentialDraft(event.target.value)}
            />
            <div className="credential-actions">
              <button
                className="primary"
                disabled={credentialBusy || credentialDraft.trim().length === 0}
                onClick={() => void saveBrokerCredential()}
              >
                {credentialBusy ? "Guardando…" : "Guardar credencial"}
              </button>
              {brokerCredential.value.protected && (
                <button
                  className="secondary"
                  disabled={credentialBusy}
                  onClick={() => void removeBrokerCredential()}
                >
                  Retirar
                </button>
              )}
            </div>
          </div>
          {credentialNotice && <small aria-live="polite">{credentialNotice}</small>}
        </>
      )}
    </section>
  );
}
