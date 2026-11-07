import type { Entry } from "@/bindings/Entry";

/** What the conversation shows, top to bottom: an entry, or a run of steps folded into one disclosure. */
export type ChatItem =
  | {
      kind: "entry";
      /** Index of the entry. */
      index: number;
      /** The first item of a block of the agent's output: a sender row (`Weaver HH:MM`) stands above it. */
      starts: boolean;
      /** The instant of that row's time; `null` when it has none. */
      sender: number | null;
    }
  | {
      kind: "work";
      /** Indexes of the first and the last entry of the run. */
      from: number;
      to: number;
      /** The tool rows and the `Thinking` blocks it holds. */
      steps: number;
      /** The instant of the first and of the last entry, in milliseconds since the Unix epoch. */
      began: number | null;
      ended: number | null;
      starts: boolean;
      sender: number | null;
    };

/** A `Thinking` block or a tool row: what a run of steps folds. Sub-agent lines are part of tool rows. */
const isStep = (entry: Entry) => entry.kind === "thinking" || entry.kind === "tool";

/**
 * The items of the conversation: every run of consecutive `Thinking` blocks and tool rows is one
 * item, and the messages of the agent (with the runs between them) form a block that starts with a
 * sender row. Permission cards, notices, started jobs and messages of the developer are never inside
 * a run and end the block. The time of a sender row is the one of the first text of the agent in the
 * block, else of its first entry.
 */
export function chatItems(entries: readonly Entry[], arrivals: readonly number[]): ChatItem[] {
  const at = (index: number): number | null => arrivals[index] ?? null;
  const items: ChatItem[] = [];
  for (let index = 0; index < entries.length; ) {
    const entry = entries[index]!;
    if (isStep(entry)) {
      let to = index;
      while (to + 1 < entries.length && isStep(entries[to + 1]!)) to += 1;
      items.push({ kind: "work", from: index, to, steps: to - index + 1, began: at(index), ended: at(to), starts: false, sender: null });
      index = to + 1;
    } else {
      items.push({ kind: "entry", index, starts: false, sender: null });
      index += 1;
    }
  }

  // The sender rows: the first item of every block of the agent's output.
  const output = (item: ChatItem) => item.kind === "work" || entries[item.index]?.kind === "agent";
  for (let position = 0; position < items.length; position += 1) {
    const item = items[position]!;
    if (!output(item) || (position > 0 && output(items[position - 1]!))) continue;
    let first: number | null = null;
    let fallback: number | null = null;
    for (let next = position; next < items.length && output(items[next]!); next += 1) {
      const candidate = items[next]!;
      const start = candidate.kind === "work" ? candidate.from : candidate.index;
      fallback ??= at(start);
      if (candidate.kind === "entry") {
        first = at(candidate.index);
        break;
      }
    }
    item.starts = true;
    item.sender = first ?? fallback;
  }
  return items;
}

/** `HH:MM` in local time, 24 h. */
export function clockText(milliseconds: number): string {
  const time = new Date(milliseconds);
  return `${String(time.getHours()).padStart(2, "0")}:${String(time.getMinutes()).padStart(2, "0")}`;
}

/**
 * `Worked for <duration> · <n> steps`, or `Working · <n> steps` while the run is still going; the
 * latest step follows the running text when the view shows it (jobs, Ask more and sub-agents).
 * A run whose instants are not known (a sub-agent's transcript) reads `Worked · <n> steps`.
 */
export function workText(
  item: Extract<ChatItem, { kind: "work" }>,
  running: boolean,
  duration: (milliseconds: number) => string,
  latest: string | null = null,
): string {
  const steps = `${item.steps} ${item.steps === 1 ? "step" : "steps"}`;
  if (running) return latest === null ? `Working · ${steps}` : `Working · ${steps} · ${latest}`;
  if (item.began === null || item.ended === null) return `Worked · ${steps}`;
  return `Worked for ${duration(Math.max(0, item.ended - item.began))} · ${steps}`;
}

/** The latest step of a run: the title of its last tool row, or `Thinking`. */
export function latestStep(item: Extract<ChatItem, { kind: "work" }>, entries: readonly Entry[]): string {
  const last = entries[item.to];
  return last?.kind === "tool" ? last.title : "Thinking";
}
