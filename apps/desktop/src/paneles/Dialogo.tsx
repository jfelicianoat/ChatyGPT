/** El dialogo modal generico: confirmaciones y entradas de texto.
 *
 * Una accion sensible pasa siempre por aqui: es el unico sitio donde el
 * usuario dice que si, y por eso conviene que sea uno solo. */
import type { RefObject } from "react";

import { dialogCopy, type DialogState } from "../dialogs";

export function Dialogo({
  dialog,
  dialogValue,
  dialogBusy,
  activeModalRef,
  setDialog,
  setDialogValue,
  submitDialog,
}: {
  dialog: DialogState | null;
  dialogValue: string;
  dialogBusy: boolean;
  activeModalRef: RefObject<HTMLElement | null>;
  setDialog: (dialog: DialogState | null) => void;
  setDialogValue: (value: string) => void;
  submitDialog: () => void | Promise<void>;
}) {
  if (!dialog) return null;
  return (
    <div className="modal-backdrop" role="presentation">
      <section
        ref={activeModalRef}
        className="modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="dialog-title"
        tabIndex={-1}
      >
        <span className="kicker">Gestión local</span>
        <h2 id="dialog-title">{dialogCopy(dialog).title}</h2>
        <p>{dialogCopy(dialog).description}</p>
        {dialogCopy(dialog).fieldLabel && (
          <label>
            <span>{dialogCopy(dialog).fieldLabel}</span>
            {dialogCopy(dialog).multiline ? (
              <textarea
                autoFocus
                value={dialogValue}
                onChange={(event) => setDialogValue(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Escape") setDialog(null);
                }}
                maxLength={dialogCopy(dialog).maxLength}
                rows={9}
                placeholder={dialogCopy(dialog).placeholder ?? "Ejemplo: responde en español, cita siempre las fuentes y separa claramente hechos de hipótesis."}
              />
            ) : (
              <input
                autoFocus
                value={dialogValue}
                onChange={(event) => setDialogValue(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void submitDialog();
                  if (event.key === "Escape") setDialog(null);
                }}
                maxLength={dialogCopy(dialog).maxLength ?? 120}
              />
            )}
          </label>
        )}
        <div className="modal-actions">
          <button className="secondary" onClick={() => setDialog(null)} disabled={dialogBusy}>
            Cancelar
          </button>
          <button
            className={dialogCopy(dialog).destructive ? "danger-button" : "primary"}
            onClick={submitDialog}
            disabled={
              dialogBusy ||
              Boolean(
                dialogCopy(dialog).fieldLabel
                && !dialogCopy(dialog).allowEmpty
                && !dialogValue.trim()
              )
            }
          >
            {dialogBusy ? dialogCopy(dialog).busyLabel ?? "Guardando…" : dialogCopy(dialog).action}
          </button>
        </div>
      </section>
    </div>
  );
}
