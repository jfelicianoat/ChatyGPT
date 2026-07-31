import { Fragment, type ReactNode } from "react";

type MarkdownContentProps = {
  text: string;
};

function safeWebUrl(value: string): string | null {
  try {
    const url = new URL(value);
    if (
      (url.protocol !== "http:" && url.protocol !== "https:") ||
      url.username ||
      url.password
    ) {
      return null;
    }
    return url.toString();
  } catch {
    return null;
  }
}

function renderInline(text: string, keyPrefix: string, depth = 0): ReactNode[] {
  if (!text || depth > 6) {
    return [text];
  }

  const tokenPattern =
    /(`[^`\n]+`|\[([^\]\n]+)\]\(([^)\s]+)(?:\s+"[^"]*")?\)|\*\*([^*\n]+)\*\*|__([^_\n]+)__|~~([^~\n]+)~~|\*([^*\n]+)\*|_([^_\n]+)_|<(https?:\/\/[^ >]+)>)/g;
  const nodes: ReactNode[] = [];
  let cursor = 0;
  let match: RegExpExecArray | null;
  let tokenIndex = 0;

  while ((match = tokenPattern.exec(text)) !== null) {
    if (match.index > cursor) {
      nodes.push(text.slice(cursor, match.index));
    }

    const key = `${keyPrefix}-${tokenIndex++}`;
    const token = match[0];
    if (token.startsWith("`")) {
      nodes.push(<code key={key}>{token.slice(1, -1)}</code>);
    } else if (match[2] !== undefined && match[3] !== undefined) {
      const href = safeWebUrl(match[3]);
      nodes.push(
        href ? (
          <a href={href} key={key} rel="noreferrer noopener" target="_blank">
            {renderInline(match[2], `${key}-label`, depth + 1)}
          </a>
        ) : (
          <Fragment key={key}>{renderInline(match[2], `${key}-label`, depth + 1)}</Fragment>
        )
      );
    } else if (match[4] !== undefined || match[5] !== undefined) {
      const value = match[4] ?? match[5];
      nodes.push(
        <strong key={key}>{renderInline(value, `${key}-strong`, depth + 1)}</strong>
      );
    } else if (match[6] !== undefined) {
      nodes.push(
        <del key={key}>{renderInline(match[6], `${key}-del`, depth + 1)}</del>
      );
    } else if (match[7] !== undefined || match[8] !== undefined) {
      const value = match[7] ?? match[8];
      nodes.push(<em key={key}>{renderInline(value, `${key}-em`, depth + 1)}</em>);
    } else if (match[9] !== undefined) {
      const href = safeWebUrl(match[9]);
      nodes.push(
        href ? (
          <a href={href} key={key} rel="noreferrer noopener" target="_blank">
            {match[9]}
          </a>
        ) : (
          match[9]
        )
      );
    }
    cursor = match.index + token.length;
  }

  if (cursor < text.length) {
    nodes.push(text.slice(cursor));
  }
  return nodes;
}

function renderParagraph(lines: string[], key: string): ReactNode {
  const content: ReactNode[] = [];
  lines.forEach((line, index) => {
    const hardBreak = /(?: {2}|\\)$/.test(line);
    const cleanLine = hardBreak ? line.replace(/(?: {2}|\\)$/, "") : line;
    content.push(...renderInline(cleanLine, `${key}-${index}`));
    if (index < lines.length - 1) {
      content.push(hardBreak ? <br key={`${key}-br-${index}`} /> : " ");
    }
  });
  return <p key={key}>{content}</p>;
}

function splitTableRow(line: string): string[] {
  const trimmed = line.trim().replace(/^\|/, "").replace(/\|$/, "");
  return trimmed.split(/(?<!\\)\|/).map((cell) => cell.trim().replace(/\\\|/g, "|"));
}

function isTableDivider(line: string): boolean {
  const cells = splitTableRow(line);
  return cells.length > 0 && cells.every((cell) => /^:?-{3,}:?$/.test(cell));
}

function isBlockStart(lines: string[], index: number): boolean {
  const line = lines[index] ?? "";
  return (
    /^ {0,3}(```|~~~)/.test(line) ||
    /^ {0,3}#{1,6}\s+/.test(line) ||
    /^ {0,3}>\s?/.test(line) ||
    /^ {0,3}(?:[-+*]|\d+[.)])\s+/.test(line) ||
    /^ {0,3}(?:(?:\*\s*){3,}|(?:-\s*){3,}|(?:_\s*){3,})$/.test(line.trim()) ||
    (line.includes("|") && isTableDivider(lines[index + 1] ?? ""))
  );
}

function renderBlocks(text: string, keyPrefix = "markdown"): ReactNode[] {
  const lines = text.replace(/\r\n?/g, "\n").split("\n");
  const blocks: ReactNode[] = [];
  let index = 0;
  let blockIndex = 0;

  while (index < lines.length) {
    const line = lines[index];
    const key = `${keyPrefix}-${blockIndex++}`;
    if (!line.trim()) {
      index += 1;
      continue;
    }

    const fence = line.match(/^ {0,3}(```|~~~)\s*([A-Za-z0-9_+#.-]*)\s*$/);
    if (fence) {
      const code: string[] = [];
      index += 1;
      while (index < lines.length && !new RegExp(`^ {0,3}${fence[1]}`).test(lines[index])) {
        code.push(lines[index]);
        index += 1;
      }
      if (index < lines.length) index += 1;
      blocks.push(
        <pre key={key}>
          <code data-language={fence[2] || undefined}>{code.join("\n")}</code>
        </pre>
      );
      continue;
    }

    const heading = line.match(/^ {0,3}(#{1,6})\s+(.+?)\s*#*\s*$/);
    if (heading) {
      const level = heading[1].length;
      const children = renderInline(heading[2], `${key}-heading`);
      if (level === 1) blocks.push(<h1 key={key}>{children}</h1>);
      else if (level === 2) blocks.push(<h2 key={key}>{children}</h2>);
      else if (level === 3) blocks.push(<h3 key={key}>{children}</h3>);
      else if (level === 4) blocks.push(<h4 key={key}>{children}</h4>);
      else if (level === 5) blocks.push(<h5 key={key}>{children}</h5>);
      else blocks.push(<h6 key={key}>{children}</h6>);
      index += 1;
      continue;
    }

    if (/^ {0,3}(?:(?:\*\s*){3,}|(?:-\s*){3,}|(?:_\s*){3,})$/.test(line.trim())) {
      blocks.push(<hr key={key} />);
      index += 1;
      continue;
    }

    if (/^ {0,3}>\s?/.test(line)) {
      const quoted: string[] = [];
      while (index < lines.length && /^ {0,3}>\s?/.test(lines[index])) {
        quoted.push(lines[index].replace(/^ {0,3}>\s?/, ""));
        index += 1;
      }
      blocks.push(<blockquote key={key}>{renderBlocks(quoted.join("\n"), `${key}-quote`)}</blockquote>);
      continue;
    }

    const listMatch = line.match(/^ {0,3}([-+*]|\d+[.)])\s+(.+)$/);
    if (listMatch) {
      const ordered = /^\d/.test(listMatch[1]);
      const start = ordered ? Number.parseInt(listMatch[1], 10) : undefined;
      const items: ReactNode[] = [];
      while (index < lines.length) {
        const item = lines[index].match(/^ {0,3}([-+*]|\d+[.)])\s+(.+)$/);
        if (!item || /^\d/.test(item[1]) !== ordered) break;
        const task = item[2].match(/^\[([ xX])\]\s+(.+)$/);
        items.push(
          <li key={`${key}-item-${items.length}`}>
            {task && (
              <input
                aria-label={task[1].toLowerCase() === "x" ? "Completada" : "Pendiente"}
                checked={task[1].toLowerCase() === "x"}
                disabled
                readOnly
                type="checkbox"
              />
            )}
            {renderInline(task ? task[2] : item[2], `${key}-item-${items.length}-inline`)}
          </li>
        );
        index += 1;
      }
      blocks.push(
        ordered ? (
          <ol key={key} start={start}>{items}</ol>
        ) : (
          <ul key={key}>{items}</ul>
        )
      );
      continue;
    }

    if (line.includes("|") && isTableDivider(lines[index + 1] ?? "")) {
      const headers = splitTableRow(line);
      const alignments = splitTableRow(lines[index + 1]).map((cell) =>
        cell.startsWith(":") && cell.endsWith(":")
          ? "center"
          : cell.endsWith(":")
            ? "right"
            : "left"
      );
      const rows: string[][] = [];
      index += 2;
      while (index < lines.length && lines[index].includes("|") && lines[index].trim()) {
        rows.push(splitTableRow(lines[index]));
        index += 1;
      }
      blocks.push(
        <div className="markdown-table-wrap" key={key}>
          <table>
            <thead>
              <tr>
                {headers.map((cell, cellIndex) => (
                  <th key={`${key}-head-${cellIndex}`} style={{ textAlign: alignments[cellIndex] }}>
                    {renderInline(cell, `${key}-head-${cellIndex}-inline`)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {rows.map((row, rowIndex) => (
                <tr key={`${key}-row-${rowIndex}`}>
                  {headers.map((_, cellIndex) => (
                    <td
                      key={`${key}-row-${rowIndex}-${cellIndex}`}
                      style={{ textAlign: alignments[cellIndex] }}
                    >
                      {renderInline(
                        row[cellIndex] ?? "",
                        `${key}-row-${rowIndex}-${cellIndex}-inline`
                      )}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      );
      continue;
    }

    const paragraph: string[] = [];
    while (
      index < lines.length &&
      lines[index].trim() &&
      (paragraph.length === 0 || !isBlockStart(lines, index))
    ) {
      paragraph.push(lines[index]);
      index += 1;
    }
    blocks.push(renderParagraph(paragraph, key));
  }

  return blocks;
}

export function MarkdownContent({ text }: MarkdownContentProps) {
  return <div className="markdown-content">{renderBlocks(text)}</div>;
}
