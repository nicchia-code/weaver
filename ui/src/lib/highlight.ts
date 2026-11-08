// Searching a text for the words of a query: case-insensitive, every word must be there. Used by
// the search of the Requirements, which filters its sections and marks the matches.

/** The look of a marked match: neutral, never a colour of state. */
export const MATCH_CLASS = "rounded-sm bg-ink/15 text-foreground";

/** The words of a query: split at white space, no empty one, none twice. */
export function searchWords(query: string): string[] {
  return [...new Set(query.split(/\s+/).filter((word) => word !== ""))];
}

const escaped = (word: string) => word.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** Whether `text` contains every word, whatever the case. No words match every text. */
export function containsWords(text: string, words: readonly string[]): boolean {
  return words.every((word) => new RegExp(escaped(word), "i").test(text));
}

export interface TextPiece {
  text: string;
  /** The piece is one of the words. */
  match: boolean;
}

/** `text` cut into the pieces that match one of `words` and the ones between them, in order. */
export function splitMatches(text: string, words: readonly string[]): TextPiece[] {
  if (words.length === 0 || text === "") return [{ text, match: false }];
  // The longest word first, so that a word is never cut short by one it contains.
  const pattern = [...words].sort((a, b) => b.length - a.length).map(escaped).join("|");
  const pieces: TextPiece[] = [];
  let from = 0;
  for (const found of text.matchAll(new RegExp(pattern, "gi"))) {
    if (found[0] === "") continue;
    if (found.index > from) pieces.push({ text: text.slice(from, found.index), match: false });
    pieces.push({ text: found[0], match: true });
    from = found.index + found[0].length;
  }
  if (from < text.length) pieces.push({ text: text.slice(from), match: false });
  return pieces;
}
