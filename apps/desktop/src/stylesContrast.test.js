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
});
