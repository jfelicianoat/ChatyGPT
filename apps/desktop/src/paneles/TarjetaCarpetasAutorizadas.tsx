import { authorizedFolderPurpose, type AuthorizedFolderView, type Loadable } from "../domain";

/** Carpetas autorizadas para leer o modificar (ajustes). Extraída de App.tsx (CH-03): recibe el estado y los manejadores tal cual. */
export type TarjetaCarpetasAutorizadasProps = {
  authorizedFolders: Loadable<AuthorizedFolderView[]>;
  folderBusy: string | null;
  authorizeGptReadFolder: () => Promise<void> | void;
  authorizeGptModifyFolder: () => Promise<void> | void;
  revokeFolder: (folderId: string) => Promise<void> | void;
};

export function TarjetaCarpetasAutorizadas({ authorizedFolders, folderBusy, authorizeGptReadFolder, authorizeGptModifyFolder, revokeFolder }: TarjetaCarpetasAutorizadasProps) {
  return (
    <section className="authorized-folders" aria-labelledby="folders-heading">
      <div>
        <span className="kicker">Permisos locales</span>
        <h3 id="folders-heading">Carpetas autorizadas</h3>
          <p>
          La escritura y la lectura para GPTs son permisos distintos. Leer siempre
          requiere una confirmación adicional en el chat y usa modelos locales.
        </p>
      </div>
      {/* Antes estaba en la cabecera «Proyectos» de la barra lateral,
          aplastado en una columna de 70 px (auditoría 14-oct, INC-05). */}
      <div className="authorized-folder-actions">
        <button
          className="secondary"
          disabled={folderBusy === "new-read-folder"}
          onClick={() => void authorizeGptReadFolder()}
        >
          {folderBusy === "new-read-folder"
            ? "Abriendo selector…"
            : "Autorizar carpeta para lectura"}
        </button>
        <button
          className="secondary"
          disabled={folderBusy === "new-modify-folder"}
          onClick={() => void authorizeGptModifyFolder()}
        >
          {folderBusy === "new-modify-folder"
            ? "Abriendo selector…"
            : "Autorizar carpeta para modificar"}
        </button>
      </div>
      {authorizedFolders.state === "loading" && <small>Cargando permisos…</small>}
      {authorizedFolders.state === "error" && (
        <small role="alert">{authorizedFolders.message}</small>
      )}
      {authorizedFolders.state === "ready" &&
        (authorizedFolders.value.length === 0 ? (
          <small>
            Todavía no has autorizado ninguna carpeta. Aparecerán aquí en cuanto
            exportes por primera vez.
          </small>
        ) : (
          <ul className="authorized-folder-list">
            {authorizedFolders.value.map((folder) => (
              <li key={folder.id} className={folder.revokedAt ? "revoked" : ""}>
                <div>
                  <strong>{folder.displayName}</strong>
                  <span>{authorizedFolderPurpose(folder)}</span>
                  <small>
                    {folder.revokedAt
                      ? `Revocada el ${folder.revokedAt}`
                      : `Autorizada el ${folder.grantedAt}`}
                  </small>
                </div>
                {!folder.revokedAt && (
                  <button
                    className="secondary"
                    disabled={folderBusy === folder.id}
                    onClick={() => void revokeFolder(folder.id)}
                  >
                    {folderBusy === folder.id ? "Revocando…" : "Revocar"}
                  </button>
                )}
              </li>
            ))}
          </ul>
        ))}
    </section>
  );
}
