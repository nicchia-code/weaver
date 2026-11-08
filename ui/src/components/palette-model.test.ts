import { describe, expect, it } from "vitest";

import { GROUP_LIMIT, GROUP_NAMES, arrange, matchRank, type PaletteGroupName, type PaletteItem } from "@/components/palette-model";

const item = (label: string, extra: Partial<PaletteItem> = {}): PaletteItem => ({ value: label, label, terms: [label], reasons: [], choice: { kind: "card", id: label }, ...extra });
const rows = (entries: Partial<Record<PaletteGroupName, PaletteItem[]>>) => new Map(Object.entries(entries) as [PaletteGroupName, PaletteItem[]][]);
const all = rows({
  Views: [item("Board"), item("Jobs")],
  Actions: [item("Scan System")],
  Projects: [item("alpha")],
  Cards: [item("Add a cart", { terms: ["P1", "Add a cart"] })],
  Requirements: [item("Cart total")],
  History: [item("Cart added")],
});

describe("the groups", () => {
  it("come in the order Views, Actions, Projects, Cards, Requirements, History", () => {
    expect(GROUP_NAMES).toEqual(["Views", "Actions", "Projects", "Cards", "Requirements", "History"]);
    expect(arrange(all, "a").map((group) => group.name)).toEqual(["Views", "Actions", "Projects", "Cards", "Requirements", "History"]);
  });

  it("show only Views, Actions and Projects while the field is empty", () => {
    expect(arrange(all, "").map((group) => group.name)).toEqual(["Views", "Actions", "Projects"]);
    expect(arrange(all, "   ").map((group) => group.name)).toEqual(["Views", "Actions", "Projects"]);
  });

  it("leave out a group with no match", () => {
    expect(arrange(all, "cart").map((group) => group.name)).toEqual(["Cards", "Requirements", "History"]);
  });

  it("hold at most 8 rows", () => {
    const many = Array.from({ length: 12 }, (_, index) => item(`Row ${index}`));
    expect(arrange(rows({ Views: many }), "").at(0)?.items).toHaveLength(GROUP_LIMIT);
    expect(arrange(rows({ Views: many }), "row")[0]?.items).toHaveLength(8);
  });
});

describe("matching", () => {
  it("ignores case and looks at names, titles and short numbers", () => {
    expect(arrange(all, "p1")[0]?.items.map((row) => row.label)).toEqual(["Add a cart"]);
    expect(arrange(all, "SCAN")[0]?.items.map((row) => row.label)).toEqual(["Scan System"]);
  });

  it("puts what starts with the text first and keeps the order otherwise", () => {
    const view = rows({ Views: [item("Chat"), item("Specs"), item("Files"), item("Notes")] });
    expect(arrange(view, "s")[0]?.items.map((row) => row.label)).toEqual(["Specs", "Chat", "Files", "Notes"].filter((label) => /s/i.test(label)).sort((a, b) => Number(!a.toLowerCase().startsWith("s")) - Number(!b.toLowerCase().startsWith("s"))));
  });

  it("ranks 0 for a start, 1 for an inside match and null for none", () => {
    expect(matchRank(["Board"], "bo")).toBe(0);
    expect(matchRank(["Dashboard"], "board")).toBe(1);
    expect(matchRank(["P1", "Add"], "p1")).toBe(0);
    expect(matchRank(["Board"], "zz")).toBeNull();
  });
});
