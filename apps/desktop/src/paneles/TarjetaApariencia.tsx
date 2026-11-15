import type { AppearancePreference, ResolvedAppearance } from "../appearance";

/** Tema claro, oscuro o el de Windows (ajustes). Extraída de App.tsx (CH-03): recibe el estado y los manejadores tal cual. */
export type TarjetaAparienciaProps = {
  appearancePreference: AppearancePreference;
  setAppearancePreference: (value: AppearancePreference) => void;
  resolvedAppearance: ResolvedAppearance;
};

export function TarjetaApariencia({ appearancePreference, setAppearancePreference, resolvedAppearance }: TarjetaAparienciaProps) {
  return (
    <section className="appearance-card" aria-labelledby="appearance-heading">
      <div>
        <span className="kicker">Preferencias locales</span>
        <h3 id="appearance-heading">Apariencia</h3>
        <p>
          Elige el aspecto de ChatyGPT. La opción se conserva únicamente en este equipo.
        </p>
      </div>
      <div className="appearance-options" role="radiogroup" aria-label="Tema de la aplicación">
        {([
          ["system", "Windows", "Sigue el tema del sistema"],
          ["light", "Claro", "Fondo luminoso"],
          ["dark", "Oscuro", "Menos luz en pantalla"]
        ] as const).map(([value, label, description]) => (
          <button
            key={value}
            type="button"
            role="radio"
            aria-checked={appearancePreference === value}
            className={appearancePreference === value ? "active" : ""}
            onClick={() => setAppearancePreference(value)}
          >
            <strong>{label}</strong>
            <span>{description}</span>
          </button>
        ))}
      </div>
      <small aria-live="polite">
        Tema visible: {resolvedAppearance === "dark" ? "oscuro" : "claro"}
        {appearancePreference === "system" ? " · cambia con Windows" : ""}.
      </small>
    </section>
  );
}
