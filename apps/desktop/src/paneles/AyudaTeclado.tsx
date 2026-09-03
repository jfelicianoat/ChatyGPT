/** Atajos de teclado, en una ventana que se abre con `?`. */
import type { RefObject } from "react";

export function AyudaTeclado({
  keyboardHelpOpen,
  activeModalRef,
  setKeyboardHelpOpen,
}: {
  keyboardHelpOpen: boolean;
  activeModalRef: RefObject<HTMLElement | null>;
  setKeyboardHelpOpen: (open: boolean) => void;
}) {
  if (!keyboardHelpOpen) return null;
  return (
    <div className="modal-backdrop" role="presentation">
      <section
        ref={activeModalRef}
        className="modal keyboard-help-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="keyboard-help-title"
        aria-describedby="keyboard-help-description"
        tabIndex={-1}
      >
        <span className="kicker">Navegación accesible</span>
        <h2 id="keyboard-help-title">Atajos de teclado</h2>
        <p id="keyboard-help-description">
          Funcionan en toda la aplicación, pero las teclas sin modificadores no interrumpen
          la escritura ni se ejecutan encima de otra ventana.
        </p>
        <dl className="keyboard-shortcut-list">
          <div><dt><kbd>Ctrl</kbd> + <kbd>N</kbd></dt><dd>Nueva conversación</dd></div>
          <div><dt><kbd>Ctrl</kbd> + <kbd>F</kbd></dt><dd>Buscar conversaciones</dd></div>
          <div><dt><kbd>/</kbd></dt><dd>Buscar cuando no estás escribiendo</dd></div>
          <div><dt><kbd>Ctrl</kbd> + <kbd>Mayús</kbd> + <kbd>M</kbd></dt><dd>Ir al mensaje</dd></div>
          <div><dt><kbd>Alt</kbd> + <kbd>1</kbd></dt><dd>Volver a Inicio</dd></div>
          <div><dt><kbd>?</kbd></dt><dd>Abrir esta ayuda</dd></div>
          <div><dt><kbd>Esc</kbd></dt><dd>Cerrar una ventana abierta</dd></div>
        </dl>
        <div className="modal-actions">
          <button className="primary" autoFocus onClick={() => setKeyboardHelpOpen(false)}>
            Cerrar
          </button>
        </div>
      </section>
    </div>
  );
}
