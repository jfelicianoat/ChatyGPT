import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");
const finalContrastBlock = css.slice(css.indexOf("Cobertura final de contraste"));

describe("contraste de superficies en el tema oscuro", () => {
  it.each([
    ".project-file-item",
    ".conversation-more-menu",
    ".workflow-inspector",
    ".attachment-picker",
    ".project-file-item button",
    ".conversation-starters button",
    ".project-knowledge-item > button",
    ".workflow-toolbar button"
  ])("cubre %s con una regla oscura explícita", (selector) => {
    expect(finalContrastBlock).toContain(selector);
  });

  it.each([
    ".archive-card",
    ".archive-row",
    ".credential-banner",
    ".composer-refresh-notice",
    ".effective-policy",
    ".policy-warning",
    ".task-cancelled",
    ".workflow-node-privacy",
    ".search-snippet"
  ])("las superficies nuevas de la auditoría del 28-sep tienen regla oscura: %s", (selector) => {
    const darkRules = css
      .split("\n")
      .filter((line) => line.startsWith(':root[data-theme="dark"]'));
    expect(darkRules.some((line) => line.includes(selector))).toBe(true);
  });

  it("no vuelve ilegibles los controles inactivos mediante opacidad", () => {
    expect(finalContrastBlock).toMatch(
      /:is\(button, input, textarea, select\):disabled\s*\{[\s\S]*?opacity:\s*1;/
    );
    expect(finalContrastBlock).toContain(".memory-item.disabled");
    expect(finalContrastBlock).toContain(".custom-gpt-knowledge-item.disabled");
  });

  it("mantiene el lanzador de Athena amplio y adaptable", () => {
    expect(css).toMatch(
      /\.athena-lanzador\s*\{[^}]*grid-template-columns:\s*minmax\(0, 1\.65fr\) minmax\(260px, \.85fr\)/
    );
    expect(css).toMatch(
      /@media \(max-width: 820px\)[\s\S]*?\.athena-lanzador\s*\{[^}]*grid-template-columns:\s*1fr/
    );
  });

  it("el botón Contexto sigue disponible en ventanas estrechas", () => {
    // Por debajo de 1100 px se ocultaban todos los botones de la barra, también
    // el que reabre el panel de contexto una vez cerrado (28-sep-2026).
    expect(css).toContain(".conversation-toolbar button:not(.danger-text):not(.context-inspector-toggle)");
  });

  // Auditoría QA del 14-oct (INC-11): medidas con un auditor de contraste sobre
  // la app real; cada una estaba entre 1,04:1 y 2,6:1 en el tema oscuro.
  it.each([
    ".activity-card",
    ".scheduler-history",
    ".scheduler-calendar-occurrence time",
    ".custom-gpt-permission-summary span",
    ".scheduler-safety",
    ".custom-gpt-safety"
  ])("la auditoría del 14-oct deja regla oscura para %s", (selector) => {
    expect(finalContrastBlock).toContain(selector);
  });

  it("el modal tiene superficie oscura propia y no hereda texto claro sobre crema", () => {
    expect(css).toMatch(/:root\[data-theme="dark"\] \.modal \{[^}]*background: var\(--dark-surface-raised\)/);
  });
});

describe("maquetación (auditoría 14-oct)", () => {
  it("la barra lateral queda fija a la ventana (INC-04)", () => {
    expect(css).toMatch(/\.sidebar \{ position: sticky; top: 0; height: 100vh; overflow-y: auto;/);
  });

  it("la ficha de un GPT no reparte el ancho con su fila de botones (INC-05)", () => {
    expect(css).toMatch(/\.custom-gpt-item \{ display: grid; grid-template-columns: minmax\(0, 1fr\);/);
  });

  it("los días del calendario no capitalizan cada palabra", () => {
    expect(css).not.toMatch(/\.scheduler-calendar-day h4 \{[^}]*text-transform: capitalize/);
  });
});
