/**
 * Diálogo «ChatyGPT quiere realizar una acción»: el turno del Broker está
 * pausado y cada propuesta de herramienta exige decisión expresa.
 *
 * Recibe el hook entero (`useConfirmaciones`) y el turno pausado. Extraída de
 * App.tsx (CH-03) tal cual, sin cambiar el DOM.
 */
import { confirmationSummary, type LocalTaskSnapshot } from "../domain";
import type { useConfirmaciones } from "./useConfirmaciones";

type Props = {
  confirmaciones: ReturnType<typeof useConfirmaciones>;
  /** Turno pausado con llamadas pendientes de decidir. */
  turn: LocalTaskSnapshot;
};

export function TarjetaConfirmacionHerramientas({ confirmaciones, turn }: Props) {
  const { toolDecisions, setToolDecisions, toolDecisionBusy, submitToolDecisions } = confirmaciones;
  return (
    <section className="tool-confirmation" aria-label="Confirmación de herramientas">
                    <span className="kicker">Confirmación necesaria</span>
                    <h3>ChatyGPT quiere realizar una acción</h3>
                    <p>
                      Revisa cada propuesta. No se ejecutará ninguna acción hasta que decidas.
                    </p>
                    <div className="tool-call-list">
                      {turn.pendingToolCalls.map((call) => {
                        const detail = confirmationSummary(call);
                        return (
                        <article key={call.toolCallId} className="tool-call-card">
                          <div className="tool-call-disclosure">
                            <strong>{detail.action}</strong>
                            <dl>
                              <div>
                                <dt>Herramienta</dt>
                                <dd>{detail.tool}</dd>
                              </div>
                              <div>
                                <dt>Recursos afectados</dt>
                                <dd>{detail.resource}</dd>
                              </div>
                              <div>
                                <dt>Datos que se enviarán</dt>
                                <dd>
                                  {detail.data.length === 0
                                    ? "Ninguno declarado"
                                    : detail.data.map((datum) => (
                                        <span key={datum.label}>
                                          {datum.label}: {datum.value}
                                        </span>
                                      ))}
                                </dd>
                              </div>
                              <div>
                                <dt>Destino</dt>
                                <dd>{detail.destination}</dd>
                              </div>
                              <div>
                                <dt>Alcance</dt>
                                <dd>{detail.scope}</dd>
                              </div>
                              <div>
                                <dt>Consecuencias</dt>
                                <dd>{detail.consequences}</dd>
                              </div>
                            </dl>
                          </div>
                          <div className="tool-decision-buttons">
                            <button
                              className={toolDecisions[call.toolCallId] === false ? "selected" : ""}
                              onClick={() => setToolDecisions((values) => ({
                                ...values,
                                [call.toolCallId]: false
                              }))}
                            >
                              Rechazar
                            </button>
                            <button
                              className={toolDecisions[call.toolCallId] === true ? "selected approve" : ""}
                              onClick={() => setToolDecisions((values) => ({
                                ...values,
                                [call.toolCallId]: true
                              }))}
                            >
                              Autorizar una vez
                            </button>
                          </div>
                        </article>
                        );
                      })}
                    </div>
                    <button
                      className="primary"
                      onClick={submitToolDecisions}
                      disabled={
                        toolDecisionBusy ||
                        turn.pendingToolCalls.some(
                          (call) => toolDecisions[call.toolCallId] === undefined
                        )
                      }
                    >
                      {toolDecisionBusy ? "Reanudando…" : "Confirmar decisiones y continuar"}
                    </button>
                  </section>
  );
}
