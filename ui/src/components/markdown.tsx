import type { Element, ElementContent, Root, RootContent } from "hast";
import { useMemo, type ReactNode } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import { toast } from "sonner";

import { MermaidBlock } from "@/components/mermaid-block";
import { call } from "@/lib/backend";
import { MATCH_CLASS, splitMatches } from "@/lib/highlight";
import { cn } from "@/lib/utils";

/** `http` and `https` are the only schemes a link opens with. */
export function isWebUrl(url: string): boolean {
  return /^https?:\/\/\S+$/i.test(url);
}

/** Any scheme (`https:`, `data:`, `file:`...) or a protocol-relative `//host`: not a path of the project. */
function isRemote(src: string): boolean {
  return /^[a-z][a-z0-9+.-]*:/i.test(src) || src.startsWith("//");
}

/**
 * A path that stays inside the project: relative, no `..`, no drive or root. Only such paths are
 * handed to `resolveImage`.
 */
export function isProjectPath(src: string): boolean {
  if (src === "" || isRemote(src) || src.startsWith("/") || src.startsWith("\\")) return false;
  return !src.split(/[\\/]/).includes("..");
}

export interface MarkdownProps {
  children: string;
  /**
   * The level of the heading of the element that holds this text (a dialog title is 2): headings
   * inside the text are one level deeper at least and never larger than that heading.
   */
  holderHeadingLevel?: number;
  /**
   * Turns a path inside the project into something an `<img>` can show (the backend serves it), or
   * `null` when it cannot be shown. Without it no image is shown: its alt text takes its place.
   */
  resolveImage?: (path: string) => string | null;
  /** Words to mark wherever they occur in the text (a search): each match is wrapped in a `mark`. Code blocks are left as they are. */
  highlight?: readonly string[];
  className?: string;
}

function textOf(node: ElementContent): string {
  if (node.type === "text") return node.value;
  if (node.type === "element") return node.children.map(textOf).join("");
  return "";
}

const NO_WORDS: readonly string[] = [];

/** A rehype plugin that wraps the matches of `words` in `mark` elements, outside code blocks. */
function markMatches(words: readonly string[]) {
  const mark = (children: RootContent[]): RootContent[] =>
    children.flatMap((child): RootContent[] => {
      if (child.type === "text") {
        return splitMatches(child.value, words).map((piece) =>
          piece.match ? { type: "element", tagName: "mark", properties: {}, children: [{ type: "text", value: piece.text }] } : { type: "text", value: piece.text },
        );
      }
      if (child.type === "element" && child.tagName !== "pre") child.children = mark(child.children) as ElementContent[];
      return [child];
    });
  return () => (tree: Root) => {
    tree.children = mark(tree.children);
  };
}

function mermaidSource(pre: Element | undefined): string | null {
  const code = pre?.children[0];
  if (code?.type !== "element" || code.tagName !== "code") return null;
  const classes = code.properties?.className;
  const isMermaid = Array.isArray(classes) && classes.includes("language-mermaid");
  return isMermaid ? code.children.map(textOf).join("").replace(/\n$/, "") : null;
}

/** Opens `url` in the system browser; an error toast when that fails. */
export function openInBrowser(url: string) {
  call("open_url", { url }).catch(() => toast.error("Could not open the link."));
}

function Link({ href, children }: { href?: string; children?: ReactNode }) {
  // Other links (relative, `mailto:`, `javascript:`...) are plain text: nothing opens inside Weaver.
  if (!href || !isWebUrl(href)) return <span>{children}</span>;
  return (
    <a
      href={href}
      target="_blank"
      rel="noreferrer noopener"
      className="text-sky-ink underline underline-offset-2 hover:no-underline"
      onClick={(event) => {
        event.preventDefault();
        openInBrowser(href);
      }}
    >
      {children}
    </a>
  );
}

/**
 * The one Markdown renderer (`react-markdown` + `remark-gfm`). Raw HTML is shown as text and
 * nothing uses `dangerouslySetInnerHTML`; supported are GFM tables, strikethrough, read-only task
 * lists, autolinks and fenced code; `mermaid` blocks are drawn on first use. Links open only when
 * `http` or `https`, in the system browser; remote images are never fetched and show as links;
 * nothing scrolls sideways except tables and code, inside their own box.
 */
export function Markdown({ children, holderHeadingLevel = 2, resolveImage, highlight = NO_WORDS, className }: MarkdownProps) {
  const rehypePlugins = useMemo(() => (highlight.length > 0 ? [markMatches(highlight)] : []), [highlight]);
  const components = useMemo<Components>(() => {
    const heading = (level: number) => {
      const Tag = `h${Math.min(6, Math.max(level, holderHeadingLevel + 1))}` as "h3";
      // One size for every heading: the body's, which no holder heading is smaller than.
      return function Heading({ children }: { children?: ReactNode }) {
        return <Tag className="mt-3 text-sm font-semibold first:mt-0">{children}</Tag>;
      };
    };
    return {
      h1: heading(1),
      h2: heading(2),
      h3: heading(3),
      h4: heading(4),
      h5: heading(5),
      h6: heading(6),
      p: ({ children }) => <p className="leading-relaxed">{children}</p>,
      a: ({ href, children }) => <Link href={href}>{children}</Link>,
      img: ({ src, alt }) => {
        const text = alt || src || "image";
        if (typeof src === "string" && isProjectPath(src)) {
          const url = resolveImage?.(src) ?? null;
          return url ? <img src={url} alt={alt ?? ""} className="max-w-full" /> : <span>{text}</span>;
        }
        // Remote images are never fetched: they show as a link (or as text when not even a web link).
        return typeof src === "string" && isWebUrl(src) ? <Link href={src}>{text}</Link> : <span>{text}</span>;
      },
      ul: ({ className: c, children }) => (
        <ul className={cn("ml-5 list-disc space-y-1", c?.includes("contains-task-list") && "ml-0 list-none")}>
          {children}
        </ul>
      ),
      ol: ({ children }) => <ol className="ml-5 list-decimal space-y-1">{children}</ol>,
      li: ({ children }) => <li className="[&>p]:inline">{children}</li>,
      input: ({ checked }) => (
        <input
          type="checkbox"
          checked={!!checked}
          readOnly
          disabled
          aria-label={checked ? "Done" : "Not done"}
          className="mr-2 align-middle"
        />
      ),
      blockquote: ({ children }) => (
        <blockquote className="border-l-2 border-border pl-3 text-muted-foreground">{children}</blockquote>
      ),
      hr: () => <hr className="border-border" />,
      del: ({ children }) => <del className="text-muted-foreground">{children}</del>,
      table: ({ children }) => (
        <div className="max-w-full overflow-x-auto">
          <table className="w-max min-w-full border-collapse text-left text-sm">{children}</table>
        </div>
      ),
      th: ({ children }) => <th className="border border-border bg-muted px-2 py-1 font-semibold">{children}</th>,
      td: ({ children }) => <td className="border border-border px-2 py-1 align-top">{children}</td>,
      pre: ({ node, children }) => {
        const diagram = mermaidSource(node);
        if (diagram !== null) return <MermaidBlock code={diagram} />;
        return <pre className="max-w-full overflow-x-auto rounded-md bg-muted p-3 font-mono text-xs">{children}</pre>;
      },
      code: ({ className: c, children }) => <code className={cn("font-mono", !c && "rounded-sm bg-muted px-1 py-0.5 text-xs")}>{children}</code>,
      mark: ({ children }) => <mark className={MATCH_CLASS}>{children}</mark>,
    };
  }, [holderHeadingLevel, resolveImage]);

  return (
    <div className={cn("min-w-0 max-w-full space-y-2 break-words text-sm", className)}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={rehypePlugins} components={components} urlTransform={(url) => url}>
        {children}
      </ReactMarkdown>
    </div>
  );
}
