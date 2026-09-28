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
      // El porcentaje se calcula sobre todo el frontend entregado, incluidos
      // App, los hooks de orquestación, los paneles y el puente IPC. Así el
      // informe no puede presentarse como cobertura del producto mientras
      // omite precisamente los recorridos que coordinan sus efectos.
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        // Solo declara un tipo: no genera codigo que ejecutar.
        "src/navegacion.ts",
        "src/main.tsx",
        "src/env.d.ts",
        "src/**/*.test.{ts,tsx}"
      ],
      thresholds: {
        // Baseline del frontend completo (28/09/2026, tras la auditoría):
        // 58,16 % de líneas y sentencias, 73,85 % de ramas y 45,33 % de
        // funciones (antes 56,20 / 72,61 / 41,53). El margen solo absorbe
        // pequeñas variaciones de instrumentación; una caída material hace
        // fallar la puerta.
        lines: 57,
        functions: 44,
        statements: 57,
        branches: 72
      }
    }
  }
});
