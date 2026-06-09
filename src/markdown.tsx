import { useState } from "react";
import type { ReactNode } from "react";

// A tiny, dependency-free Markdown renderer for AI chat replies.
//
// It deliberately covers only what assistant output actually uses: fenced code
// blocks (with a copy button), inline code, bold/italic, links, headings,
// unordered/ordered lists, blockquotes and horizontal rules. Anything it does
// not understand degrades to plain text, so untrusted model output can never
// inject markup — we never use dangerouslySetInnerHTML.

type Block =
  | { kind: "code"; lang: string; content: string }
  | { kind: "heading"; level: number; text: string }
  | { kind: "list"; ordered: boolean; items: string[] }
  | { kind: "quote"; lines: string[] }
  | { kind: "rule" }
  | { kind: "paragraph"; lines: string[] };

const FENCE = /^\s*```(.*)$/;
const HEADING = /^(#{1,6})\s+(.*)$/;
const UNORDERED = /^\s*[-*+]\s+(.*)$/;
const ORDERED = /^\s*\d+[.)]\s+(.*)$/;
const QUOTE = /^\s*>\s?(.*)$/;
const RULE = /^\s*([-*_])(\s*\1){2,}\s*$/;

function parseBlocks(source: string): Block[] {
  const lines = source.replace(/\r\n?/g, "\n").split("\n");
  const blocks: Block[] = [];
  let i = 0;

  while (i < lines.length) {
    const line = lines[i];

    // Fenced code block: capture verbatim until the closing fence.
    const fence = line.match(FENCE);
    if (fence) {
      const lang = fence[1].trim();
      const body: string[] = [];
      i += 1;
      while (i < lines.length && !FENCE.test(lines[i])) {
        body.push(lines[i]);
        i += 1;
      }
      // Skip the closing fence if present.
      if (i < lines.length) {
        i += 1;
      }
      blocks.push({ kind: "code", lang, content: body.join("\n") });
      continue;
    }

    if (line.trim() === "") {
      i += 1;
      continue;
    }

    if (RULE.test(line)) {
      blocks.push({ kind: "rule" });
      i += 1;
      continue;
    }

    const heading = line.match(HEADING);
    if (heading) {
      blocks.push({ kind: "heading", level: heading[1].length, text: heading[2] });
      i += 1;
      continue;
    }

    if (QUOTE.test(line)) {
      const quoteLines: string[] = [];
      while (i < lines.length && QUOTE.test(lines[i])) {
        quoteLines.push(lines[i].match(QUOTE)![1]);
        i += 1;
      }
      blocks.push({ kind: "quote", lines: quoteLines });
      continue;
    }

    if (UNORDERED.test(line) || ORDERED.test(line)) {
      const ordered = ORDERED.test(line) && !UNORDERED.test(line);
      const items: string[] = [];
      while (i < lines.length) {
        const u = lines[i].match(UNORDERED);
        const o = lines[i].match(ORDERED);
        if (ordered && o) {
          items.push(o[1]);
        } else if (!ordered && u) {
          items.push(u[1]);
        } else {
          break;
        }
        i += 1;
      }
      blocks.push({ kind: "list", ordered, items });
      continue;
    }

    // Paragraph: gather consecutive non-blank lines that don't start a new block.
    const paraLines: string[] = [];
    while (i < lines.length && lines[i].trim() !== "") {
      const l = lines[i];
      if (
        FENCE.test(l) ||
        HEADING.test(l) ||
        QUOTE.test(l) ||
        RULE.test(l) ||
        UNORDERED.test(l) ||
        ORDERED.test(l)
      ) {
        break;
      }
      paraLines.push(l);
      i += 1;
    }
    blocks.push({ kind: "paragraph", lines: paraLines });
  }

  return blocks;
}

// Inline parser: walks the string once, emitting React nodes for code spans,
// bold, italic and links. Unmatched markers are treated as literal text.
function renderInline(text: string, keyPrefix: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  let buffer = "";
  let key = 0;
  let i = 0;

  const flush = () => {
    if (buffer) {
      nodes.push(buffer);
      buffer = "";
    }
  };

  while (i < text.length) {
    const rest = text.slice(i);

    // Inline code: `code`
    if (text[i] === "`") {
      const end = text.indexOf("`", i + 1);
      if (end > i) {
        flush();
        nodes.push(
          <code className="mdInlineCode" key={`${keyPrefix}-c${key++}`}>
            {text.slice(i + 1, end)}
          </code>,
        );
        i = end + 1;
        continue;
      }
    }

    // Link: [label](url)
    if (text[i] === "[") {
      const match = rest.match(/^\[([^\]]+)\]\(([^)\s]+)\)/);
      if (match) {
        flush();
        nodes.push(
          <span className="mdLink" key={`${keyPrefix}-l${key++}`} title={match[2]}>
            {match[1]}
          </span>,
        );
        i += match[0].length;
        continue;
      }
    }

    // Bold: **text** or __text__
    if (rest.startsWith("**") || rest.startsWith("__")) {
      const marker = rest.slice(0, 2);
      const end = text.indexOf(marker, i + 2);
      if (end > i + 1) {
        flush();
        nodes.push(
          <strong key={`${keyPrefix}-b${key++}`}>
            {renderInline(text.slice(i + 2, end), `${keyPrefix}-b${key}`)}
          </strong>,
        );
        i = end + 2;
        continue;
      }
    }

    // Italic: *text* or _text_ (single marker, not part of a bold run)
    if ((text[i] === "*" || text[i] === "_") && text[i + 1] !== text[i]) {
      const marker = text[i];
      const end = text.indexOf(marker, i + 1);
      if (end > i) {
        flush();
        nodes.push(
          <em key={`${keyPrefix}-i${key++}`}>
            {renderInline(text.slice(i + 1, end), `${keyPrefix}-i${key}`)}
          </em>,
        );
        i = end + 1;
        continue;
      }
    }

    buffer += text[i];
    i += 1;
  }

  flush();
  return nodes;
}

function CodeBlock({ lang, content }: { lang: string; content: string }) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(content);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1400);
    } catch {
      // Clipboard may be unavailable in the web preview; ignore.
    }
  };

  return (
    <div className="mdCodeBlock">
      <div className="mdCodeBlockHeader">
        <span className="mdCodeLang">{lang || "code"}</span>
        <button type="button" className="mdCodeCopy" onClick={copy}>
          {copied ? "已复制" : "复制"}
        </button>
      </div>
      <pre>
        <code>{content}</code>
      </pre>
    </div>
  );
}

export function Markdown({ content }: { content: string }) {
  const blocks = parseBlocks(content);

  return (
    <div className="mdRoot">
      {blocks.map((block, index) => {
        const key = `b${index}`;
        switch (block.kind) {
          case "code":
            return <CodeBlock key={key} lang={block.lang} content={block.content} />;
          case "heading":
            return (
              <p className="mdHeading" data-level={Math.min(block.level, 6)} key={key}>
                {renderInline(block.text, key)}
              </p>
            );
          case "list":
            return block.ordered ? (
              <ol className="mdList" key={key}>
                {block.items.map((item, idx) => (
                  <li key={`${key}-${idx}`}>{renderInline(item, `${key}-${idx}`)}</li>
                ))}
              </ol>
            ) : (
              <ul className="mdList" key={key}>
                {block.items.map((item, idx) => (
                  <li key={`${key}-${idx}`}>{renderInline(item, `${key}-${idx}`)}</li>
                ))}
              </ul>
            );
          case "quote":
            return (
              <blockquote className="mdQuote" key={key}>
                {renderInline(block.lines.join("\n"), key)}
              </blockquote>
            );
          case "rule":
            return <hr className="mdRule" key={key} />;
          case "paragraph":
          default:
            return (
              <p className="mdParagraph" key={key}>
                {block.lines.map((line, idx) => (
                  <span key={`${key}-${idx}`}>
                    {idx > 0 ? <br /> : null}
                    {renderInline(line, `${key}-${idx}`)}
                  </span>
                ))}
              </p>
            );
        }
      })}
    </div>
  );
}
