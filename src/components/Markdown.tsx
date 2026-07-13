import { Fragment, useState, type ReactNode } from "react";

import { Icon } from "./Icon";

interface MarkdownProps {
  content: string;
  streaming?: boolean;
  onCopied?: () => void;
  onOpenLink?: (url: string) => void;
}

type Block =
  | { type: "code"; language: string; value: string }
  | { type: "heading"; level: number; value: string }
  | { type: "paragraph"; value: string }
  | { type: "quote"; value: string }
  | { type: "list"; ordered: boolean; values: string[] }
  | { type: "table"; rows: string[][] }
  | { type: "rule" };

const isSpecialLine = (line: string, next = "") =>
  /^```/.test(line) ||
  /^#{1,4}\s/.test(line) ||
  /^>\s?/.test(line) ||
  /^\s*[-*+]\s+/.test(line) ||
  /^\s*\d+[.)]\s+/.test(line) ||
  /^\s*(?:---+|___+|\*\*\*+)\s*$/.test(line) ||
  (line.includes("|") && /^\s*\|?\s*:?-{3,}/.test(next));

function tableCells(line: string) {
  return line
    .trim()
    .replace(/^\|/, "")
    .replace(/\|$/, "")
    .split("|")
    .map((cell) => cell.trim());
}

function parseBlocks(content: string): Block[] {
  const lines = content.replace(/\r\n?/g, "\n").split("\n");
  const blocks: Block[] = [];
  let index = 0;

  while (index < lines.length) {
    const line = lines[index] ?? "";
    if (!line.trim()) {
      index += 1;
      continue;
    }

    const fence = line.match(/^```\s*([^\s`]*)/);
    if (fence) {
      const code: string[] = [];
      index += 1;
      while (index < lines.length && !/^```\s*$/.test(lines[index] ?? "")) {
        code.push(lines[index] ?? "");
        index += 1;
      }
      if (index < lines.length) index += 1;
      blocks.push({
        type: "code",
        language: (fence[1] ?? "").replace(/[^a-zA-Z0-9+#._-]/g, "").slice(0, 24),
        value: code.join("\n"),
      });
      continue;
    }

    const heading = line.match(/^(#{1,4})\s+(.+)$/);
    if (heading) {
      blocks.push({
        type: "heading",
        level: (heading[1] ?? "#").length,
        value: heading[2] ?? "",
      });
      index += 1;
      continue;
    }

    if (/^\s*(?:---+|___+|\*\*\*+)\s*$/.test(line)) {
      blocks.push({ type: "rule" });
      index += 1;
      continue;
    }

    if (line.includes("|") && /^\s*\|?\s*:?-{3,}/.test(lines[index + 1] ?? "")) {
      const rows = [tableCells(line)];
      index += 2;
      while (
        index < lines.length &&
        (lines[index] ?? "").includes("|") &&
        (lines[index] ?? "").trim()
      ) {
        rows.push(tableCells(lines[index] ?? ""));
        index += 1;
      }
      blocks.push({ type: "table", rows });
      continue;
    }

    if (/^>\s?/.test(line)) {
      const quoted: string[] = [];
      while (index < lines.length && /^>\s?/.test(lines[index] ?? "")) {
        quoted.push((lines[index] ?? "").replace(/^>\s?/, ""));
        index += 1;
      }
      blocks.push({ type: "quote", value: quoted.join(" ") });
      continue;
    }

    const unordered = line.match(/^\s*[-*+]\s+(.+)$/);
    const ordered = line.match(/^\s*\d+[.)]\s+(.+)$/);
    if (unordered || ordered) {
      const values: string[] = [];
      const matcher = ordered ? /^\s*\d+[.)]\s+(.+)$/ : /^\s*[-*+]\s+(.+)$/;
      while (index < lines.length) {
        const item = (lines[index] ?? "").match(matcher);
        if (!item) break;
        values.push(item[1] ?? "");
        index += 1;
      }
      blocks.push({ type: "list", ordered: Boolean(ordered), values });
      continue;
    }

    const paragraph = [line.trim()];
    index += 1;
    while (
      index < lines.length &&
      (lines[index] ?? "").trim() &&
      !isSpecialLine(lines[index] ?? "", lines[index + 1] ?? "")
    ) {
      paragraph.push((lines[index] ?? "").trim());
      index += 1;
    }
    blocks.push({ type: "paragraph", value: paragraph.join(" ") });
  }

  return blocks;
}

function safeLink(value: string) {
  try {
    const parsed = new URL(value);
    return parsed.protocol === "https:" ? parsed.toString() : null;
  } catch {
    return null;
  }
}

function inline(value: string, keyPrefix: string, onOpenLink?: (url: string) => void): ReactNode[] {
  const pattern = /(`[^`\n]+`|\*\*[^*\n]+\*\*|__[^_\n]+__|\*[^*\n]+\*|\[[^\]\n]+\]\([^\s)]+\))/g;
  const nodes: ReactNode[] = [];
  let cursor = 0;
  let match: RegExpExecArray | null;
  let tokenIndex = 0;
  while ((match = pattern.exec(value))) {
    if (match.index > cursor) nodes.push(value.slice(cursor, match.index));
    const token = match[0];
    const key = `${keyPrefix}-${String(tokenIndex)}`;
    tokenIndex += 1;
    if (token.startsWith("`")) {
      nodes.push(
        <code className="inline-code" key={key}>
          {token.slice(1, -1)}
        </code>,
      );
    } else if (token.startsWith("**") || token.startsWith("__")) {
      nodes.push(<strong key={key}>{token.slice(2, -2)}</strong>);
    } else if (token.startsWith("*")) {
      nodes.push(<em key={key}>{token.slice(1, -1)}</em>);
    } else {
      const link = token.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
      const rawHref = link?.[2];
      const href = rawHref ? safeLink(rawHref) : null;
      nodes.push(
        href ? (
          <button
            aria-label={`${link?.[1] ?? href} (opens ${new URL(href).hostname} in external browser)`}
            className="markdown-link"
            key={key}
            type="button"
            onClick={() => {
              onOpenLink?.(href);
            }}
          >
            {link?.[1]}
            {link?.[1] !== href && (
              <span className="link-destination">{new URL(href).hostname}</span>
            )}
          </button>
        ) : (
          <span key={key}>{link?.[1] ?? token}</span>
        ),
      );
    }
    cursor = pattern.lastIndex;
  }
  if (cursor < value.length) nodes.push(value.slice(cursor));
  return nodes;
}

function CodeBlock({
  language,
  value,
  onCopied,
}: {
  language: string;
  value: string;
  onCopied?: () => void;
}) {
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle");
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(value);
      setCopyState("copied");
      onCopied?.();
    } catch {
      setCopyState("failed");
    }
    window.setTimeout(() => {
      setCopyState("idle");
    }, 1600);
  };
  return (
    <section className="code-block" aria-label={`${language || "Plain text"} code block`}>
      <header>
        <span>{language || "text"}</span>
        <button
          className="code-copy"
          type="button"
          onClick={() => void copy()}
          aria-label="Copy code"
        >
          <Icon name={copyState === "copied" ? "check" : "copy"} size={14} />
          {copyState === "copied" ? "Copied" : copyState === "failed" ? "Copy failed" : "Copy"}
        </button>
      </header>
      <pre>
        <code>{value}</code>
      </pre>
    </section>
  );
}

export function Markdown({ content, streaming = false, onCopied, onOpenLink }: MarkdownProps) {
  const blocks = parseBlocks(content);
  return (
    <div className="markdown-body">
      {blocks.map((block, index) => {
        const key = `block-${String(index)}`;
        switch (block.type) {
          case "code":
            return (
              <CodeBlock
                key={key}
                language={block.language}
                value={block.value}
                onCopied={onCopied}
              />
            );
          case "heading": {
            const Tag =
              block.level === 1 ? "h1" : block.level === 2 ? "h2" : block.level === 3 ? "h3" : "h4";
            return <Tag key={key}>{inline(block.value, key, onOpenLink)}</Tag>;
          }
          case "paragraph":
            return <p key={key}>{inline(block.value, key, onOpenLink)}</p>;
          case "quote":
            return <blockquote key={key}>{inline(block.value, key, onOpenLink)}</blockquote>;
          case "list": {
            const Tag = block.ordered ? "ol" : "ul";
            return (
              <Tag key={key}>
                {block.values.map((value, item) => (
                  <li key={`${key}-${String(item)}`}>
                    {inline(value, `${key}-${String(item)}`, onOpenLink)}
                  </li>
                ))}
              </Tag>
            );
          }
          case "table":
            return (
              <div className="table-scroll" key={key} tabIndex={0}>
                <table>
                  <thead>
                    <tr>
                      {block.rows[0]?.map((cell, cellIndex) => (
                        <th key={cellIndex}>
                          {inline(cell, `${key}-head-${String(cellIndex)}`, onOpenLink)}
                        </th>
                      ))}
                    </tr>
                  </thead>
                  <tbody>
                    {block.rows.slice(1).map((row, rowIndex) => (
                      <tr key={rowIndex}>
                        {row.map((cell, cellIndex) => (
                          <td key={cellIndex}>
                            {inline(
                              cell,
                              `${key}-${String(rowIndex)}-${String(cellIndex)}`,
                              onOpenLink,
                            )}
                          </td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            );
          case "rule":
            return <hr key={key} />;
          default:
            return <Fragment key={key} />;
        }
      })}
      {streaming && <span className="stream-caret" aria-hidden="true" />}
    </div>
  );
}
