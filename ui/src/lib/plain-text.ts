// Markdown shown as plain text: a one-line summary, the first paragraph of a body.

/** A text without its inline Markdown marks (links give their text), for a row or a card. */
export function plainText(markdown: string): string {
  return markdown
    .replace(/!?\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/(\*\*|__|~~|[*_`])/g, "")
    .trim();
}

/**
 * The first paragraph of a Markdown body as plain text: the first block between blank lines that
 * is not a fenced code block, with its heading, list and quote marks removed and its lines joined
 * by spaces. Empty when the body has none.
 */
export function firstParagraph(markdown: string): string {
  for (const block of markdown.split(/\n\s*\n/)) {
    const trimmed = block.trim();
    if (trimmed === "" || trimmed.startsWith("```") || trimmed.startsWith("~~~")) continue;
    const lines = trimmed.split("\n").map((line) => line.replace(/^\s*(#{1,6}\s+|[-*+]\s+|\d+[.)]\s+|>\s?)/, "").trim());
    const text = plainText(lines.join(" "));
    if (text !== "") return text;
  }
  return "";
}
