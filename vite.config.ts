// `vitest/config` en lugar de `vite`: es el que tipa el bloque `test`.
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  root: "apps/desktop",
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
      // Solo lógica no visual: `App.tsx` es la capa de presentación y todavía no
      // tiene pruebas de componente, así que se mide aparte para no inflar la
      // cifra con archivos que nadie cubre.
      include: ["apps/desktop/src/**/*.{ts,tsx}"],
      exclude: [
        "apps/desktop/src/App.tsx",
        "apps/desktop/src/main.tsx",
        "apps/desktop/src/env.d.ts",
        "apps/desktop/src/platform.ts",
        "apps/desktop/src/**/*.test.{ts,tsx}"
      ],
      thresholds: {
        // Umbral del encargo para lógica no visual.
        lines: 70,
        functions: 70,
        statements: 70,
        branches: 70
      }
    }
  }
});
