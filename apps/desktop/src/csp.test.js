/**
 * La CSP de Tauri decide qué puede cargar la vista web.
 *
 * Las vistas previas de captura de pantalla y de foto, y el recorte, cargan la
 * imagen desde una URL `blob:` creada por la propia página. Sin `blob:` en
 * `img-src` la vista previa salía en blanco y recortar fallaba con «No se pudo
 * leer la imagen para recortarla» (encontrado probando la app el 14-nov-2026).
 */
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";

const config = JSON.parse(
  readFileSync(new URL("../src-tauri/tauri.conf.json", import.meta.url), "utf8")
);

const directiva = (nombre) =>
  config.app.security.csp
    .split(";")
    .map((parte) => parte.trim().split(/\s+/))
    .find(([clave]) => clave === nombre)
    ?.slice(1) ?? [];

it("csp: las imágenes blob: de capturas y fotos se pueden mostrar y recortar", () => {
  expect(directiva("img-src")).toContain("blob:");
});

it("csp: blob: se permite solo para imágenes, no para scripts ni conexiones", () => {
  expect(directiva("default-src")).not.toContain("blob:");
  expect(directiva("connect-src")).not.toContain("blob:");
  expect(directiva("script-src")).not.toContain("blob:");
});
