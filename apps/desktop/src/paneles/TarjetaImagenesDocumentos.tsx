import type { ImageDescriptionPreference } from "../ingestionPreferences";

/** Si el Broker describe las imágenes de los documentos (ajustes). Extraída de App.tsx (CH-03): recibe el estado y los manejadores tal cual. */
export type TarjetaImagenesDocumentosProps = {
  imageDescriptionPreference: ImageDescriptionPreference;
  setImageDescriptionPreference: (value: ImageDescriptionPreference) => void;
};

export function TarjetaImagenesDocumentos({ imageDescriptionPreference, setImageDescriptionPreference }: TarjetaImagenesDocumentosProps) {
  return (
    <section
      className="appearance-card attachment-processing-card"
      aria-labelledby="attachment-processing-heading"
    >
      <div>
        <span className="kicker">Procesamiento de archivos</span>
        <h3 id="attachment-processing-heading">Imágenes en documentos</h3>
        <p>
          Decide si el Broker debe describir las figuras con un modelo de visión
          cuando adjuntes un archivo nuevo.
        </p>
      </div>
      <div
        className="appearance-options"
        role="radiogroup"
        aria-label="Tratamiento de imágenes en documentos"
      >
        {([
          ["describe", "Con imágenes", "Extrae y describe figuras; tarda más"],
          ["ignore", "Sin imágenes", "Ignora figuras y acelera la conversión"]
        ] as const).map(([value, label, description]) => (
          <button
            key={value}
            type="button"
            role="radio"
            aria-checked={imageDescriptionPreference === value}
            className={imageDescriptionPreference === value ? "active" : ""}
            onClick={() => setImageDescriptionPreference(value)}
          >
            <strong>{label}</strong>
            <span>{description}</span>
          </button>
        ))}
      </div>
      <small aria-live="polite">
        Se aplicará a los próximos archivos. Las imágenes adjuntadas directamente
        y las capturas siempre se procesan.
      </small>
    </section>
  );
}
