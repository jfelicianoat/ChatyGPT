/**
 * Informes que el modelo entrega envueltos en un cercado.
 *
 * Caso real del 6 de agosto de 2026: una Investigación profunda devolvió el
 * informe entero dentro de ```markdown y la interfaz lo pintó como una caja de
 * código dentro de la burbuja de la respuesta, con el Markdown en crudo y sin
 * ajuste de línea. La presentación no debe mostrar Markdown como texto plano.
 */

import { describe, expect, it } from "vitest";
import { unwrapFencedDocument } from "./MarkdownContent";

const fence = "```";
const tildeFence = "~~~";

describe("desenvolver un documento cercado", () => {
  it("desenvuelve un informe etiquetado como markdown", () => {
    const wrapped = [
      `${fence}markdown`,
      "# Informe",
      "",
      "Texto con **negrita**.",
      fence
    ].join("\n");
    expect(unwrapFencedDocument(wrapped)).toBe("# Informe\n\nTexto con **negrita**.");
  });

  it("acepta la etiqueta corta, las mayúsculas y las tildes", () => {
    expect(unwrapFencedDocument(`${fence}MD\n# Título\n${fence}`)).toBe("# Título");
    expect(unwrapFencedDocument(`${tildeFence}markdown\n# Título\n${tildeFence}`)).toBe(
      "# Título"
    );
  });

  it("respeta el código de verdad", () => {
    // Un bloque sin lenguaje puede ser un comando o un script: convertirlo en
    // prosa sería peor que el problema que se arregla.
    const sinLenguaje = `${fence}\nnpm run build\n${fence}`;
    expect(unwrapFencedDocument(sinLenguaje)).toBe(sinLenguaje);
    const python = `${fence}python\nprint("hola")\n${fence}`;
    expect(unwrapFencedDocument(python)).toBe(python);
  });

  it("no desenvuelve si el cercado no es todo el mensaje", () => {
    // Aquí el cercado es un ejemplo dentro de una explicación, no el documento.
    const mixto = [
      "Mira este ejemplo:",
      "",
      `${fence}markdown`,
      "# Título",
      fence,
      "",
      "Y ya está."
    ].join("\n");
    expect(unwrapFencedDocument(mixto)).toBe(mixto);
  });

  it("no rompe un documento con un cercado interno sin cerrar", () => {
    const roto = [`${fence}markdown`, "# Informe", "", `${fence}python`, "print(1)", fence].join(
      "\n"
    );
    expect(unwrapFencedDocument(roto)).toBe(roto);
  });

  it("conserva los cercados internos equilibrados al desenvolver", () => {
    const conCodigo = [
      `${fence}markdown`,
      "# Informe",
      "",
      `${fence}python`,
      "print(1)",
      fence,
      "",
      "Fin.",
      fence
    ].join("\n");
    const inner = unwrapFencedDocument(conCodigo);
    expect(inner.startsWith("# Informe")).toBe(true);
    // El código de dentro sigue siendo código.
    expect(inner).toContain(`${fence}python`);
    expect(inner.endsWith("Fin.")).toBe(true);
  });

  it("deja intacto un texto que no es un cercado", () => {
    expect(unwrapFencedDocument("# Informe normal")).toBe("# Informe normal");
    expect(unwrapFencedDocument("")).toBe("");
  });
});
