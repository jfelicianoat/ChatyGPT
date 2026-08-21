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
});
