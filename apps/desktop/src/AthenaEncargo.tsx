/**
 * Cambiar el encargo de un run vivo, diciendo sobre qué revisión se escribe.
 *
 * Vive en su propio componente porque tiene una máquina de estados pequeña pero real
 * —leer la revisión, escribir, chocar, decidir— y meterla en el área entera la habría
 * dejado sin pruebas, como el resto del JSX grande de esta aplicación.
 *
 * Dos reglas gobiernan esto y vienen de ADR-029:
 *
 * 1. **La revisión no la elige la interfaz.** La manda el núcleo, que la mantiene al día
 *    con los eventos. Aquí sólo se enseña.
 * 2. **Un conflicto no se resuelve reintentando.** Otro cambió el encargo antes; su
 *    versión puede ser incompatible con la que se estaba escribiendo, y repetir sin
 *    mirarla la pisaría. Se enseña la vigente y se espera una decisión.
 */

import { useCallback, useEffect, useState } from "react";

import type { AthenaObjetivo, AthenaRevisionObjetivo, AthenaRun } from "./domain";

type Props = {
  run: AthenaRun;
  /** Relee el encargo de Athena y devuelve el vigente, con su revisión. */
  onLeer: () => Promise<AthenaObjetivo>;
  /** Escribe una revisión. El número base lo pone el núcleo. */
  onRevisar: (objetivo: string, motivo: string) => Promise<AthenaRevisionObjetivo>;
};

export function AthenaEncargo({ run, onLeer, onRevisar }: Props) {
  const [abierto, setAbierto] = useState(false);
  const [borrador, setBorrador] = useState("");
  const [motivo, setMotivo] = useState("");
  const [vigente, setVigente] = useState<AthenaObjetivo | null>(null);
  const [conflicto, setConflicto] = useState<AthenaObjetivo | null>(null);
  const [escrito, setEscrito] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);

  const leer = useCallback(async () => {
    setOcupado(true);
    setError(null);
    try {
      const objetivo = await onLeer();
      setVigente(objetivo);
      return objetivo;
    } catch (fallo) {
      setError(fallo instanceof Error ? fallo.message : String(fallo));
      return null;
    } finally {
      setOcupado(false);
    }
  }, [onLeer]);

  // Al abrir se relee, no se confía en lo que hubiera en pantalla: entre que se pintó
  // el run y alguien decide cambiarlo pueden pasar minutos, y en ese hueco cabe otra
  // persona escribiendo desde Telegram.
  useEffect(() => {
    if (!abierto) {
      return;
    }
    void (async () => {
      const objetivo = await leer();
      if (objetivo) {
        setBorrador(objetivo.text);
      }
    })();
  }, [abierto, leer]);

  const enviar = async () => {
    if (!borrador.trim()) {
      setError("El encargo no puede quedarse vacío.");
      return;
    }
    setOcupado(true);
    setError(null);
    setEscrito(false);
    setConflicto(null);
    try {
      const resultado = await onRevisar(borrador, motivo);
      if (resultado.resultado === "conflicto") {
        // No se reintenta ni se fusiona. Se enseña lo que hay ahora y quien escribió
        // decide: puede que su cambio ya no tenga sentido contra el encargo nuevo.
        setConflicto(resultado.vigente);
        setVigente(resultado.vigente);
        return;
      }
      setVigente(resultado.objetivo);
      setEscrito(true);
      setMotivo("");
    } catch (fallo) {
      setError(fallo instanceof Error ? fallo.message : String(fallo));
    } finally {
      setOcupado(false);
    }
  };

  const revisionConocida = vigente?.revision ?? run.objetivoRevision;

  return (
    <details
      className="athena-encargo"
      open={abierto}
      onToggle={(evento) => setAbierto(evento.currentTarget.open)}
    >
      <summary>
        Encargo
        {revisionConocida > 0 ? (
          <span className="athena-rol">revisión {revisionConocida}</span>
        ) : null}
      </summary>

      <p className="athena-motivo">{vigente?.text ?? run.objetivo}</p>
      {run.motivoRevision ? (
        <p className="athena-nota">Último cambio: {run.motivoRevision}</p>
      ) : null}

      <label>
        Nuevo encargo
        <textarea
          value={borrador}
          onChange={(evento) => setBorrador(evento.target.value)}
          rows={4}
          disabled={ocupado}
        />
      </label>
      <label>
        Por qué lo cambias
        <input
          type="text"
          value={motivo}
          onChange={(evento) => setMotivo(evento.target.value)}
          disabled={ocupado}
        />
      </label>

      <div className="athena-decision">
        <button type="button" onClick={() => void enviar()} disabled={ocupado}>
          {ocupado ? "Enviando…" : "Cambiar el encargo"}
        </button>
      </div>

      {/* Escrito no es aplicado, y decir «ya está trabajando en ello» sería cómodo y
          falso: Athena recoge la revisión entre iteraciones, no a mitad de una. */}
      {escrito ? (
        <p className="athena-nota">
          Escrito como revisión {vigente?.revision}. Athena lo recogerá al terminar la
          iteración en curso; hasta entonces sigue con el anterior.
        </p>
      ) : null}

      {conflicto ? (
        <div className="athena-aviso" role="alert">
          <p>
            El encargo cambió mientras escribías: va por la revisión {conflicto.revision}.
            Tu cambio no se ha guardado.
          </p>
          <p className="athena-motivo">{conflicto.text}</p>
          {conflicto.reason ? (
            <p className="athena-nota">Lo cambiaron porque: {conflicto.reason}</p>
          ) : null}
          <div className="athena-decision">
            <button
              type="button"
              disabled={ocupado}
              onClick={() => {
                // Repetir lo que se escribió, ahora contra la revisión nueva. Es una
                // decisión, no una recuperación: quien pulsa ya ha visto el otro encargo.
                setConflicto(null);
                void enviar();
              }}
            >
              Escribir igualmente sobre la revisión {conflicto.revision}
            </button>
            <button
              type="button"
              disabled={ocupado}
              onClick={() => {
                setBorrador(conflicto.text);
                setConflicto(null);
              }}
            >
              Partir del encargo nuevo
            </button>
          </div>
        </div>
      ) : null}

      {error ? <p className="athena-aviso">{error}</p> : null}
    </details>
  );
}
