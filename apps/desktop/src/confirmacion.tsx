/** Confirmaciones con el modal propio de la aplicación (auditoría 14-oct, INC-07).
 *
 * `window.confirm` abre el diálogo nativo de WebView2 («tauri.localhost dice»),
 * que rompe el patrón visual y habla de la tecnología, no de la acción. Las
 * confirmaciones nacen en hooks y paneles repartidos, así que se piden con una
 * promesa y las pinta un único anfitrión montado en `App`. */
import { useEffect, useRef, useState, type KeyboardEvent } from "react";

export type ConfirmOptions = {
  /** Texto del botón que confirma. */
  action?: string;
  /** Señala la acción como destructiva o irreversible. */
  destructive?: boolean;
};

type Pending = ConfirmOptions & { message: string; resolve: (accepted: boolean) => void };

let show: ((pending: Pending) => void) | null = null;

/** Pregunta a la persona y resuelve `true` solo si confirma. */
export function confirmAction(message: string, options: ConfirmOptions = {}): Promise<boolean> {
  // Sin anfitrión (un panel probado de forma aislada) se recurre al diálogo
  // nativo para no perder nunca la pregunta; dentro de la app siempre lo hay.
  if (!show) return Promise.resolve(window.confirm(message));
  const open = show;
  return new Promise((resolve) => open({ ...options, message, resolve }));
}

const FOCUSABLE = "button:not([disabled])";

export function ConfirmacionHost() {
  const [queue, setQueue] = useState<Pending[]>([]);
  const modalRef = useRef<HTMLElement>(null);
  const current = queue[0] ?? null;

  useEffect(() => {
    show = (pending) => setQueue((items) => [...items, pending]);
    return () => {
      show = null;
    };
  }, []);

  useEffect(() => {
    if (!current) return;
    const previousFocus = document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
    // Foco en «Cancelar»: un Enter de más nunca debe confirmar un borrado.
    modalRef.current?.querySelector<HTMLElement>(FOCUSABLE)?.focus();
    return () => previousFocus?.focus();
  }, [current]);

  if (!current) return null;

  const answer = (accepted: boolean) => {
    current.resolve(accepted);
    setQueue((items) => items.slice(1));
  };

  const onKeyDown = (event: KeyboardEvent) => {
    // Los atajos globales escuchan en `window`: con la pregunta abierta, no.
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      answer(false);
      return;
    }
    if (event.key !== "Tab" || !modalRef.current) return;
    const buttons = [...modalRef.current.querySelectorAll<HTMLElement>(FOCUSABLE)];
    const first = buttons[0];
    const last = buttons[buttons.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first?.focus();
    }
  };

  return (
    <div className="modal-backdrop" role="presentation" onKeyDown={onKeyDown}>
      <section
        ref={modalRef}
        className="modal"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby="confirm-title"
        tabIndex={-1}
      >
        <span className="kicker">Confirmación</span>
        <h2 id="confirm-title" className="confirm-message">{current.message}</h2>
        <div className="modal-actions">
          <button className="secondary" onClick={() => answer(false)}>
            Cancelar
          </button>
          <button
            className={current.destructive ? "danger-button" : "primary"}
            onClick={() => answer(true)}
          >
            {current.action ?? "Confirmar"}
          </button>
        </div>
      </section>
    </div>
  );
}
