// `vitest/config` en lugar de `vite`: es el que tipa el bloque `test`.
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

import paquete from "./package.json";

export default defineConfig({
  plugins: [react()],
  root: "apps/desktop",
  // La version se inyecta desde package.json: la ventana la ensena y nadie
  // tiene que mantenerla en dos sitios.
  define: { __APP_VERSION__: JSON.stringify(paquete.version) },
  clearScreen: false,
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true
  },
  build: {
    outDir: "../../dist",
    emptyOutDir: true,
    sourcemap: true
  },
  test: {
    coverage: {
      provider: "v8",
      reporter: ["text-summary", "lcov"],
      // Las rutas son relativas a `root` (`apps/desktop`). Escribirlas desde la
      // raíz del repositorio hacía que `include` no coincidiera con ningún
      // archivo y la cobertura se calculara sobre 0 de 0, con lo que el umbral
      // se cumplía sin medir nada.
      //
      // `App.tsx` queda fuera del umbral —no de las pruebas— porque son 7.000
      // líneas de presentación con solo un puñado de pruebas de componente:
      // incluirlo hundiría la cifra de la lógica que sí está cubierta y
      // ocultaría una regresión real en ella.
      // `platform.ts` queda fuera porque no contiene decisiones: son envoltorios
      // mecánicos de `invoke`. Su corrección real —que cada nombre de orden y
      // cada argumento existan en Rust— la comprueba
      // `tests/test_frontend_contract.py` contra el código del backend, que es
      // una garantía más fuerte que una prueba afirmando el literal que acabo
      // de escribir.
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/App.tsx",
        // Los paneles salieron de `App.tsx` al partirlo: son el mismo JSX de
        // presentacion, movido de sitio. Incluirlos aqui bajaria el umbral de
        // la logica que si esta cubierta solo por haber cambiado de fichero,
        // que es exactamente lo contrario de lo que mide esta puerta.
        "src/paneles/**",
        "src/platform.ts",
        "src/main.tsx",
        "src/env.d.ts",
        "src/**/*.test.{ts,tsx}"
      ],
      thresholds: {
        // El encargo pide 70 % para lógica no visual. Se sube a 82 porque lo
        // medido ronda el 85 % tras extraer la lógica de `App.tsx`: un umbral
        // muy por debajo de lo alcanzado deja de detectar regresiones, que es
        // justo lo que pasaba mientras el patrón `include` no coincidía con
        // ningún archivo.
        lines: 82,
        functions: 82,
        statements: 82,
        branches: 82
      }
    }
  }
});
