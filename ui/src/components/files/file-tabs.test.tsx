import { undo } from "@codemirror/commands";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import { FileTabs } from "@/components/files/file-tabs";
import { FileTabsProvider, useFileTabs, type FileTabsValue } from "@/components/files/file-tabs-context";
import { editorText, editorView, installEditorShims, typeAtEnd } from "@/components/files/test-support";
import { ProjectContext } from "@/components/project-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const toasts = vi.hoisted(() => ({ notice: vi.fn(), warning: vi.fn(), error: vi.fn() }));
vi.mock("sonner", () => ({ toast: Object.assign(toasts.notice, { warning: toasts.warning, error: toasts.error }) }));

const PROJECT = "0123456789abcdef";

interface DiskFile {
  text: string;
  binary?: boolean;
  /** A binary file that is an image Weaver can show. */
  image?: { dataUrl: string; size: number };
  /** A binary file with an image signature over 10 MB. */
  imageTooLarge?: boolean;
  readOnly?: "tooLarge" | "weaver";
  locked?: boolean;
  /** The file on disk changed since the tab read it (a save finds it). */
  stale?: boolean;
}

/** What `file_open` and `file_reload` tell of a file. */
function contentOf(file: DiskFile) {
  const kind = file.image ? "image" : file.binary ? "binary" : "text";
  return {
    kind,
    text: kind === "text" ? file.text : null,
    hash: "h",
    readOnly: file.readOnly ?? null,
    image: file.image ?? null,
    imageTooLarge: file.imageTooLarge ?? false,
  };
}

let disk = new Map<string, DiskFile>();
let handlers = new Map<string, (payload: unknown) => void>();
let generation = 0;
/** Rejects the next write with this reason. */
let failWrite: string | null = null;
let api: FileTabsValue;

function Capture() {
  api = useFileTabs();
  return null;
}

function mount() {
  return render(
    <TooltipProvider>
      <ProjectContext.Provider value={{ projectId: PROJECT, showView: vi.fn(), openJob: vi.fn(), openedJob: null, openedJobRequest: 0 }}>
        <FileTabsProvider>
          <Capture />
          <FileTabs />
        </FileTabsProvider>
      </ProjectContext.Provider>
    </TooltipProvider>,
  );
}

function install() {
  handlers = new Map();
  generation = 0;
  failWrite = null;
  vi.mocked(on).mockImplementation((async (name: string, handler: (payload: unknown) => void) => {
    handlers.set(name, handler);
    return () => {};
  }) as unknown as typeof on);
  vi.mocked(call).mockImplementation((async (name: string, args: Record<string, unknown>) => {
    const path = args.path as string;
    switch (name) {
      case "file_open": {
        const file = disk.get(path);
        if (!file) throw { code: "refused", message: "The file does not exist." };
        const tabs = args.tabs as { path: string; unsaved: boolean }[];
        let closed: string | null = null;
        if (tabs.length >= 10) {
          const victim = tabs.find((tab) => !tab.unsaved);
          if (!victim) return { status: "full" };
          closed = victim.path;
        }
        return { status: "opened", path, ...contentOf(file), locked: file.locked ?? false, closed };
      }
      case "file_reload": {
        const file = disk.get(path);
        if (!file) return { status: "gone" };
        return { status: "read", ...contentOf(file) };
      }
      case "file_save": {
        const file = disk.get(path);
        if (failWrite) return { status: "failed", reason: failWrite };
        if (args.mode === "check" && !file) return { status: "gone" };
        if (args.mode === "check" && file?.stale) return { status: "changed" };
        if (args.mode === "again" && file) return { status: "changed" };
        disk.set(path, { ...(file ?? {}), text: args.text as string, stale: false });
        return { status: "saved", hash: "h2" };
      }
      default:
        return null;
    }
  }) as unknown as typeof call);
}

function sent(name: string) {
  return vi.mocked(call).mock.calls.filter(([command]) => command === name).map(([, args]) => args as Record<string, unknown>);
}

function emit(name: string, payload: Record<string, unknown>) {
  const handler = handlers.get(name);
  if (!handler) throw new Error(`nobody listens to ${name}`);
  act(() => handler({ projectId: PROJECT, ...payload }));
}

function emitDisk(name: "file-tab-changed" | "file-lock-changed", payload: Record<string, unknown>) {
  emit(name, { generation: ++generation, ...payload });
}

async function open(path: string) {
  act(() => api.openFile(path));
  await waitFor(() => expect(api.tabs.some((tab) => tab.path === path)).toBe(true));
}

function tabOf(name: string) {
  return screen.getByRole("tab", { name: new RegExp(`^${name.replace(".", "\\.")}`) });
}

beforeAll(installEditorShims);

beforeEach(() => {
  vi.clearAllMocks();
  disk = new Map([
    ["src/main.rs", { text: "fn main() {}\n" }],
    ["notes.txt", { text: "hello\n" }],
    ["logo.png", { text: "", binary: true }],
    ["big.log", { text: "x\n", readOnly: "tooLarge" }],
    [".weaver/model/system.md", { text: "# System\n", readOnly: "weaver" }],
  ]);
  install();
});

afterEach(() => vi.useRealTimers());

describe("opening and closing tabs", () => {
  it("says where to start without tabs, and opens a file in a tab with its path, its text and its colours", async () => {
    const { container } = mount();
    expect(screen.getByText("Open a file from the tree.")).toBeInTheDocument();
    await open("src/main.rs");
    const tab = tabOf("main.rs");
    expect(tab).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tablist", { name: "Open files" })).toContainElement(tab);
    expect(screen.getByRole("tabpanel")).toBeInTheDocument();
    expect(editorText(container)).toBe("fn main() {}\n");
    expect(screen.getByText("src/main.rs", { selector: "span.font-mono" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open in editor" })).toBeInTheDocument();
    await waitFor(() => expect(container.querySelector("[data-language]")).toHaveAttribute("data-language", "Rust"));
    expect(screen.queryByText("Open a file from the tree.")).toBeNull();
  });

  it("shows the tab of a file that is already open and opens no second one", async () => {
    mount();
    await open("notes.txt");
    await open("src/main.rs");
    act(() => api.openFile("notes.txt"));
    expect(tabOf("notes.txt")).toHaveAttribute("aria-selected", "true");
    expect(screen.getAllByRole("tab")).toHaveLength(2);
    expect(sent("file_open")).toHaveLength(2);
  });

  it("follows what the backend says when the same file is opened by another path", async () => {
    mount();
    await open("notes.txt");
    await open("src/main.rs");
    vi.mocked(call).mockImplementationOnce((async () => ({ status: "alreadyOpen", path: "notes.txt" })) as unknown as typeof call);
    act(() => api.openFile("./notes.txt"));
    await waitFor(() => expect(tabOf("notes.txt")).toHaveAttribute("aria-selected", "true"));
    expect(screen.getAllByRole("tab")).toHaveLength(2);
  });

  it("sends the tabs, least recently shown first, with the unsaved ones, and drops the tab the limit closed", async () => {
    mount();
    await open("notes.txt");
    await open("src/main.rs");
    act(() => api.showTab("notes.txt"));
    disk.set("c.txt", { text: "c" });
    await open("c.txt");
    expect(sent("file_open")[2]?.tabs).toEqual([
      { path: "src/main.rs", unsaved: false },
      { path: "notes.txt", unsaved: false },
    ]);
    // The backend closed a tab to make room for this one.
    vi.mocked(call).mockImplementationOnce((async (_name: string, args: Record<string, unknown>) => ({
      status: "opened", path: args.path, kind: "text", text: "d", hash: "h", readOnly: null, locked: false, closed: "src/main.rs",
    })) as unknown as typeof call);
    await open("d.txt");
    expect(screen.queryByRole("tab", { name: /^main\.rs/ })).toBeNull();
    expect(screen.getAllByRole("tab")).toHaveLength(3);
  });

  it("warns, and opens nothing, when every tab has unsaved changes", async () => {
    mount();
    vi.mocked(call).mockImplementationOnce((async () => ({ status: "full" })) as unknown as typeof call);
    act(() => api.openFile("notes.txt"));
    await waitFor(() => expect(toasts.warning).toHaveBeenCalledWith("Every tab has unsaved changes: save or close one to open another file."));
    expect(screen.queryByRole("tab")).toBeNull();
  });

  it("shows the reason in an error toast when a file cannot be opened", async () => {
    mount();
    act(() => api.openFile("missing.txt"));
    await waitFor(() => expect(toasts.error).toHaveBeenCalledWith("Could not open missing.txt.", { description: "The file does not exist." }));
  });

  it("shows a binary file as a panel with `Open in editor` and no text", async () => {
    const { container } = mount();
    await open("logo.png");
    expect(screen.getByText("Binary file")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open in editor" })).toBeInTheDocument();
    expect(container.querySelector(".cm-content")?.textContent ?? "").toBe("");
    emitDisk("file-tab-changed", { path: "logo.png", change: "deleted" });
    expect(screen.getByText("This file was deleted on disk.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Save again" })).toBeNull();
    emitDisk("file-tab-changed", { path: "logo.png", change: "changed" });
    expect(screen.queryByText("This file was deleted on disk.")).toBeNull();
  });

  it("says why a large file and a file of .weaver are read-only, and the editor cannot change them", async () => {
    const { container } = mount();
    await open("big.log");
    expect(screen.getByText("This file is larger than 2 MB: read-only, without colours.")).toBeInTheDocument();
    expect(editorView(container).state.readOnly).toBe(true);
    expect(container.querySelector("[data-language]")).toBeNull();
    await open(".weaver/model/system.md");
    expect(screen.getByText("Files in .weaver are written by Weaver: read-only here.")).toBeInTheDocument();
    expect(editorView(container).state.readOnly).toBe(true);
  });

  it("opens a file the agent is already changing as Locked: the text cannot be changed", async () => {
    const { container } = mount();
    disk.set("src/lib.rs", { text: "x", locked: true });
    await open("src/lib.rs");
    expect(screen.getByText("The agent is editing this file.")).toBeInTheDocument();
    expect(within(tabOf("lib.rs")).getByRole("img", { name: "The agent is editing this file" })).toBeInTheDocument();
    expect(editorView(container).state.readOnly).toBe(true);
    expect(api.lockedPaths.has("src/lib.rs")).toBe(true);
  });

  it("closes a tab without unsaved changes with its `Close` button, and tells the backend", async () => {
    const user = userEvent.setup();
    mount();
    await open("notes.txt");
    await open("src/main.rs");
    await user.click(within(tabOf("main.rs").parentElement as HTMLElement).getByRole("button", { name: "Close" }));
    expect(screen.queryByRole("tab", { name: /^main\.rs/ })).toBeNull();
    expect(tabOf("notes.txt")).toHaveAttribute("aria-selected", "true");
    expect(sent("file_close")).toEqual([{ projectId: PROJECT, path: "src/main.rs" }]);
  });

  it("closing a tab with unsaved changes asks `Save changes to <name>?`: Cancel keeps it, Discard loses the changes, Save writes them first", async () => {
    const user = userEvent.setup();
    const { container } = mount();
    await open("notes.txt");
    typeAtEnd(container, "more");
    const close = () => within(tabOf("notes.txt").parentElement as HTMLElement).getByRole("button", { name: "Close" });

    await user.click(close());
    const dialog = await screen.findByRole("alertdialog", { name: "Save changes to notes.txt?" });
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    expect(within(dialog).getByRole("button", { name: "Discard" })).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(tabOf("notes.txt")).toBeInTheDocument();
    expect(sent("file_save")).toHaveLength(0);

    await user.click(close());
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(screen.queryByRole("tab")).toBeNull());
    expect(sent("file_save")).toEqual([{ projectId: PROJECT, path: "notes.txt", text: "hello\nmore", hash: "h", mode: "check" }]);

    await open("notes.txt");
    typeAtEnd(container, "again");
    await user.click(close());
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Discard" }));
    await waitFor(() => expect(screen.queryByRole("tab")).toBeNull());
    expect(sent("file_save")).toHaveLength(1);
  });

  it("a refused save leaves the tab open when closing", async () => {
    const user = userEvent.setup();
    const { container } = mount();
    await open("notes.txt");
    typeAtEnd(container, "x");
    failWrite = "disk full";
    await user.click(within(tabOf("notes.txt").parentElement as HTMLElement).getByRole("button", { name: "Close" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Save" }));
    await waitFor(() => expect(toasts.error).toHaveBeenCalledWith("Could not save notes.txt.", { description: "disk full" }));
    expect(tabOf("notes.txt")).toBeInTheDocument();
    expect(within(tabOf("notes.txt")).getByRole("img", { name: "Unsaved changes" })).toBeInTheDocument();
    expect(editorText(container)).toBe("hello\nx");
  });
});

describe("editing and saving", () => {
  it("marks a tab with unsaved changes with a dot named `Unsaved changes` and tells the backend which files", async () => {
    const { container } = mount();
    await open("notes.txt");
    await open("src/main.rs");
    await waitFor(() => expect(sent("files_unsaved").at(-1)).toEqual({ projectId: PROJECT, paths: [] }));
    typeAtEnd(container, "// more");
    expect(within(tabOf("main.rs")).getByRole("img", { name: "Unsaved changes" })).toBeInTheDocument();
    expect(within(tabOf("notes.txt")).queryByRole("img", { name: "Unsaved changes" })).toBeNull();
    await waitFor(() => expect(sent("files_unsaved").at(-1)).toEqual({ projectId: PROJECT, paths: ["src/main.rs"] }));
    expect(api.unsavedPaths).toEqual(["src/main.rs"]);
    expect(api.unsavedUnder("src")).toEqual(["src/main.rs"]);
    expect(api.unsavedUnder("docs")).toEqual([]);
  });

  it("keeps the text and the undo history of each tab when another one is shown", async () => {
    const { container } = mount();
    await open("notes.txt");
    typeAtEnd(container, "A");
    await open("src/main.rs");
    expect(editorText(container)).toBe("fn main() {}\n");
    act(() => api.showTab("notes.txt"));
    await waitFor(() => expect(editorText(container)).toBe("hello\nA"));
  });

  for (const [name, init] of [
    ["Ctrl+S", { ctrlKey: true }],
    ["Cmd+S", { metaKey: true }],
  ] as const) {
    it(`${name} saves the active tab with the text of its editor`, async () => {
      const { container } = mount();
      await open("notes.txt");
      typeAtEnd(container, "more");
      fireEvent.keyDown(window, { key: "s", ...init });
      await waitFor(() => expect(within(tabOf("notes.txt")).queryByRole("img", { name: "Unsaved changes" })).toBeNull());
      expect(sent("file_save")).toEqual([{ projectId: PROJECT, path: "notes.txt", text: "hello\nmore", hash: "h", mode: "check" }]);
      expect(disk.get("notes.txt")?.text).toBe("hello\nmore");
    });
  }

  it("saving a tab with nothing to save, or one that cannot be changed, does nothing", async () => {
    mount();
    await open("notes.txt");
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    await open("big.log");
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    expect(sent("file_save")).toHaveLength(0);
  });

  it("each save carries the hash the tab read or wrote last", async () => {
    const { container } = mount();
    await open("notes.txt");
    typeAtEnd(container, "1");
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    await waitFor(() => expect(sent("file_save")).toHaveLength(1));
    await waitFor(() => expect(within(tabOf("notes.txt")).queryByRole("img", { name: "Unsaved changes" })).toBeNull());
    typeAtEnd(container, "2");
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    await waitFor(() => expect(sent("file_save")).toHaveLength(2));
    expect(sent("file_save").map((args) => args.hash)).toEqual(["h", "h2"]);
  });

  it("a text written while the save runs stays unsaved", async () => {
    const { container } = mount();
    await open("notes.txt");
    typeAtEnd(container, "1");
    vi.mocked(call).mockImplementationOnce((async () => {
      typeAtEnd(container, "2");
      return { status: "saved", hash: "h2" };
    }) as unknown as typeof call);
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    await waitFor(() => expect(sent("files_unsaved").length).toBeGreaterThan(1));
    expect(within(tabOf("notes.txt")).getByRole("img", { name: "Unsaved changes" })).toBeInTheDocument();
  });

  it("a save that finds the file changed asks what to keep: Cancel changes nothing, Overwrite writes anyway, Reload from disk takes the file", async () => {
    const user = userEvent.setup();
    const { container } = mount();
    await open("notes.txt");
    typeAtEnd(container, "mine");
    disk.set("notes.txt", { text: "theirs\n", stale: true });
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    const dialog = await screen.findByRole("alertdialog", { name: "This file changed on disk since it was opened." });
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(editorText(container)).toBe("hello\nmine");
    expect(sent("file_save")).toHaveLength(1);

    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Overwrite" }));
    await waitFor(() => expect(sent("file_save").at(-1)?.mode).toBe("overwrite"));
    await waitFor(() => expect(within(tabOf("notes.txt")).queryByRole("img", { name: "Unsaved changes" })).toBeNull());

    typeAtEnd(container, "!");
    disk.set("notes.txt", { text: "theirs\n", stale: true });
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Reload from disk" }));
    await waitFor(() => expect(editorText(container)).toBe("theirs\n"));
    expect(within(tabOf("notes.txt")).queryByRole("img", { name: "Unsaved changes" })).toBeNull();
  });

  it("a failed write shows an error toast with the reason and keeps the text", async () => {
    const { container } = mount();
    await open("notes.txt");
    typeAtEnd(container, "x");
    failWrite = "permission denied";
    fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    await waitFor(() => expect(toasts.error).toHaveBeenCalledWith("Could not save notes.txt.", { description: "permission denied" }));
    expect(within(tabOf("notes.txt")).getByRole("img", { name: "Unsaved changes" })).toBeInTheDocument();
    expect(editorText(container)).toBe("hello\nx");
  });
});

describe("what the disk and the agent do", () => {
  it("reads a Saved tab again when its file changes on disk", async () => {
    const { container } = mount();
    await open("notes.txt");
    disk.set("notes.txt", { text: "changed elsewhere\n" });
    emitDisk("file-tab-changed", { path: "notes.txt", change: "changed" });
    await waitFor(() => expect(editorText(container)).toBe("changed elsewhere\n"));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("a Modified tab keeps its text and shows a bar when the file changes on disk", async () => {
    const { container } = mount();
    await open("notes.txt");
    typeAtEnd(container, "mine");
    emitDisk("file-tab-changed", { path: "notes.txt", change: "changed" });
    expect(screen.getByRole("alert")).toHaveTextContent("This file changed on disk. Saving will ask what to keep.");
    expect(within(tabOf("notes.txt")).getByRole("img", { name: "Needs your attention" })).toBeInTheDocument();
    expect(editorText(container)).toBe("hello\nmine");
    expect(sent("file_reload")).toHaveLength(0);
  });

  it("ignores events of an older generation", async () => {
    const { container } = mount();
    await open("notes.txt");
    disk.set("notes.txt", { text: "newer\n" });
    emit("file-tab-changed", { generation: 5, path: "notes.txt", change: "changed" });
    await waitFor(() => expect(editorText(container)).toBe("newer\n"));
    disk.set("notes.txt", { text: "stale\n" });
    emit("file-tab-changed", { generation: 4, path: "notes.txt", change: "changed" });
    expect(sent("file_reload")).toHaveLength(1);
  });

  it("a deleted file keeps its tab and text, says so, and `Save again` writes it again", async () => {
    const user = userEvent.setup();
    const { container } = mount();
    await open("notes.txt");
    typeAtEnd(container, "kept");
    disk.delete("notes.txt");
    emitDisk("file-tab-changed", { path: "notes.txt", change: "deleted" });
    expect(screen.getByRole("alert")).toHaveTextContent("This file was deleted on disk.");
    expect(editorText(container)).toBe("hello\nkept");
    await user.click(screen.getByRole("button", { name: "Save again" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(sent("file_save").at(-1)).toMatchObject({ mode: "again", text: "hello\nkept" });
    expect(disk.get("notes.txt")?.text).toBe("hello\nkept");
  });

  it("a deleted file that appears again goes back to Saved, or to Modified with the bar of the change", async () => {
    const { container } = mount();
    await open("notes.txt");
    emitDisk("file-tab-changed", { path: "notes.txt", change: "deleted" });
    expect(screen.getByRole("alert")).toBeInTheDocument();
    emitDisk("file-tab-changed", { path: "notes.txt", change: "changed" });
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());

    typeAtEnd(container, "m");
    emitDisk("file-tab-changed", { path: "notes.txt", change: "deleted" });
    emitDisk("file-tab-changed", { path: "notes.txt", change: "changed" });
    expect(screen.getByRole("alert")).toHaveTextContent("This file changed on disk. Saving will ask what to keep.");
    expect(screen.queryByRole("button", { name: "Save again" })).toBeNull();
  });

  it("Locked keeps the unsaved changes; the release with changes offers the agent's version, mine, or neither", async () => {
    const user = userEvent.setup();
    const { container } = mount();
    await open("notes.txt");
    typeAtEnd(container, "mine");
    emitDisk("file-lock-changed", { path: "notes.txt", locked: true, movedTo: null });
    expect(screen.getByText("The agent is editing this file.")).toBeInTheDocument();
    expect(editorView(container).state.readOnly).toBe(true);
    expect(within(tabOf("notes.txt")).getByRole("img", { name: "Unsaved changes" })).toBeInTheDocument();
    expect(within(tabOf("notes.txt")).getByRole("img", { name: "The agent is editing this file" })).toBeInTheDocument();

    emitDisk("file-lock-changed", { path: "notes.txt", locked: false, movedTo: null });
    expect(screen.getByRole("alert")).toHaveTextContent("The agent changed this file while it had unsaved changes.");
    expect(editorView(container).state.readOnly).toBe(false);
    expect(editorText(container)).toBe("hello\nmine");
    expect(screen.getByRole("button", { name: "Take the agent's version" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("alert")).toBeNull();
    expect(editorText(container)).toBe("hello\nmine");

    emitDisk("file-lock-changed", { path: "notes.txt", locked: true, movedTo: null });
    emitDisk("file-lock-changed", { path: "notes.txt", locked: false, movedTo: null });
    await user.click(screen.getByRole("button", { name: "Keep mine" }));
    await waitFor(() => expect(sent("file_save").at(-1)).toMatchObject({ mode: "overwrite", text: "hello\nmine" }));
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());

    typeAtEnd(container, "2");
    emitDisk("file-lock-changed", { path: "notes.txt", locked: true, movedTo: null });
    emitDisk("file-lock-changed", { path: "notes.txt", locked: false, movedTo: null });
    disk.set("notes.txt", { text: "agent\n" });
    await user.click(screen.getByRole("button", { name: "Take the agent's version" }));
    await waitFor(() => expect(editorText(container)).toBe("agent\n"));
    expect(within(tabOf("notes.txt")).queryByRole("img", { name: "Unsaved changes" })).toBeNull();
  });

  it("the release of a Saved tab reads the file again, at the new path when the agent moved it", async () => {
    const { container } = mount();
    await open("notes.txt");
    emitDisk("file-lock-changed", { path: "notes.txt", locked: true, movedTo: null });
    disk.set("docs/notes.txt", { text: "moved\n" });
    emitDisk("file-lock-changed", { path: "notes.txt", locked: false, movedTo: "docs/notes.txt" });
    await waitFor(() => expect(editorText(container)).toBe("moved\n"));
    expect(sent("file_reload").at(-1)).toEqual({ projectId: PROJECT, path: "docs/notes.txt" });
    expect(api.tabs[0]?.path).toBe("docs/notes.txt");
    expect(screen.queryByText("The agent is editing this file.")).toBeNull();
  });

  it("says that the agent deleted a file it had locked", async () => {
    mount();
    await open("notes.txt");
    emitDisk("file-lock-changed", { path: "notes.txt", locked: true, movedTo: null });
    disk.delete("notes.txt");
    emitDisk("file-lock-changed", { path: "notes.txt", locked: false, movedTo: null });
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("The agent deleted this file."));
    expect(screen.getByRole("button", { name: "Save again" })).toBeInTheDocument();
  });
  it("tells that the agent is editing the file in a polite live region that exists before the line, for Locked and Read-only tabs", async () => {
    mount();
    await open("notes.txt");
    const region = document.querySelector('[aria-live="polite"]') as HTMLElement;
    expect(region).not.toBeNull();
    expect(region).toBeEmptyDOMElement();
    emitDisk("file-lock-changed", { path: "notes.txt", locked: true, movedTo: null });
    expect(region).toHaveTextContent("The agent is editing this file.");
    emitDisk("file-lock-changed", { path: "notes.txt", locked: false, movedTo: null });
    await waitFor(() => expect(region).toBeEmptyDOMElement());

    await open("big.log");
    const readOnlyRegion = document.querySelector('[aria-live="polite"]') as HTMLElement;
    emitDisk("file-lock-changed", { path: "big.log", locked: true, movedTo: null });
    expect(readOnlyRegion).toHaveTextContent("The agent is editing this file.");
    expect(screen.getByText("This file is larger than 2 MB: read-only, without colours.")).toBeInTheDocument();
    emitDisk("file-lock-changed", { path: "big.log", locked: false, movedTo: null });
    await waitFor(() => expect(readOnlyRegion).toBeEmptyDOMElement());
  });
});

describe("shortcuts and the tab bar", () => {
  it("Left and Right move between the tabs, with Home and End", async () => {
    const user = userEvent.setup();
    mount();
    await open("notes.txt");
    await open("src/main.rs");
    await open("big.log");
    tabOf("big.log").focus();
    await user.keyboard("{ArrowLeft}");
    expect(tabOf("main.rs")).toHaveAttribute("aria-selected", "true");
    expect(tabOf("main.rs")).toHaveFocus();
    await user.keyboard("{ArrowRight}{ArrowRight}");
    expect(tabOf("notes.txt")).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{End}");
    expect(tabOf("big.log")).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{Home}");
    expect(tabOf("notes.txt")).toHaveAttribute("aria-selected", "true");
    expect(tabOf("notes.txt")).toHaveAttribute("tabindex", "0");
    expect(tabOf("big.log")).toHaveAttribute("tabindex", "-1");
  });

  it("Ctrl+Tab and Ctrl+Shift+Tab show the next and the previous tab", async () => {
    mount();
    await open("notes.txt");
    await open("src/main.rs");
    await open("big.log");
    fireEvent.keyDown(window, { key: "Tab", ctrlKey: true });
    expect(tabOf("notes.txt")).toHaveAttribute("aria-selected", "true");
    fireEvent.keyDown(window, { key: "Tab", ctrlKey: true, shiftKey: true });
    expect(tabOf("big.log")).toHaveAttribute("aria-selected", "true");
  });

  for (const [name, init] of [
    ["Ctrl+W", { ctrlKey: true }],
    ["Cmd+W", { metaKey: true }],
  ] as const) {
    it(`${name} closes the active tab`, async () => {
      mount();
      await open("notes.txt");
      await open("src/main.rs");
      fireEvent.keyDown(window, { key: "w", ...init });
      expect(screen.queryByRole("tab", { name: /^main\.rs/ })).toBeNull();
      expect(tabOf("notes.txt")).toHaveAttribute("aria-selected", "true");
    });
  }

  it("the menu's Close Window closes the active tab, and with unsaved changes asks first", async () => {
    const { container } = mount();
    await open("notes.txt");
    typeAtEnd(container, "x");
    emit("files-close-tab", {});
    expect(await screen.findByRole("alertdialog", { name: "Save changes to notes.txt?" })).toBeInTheDocument();
  });

  it("tells the backend that Close Window closes a tab while there is one", async () => {
    mount();
    await open("notes.txt");
    await waitFor(() => expect(sent("files_close_key").at(-1)).toEqual({ projectId: PROJECT, claimed: true }));
    act(() => api.closeAll());
    await waitFor(() => expect(sent("files_close_key").at(-1)).toEqual({ projectId: PROJECT, claimed: false }));
  });

  it("gives every tab a tooltip with its path in monospace", async () => {
    const user = userEvent.setup();
    mount();
    await open("src/main.rs");
    await user.hover(tabOf("main.rs"));
    expect(await screen.findByRole("tooltip")).toHaveTextContent("src/main.rs");
  });
});

describe("Save all, and what the tree asks", () => {
  it("answers the flush of a closing window with the texts of the tabs with unsaved changes", async () => {
    const { container } = mount();
    await open("notes.txt");
    await open("src/main.rs");
    typeAtEnd(container, "// x");
    emit("files-flush", {});
    await waitFor(() => expect(sent("files_flushed")).toEqual([{ projectId: PROJECT, tabs: [{ path: "src/main.rs", text: "fn main() {}\n// x" }] }]));
  });

  it("shows the tab of a file the window could not save, with the reason, and marks the others saved", async () => {
    const { container } = mount();
    await open("notes.txt");
    typeAtEnd(container, "1");
    await open("src/main.rs");
    typeAtEnd(container, "2");
    emit("files-flush", {});
    await waitFor(() => expect(sent("files_flushed")).toHaveLength(1));
    act(() => api.showTab("src/main.rs"));
    emit("files-save-failed", {
      saved: [{ path: "src/main.rs", hash: "h3" }],
      failures: [{ path: "notes.txt", reason: "the file changed on disk since it was opened" }],
    });
    expect(tabOf("notes.txt")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("alert")).toHaveTextContent("Could not save notes.txt: the file changed on disk since it was opened.");
    expect(within(tabOf("main.rs")).queryByRole("img", { name: "Unsaved changes" })).toBeNull();
    expect(within(tabOf("notes.txt")).getByRole("img", { name: "Unsaved changes" })).toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("saveAll saves every tab with unsaved changes and returns the files it could not save", async () => {
    const { container } = mount();
    await open("notes.txt");
    typeAtEnd(container, "1");
    await open("src/main.rs");
    typeAtEnd(container, "2");
    disk.set("src/main.rs", { text: "other\n", stale: true });
    let failures: { path: string; reason: string }[] = [];
    await act(async () => {
      failures = await api.saveAll();
    });
    expect(failures).toEqual([{ path: "src/main.rs", reason: "the file changed on disk since it was opened" }]);
    expect(disk.get("notes.txt")?.text).toBe("hello\n1");
    expect(tabOf("main.rs")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("alert")).toHaveTextContent("Could not save main.rs");
    expect(api.unsavedPaths).toEqual(["src/main.rs"]);
  });

  it("closeAll closes every tab, losing what is unsaved", async () => {
    const { container } = mount();
    await open("notes.txt");
    typeAtEnd(container, "1");
    act(() => api.closeAll());
    expect(screen.queryByRole("tab")).toBeNull();
    expect(api.unsavedPaths).toEqual([]);
    expect(sent("file_close")).toEqual([{ projectId: PROJECT, path: "notes.txt" }]);
  });

  it("closeUnder closes the tabs of a deleted entry, and moveUnder makes them follow a renamed one", async () => {
    const { container } = mount();
    await open("src/main.rs");
    typeAtEnd(container, "kept");
    await open("notes.txt");
    act(() => api.moveUnder("src", "lib"));
    expect(api.tabs.map((tab) => tab.path)).toEqual(["lib/main.rs", "notes.txt"]);
    expect(api.unsavedPaths).toEqual(["lib/main.rs"]);
    act(() => api.closeUnder("lib"));
    expect(api.tabs.map((tab) => tab.path)).toEqual(["notes.txt"]);
    expect(sent("file_close")).toHaveLength(0);
    expect(api.unsavedPaths).toEqual([]);
  });

  it("tells the backend that the File view has no unsaved change when it goes away", async () => {
    const { container, unmount } = mount();
    await open("notes.txt");
    typeAtEnd(container, "x");
    await waitFor(() => expect(sent("files_unsaved").at(-1)).toEqual({ projectId: PROJECT, paths: ["notes.txt"] }));
    unmount();
    expect(sent("files_unsaved").at(-1)).toEqual({ projectId: PROJECT, paths: [] });
    expect(sent("files_close_key").at(-1)).toEqual({ projectId: PROJECT, claimed: false });
  });
});

/** The `<img>` of the image view loads, with the size the browser would find. */
function loaded(image: HTMLElement, width: number, height: number) {
  Object.defineProperty(image, "naturalWidth", { configurable: true, value: width });
  Object.defineProperty(image, "naturalHeight", { configurable: true, value: height });
  fireEvent.load(image);
}

const PNG = { dataUrl: "data:image/png;base64,AAAA", size: 12_700 };

describe("images", () => {
  beforeEach(() => {
    disk.set("pics/logo.png", { text: "", image: PNG });
  });

  it("shows an image with its size in pixels and its weight, and no text, at the size of the panel", async () => {
    const { container } = mount();
    await open("pics/logo.png");
    const image = screen.getByAltText("logo.png");
    expect(image).toHaveAttribute("src", PNG.dataUrl);
    expect(screen.queryByText("Binary file")).toBeNull();
    expect(container.querySelector(".cm-content")?.textContent ?? "").toBe("");
    // The size shows once the image is drawn.
    expect(screen.queryByText(/ px$/)).toBeNull();
    loaded(image, 640, 480);
    expect(screen.getByText("640 × 480 px")).toBeInTheDocument();
    expect(screen.getByText("12.4 KB")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Show actual size" })).toBe(image);
    expect(tabOf("logo.png")).toBeInTheDocument();
  });

  it("`Open in editor` on an image opens the file without a line", async () => {
    const user = userEvent.setup();
    mount();
    await open("pics/logo.png");
    await user.click(screen.getByRole("button", { name: "Open in editor" }));
    expect(sent("open_in_editor").at(-1)).toMatchObject({ file: "pics/logo.png", line: null });
  });

  it("a click, or Enter or Space with the focus, switches between fitted and actual size, and the tab keeps the choice", async () => {
    const user = userEvent.setup();
    mount();
    await open("pics/logo.png");
    const image = screen.getByAltText("logo.png");
    expect(image.className).toContain("cursor-zoom-in");
    await user.click(image);
    expect(screen.getByRole("button", { name: "Fit to panel" })).toBe(image);
    expect(image.className).toContain("cursor-zoom-out");
    image.focus();
    await user.keyboard("{Enter}");
    expect(screen.getByRole("button", { name: "Show actual size" })).toBe(image);
    await user.keyboard(" ");
    expect(screen.getByRole("button", { name: "Fit to panel" })).toBe(image);
    // Another tab and back, and a new image read: the tab stays at actual size.
    await open("notes.txt");
    act(() => api.showTab("pics/logo.png"));
    expect(screen.getByRole("button", { name: "Fit to panel" })).toBeInTheDocument();
    disk.set("pics/logo.png", { text: "", image: { dataUrl: "data:image/png;base64,BBBB", size: 3 } });
    emitDisk("file-tab-changed", { path: "pics/logo.png", change: "changed" });
    await waitFor(() => expect(screen.getByAltText("logo.png")).toHaveAttribute("src", "data:image/png;base64,BBBB"));
    expect(screen.getByRole("button", { name: "Fit to panel" })).toBeInTheDocument();
  });

  it("reads an image again after a change: the new image shows at once, once per change", async () => {
    mount();
    await open("pics/logo.png");
    loaded(screen.getByAltText("logo.png"), 640, 480);
    disk.set("pics/logo.png", { text: "", image: { dataUrl: "data:image/png;base64,CCCC", size: 2_097_152 } });
    emitDisk("file-tab-changed", { path: "pics/logo.png", change: "changed" });
    await waitFor(() => expect(screen.getByAltText("logo.png")).toHaveAttribute("src", "data:image/png;base64,CCCC"));
    expect(sent("file_reload")).toHaveLength(1);
    // The size of the new image comes with its own load; the weight is already the new one.
    expect(screen.queryByText("640 × 480 px")).toBeNull();
    expect(screen.getByText("2.0 MB")).toBeInTheDocument();
    loaded(screen.getByAltText("logo.png"), 32, 16);
    expect(screen.getByText("32 × 16 px")).toBeInTheDocument();
  });

  it("turns into the binary panel when the file is no longer an image, and says when it is too large", async () => {
    mount();
    await open("pics/logo.png");
    disk.set("pics/logo.png", { text: "", binary: true });
    emitDisk("file-tab-changed", { path: "pics/logo.png", change: "changed" });
    await waitFor(() => expect(screen.getByText("Binary file")).toBeInTheDocument());
    expect(screen.queryByText(/larger than 10 MB/)).toBeNull();
    expect(screen.queryByAltText("logo.png")).toBeNull();
    // It is an image again, then one that is too large.
    disk.set("pics/logo.png", { text: "", image: PNG });
    emitDisk("file-tab-changed", { path: "pics/logo.png", change: "changed" });
    await waitFor(() => expect(screen.getByAltText("logo.png")).toBeInTheDocument());
    expect(screen.queryByText("Binary file")).toBeNull();
    disk.set("pics/logo.png", { text: "", binary: true, imageTooLarge: true });
    emitDisk("file-tab-changed", { path: "pics/logo.png", change: "changed" });
    await waitFor(() => expect(screen.getByText("This image is larger than 10 MB: no preview.")).toBeInTheDocument());
    expect(screen.getByText("Binary file")).toBeInTheDocument();
  });

  it("opens a file over 10 MB with an image signature as binary with the reason", async () => {
    disk.set("pics/huge.png", { text: "", binary: true, imageTooLarge: true });
    mount();
    await open("pics/huge.png");
    expect(screen.getByText("Binary file")).toBeInTheDocument();
    expect(screen.getByText("This image is larger than 10 MB: no preview.")).toBeInTheDocument();
    expect(screen.queryByRole("img", { name: "huge.png" })).toBeNull();
  });

  it("an image that cannot be drawn shows the binary panel with the line that it cannot be shown", async () => {
    mount();
    await open("pics/logo.png");
    fireEvent.error(screen.getByAltText("logo.png"));
    expect(screen.getByText("Binary file")).toBeInTheDocument();
    expect(screen.getByText("This image cannot be shown.")).toBeInTheDocument();
    expect(screen.queryByAltText("logo.png")).toBeNull();
    expect(screen.getByRole("button", { name: "Open in editor" })).toBeInTheDocument();
  });

  it("a deleted image stays with a bar and no `Save again`, and the bar goes when the file is back", async () => {
    mount();
    await open("pics/logo.png");
    disk.delete("pics/logo.png");
    emitDisk("file-tab-changed", { path: "pics/logo.png", change: "deleted" });
    expect(screen.getByRole("alert")).toHaveTextContent("This file was deleted on disk.");
    expect(screen.queryByRole("button", { name: "Save again" })).toBeNull();
    expect(screen.getByAltText("logo.png")).toBeInTheDocument();
    disk.set("pics/logo.png", { text: "", image: PNG });
    emitDisk("file-tab-changed", { path: "pics/logo.png", change: "changed" });
    await waitFor(() => expect(screen.queryByRole("alert")).toBeNull());
    expect(screen.getByAltText("logo.png")).toBeInTheDocument();
  });

  it("an image has no lock line and nothing in it can be saved", async () => {
    mount();
    await open("pics/logo.png");
    emitDisk("file-lock-changed", { path: "pics/logo.png", locked: true, movedTo: null });
    expect(screen.queryByText("The agent is editing this file.")).toBeNull();
    act(() => api.save(api.activeId ?? ""));
    expect(sent("file_save")).toHaveLength(0);
  });
});

describe("SVG files", () => {
  const SVG = '<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>\n';
  const dataUrlOf = (text: string) => `data:image/svg+xml;charset=utf-8,${encodeURIComponent(text)}`;

  beforeEach(() => {
    disk.set("art/shape.svg", { text: SVG });
    disk.set("art/UPPER.SVG", { text: SVG });
    disk.set("art/shape.svg.txt", { text: SVG });
  });

  it("opens on the drawing, built from the text as a data URL in an image, with its size and its weight", async () => {
    const { container } = mount();
    await open("art/shape.svg");
    const image = screen.getByAltText("shape.svg");
    expect(image).toHaveAttribute("src", dataUrlOf(SVG));
    expect(screen.getByRole("radio", { name: "Preview" })).toBeChecked();
    expect(container.querySelector("[data-language]")).toHaveAttribute("hidden");
    loaded(image, 10, 10);
    expect(screen.getByText("10 × 10 px")).toBeInTheDocument();
    expect(screen.getByText(`${new TextEncoder().encode(SVG).length} B`)).toBeInTheDocument();
  });

  it("the name decides, whatever the case, and the content never does", async () => {
    mount();
    await open("art/UPPER.SVG");
    expect(screen.getByAltText("UPPER.SVG")).toBeInTheDocument();
    await open("art/shape.svg.txt");
    expect(screen.queryByAltText("shape.svg.txt")).toBeNull();
    expect(screen.queryByRole("radio", { name: "Code" })).toBeNull();
    await open("notes.txt");
    expect(screen.queryByRole("radio", { name: "Preview" })).toBeNull();
  });

  it("switches between the drawing and the text with Code and Preview, keeping the cursor, the selection and the undo history", async () => {
    const user = userEvent.setup();
    const { container } = mount();
    await open("art/shape.svg");
    await user.click(screen.getByRole("radio", { name: "Code" }));
    expect(container.querySelector("[data-language]")).not.toHaveAttribute("hidden");
    expect(screen.queryByAltText("shape.svg")).toBeNull();
    const view = editorView(container);
    typeAtEnd(container, "<!-- x -->");
    act(() => view.dispatch({ selection: { anchor: 3, head: 7 } }));
    await user.click(screen.getByRole("radio", { name: "Preview" }));
    // The drawing is the text of the tab, unsaved changes included.
    expect(screen.getByAltText("shape.svg")).toHaveAttribute("src", dataUrlOf(`${SVG}<!-- x -->`));
    await user.click(screen.getByRole("radio", { name: "Code" }));
    expect(editorView(container)).toBe(view);
    expect(view.state.selection.main).toMatchObject({ anchor: 3, head: 7 });
    expect(view.state.doc.toString()).toBe(`${SVG}<!-- x -->`);
    // The switch added nothing to the history: one undo takes back the one typed edit.
    act(() => {
      undo(view);
    });
    expect(view.state.doc.toString()).toBe(SVG);
    // The file was never touched by the switch.
    expect(sent("file_save")).toHaveLength(0);
  });

  it("the choice holds for the tab while it stays open", async () => {
    const user = userEvent.setup();
    mount();
    await open("art/shape.svg");
    await user.click(screen.getByRole("radio", { name: "Code" }));
    await open("notes.txt");
    act(() => api.showTab("art/shape.svg"));
    expect(screen.getByRole("radio", { name: "Code" })).toBeChecked();
    expect(screen.queryByAltText("shape.svg")).toBeNull();
  });

  it("Left and Right move between Code and Preview", async () => {
    const user = userEvent.setup();
    mount();
    await open("art/shape.svg");
    screen.getByRole("radio", { name: "Preview" }).focus();
    await user.keyboard("{ArrowLeft}");
    expect(screen.getByRole("radio", { name: "Code" })).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("radio", { name: "Preview" })).toHaveFocus();
  });

  it("draws nothing while the text is shown, however much is typed, and builds the drawing again when it is shown", async () => {
    const user = userEvent.setup();
    const { container } = mount();
    await open("art/shape.svg");
    await user.click(screen.getByRole("radio", { name: "Code" }));
    const encode = vi.spyOn(globalThis, "encodeURIComponent");
    typeAtEnd(container, "<a/>");
    typeAtEnd(container, "<b/>");
    expect(encode).not.toHaveBeenCalled();
    await user.click(screen.getByRole("radio", { name: "Preview" }));
    expect(encode).toHaveBeenCalledTimes(1);
    encode.mockRestore();
  });

  it("draws the text again when it is read from the disk while the drawing is shown", async () => {
    mount();
    await open("art/shape.svg");
    const next = '<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"/>\n';
    disk.set("art/shape.svg", { text: next });
    emitDisk("file-tab-changed", { path: "art/shape.svg", change: "changed" });
    await waitFor(() => expect(screen.getByAltText("shape.svg")).toHaveAttribute("src", dataUrlOf(next)));
  });

  it("a text that cannot be drawn says so in muted text in place of the drawing", async () => {
    mount();
    await open("art/shape.svg");
    fireEvent.error(screen.getByAltText("shape.svg"));
    expect(screen.getByText("This SVG cannot be drawn.")).toBeInTheDocument();
    expect(screen.queryByAltText("shape.svg")).toBeNull();
    expect(screen.queryByText(/ px$/)).toBeNull();
  });

  it("the switch is there for a read-only SVG too, and Cmd+S still saves the text", async () => {
    const user = userEvent.setup();
    const { container } = mount();
    disk.set("art/big.svg", { text: SVG, readOnly: "tooLarge" });
    await open("art/big.svg");
    expect(screen.getByText("This file is larger than 2 MB: read-only, without colours.")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Code" })).toBeInTheDocument();
    await open("art/shape.svg");
    await user.click(screen.getByRole("radio", { name: "Code" }));
    typeAtEnd(container, "<!-- y -->");
    await user.click(screen.getByRole("radio", { name: "Preview" }));
    fireEvent.keyDown(window, { key: "s", metaKey: true });
    await waitFor(() => expect(sent("file_save")).toHaveLength(1));
    expect(sent("file_save")[0]).toMatchObject({ path: "art/shape.svg", text: `${SVG}<!-- y -->` });
  });

  it("`Open in editor` on the drawing opens the file at the cursor's line", async () => {
    const user = userEvent.setup();
    mount();
    await open("art/shape.svg");
    await user.click(screen.getByRole("button", { name: "Open in editor" }));
    expect(sent("open_in_editor").at(-1)).toMatchObject({ file: "art/shape.svg", line: 1 });
  });
});
