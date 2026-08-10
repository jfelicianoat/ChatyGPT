import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { MarkdownContent, unwrapFencedDocument } from "./MarkdownContent";

function render(text: string): string {
  return renderToStaticMarkup(<MarkdownContent text={text} />);
}

describe("MarkdownContent", () => {
  it("renders headings, emphasis, lists, tables and fenced code as readable HTML", () => {
    const html = render(`# Informe

Texto con **negrita**, *cursiva* y \`código\`.

- Primero
- Segundo

| Nombre | Valor |
| :--- | ---: |
| Media | 42 |

\`\`\`python
print("hola")
\`\`\``);

    expect(html).toContain("<h1>Informe</h1>");
    expect(html).toContain("<strong>negrita</strong>");
    expect(html).toContain("<em>cursiva</em>");
    expect(html).toContain("<ul><li>Primero</li><li>Segundo</li></ul>");
    expect(html).toContain("<table>");
    expect(html).toContain('text-align:right">42</td>');
    expect(html).toContain('data-language="python"');
  });

  it("keeps raw HTML inert and only activates safe web links", () => {
    const html = render(`<script>alert("no")</script>

[Seguro](https://example.com/informe)
[Peligroso](javascript:alert(1))`);

    expect(html).toContain("&lt;script&gt;");
    expect(html).not.toContain("<script>");
    expect(html).toContain('href="https://example.com/informe"');
    expect(html).not.toContain("href=\"javascript:");
    expect(html).toContain("Peligroso");
  });

  it("supports quotes, ordered lists, tasks and explicit line breaks", () => {
    const html = render(`> Nota importante

3. Tercero
4. Cuarto

- [x] Hecho
- [ ] Pendiente

línea uno  
línea dos`);

    expect(html).toContain("<blockquote>");
    expect(html).toContain('<ol start="3">');
    expect(html).toContain('aria-label="Completada"');
    expect(html).toContain("<br/>");
  });

  it("presenta como Markdown un informe que el modelo envolvió en un cercado", () => {
    // Caso real de una Investigación profunda: el informe llegó dentro de
    // ```markdown y se pintaba como una caja de código con el Markdown crudo.
    const html = render(`\`\`\`markdown
# Informe

Texto con **negrita**.

- Uno
- Dos
\`\`\``);

    expect(html).toContain("<h1>Informe</h1>");
    expect(html).toContain("<strong>negrita</strong>");
    expect(html).toContain("<ul><li>Uno</li><li>Dos</li></ul>");
    // Lo que no debe quedar: la caja con el Markdown sin interpretar.
    expect(html).not.toContain("<pre>");
    expect(html).not.toContain("# Informe");
  });
});
