import type { ViewId } from "@/views";

/** What choosing a row does. */
export type PaletteChoice =
  | { kind: "view"; view: ViewId }
  | { kind: "action"; action: "scan" | "run-all" | "new-request" | "open-folder" | "usage" }
  | { kind: "project"; id: string; path: string; name: string; current: boolean }
  | { kind: "card"; id: string }
  | { kind: "request"; id: string }
  | { kind: "section"; key: string }
  | { kind: "history"; id: string };

export interface PaletteItem {
  /** Identifies the row in the list. */
  value: string;
  label: string;
  /** What follows the label in Muted: the Area and fragment of a section, a path, a date. */
  detail?: string;
  /** What the text typed is compared with: names, titles and short numbers. */
  terms: readonly string[];
  /** Why the row cannot be chosen, in Muted; empty when it can. */
  reasons: readonly string[];
  choice: PaletteChoice;
}

export const GROUP_NAMES = ["Views", "Actions", "Projects", "Cards", "Requirements", "History"] as const;
export type PaletteGroupName = (typeof GROUP_NAMES)[number];

export interface PaletteGroup {
  name: PaletteGroupName;
  items: PaletteItem[];
}

/** The groups that show while the field is empty. */
const WITHOUT_TEXT: readonly PaletteGroupName[] = ["Views", "Actions", "Projects"];

/** At most this many rows in a group. */
export const GROUP_LIMIT = 8;

/** 0 when a term starts with the text, 1 when one contains it, `null` when none does; case-insensitive. */
export function matchRank(terms: readonly string[], text: string): 0 | 1 | null {
  const needle = text.trim().toLowerCase();
  let rank: 0 | 1 | null = null;
  for (const term of terms) {
    const lower = term.toLowerCase();
    if (lower.startsWith(needle)) return 0;
    if (lower.includes(needle)) rank = 1;
  }
  return rank;
}

/**
 * The rows of the palette for what was typed: the groups in their order, each with the rows that
 * match, those that start with the text first (otherwise in the order given), at most
 * [`GROUP_LIMIT`] per group. With the field empty only Views, Actions and Projects show. A group
 * with no row is left out.
 */
export function arrange(groups: ReadonlyMap<PaletteGroupName, readonly PaletteItem[]>, text: string): PaletteGroup[] {
  const typed = text.trim() !== "";
  const shown: PaletteGroup[] = [];
  for (const name of GROUP_NAMES) {
    if (!typed && !WITHOUT_TEXT.includes(name)) continue;
    const ranked = (groups.get(name) ?? []).flatMap((item) => {
      const rank = typed ? matchRank(item.terms, text) : 0;
      return rank === null ? [] : [{ item, rank }];
    });
    // `sort` is stable: rows of the same rank keep the order they came in.
    const items = ranked
      .sort((a, b) => a.rank - b.rank)
      .slice(0, GROUP_LIMIT)
      .map(({ item }) => item);
    if (items.length > 0) shown.push({ name, items });
  }
  return shown;
}
