import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState, type KeyboardEvent } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { expectOneGeometry, pressWhileComposing } from "@/components/assets/tag-box-checks";
import { TaggedTextarea, type TaggedTextareaProps } from "@/components/assets/tagged-textarea";
import { AssetsProvider } from "@/components/assets/use-assets";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({ call: vi.fn(), on: vi.fn() }));

const handlers = new Map<string, (payload: unknown) => void>();
let generation = 1;
let assets: { name: string; size: number }[] = [];

const asset = (name: string) => ({ name, size: 10 });

beforeEach(() => {
  vi.clearAllMocks();
  handlers.clear();
  generation = 1;
  assets = ["logo.png", "Logo-dark.svg", "my notes.txt", "spec.pdf", "notes.md", "banner.png"].map(asset);
  vi.mocked(on).mockImplementation((async (name: string, handler: (payload: unknown) => void) => {
    handlers.set(name, handler);
    return () => {};
  }) as unknown as typeof on);
  vi.mocked(call).mockImplementation((async (name: string) => (name === "assets_list" ? { generation, assets } : null)) as unknown as typeof call);
});

function Box({ onKey, initial = "", extra }: BoxProps) {
  const [text, setText] = useState(initial);
  return (
    <>
      <TaggedTextarea aria-label="Message" value={text} onChange={(event) => setText(event.target.value)} onKeyDown={onKey} {...extra} />
      <button type="button">Elsewhere</button>
    </>
  );
}

interface BoxProps {
  onKey?: (event: KeyboardEvent<HTMLTextAreaElement>) => void;
  initial?: string;
  /** Props the box passes on to the component. */
  extra?: Partial<TaggedTextareaProps>;
}

async function mount(props: BoxProps = {}) {
  const view = render(
    <AssetsProvider projectId="p1">
      <Box {...props} />
    </AssetsProvider>,
  );
  const box = screen.getByRole("combobox", { name: "Message" }) as HTMLTextAreaElement;
  await waitFor(() => expect(vi.mocked(call)).toHaveBeenCalledWith("assets_list", { projectId: "p1" }));
  await act(async () => {});
  return { ...view, box };
}

const options = () => screen.queryAllByRole("option").map((option) => option.textContent);
const tagged = (container: HTMLElement) => [...container.querySelectorAll("[data-tag]")].map((node) => node.textContent);

function changeAssets(next: string[]) {
  assets = next.map(asset);
  const handler = handlers.get("assets-changed");
  if (!handler) throw new Error("nobody listens to assets-changed");
  act(() => handler({ projectId: "p1", generation: ++generation, assets }));
}

describe("the list of suggestions", () => {
  it("opens when `@` is typed at the start, under the caret, with every asset in name order, and announces itself as a combobox", async () => {
    const user = userEvent.setup();
    const { box } = await mount();
    expect(box).toHaveAttribute("aria-expanded", "false");
    await user.click(box);
    await user.type(box, "@");
    expect(options()).toEqual(["banner.png", "Logo-dark.svg", "logo.png", "my notes.txt", "notes.md", "spec.pdf"]);
    expect(box).toHaveAttribute("aria-expanded", "true");
    const list = screen.getByRole("listbox");
    expect(box).toHaveAttribute("aria-controls", list.id);
    const active = screen.getAllByRole("option").find((option) => option.id === box.getAttribute("aria-activedescendant"));
    expect(active).toBe(screen.getAllByRole("option")[0]);
    expect(within(list).getAllByRole("option")[0]?.querySelector("svg")).not.toBeNull();
    // The focus stays in the box.
    expect(box).toHaveFocus();
  });

  it("opens after whitespace too, and never after a word, and never with no asset in the project", async () => {
    const user = userEvent.setup();
    const { box } = await mount();
    await user.click(box);
    await user.type(box, "mail me@");
    expect(options()).toEqual([]);
    await user.type(box, " and @");
    expect(options()).toHaveLength(6);
  });

  it("never opens with no asset", async () => {
    assets = [];
    const user = userEvent.setup();
    const { box } = await mount();
    await user.click(box);
    await user.type(box, "@l");
    expect(options()).toEqual([]);
    expect(box).toHaveAttribute("aria-expanded", "false");
  });

  it("filters while typing: the names that start with the text first, then the others that contain it, case ignored", async () => {
    const user = userEvent.setup();
    const { box } = await mount();
    await user.click(box);
    await user.type(box, "@no");
    expect(options()).toEqual(["notes.md", "my notes.txt"]);
    await user.clear(box);
    await user.type(box, "@LOGO");
    expect(options()).toEqual(["Logo-dark.svg", "logo.png"]);
    await user.type(box, "-d");
    expect(options()).toEqual(["Logo-dark.svg"]);
  });

  it("shows at most eight rows and scrolls the rest", async () => {
    assets = Array.from({ length: 12 }, (_, n) => asset(`file-${String(n).padStart(2, "0")}.txt`));
    const user = userEvent.setup();
    const { box } = await mount();
    await user.click(box);
    await user.type(box, "@");
    expect(options()).toHaveLength(12);
    const list = screen.getByRole("listbox");
    // 8 rows of 2 rem each.
    expect(list.className).toContain("max-h-64");
    expect(list.className).toContain("overflow-y-auto");
  });

  it("moves with Up and Down, and Enter inserts `@<name>` and a space in place of what was typed", async () => {
    const user = userEvent.setup();
    const onKey = vi.fn();
    const { box } = await mount({ onKey });
    await user.click(box);
    await user.type(box, "Look at @no");
    expect(options()).toEqual(["notes.md", "my notes.txt"]);
    await user.keyboard("{ArrowDown}");
    expect(screen.getAllByRole("option")[1]).toHaveAttribute("aria-selected", "true");
    expect(box).toHaveAttribute("aria-activedescendant", screen.getAllByRole("option")[1]?.id);
    await user.keyboard("{ArrowDown}");
    expect(screen.getAllByRole("option")[0]).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{ArrowUp}");
    expect(screen.getAllByRole("option")[1]).toHaveAttribute("aria-selected", "true");
    onKey.mockClear();
    await user.keyboard("{Enter}");
    expect(box.value).toBe("Look at @my notes.txt ");
    expect(box.selectionStart).toBe("Look at @my notes.txt ".length);
    expect(options()).toEqual([]);
    // The owner heard of no Enter, so nothing was sent.
    expect(onKey.mock.calls.filter(([event]) => event.key === "Enter")).toHaveLength(0);
    // Once the list is closed Enter reaches the owner again.
    await user.keyboard("{Enter}");
    expect(onKey.mock.calls.filter(([event]) => event.key === "Enter")).toHaveLength(1);
  });

  it("leaves the DOM caret after the inserted name, so that what is typed next lands there (checked on the box, not through user-event's own caret)", async () => {
    const user = userEvent.setup();
    const { box, container } = await mount();
    await user.click(box);
    await user.type(box, "Read @sp");
    await user.keyboard("{Enter}");
    expect(box.value).toBe("Read @spec.pdf ");
    expect(box.selectionStart).toBe(box.value.length);
    expect(box.selectionEnd).toBe(box.value.length);
    // What a browser does when the developer types: the text goes in at the DOM caret.
    const at = box.selectionStart;
    fireEvent.change(box, { target: { value: `${box.value.slice(0, at)}and @logo.png${box.value.slice(at)}` } });
    expect(box.value).toBe("Read @spec.pdf and @logo.png");
    expect(tagged(container)).toEqual(["@spec.pdf", "@logo.png"]);
  });

  it("inserts in the middle of the text and keeps what follows", async () => {
    const user = userEvent.setup();
    const { box } = await mount({ initial: "see  then" });
    await user.click(box);
    await user.type(box, "@sp", { initialSelectionStart: 5, initialSelectionEnd: 5 });
    expect(options()).toEqual(["spec.pdf"]);
    await user.keyboard("{Enter}");
    expect(box.value).toBe("see  @spec.pdf then");
    expect(box.selectionStart).toBe("see  @spec.pdf ".length);
  });

  it("Tab inserts too, and never moves the focus while the list is open", async () => {
    const user = userEvent.setup();
    const { box } = await mount();
    await user.click(box);
    await user.type(box, "@sp");
    await user.keyboard("{Tab}");
    expect(box.value).toBe("@spec.pdf ");
    expect(box).toHaveFocus();
    // With no list open Tab moves on as usual.
    await user.keyboard("{Tab}");
    expect(screen.getByRole("button", { name: "Elsewhere" })).toHaveFocus();
  });

  it("a click on a row inserts it", async () => {
    const user = userEvent.setup();
    const { box } = await mount();
    await user.click(box);
    await user.type(box, "@ban");
    await user.click(screen.getByRole("option", { name: /banner\.png/ }));
    expect(box.value).toBe("@banner.png ");
    expect(options()).toEqual([]);
    expect(box).toHaveFocus();
  });

  it("Esc closes the list and leaves the text as it is", async () => {
    const user = userEvent.setup();
    const onKey = vi.fn();
    const { box } = await mount({ onKey });
    await user.click(box);
    await user.type(box, "@lo");
    expect(options()).not.toEqual([]);
    onKey.mockClear();
    await user.keyboard("{Escape}");
    expect(options()).toEqual([]);
    expect(box.value).toBe("@lo");
    expect(onKey).not.toHaveBeenCalled();
    // Typing again opens it again.
    await user.type(box, "g");
    expect(options()).toEqual(["Logo-dark.svg", "logo.png"]);
  });

  it("closes by itself when nothing matches, when the caret leaves the tag and when the box loses the focus", async () => {
    const user = userEvent.setup();
    const { box } = await mount();
    await user.click(box);
    await user.type(box, "@zzz");
    expect(options()).toEqual([]);
    await user.clear(box);
    await user.type(box, "@lo");
    expect(options()).not.toEqual([]);
    await user.keyboard("{ArrowLeft}{ArrowLeft}{ArrowLeft}");
    expect(options()).toEqual([]);
    await user.keyboard("{End}");
    expect(options()).not.toEqual([]);
    await user.click(screen.getByRole("button", { name: "Elsewhere" }));
    expect(options()).toEqual([]);
    expect(box).toHaveAttribute("aria-expanded", "false");
  });

  it("follows the list of assets while it is open", async () => {
    const user = userEvent.setup();
    const { box } = await mount();
    await user.click(box);
    await user.type(box, "@sp");
    expect(options()).toEqual(["spec.pdf"]);
    changeAssets(["spec.pdf", "spec2.pdf"]);
    expect(options()).toEqual(["spec.pdf", "spec2.pdf"]);
    changeAssets(["other.txt"]);
    expect(options()).toEqual([]);
  });
});

describe("the highlight", () => {
  it("draws every recognised tag, `@` included, and leaves a tag that names no asset in the normal colour", async () => {
    const user = userEvent.setup();
    const { box, container } = await mount();
    await user.click(box);
    await user.type(box, "Use @logo.png, @my notes.txt and @missing.png. Not me@spec.pdf");
    expect(tagged(container)).toEqual(["@logo.png", "@my notes.txt"]);
    const mirror = screen.getByTestId("tag-mirror");
    expect(mirror).toHaveAttribute("aria-hidden", "true");
    expect(mirror.textContent).toContain("Use @logo.png, @my notes.txt and @missing.png. Not me@spec.pdf");
    const mark = container.querySelector("[data-tag='logo.png']");
    expect(mark?.className).toContain("text-sky-ink");
    // The textarea holds the text, transparent, with its caret visible.
    expect(box.className).toContain("text-transparent");
    expect(box.className).toContain("caret-foreground");
  });

  it("is case sensitive and follows every change of the text", async () => {
    const user = userEvent.setup();
    const { box, container } = await mount();
    await user.click(box);
    await user.type(box, "@Logo.png");
    expect(tagged(container)).toEqual([]);
    await user.clear(box);
    await user.type(box, "@logo.pn");
    expect(tagged(container)).toEqual([]);
    await user.type(box, "g");
    expect(tagged(container)).toEqual(["@logo.png"]);
    await user.type(box, "x");
    expect(tagged(container)).toEqual([]);
  });

  it("follows every change of the assets", async () => {
    const user = userEvent.setup();
    const { box, container } = await mount({ initial: "see @new.png and @logo.png" });
    expect(tagged(container)).toEqual(["@logo.png"]);
    changeAssets(["logo.png", "new.png"]);
    expect(tagged(container)).toEqual(["@new.png", "@logo.png"]);
    changeAssets(["new.png"]);
    expect(tagged(container)).toEqual(["@new.png"]);
    await user.click(box);
    expect(box.value).toBe("see @new.png and @logo.png");
  });

  it("highlights the text at once when it is set from outside, and keeps the box a plain textarea otherwise", async () => {
    const { box, container } = await mount({ initial: "@spec.pdf" });
    expect(tagged(container)).toEqual(["@spec.pdf"]);
    expect(box.tagName).toBe("TEXTAREA");
    expect(box.value).toBe("@spec.pdf");
  });
});

describe("one geometry for both layers", () => {
  it("gives the textarea and the mirror the same metrics from one place, the font included", async () => {
    const { box } = await mount({ extra: { metricsClassName: "font-mono text-xs px-4", className: "h-full", areaClassName: "resize-none border-transparent" } });
    expectOneGeometry(box, { base: ["border", "py-2", "whitespace-pre-wrap", "break-words"], has: ["font-mono", "text-xs", "px-4"], lacks: ["text-sm", "px-3"] });
  });

  it("refuses a metric given to one layer only, in a class or in a style", () => {
    const quiet = vi.spyOn(console, "error").mockImplementation(() => {});
    const draw = (props: Partial<TaggedTextareaProps>) =>
      render(
        <AssetsProvider projectId="p1">
          <TaggedTextarea aria-label="Message" value="" onChange={() => {}} {...props} />
        </AssetsProvider>,
      );
    for (const props of [
      { areaClassName: "px-0" },
      { areaClassName: "md:py-1" },
      { areaClassName: "border-2" },
      { areaClassName: "font-mono" },
      { areaClassName: "text-base" },
      { areaClassName: "leading-6" },
      { areaClassName: "tracking-wide" },
      { areaClassName: "whitespace-nowrap" },
      { areaClassName: "[font-family:inherit]" },
      { className: "px-2" },
      { className: "font-mono" },
      { style: { letterSpacing: "1px" } },
      { style: { paddingLeft: 0 } },
    ] satisfies Partial<TaggedTextareaProps>[]) {
      expect(() => draw(props), JSON.stringify(props)).toThrow(/metric of the box/);
    }
    quiet.mockRestore();
  });

  it("lets through what moves no character: background, border colour, focus ring, resize, height", async () => {
    const { box } = await mount({
      extra: {
        className: "max-h-60 min-h-9 bg-transparent",
        areaClassName: "resize-none border-transparent bg-transparent shadow-none focus-visible:ring-0 focus-visible:ring-offset-0",
        style: { height: "40px" },
      },
    });
    expect(box).toHaveClass("resize-none", "border-transparent");
    expect(box.style.height).toBe("40px");
    expectOneGeometry(box);
  });

  it("measures the caret with a marker that has no width and no place to break a line", async () => {
    const user = userEvent.setup();
    const { box } = await mount({ initial: "a long-unbroken-word" });
    await user.click(box);
    await user.type(box, " @sp");
    expect(options()).toEqual(["spec.pdf"]);
    const mirror = screen.getByTestId("tag-mirror");
    const marker = within(mirror).getByTestId("tag-caret");
    expect(marker.tagName).toBe("SPAN");
    expect(marker.textContent).toBe("");
    expect(marker.className).toBe("");
    expect(marker.getAttribute("style")).toBeNull();
    // The text around it is the text of the box, cut in pieces and nothing more.
    expect(mirror.textContent).toBe("a long-unbroken-word @sp\u200b");
  });
});

describe("a composition of the input method", () => {
  it("is left alone by Enter and Tab: no send, no suggestion picked, no focus moved, with the list open or closed", async () => {
    const user = userEvent.setup();
    const onKey = vi.fn();
    const { box } = await mount({ onKey });
    await user.click(box);
    await user.type(box, "see @sp");
    expect(options()).toEqual(["spec.pdf"]);
    onKey.mockClear();
    for (const key of ["Enter", "Tab"] as const) {
      pressWhileComposing(box, key);
      expect(options()).toEqual(["spec.pdf"]);
      expect(box.value).toBe("see @sp");
      expect(box).toHaveFocus();
    }
    expect(onKey).not.toHaveBeenCalled();

    // The same key once the composition is over chooses the suggestion.
    await user.keyboard("{Tab}");
    expect(box.value).toBe("see @spec.pdf ");

    pressWhileComposing(box, "Enter");
    pressWhileComposing(box, "Tab");
    expect(onKey).not.toHaveBeenCalled();
    expect(box).toHaveFocus();
  });

  it("is also left alone when the browser reports it only by the key code 229", async () => {
    const onKey = vi.fn();
    const { box } = await mount({ onKey });
    expect(fireEvent.keyDown(box, { key: "Enter", keyCode: 229 })).toBe(true);
    expect(onKey).not.toHaveBeenCalled();
  });
});
