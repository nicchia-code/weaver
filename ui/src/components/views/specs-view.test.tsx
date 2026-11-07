import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ViewSnapshot } from "@/bindings/ViewSnapshot";
import type { FocusSubject } from "@/components/views/focus-model";
import { fieldTitle, SpecsView } from "@/components/views/specs-view";
import { useViews } from "@/components/views/views-state";
import { snapshot } from "@/components/views/view-fixtures";
import { fakeBackend, renderViews, type FakeBackend } from "@/components/views/view-test-utils";

// The collection of logos, held back until a test lets it through, so that a tile can be seen before it arrives.
const collection = vi.hoisted(() => {
  // `Promise.withResolvers` is not in the lib of this project's tsconfig.
  let open!: () => void;
  const gate = new Promise<void>((resolve) => (open = resolve));
  return { gate, open };
});

vi.mock("simple-icons", async () => {
  await collection.gate;
  const logo = (title: string, slug: string, path: string) => ({ title, slug, hex: "FF0000", path });
  return {
    siTypescript: logo("TypeScript", "typescript", "M1 1h22v22H1z"),
    siTailwindcss: logo("Tailwind CSS", "tailwindcss", "M2 2h20v20H2z"),
    siNodedotjs: logo("Node.js", "nodedotjs", "M3 3h18v18H3z"),
    siTauri: logo("Tauri", "tauri", "M4 4h16v16H4z"),
    siReact: logo("React", "react", "M5 5h14v14H5z"),
    siMarkdown: logo("Markdown", "markdown", "M6 6h12v12H6z"),
  };
});

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

let current: ViewSnapshot;
let backend: FakeBackend;

beforeEach(() => {
  current = snapshot();
  backend = fakeBackend(() => current);
});

function FocusOpener({ refs, subject }: { refs: string[]; subject: FocusSubject }) {
  const { focusOn } = useViews();
  return <button onClick={() => focusOn("specs", refs, subject)}>open focus</button>;
}

async function show(extra?: React.ReactNode) {
  renderViews(
    <>
      {extra}
      <SpecsView />
    </>,
  );
  await screen.findByRole("heading", { name: "Languages" });
}

/** The tile of an entry: its icon is a logo (the path it draws) or a lucide icon of the field. */
function tileOf(entry: HTMLElement) {
  const icon = entry.querySelector("span[aria-hidden] svg")!;
  const classes = icon.getAttribute("class") ?? "";
  return { svg: icon, path: classes.includes("lucide") ? null : (icon.querySelector("path")?.getAttribute("d") ?? null), classes };
}

describe("the logos of the Specs", () => {
  const entry = (name: string) => ({ name, version: null, note: null, chosenReason: null, change: null, before: null });

  it("shows the field's icon in every tile until the collection is there, then each logo in its place, and never waits for it", async () => {
    current = snapshot({}, { specs: { fields: [{ ...snapshot().data.specs!.fields[0]!, entries: [entry("TypeScript"), entry("Rust")] }] } });
    await show();
    const before = within(screen.getByRole("region", { name: "Languages" })).getAllByRole("listitem");
    expect(before.map((item) => tileOf(item).classes)).toEqual([expect.stringContaining("lucide-code"), expect.stringContaining("lucide-code")]);
    collection.open();
    await waitFor(() => expect(tileOf(screen.getByRole("listitem", { name: "TypeScript" })).path).toBe("M1 1h22v22H1z"));
    expect(tileOf(screen.getByRole("listitem", { name: "Rust" })).classes).toContain("lucide-code");
  });

  it("draws a logo in Ink 2, 16 px, decorative and with no brand colour", async () => {
    current = snapshot({}, { specs: { fields: [{ ...snapshot().data.specs!.fields[0]!, entries: [entry("TypeScript")] }] } });
    collection.open();
    await show();
    await waitFor(() => expect(tileOf(screen.getByRole("listitem", { name: "TypeScript" })).path).not.toBeNull());
    const tile = screen.getByRole("listitem", { name: "TypeScript" }).querySelector("span[aria-hidden]")!;
    const { svg } = tileOf(screen.getByRole("listitem", { name: "TypeScript" }));
    expect(tile).toHaveClass("size-[26px]", "rounded-sm", "bg-paper", "text-ink-2");
    expect(svg).toHaveAttribute("fill", "currentColor");
    expect(svg).toHaveAttribute("aria-hidden", "true");
    expect(svg).toHaveClass("size-4");
    expect(svg.outerHTML).not.toContain("FF0000");
  });

  it("matches the whole name ignoring case, spaces and punctuation, against a title or a slug, and never partly", async () => {
    current = snapshot(
      {},
      {
        specs: {
          fields: [
            {
              ...snapshot().data.specs!.fields[0]!,
              entries: ["Tailwind CSS", "Node.js", "nodedotjs", "TAILWIND-css", "Tauri plugins", "react-markdown"].map(entry),
            },
          ],
        },
      },
    );
    collection.open();
    await show();
    await waitFor(() => expect(tileOf(screen.getByRole("listitem", { name: "Node.js" })).path).not.toBeNull());
    const path = (name: string) => tileOf(screen.getByRole("listitem", { name })).path;
    expect(path("Tailwind CSS")).toBe("M2 2h20v20H2z");
    expect(path("Node.js")).toBe("M3 3h18v18H3z");
    expect(path("nodedotjs")).toBe("M3 3h18v18H3z");
    expect(path("TAILWIND-css")).toBe("M2 2h20v20H2z");
    expect(path("Tauri plugins")).toBeNull();
    expect(path("react-markdown")).toBeNull();
    expect(tileOf(screen.getByRole("listitem", { name: "Tauri plugins" })).classes).toContain("lucide-code");
  });

  it("keeps the field icons, with no notice, when the collection cannot be loaded", async () => {
    vi.resetModules();
    vi.doMock("simple-icons", () => {
      throw new Error("no collection");
    });
    const { loadLogos } = await import("@/components/views/spec-logos");
    await expect(loadLogos()).resolves.toBeNull();
    vi.doUnmock("simple-icons");
  });
});

describe("the field icons of the Specs", () => {
  const icons: Record<string, string> = {
    languages: "lucide-code",
    frameworks: "lucide-layers",
    data: "lucide-database",
    testing: "lucide-flask-conical",
    ci: "lucide-workflow",
    repository: "lucide-git-branch",
    deployment: "lucide-rocket",
    cloud: "lucide-cloud",
    infrastructure: "lucide-server",
    notes: "lucide-notebook-pen",
  };

  it("puts a 16 px decorative icon before each title, one per field", async () => {
    const base = snapshot().data.specs!.fields[0]!;
    const fields = Object.keys(icons).map((name) =>
      name === "notes"
        ? { ...base, name, entries: [], notes: "Notes." }
        : { ...base, name, entries: [{ name: "Thing", version: null, note: null, chosenReason: null, change: null, before: null }] },
    );
    current = snapshot({}, { specs: { fields } });
    renderViews(<SpecsView />);
    await screen.findByRole("heading", { name: "Languages" });
    for (const [name, lucide] of Object.entries(icons)) {
      const heading = document.getElementById(`specs-${name}`)!;
      const icon = heading.previousElementSibling!;
      expect(icon.getAttribute("class")).toContain(lucide);
      expect(icon).toHaveClass("size-4");
      expect(icon).toHaveAttribute("aria-hidden", "true");
    }
  });

  it("keeps the content on the left edge of the view header's title, under the focus too", async () => {
    await show(<FocusOpener refs={["specs#languages"]} subject={{ kind: "ref", ref: "specs#languages" }} />);
    const section = screen.getByRole("region", { name: "Languages" });
    expect(section).toHaveClass("-ml-4", "pl-3", "border-l-4", "border-transparent");
    await userEvent.setup().click(screen.getByRole("button", { name: "open focus" }));
    await waitFor(() => expect(section).toHaveClass("border-sky", "-ml-4", "pl-3"));
  });
});

describe("the Specs page", () => {
  it("shows the fields in their order and names them for people", async () => {
    await show();
    expect(screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent)).toEqual(["Languages", "Notes"]);
    expect(fieldTitle("ci")).toBe("CI");
    expect(fieldTitle("infrastructure")).toBe("Infrastructure");
  });

  it("has the title Specs, its subtitle with the file name in monospace and the pills `All` and `Agent's choice` with their counts, and no overlay picker or `Ask more`", async () => {
    await show();
    const header = screen.getByRole("heading", { level: 1, name: "Specs" }).closest("header")!;
    expect(within(header).getByText("specs.yaml")).toHaveClass("font-mono");
    expect(header).toHaveTextContent("The stack the agents build with · specs.yaml");
    expect(within(header).getAllByRole("button").map((b) => b.textContent)).toEqual(["All2", "Agent's choice1"]);
    expect(within(header).getByRole("button", { name: "All 2" })).toHaveAttribute("aria-pressed", "true");
    expect(within(header).getByRole("button", { name: "Agent's choice 1" })).toHaveAttribute("aria-pressed", "false");
    expect(within(header).queryByRole("combobox")).toBeNull();
    expect(screen.queryByRole("combobox", { name: "Overlay" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Ask more/ })).not.toBeInTheDocument();
    expect(screen.queryByText(/Local only/)).not.toBeInTheDocument();
  });

  it("shows only what the agent chose under `Agent's choice`, hides the fields left empty and goes back with `All`", async () => {
    const user = userEvent.setup();
    await show();
    await user.click(screen.getByRole("button", { name: "Agent's choice 1" }));
    expect(screen.getByRole("button", { name: "Agent's choice 1" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("button", { name: "All 2" })).toHaveAttribute("aria-pressed", "false");
    const entries = within(screen.getByRole("region", { name: "Languages" })).getAllByRole("listitem");
    expect(entries.map((entry) => entry.getAttribute("aria-label"))).toEqual(["Rust"]);
    expect(screen.queryByRole("region", { name: "Notes" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "All 2" }));
    expect(within(screen.getByRole("region", { name: "Languages" })).getAllByRole("listitem")).toHaveLength(2);
    expect(screen.getByRole("region", { name: "Notes" })).toBeInTheDocument();
  });

  it("gives each field its title, its number of entries, a rule and `Open in editor` at the field's line", async () => {
    const user = userEvent.setup();
    await show();
    const field = screen.getByRole("region", { name: "Languages" });
    expect(within(field).getByRole("heading", { level: 2, name: "Languages" })).toHaveClass("font-semibold");
    expect(within(field).getByText("2")).toHaveClass("text-muted-foreground");
    await user.click(within(field).getByRole("button", { name: "Open in editor" }));
    await waitFor(() => expect(backend.opened).toEqual([{ file: ".weaver/specs.yaml", line: 2 }]));
  });

  it("starts with the view header titled Specs, with the focus bar full width right under it", async () => {
    await show(<FocusOpener refs={["specs#languages"]} subject={{ kind: "ref", ref: "specs#languages" }} />);
    await userEvent.setup().click(screen.getByRole("button", { name: "open focus" }));
    const bar = await screen.findByRole("region", { name: "Focus" });
    const header = screen.getByRole("heading", { level: 1, name: "Specs" }).closest("header")!;
    expect(header).toHaveClass("h-[76px]", "border-b");
    expect(header.nextElementSibling).toBe(bar);
  });

  it("shows an entry with its tile, name, version in monospace and note, in a grid of columns of at least 250 px, at most 1180 px wide", async () => {
    await show();
    const field = screen.getByRole("region", { name: "Languages" });
    const entries = within(field).getAllByRole("listitem");
    expect(entries).toHaveLength(2);
    expect(within(entries[0]!).queryByText("T")).not.toBeInTheDocument();
    expect(within(entries[0]!).getByText("TypeScript")).toBeInTheDocument();
    expect(within(entries[0]!).getByText("5.9")).toHaveClass("font-mono");
    expect(within(entries[0]!).getByText("Strict")).toBeInTheDocument();
    expect(entries[0]!.parentElement).toHaveClass("grid-cols-[repeat(auto-fill,minmax(250px,1fr))]");
    expect(field.parentElement).toHaveClass("max-w-[1180px]");
  });

  it("marks what the agent chose with the pill `Agent's choice`, whose tooltip is the reason", async () => {
    const user = userEvent.setup();
    await show();
    const entries = within(screen.getByRole("region", { name: "Languages" })).getAllByRole("listitem");
    expect(within(entries[0]!).queryByText("Agent's choice")).not.toBeInTheDocument();
    const mark = within(entries[1]!).getByText("Agent's choice");
    await user.hover(mark);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Fast and safe");
  });

  it("renders the notes as Markdown in one box and opens the file at the field's line", async () => {
    const user = userEvent.setup();
    await show();
    const notes = screen.getByRole("region", { name: "Notes" });
    expect(within(notes).getByText("notes").tagName).toBe("EM");
    expect(within(notes).getByText("notes").closest("div.ring-1")).not.toBeNull();
    await user.click(within(notes).getByRole("button", { name: "Open in editor" }));
    await waitFor(() => expect(backend.opened).toEqual([{ file: ".weaver/specs.yaml", line: 9 }]));
  });

  it("says so when the project has no Specs", async () => {
    current = snapshot({}, { specs: null });
    renderViews(<SpecsView />);
    expect(await screen.findByText("The project has no Specs.")).toBeInTheDocument();
  });
});

describe("the overlay labels of the Specs", () => {
  it("label entries added, changed and removed in words, and `Show before` shows how a changed one read", async () => {
    current = snapshot({ overlay: "pppppppp", overlays: [{ id: "pppppppp", shortNumber: "P3", title: "Pay" }] });
    const field = current.data.specs!.fields[0]!;
    field.entries = [
      { name: "TypeScript", version: "6.0", note: null, chosenReason: null, change: "changed", before: "TypeScript 5.9 — Strict" },
      { name: "Go", version: null, note: null, chosenReason: null, change: "added", before: null },
      { name: "Rust", version: null, note: null, chosenReason: null, change: "removed", before: null },
    ];
    const user = userEvent.setup();
    await show();
    expect(screen.getByRole("listitem", { name: "TypeScript, changed" })).toBeInTheDocument();
    expect(screen.getByRole("listitem", { name: "Go, added" })).toBeInTheDocument();
    const removed = screen.getByRole("listitem", { name: "Rust, removed" });
    expect(within(removed).getByText("Rust")).toHaveClass("line-through");
    await user.click(within(screen.getByRole("listitem", { name: "TypeScript, changed" })).getByRole("button", { name: "Show before" }));
    expect(screen.getByText("Before: TypeScript 5.9 — Strict")).toBeInTheDocument();
  });
});

describe("the focus of the Specs", () => {
  const subject: FocusSubject = { kind: "task", id: "tttttttt", number: "T4", title: "Fix" };
  const scrolled: HTMLElement[] = [];

  beforeEach(() => {
    scrolled.length = 0;
    Element.prototype.scrollIntoView = function (this: HTMLElement) {
      scrolled.push(this);
    };
  });

  async function focused(refs: string[]) {
    const user = userEvent.setup();
    await show(<FocusOpener refs={refs} subject={subject} />);
    await user.click(screen.getByRole("button", { name: "open focus" }));
    await screen.findByRole("region", { name: "Focus" });
    return user;
  }

  it("tints the focused fields, scrolls to the step, and counts the fields it cannot find", async () => {
    await focused(["specs#notes", "specs#languages", "specs#cloud"]);
    const bar = screen.getByRole("region", { name: "Focus" });
    expect(within(bar).getByText("languages")).toBeInTheDocument();
    expect(within(bar).getByText("1 of 2")).toBeInTheDocument();
    expect(within(bar).getByText("1 not found")).toBeInTheDocument();
    expect(within(bar).queryByRole("checkbox", { name: "Only these" })).not.toBeInTheDocument();
    await waitFor(() => expect(new Set(scrolled.map((el) => el.dataset.stepKey))).toEqual(new Set(["languages"])));
    expect(screen.getByRole("region", { name: "Languages" })).toHaveClass("border-sky");
    const user = userEvent.setup();
    await user.click(within(bar).getByRole("button", { name: "Next" }));
    await waitFor(() => expect(scrolled[scrolled.length - 1]?.dataset.stepKey).toBe("notes"));
  });

  it("clears with Esc", async () => {
    const user = await focused(["specs#notes"]);
    within(screen.getByRole("region", { name: "Focus" })).getByRole("button", { name: "Clear" }).focus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("region", { name: "Focus" })).not.toBeInTheDocument();
  });
});
