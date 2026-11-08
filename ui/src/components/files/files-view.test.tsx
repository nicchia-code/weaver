import type { PhysicalPosition } from "@tauri-apps/api/dpi";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { FileEntry } from "@/bindings/FileEntry";
import type { TreeChanged } from "@/bindings/TreeChanged";
import type { TreeUpdate } from "@/bindings/TreeUpdate";
import { FilesView } from "@/components/files/files-view";
import { ProjectWindow } from "@/components/project-window";
import { TooltipProvider } from "@/components/ui/tooltip";
import type * as BackendModule from "@/lib/backend";
import { call, on } from "@/lib/backend";

const { tabs, toast, webview } = vi.hoisted(() => ({
  tabs: {
    openFile: vi.fn(),
    closeUnder: vi.fn(),
    moveUnder: vi.fn(),
    unsavedUnder: vi.fn((_path: string) => [] as string[]),
    unsavedPaths: [] as string[],
    lockedPaths: new Set<string>(),
    saveAll: vi.fn(async () => [] as { path: string; reason: string }[]),
    closeAll: vi.fn(),
  },
  toast: { error: vi.fn(), warning: vi.fn(), loading: vi.fn(), dismiss: vi.fn() },
  webview: { handlers: new Set<(event: { payload: unknown }) => void>() },
}));

vi.mock("@/hooks/use-omp", () => ({ useOmp: () => ({ status: null }) }));
vi.mock("sonner", () => ({ toast }));
vi.mock("@/lib/backend", async (importOriginal) => ({
  ...(await importOriginal<typeof BackendModule>()),
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));
// The webview of the window: its drag and drop handler is Tauri's, and the tests emit its events.
vi.mock("@tauri-apps/api/webview", () => ({
  getCurrentWebview: () => ({
    onDragDropEvent: (handler: (event: { payload: unknown }) => void) => {
      webview.handlers.add(handler);
      return Promise.resolve(() => webview.handlers.delete(handler));
    },
  }),
}));
vi.mock("@/components/files/file-tabs-context", () => ({
  FileTabsProvider: ({ children }: { children: unknown }) => children,
  useFileTabs: () => tabs,
}));
vi.mock("@/components/files/file-tabs", () => ({ FileTabs: () => <p>Open a file from the tree.</p> }));

const PROJECT = "0123456789abcdef";
const dir = (path: string): FileEntry => ({ name: path.split("/").pop() ?? path, path, kind: "folder" });
const file = (path: string): FileEntry => ({ name: path.split("/").pop() ?? path, path, kind: "file" });

let generation: number;
let listings: Record<string, FileEntry[]>;
let push: (changed: TreeChanged) => void;
let calls: [string, Record<string, unknown>][];
let failWith: Record<string, string>;
/** The folders the backend holds expanded, and what the next copy in answers, and when. */
let expanded: Set<string>;
let copied: string[];
let copyGate: Promise<void> | null;

const update = (...paths: string[]): TreeUpdate => ({
  generation: ++generation,
  folders: paths.map((path) => ({ path, entries: listings[path] ?? [] })),
  removed: [],
});

function backend(name: string, args: Record<string, unknown>): Promise<unknown> {
  calls.push([name, args]);
  if (failWith[name]) return Promise.reject({ code: "refused", message: failWith[name] });
  switch (name) {
    case "files_open_view":
      return Promise.resolve({ generation: ++generation, folders: [{ path: "", entries: listings[""] }] });
    case "files_expand":
      expanded.add(args.path as string);
      return Promise.resolve(update(args.path as string));
    case "files_collapse":
    case "files_leave":
      return Promise.resolve(null);
    case "files_folders":
      return Promise.resolve((listings[args.path as string] ?? []).filter((entry) => entry.kind === "folder"));
    case "files_create":
      return Promise.resolve({ path: `${args.parent ? `${args.parent}/` : ""}${args.name}`, update: update(args.parent as string) });
    case "files_rename": {
      const path = args.path as string;
      const parent = path.includes("/") ? path.slice(0, path.lastIndexOf("/") + 1) : "";
      return Promise.resolve({ path: `${parent}${args.name}`, update: update() });
    }
    case "files_move":
      return Promise.resolve({ path: `${args.target ? `${args.target}/` : ""}${(args.path as string).split("/").pop()}`, update: update() });
    case "files_copy_in": {
      const target = args.target as string;
      const done = async () => ({ paths: copied, update: update(...(target === "" || expanded.has(target) ? [target] : [])) });
      return copyGate ? copyGate.then(done) : done();
    }
    case "files_delete":
      return Promise.resolve(update());
    case "project_status":
      return Promise.resolve({ state: "has-model" });
    default:
      return Promise.reject(new Error(`unexpected command ${name}`));
  }
}

beforeEach(() => {
  generation = 0;
  calls = [];
  failWith = {};
  listings = {
    "": [dir(".weaver"), dir("docs"), dir("src"), file("README.md"), file("notes.txt")],
    ".weaver": [file(".weaver/system.md")],
    docs: [file("docs/guide.md")],
    src: [dir("src/deep"), file("src/main.ts"), file("src/lib.ts")],
    "src/deep": [file("src/deep/leaf.ts")],
  };
  tabs.openFile.mockReset();
  tabs.closeUnder.mockReset();
  tabs.moveUnder.mockReset();
  tabs.unsavedUnder.mockReset().mockReturnValue([]);
  tabs.unsavedPaths = [];
  tabs.lockedPaths = new Set();
  tabs.saveAll.mockReset().mockResolvedValue([]);
  tabs.closeAll.mockReset();
  toast.error.mockReset();
  toast.loading.mockReset().mockReturnValue("copy-toast");
  toast.dismiss.mockReset();
  webview.handlers.clear();
  document.elementFromPoint = vi.fn(() => null);
  expanded = new Set();
  copyGate = null;
  copied = [];
  vi.mocked(call).mockReset().mockImplementation(((name: string, args: Record<string, unknown>) => backend(name, args)) as unknown as typeof call);
  vi.mocked(on).mockImplementation(((name: string, handler: (changed: TreeChanged) => void) => {
    if (name === "files-tree-changed") push = handler;
    return Promise.resolve(() => {});
  }) as unknown as typeof on);
});

const called = (name: string) => calls.filter(([n]) => n === name).map(([, args]) => args);

function renderView() {
  return render(
    <TooltipProvider>
      <ProjectWindow projectId={PROJECT} initialView="files" views={{ files: <FilesView />, board: <p>Board content</p> }} />
    </TooltipProvider>,
  );
}

const tree = () => screen.findByRole("tree", { name: "Project files" });
const item = (name: string) => screen.getByRole("treeitem", { name: new RegExp(`^${name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`) });
const names = () => within(screen.getByRole("tree", { name: "Project files" })).getAllByRole("treeitem").map((row) => row.textContent);

/** Right click on a row (or the empty space of the tree) and the menu that opens. */
async function openMenu(target: HTMLElement) {
  fireEvent.contextMenu(target);
  return screen.findByRole("menu");
}

describe("the Files entry", () => {
  it("opens the view, reads only the root and shows folders first with every folder collapsed", async () => {
    renderView();
    await tree();
    expect(names()).toEqual([".weaver", "docs", "src", "README.md", "notes.txt"]);
    expect(called("files_open_view")).toEqual([{ projectId: PROJECT }]);
    expect(called("files_expand")).toEqual([]);
    const rows = within(screen.getByRole("tree")).getAllByRole("treeitem");
    expect(rows.map((row) => row.getAttribute("aria-expanded"))).toEqual(["false", "false", "false", null, null]);
    expect(rows.every((row) => row.getAttribute("aria-level") === "1")).toBe(true);
    expect(screen.getByText("Open a file from the tree.")).toBeInTheDocument();
  });

  it("starts with the view header across the tree and the tabs", async () => {
    renderView();
    const treeNode = await tree();
    const header = screen.getByRole("heading", { level: 1, name: "Files" }).closest("header")!;
    expect(header).toHaveClass("h-[76px]", "border-b");
    expect(header.contains(treeNode)).toBe(false);
    expect(header.nextElementSibling).toContainElement(treeNode);
    expect(header.nextElementSibling).toContainElement(screen.getByText("Open a file from the tree."));
  });

  it("is a sidebar entry of the PROJECT group after Specs, and choosing another entry closes the view and tells the backend", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    const entries = within(screen.getByRole("group", { name: "Project" }))
      .getAllByRole("button")
      .map((button) => button.textContent);
    expect(entries.slice(0, 4)).toEqual(["Requirements", "Specs", "Files", "Assets"]);
    await user.click(screen.getByRole("button", { name: "Board" }));
    expect(screen.queryByRole("tree", { name: "Project files" })).not.toBeInTheDocument();
    expect(screen.getByText("Board content")).toBeInTheDocument();
    expect(tabs.closeAll).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(called("files_leave")).toEqual([{ projectId: PROJECT }]));
    // Coming back reads the root again and keeps no state of the view.
    await user.click(screen.getByRole("button", { name: "Files" }));
    await tree();
    expect(called("files_open_view")).toHaveLength(2);
    expect(called("files_leave")).toHaveLength(1);
    expect(called("files_open_view").length).toBeGreaterThan(called("files_leave").length - 1);
  });
});

describe("the tree with the keyboard and the mouse", () => {
  it("moves with Up and Down, expands with Right, goes to the first child with Right again and back to the parent with Left", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    act(() => item(".weaver").focus());
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(item("src")).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    await waitFor(() => expect(names()).toContain("main.ts"));
    expect(item("src")).toHaveAttribute("aria-expanded", "true");
    expect(called("files_expand")).toEqual([{ projectId: PROJECT, path: "src" }]);
    expect(names()).toEqual([".weaver", "docs", "src", "deep", "main.ts", "lib.ts", "README.md", "notes.txt"].map((n) => n));
    await user.keyboard("{ArrowRight}");
    expect(item("deep")).toHaveFocus();
    expect(item("deep")).toHaveAttribute("aria-level", "2");
    await user.keyboard("{ArrowLeft}");
    expect(item("src")).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    await waitFor(() => expect(item("src")).toHaveAttribute("aria-expanded", "false"));
    expect(called("files_collapse")).toEqual([{ projectId: PROJECT, path: "src" }]);
    expect(names()).not.toContain("main.ts");
    await user.keyboard("{ArrowUp}");
    expect(item("docs")).toHaveFocus();
    await user.keyboard("{End}");
    expect(item("notes.txt")).toHaveFocus();
    await user.keyboard("{Home}");
    expect(item(".weaver")).toHaveFocus();
  });

  it("opens a file in a tab with Enter and with a click, and toggles a folder the same way", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    await user.click(item("README.md"));
    expect(tabs.openFile).toHaveBeenCalledWith("README.md");
    await user.keyboard("{ArrowDown}");
    expect(item("notes.txt")).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(tabs.openFile).toHaveBeenLastCalledWith("notes.txt");
    await user.click(item("docs"));
    await waitFor(() => expect(item("docs")).toHaveAttribute("aria-expanded", "true"));
    await user.click(item("docs"));
    await waitFor(() => expect(item("docs")).toHaveAttribute("aria-expanded", "false"));
    item("docs").focus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(item("docs")).toHaveAttribute("aria-expanded", "true"));
    expect(tabs.openFile).toHaveBeenCalledTimes(2);
  });

  it("marks the selected row and carries the position of each row", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    await user.click(item("README.md"));
    expect(item("README.md")).toHaveAttribute("aria-selected", "true");
    expect(item("notes.txt")).toHaveAttribute("aria-selected", "false");
    expect(item("README.md")).toHaveAttribute("aria-posinset", "4");
    expect(item("README.md")).toHaveAttribute("aria-setsize", "5");
  });

  it("shows a lock with its name and tooltip on a file open in a tab whose lock is held", async () => {
    const user = userEvent.setup();
    tabs.lockedPaths = new Set(["README.md"]);
    renderView();
    await tree();
    const lock = within(item("README.md")).getByRole("img", { name: "The agent is editing this file" });
    expect(lock).toBeInTheDocument();
    expect(within(item("notes.txt")).queryByRole("img")).not.toBeInTheDocument();
    await user.hover(lock);
    expect((await screen.findAllByText("The agent is editing this file")).length).toBeGreaterThan(0);
  });

  it("applies what the backend says when a listed folder changes, and ignores an older answer", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    await user.click(item("docs"));
    await waitFor(() => expect(names()).toContain("guide.md"));
    act(() =>
      push({ projectId: PROJECT, generation: 50, folders: [{ path: "docs", entries: [file("docs/guide.md"), file("docs/new.md")] }], removed: [] }),
    );
    expect(names()).toContain("new.md");
    act(() => push({ projectId: PROJECT, generation: 40, folders: [{ path: "docs", entries: [file("docs/old.md")] }], removed: [] }));
    expect(names()).toContain("new.md");
    expect(names()).not.toContain("old.md");
    act(() => push({ projectId: "ffffffffffffffff", generation: 99, folders: [{ path: "docs", entries: [] }], removed: [] }));
    expect(names()).toContain("new.md");
    act(() => push({ projectId: PROJECT, generation: 60, folders: [], removed: ["docs"] }));
    expect(names()).not.toContain("new.md");
    expect(item("docs")).toHaveAttribute("aria-expanded", "false");
  });
});

describe("the context menu", () => {
  it("offers New file, New folder, Rename, Move to… and Delete on a folder", async () => {
    renderView();
    await tree();
    const menu = await openMenu(item("docs"));
    expect(within(menu).getAllByRole("menuitem").map((entry) => entry.textContent)).toEqual(["New file", "New folder", "Rename", "Move to…", "Delete"]);
  });

  it("leaves New file and New folder out on a file", async () => {
    renderView();
    await tree();
    const menu = await openMenu(item("README.md"));
    expect(within(menu).getAllByRole("menuitem").map((entry) => entry.textContent)).toEqual(["Rename", "Move to…", "Delete"]);
  });

  it("offers only New file and New folder on the empty space, for the root", async () => {
    renderView();
    const view = await tree();
    const menu = await openMenu(view);
    expect(within(menu).getAllByRole("menuitem").map((entry) => entry.textContent)).toEqual(["New file", "New folder"]);
  });

  it("opens with Shift+F10 and with the Menu key on the selected row", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    await user.click(item("README.md"));
    await user.keyboard("{Shift>}{F10}{/Shift}");
    expect(within(await screen.findByRole("menu")).getByRole("menuitem", { name: "Rename" })).toBeInTheDocument();
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
    expect(item("README.md")).toHaveFocus();
    await user.keyboard("{ContextMenu}");
    expect(await screen.findByRole("menu")).toBeInTheDocument();
  });

  it("shows the actions that would change .weaver disabled, with the reason as tooltip and description", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    const menu = await openMenu(item(".weaver"));
    for (const label of ["New file", "New folder", "Rename", "Move to…", "Delete"]) {
      const entry = within(menu).getByRole("menuitem", { name: label });
      expect(entry).toHaveAttribute("aria-disabled", "true");
      expect(entry).toHaveAccessibleDescription(".weaver is written by Weaver: read-only here.");
    }
    await user.click(within(menu).getByRole("menuitem", { name: "Delete" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    await user.hover(within(menu).getByRole("menuitem", { name: "Rename" }));
    expect((await screen.findAllByText(".weaver is written by Weaver: read-only here.")).length).toBeGreaterThan(0);
  });

  it("disables them for what is inside .weaver too, and F2 and Delete do nothing there", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    await user.click(item(".weaver"));
    await waitFor(() => expect(names()).toContain("system.md"));
    const menu = await openMenu(item("system.md"));
    expect(within(menu).getByRole("menuitem", { name: "Rename" })).toHaveAttribute("aria-disabled", "true");
    await user.keyboard("{Escape}");
    item("system.md").focus();
    await user.keyboard("{F2}");
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    await user.keyboard("{Delete}");
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });
});

describe("naming an entry in the tree", () => {
  it("renames with F2: the field holds the name, selected without its extension; Enter confirms", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    await user.click(item("README.md"));
    await user.keyboard("{F2}");
    const field = await screen.findByRole("textbox", { name: "New name for README.md" });
    expect(field).toHaveValue("README.md");
    expect(field).toHaveFocus();
    expect((field as HTMLInputElement).selectionStart).toBe(0);
    expect((field as HTMLInputElement).selectionEnd).toBe(6);
    await user.keyboard("GUIDE{Enter}");
    await waitFor(() => expect(called("files_rename")).toEqual([{ projectId: PROJECT, path: "README.md", name: "GUIDE.md" }]));
    expect(tabs.moveUnder).toHaveBeenCalledWith("README.md", "GUIDE.md");
    await waitFor(() => expect(screen.queryByRole("textbox")).not.toBeInTheDocument());
  });

  it("cancels with Esc and by leaving the field", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    await user.click(item("README.md"));
    await user.keyboard("{F2}");
    await screen.findByRole("textbox");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    await user.keyboard("{F2}");
    await screen.findByRole("textbox");
    await user.click(screen.getByRole("tree", { name: "Project files" }));
    await waitFor(() => expect(screen.queryByRole("textbox")).not.toBeInTheDocument());
    expect(called("files_rename")).toEqual([]);
  });

  it("does nothing for the same name, and keeps the field with a toast when the rename is refused", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    await user.click(item("README.md"));
    await user.keyboard("{F2}{Enter}");
    expect(called("files_rename")).toEqual([]);
    await waitFor(() => expect(screen.queryByRole("textbox")).not.toBeInTheDocument());
    failWith.files_rename = 'A file or folder named "notes.txt" already exists here.';
    await user.keyboard("{F2}");
    await screen.findByRole("textbox");
    await user.keyboard("{Control>}a{/Control}notes.txt{Enter}");
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('A file or folder named "notes.txt" already exists here.'));
    expect(screen.getByRole("textbox")).toBeInTheDocument();
    expect(tabs.moveUnder).not.toHaveBeenCalled();
  });

  it("renames a folder with the menu and selects the whole name", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    const menu = await openMenu(item("docs"));
    await user.click(within(menu).getByRole("menuitem", { name: "Rename" }));
    const field = (await screen.findByRole("textbox", { name: "New name for docs" })) as HTMLInputElement;
    expect([field.selectionStart, field.selectionEnd]).toEqual([0, 4]);
    expect(field).toHaveFocus();
  });

  it("makes a file in a folder: the folder opens, a field shows in its first row, Enter creates it", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    const menu = await openMenu(item("docs"));
    await user.click(within(menu).getByRole("menuitem", { name: "New file" }));
    const field = await screen.findByRole("textbox", { name: "New file name" });
    expect(field).toHaveFocus();
    expect(field).toHaveValue("");
    const rows = within(screen.getByRole("tree")).getAllByRole("treeitem");
    const at = rows.findIndex((row) => within(row).queryByRole("textbox"));
    expect(rows[at - 1]).toBe(item("docs"));
    expect(rows[at]).toHaveAttribute("aria-level", "2");
    await user.keyboard("todo.md{Enter}");
    await waitFor(() => expect(called("files_create")).toEqual([{ projectId: PROJECT, parent: "docs", name: "todo.md", folder: false }]));
    await waitFor(() => expect(screen.queryByRole("textbox")).not.toBeInTheDocument());
  });

  it("makes a folder in the root from the empty space and cancels it with Esc", async () => {
    const user = userEvent.setup();
    renderView();
    const view = await tree();
    const menu = await openMenu(view);
    await user.click(within(menu).getByRole("menuitem", { name: "New folder" }));
    const field = await screen.findByRole("textbox", { name: "New folder name" });
    expect(within(screen.getByRole("tree")).getAllByRole("treeitem")[0]).toContainElement(field);
    await user.keyboard("assets{Enter}");
    await waitFor(() => expect(called("files_create")).toEqual([{ projectId: PROJECT, parent: "", name: "assets", folder: true }]));
    const again = await openMenu(view);
    await user.click(within(again).getByRole("menuitem", { name: "New file" }));
    await screen.findByRole("textbox", { name: "New file name" });
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(called("files_create")).toHaveLength(1);
  });

  it("shows the reason of a refused creation in a toast and keeps the field", async () => {
    const user = userEvent.setup();
    failWith.files_create = "The name cannot be empty.";
    renderView();
    const view = await tree();
    const menu = await openMenu(view);
    await user.click(within(menu).getByRole("menuitem", { name: "New file" }));
    await screen.findByRole("textbox", { name: "New file name" });
    await user.keyboard("{Enter}");
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith("The name cannot be empty."));
    expect(screen.getByRole("textbox", { name: "New file name" })).toBeInTheDocument();
  });
});

describe("deleting", () => {
  it("asks first with Delete, the focus on Cancel; Move to Trash deletes and closes the tabs on it", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    await user.click(item("notes.txt"));
    await user.keyboard("{Delete}");
    const dialog = await screen.findByRole("alertdialog", { name: 'Move "notes.txt" to the trash?' });
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    expect(within(dialog).queryByText(/unsaved changes/)).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Move to Trash" }));
    await waitFor(() => expect(called("files_delete")).toEqual([{ projectId: PROJECT, path: "notes.txt" }]));
    expect(tabs.closeUnder).toHaveBeenCalledWith("notes.txt");
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
  });

  it("says how many open files have unsaved changes that will be lost", async () => {
    const user = userEvent.setup();
    tabs.unsavedUnder.mockImplementation((path: string) => (path === "src" ? ["src/main.ts", "src/lib.ts"] : []));
    renderView();
    await tree();
    const menu = await openMenu(item("src"));
    await user.click(within(menu).getByRole("menuitem", { name: "Delete" }));
    const dialog = await screen.findByRole("alertdialog", { name: 'Move "src" to the trash?' });
    expect(within(dialog).getByText("2 open files have unsaved changes that will be lost.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Move to Trash" })).toHaveClass("text-destructive", "border-destructive-line");
  });

  it("changes nothing with Cancel, and a refused deletion shows its reason", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    await user.click(item("notes.txt"));
    await user.keyboard("{Delete}");
    await user.click(await screen.findByRole("button", { name: "Cancel" }));
    expect(called("files_delete")).toEqual([]);
    expect(tabs.closeUnder).not.toHaveBeenCalled();
    failWith.files_delete = 'Could not move "notes.txt" to the trash: there is no trash';
    await user.keyboard("{Delete}");
    await user.click(await screen.findByRole("button", { name: "Move to Trash" }));
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Could not move "notes.txt" to the trash: there is no trash'));
    expect(tabs.closeUnder).not.toHaveBeenCalled();
    expect(names()).toContain("notes.txt");
  });
});

/** What is highlighted as the target of a drop: a folder's path, or `tree` for the tree itself. */
const highlighted = () => [...document.querySelectorAll("[data-drop-target]")].map((node) => node.getAttribute("data-path") ?? "tree");
const dragLabel = () => document.querySelector<HTMLElement>("[data-drag-label]");

/** What `document.elementFromPoint` answers (jsdom lays nothing out). */
const pointerIsOver = (element: Element | null) => vi.mocked(document.elementFromPoint).mockReturnValue(element);

/** Where the tree is on screen, and how far it is scrolled: what the drag aids need. */
function placeTree(view: HTMLElement, scrollTop: number) {
  const state = { scrollTop };
  Object.defineProperty(view, "scrollTop", { configurable: true, get: () => state.scrollTop, set: (value: number) => (state.scrollTop = value) });
  view.getBoundingClientRect = () => ({ top: 100, bottom: 500, left: 0, right: 300, width: 300, height: 400, x: 0, y: 100, toJSON: () => ({}) });
  return state;
}

/** Presses the primary button on a row; the drag then moves and ends through the returned functions. */
function press(name: string, x = 10, y = 10) {
  fireEvent.pointerDown(item(name), { pointerId: 1, button: 0, clientX: x, clientY: y });
  return {
    /** The pointer moves over `target` (the empty space of the tree for the root). */
    moveTo: (target: Element | null, at = { x: 80, y: 80 }) => {
      pointerIsOver(target);
      fireEvent.pointerMove(item(name), { pointerId: 1, clientX: at.x, clientY: at.y });
    },
    release: (target: Element | null = null, at = { x: 80, y: 80 }) => {
      pointerIsOver(target);
      fireEvent.pointerUp(item(name), { pointerId: 1, clientX: at.x, clientY: at.y });
    },
  };
}

describe("dragging a row", () => {
  it("uses pointer events only: no row is draggable", async () => {
    const { container } = renderView();
    await tree();
    expect(container.querySelector("[draggable]")).toBeNull();
  });

  it("starts after 4 px of movement; a press that moves less stays a click", async () => {
    renderView();
    await tree();
    const drag = press("README.md");
    drag.moveTo(item("docs"), { x: 12, y: 12 });
    expect(dragLabel()).toBeNull();
    expect(highlighted()).toEqual([]);
    drag.release(item("docs"), { x: 12, y: 12 });
    fireEvent.click(item("README.md"));
    expect(tabs.openFile).toHaveBeenCalledWith("README.md");
    expect(called("files_move")).toEqual([]);

    const next = press("docs");
    next.moveTo(item("docs"), { x: 10, y: 14 });
    expect(dragLabel()).not.toBeNull();
  });

  it("follows the pointer with a fixed label of the row's icon and name that never takes the pointer", async () => {
    renderView();
    await tree();
    const drag = press("src");
    drag.moveTo(null, { x: 100, y: 50 });
    const label = dragLabel()!;
    expect(label).toHaveTextContent("src");
    expect(label.style.position).toBe("fixed");
    expect(label.style.pointerEvents).toBe("none");
    expect(label.querySelector("svg")).not.toBeNull();
    drag.moveTo(null, { x: 140, y: 90 });
    expect(dragLabel()).toBe(label);
    expect(label.style.left).toBe("152px");
    expect(label.style.top).toBe("102px");
    drag.release();
    expect(dragLabel()).toBeNull();
  });

  it("moves a row released on a folder, highlighting the folder under the pointer", async () => {
    renderView();
    await tree();
    const drag = press("README.md");
    drag.moveTo(item("docs"));
    expect(highlighted()).toEqual(["docs"]);
    drag.moveTo(item("notes.txt"));
    expect(highlighted()).toEqual([]);
    drag.moveTo(item("docs"));
    drag.release(item("docs"));
    await waitFor(() => expect(called("files_move")).toEqual([{ projectId: PROJECT, path: "README.md", target: "docs" }]));
    expect(tabs.moveUnder).toHaveBeenCalledWith("README.md", "docs/README.md");
    expect(highlighted()).toEqual([]);
    expect(dragLabel()).toBeNull();
    // The click that ends the drag does not open or expand what is under the pointer.
    fireEvent.click(item("docs"));
    expect(called("files_expand")).toEqual([]);
    expect(tabs.openFile).not.toHaveBeenCalled();
  });

  it("moves a row released on the empty space of the tree to the root, which is highlighted as the tree", async () => {
    const user = userEvent.setup();
    renderView();
    const view = await tree();
    await user.click(item("docs"));
    await waitFor(() => expect(names()).toContain("guide.md"));
    const drag = press("guide.md");
    drag.moveTo(view);
    expect(highlighted()).toEqual(["tree"]);
    drag.release(view);
    await waitFor(() => expect(called("files_move")).toEqual([{ projectId: PROJECT, path: "docs/guide.md", target: "" }]));
  });

  it("highlights nothing and moves nothing where the move can never be: its own folder, itself, inside itself, .weaver", async () => {
    const user = userEvent.setup();
    renderView();
    const view = await tree();
    await user.click(item("src"));
    await waitFor(() => expect(names()).toContain("deep"));
    const refused = (name: string, target: Element | null) => {
      const drag = press(name);
      drag.moveTo(target);
      expect(highlighted()).toEqual([]);
      drag.release(target);
      expect(dragLabel()).toBeNull();
    };
    // Onto the folder it is already in, and onto the root while in the root.
    refused("main.ts", item("src"));
    refused("README.md", view);
    // A folder onto itself and into a folder inside it.
    refused("src", item("src"));
    refused("src", item("deep"));
    // Into .weaver; onto a file; and out of it, which is not even a drag.
    refused("README.md", item(".weaver"));
    refused("docs", item("notes.txt"));
    const fromWeaver = press(".weaver");
    fromWeaver.moveTo(item("docs"));
    expect(dragLabel()).toBeNull();
    expect(highlighted()).toEqual([]);
    expect(called("files_move")).toEqual([]);
  });

  it("ends without a change when released elsewhere, with Esc, with pointercancel or when the capture is lost", async () => {
    renderView();
    await tree();
    const elsewhere = press("README.md");
    elsewhere.moveTo(item("docs"));
    elsewhere.release(document.body);
    expect(dragLabel()).toBeNull();

    const escaped = press("README.md");
    escaped.moveTo(item("docs"));
    expect(highlighted()).toEqual(["docs"]);
    fireEvent.keyDown(window, { key: "Escape" });
    expect(dragLabel()).toBeNull();
    expect(highlighted()).toEqual([]);
    escaped.release(item("docs"));

    const cancelled = press("README.md");
    cancelled.moveTo(item("docs"));
    fireEvent.pointerCancel(item("README.md"), { pointerId: 1 });
    expect(dragLabel()).toBeNull();
    cancelled.release(item("docs"));

    const lost = press("README.md");
    lost.moveTo(item("docs"));
    fireEvent.lostPointerCapture(item("README.md"), { pointerId: 1 });
    expect(dragLabel()).toBeNull();
    expect(highlighted()).toEqual([]);
    lost.release(item("docs"));
    expect(called("files_move")).toEqual([]);
  });

  it("shows the reason of a refused move in an error toast and changes nothing", async () => {
    renderView();
    await tree();
    failWith.files_move = 'A file or folder named "README.md" already exists here.';
    const drag = press("README.md");
    drag.moveTo(item("docs"));
    drag.release(item("docs"));
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('A file or folder named "README.md" already exists here.'));
    expect(tabs.moveUnder).not.toHaveBeenCalled();
  });

  describe("the drag aids", () => {
    afterEach(() => {
      vi.useRealTimers();
    });

    it("expands a collapsed folder that stays under the pointer for a second", async () => {
      renderView();
      await tree();
      await waitFor(() => expect(names()).toContain("README.md"));
      vi.useFakeTimers();
      const drag = press("README.md");
      drag.moveTo(item("docs"));
      await act(() => vi.advanceTimersByTimeAsync(999));
      expect(called("files_expand")).toEqual([]);
      await act(() => vi.advanceTimersByTimeAsync(1));
      expect(called("files_expand")).toEqual([{ projectId: PROJECT, path: "docs" }]);
      drag.release();
    });

    it("starts the second again when the pointer leaves the folder, and never expands what it holds a file over", async () => {
      renderView();
      await tree();
      await waitFor(() => expect(names()).toContain("README.md"));
      vi.useFakeTimers();
      const drag = press("README.md");
      drag.moveTo(item("docs"));
      await act(() => vi.advanceTimersByTimeAsync(600));
      drag.moveTo(item("notes.txt"));
      await act(() => vi.advanceTimersByTimeAsync(600));
      drag.moveTo(item("docs"));
      await act(() => vi.advanceTimersByTimeAsync(600));
      expect(called("files_expand")).toEqual([]);
      drag.moveTo(item("notes.txt"));
      await act(() => vi.advanceTimersByTimeAsync(5000));
      expect(called("files_expand")).toEqual([]);
      drag.release();
    });

    it("scrolls the tree within 24 px of its top or bottom edge, faster nearer the edge, and stops when the drag ends", async () => {
      renderView();
      const view = await tree();
      await waitFor(() => expect(names()).toContain("README.md"));
      const scroll = placeTree(view, 200);
      vi.useFakeTimers();
      const drag = press("README.md", 150, 300);
      const tick = async (ms = 16) => {
        const before = scroll.scrollTop;
        await act(() => vi.advanceTimersByTimeAsync(ms));
        return scroll.scrollTop - before;
      };

      drag.moveTo(null, { x: 150, y: 110 });
      const slow = await tick();
      expect(slow).toBeLessThan(0);
      drag.moveTo(null, { x: 150, y: 101 });
      expect(await tick()).toBeLessThan(slow);

      drag.moveTo(null, { x: 150, y: 300 });
      expect(await tick(160)).toBe(0);

      drag.moveTo(null, { x: 150, y: 490 });
      const down = await tick();
      expect(down).toBeGreaterThan(0);
      drag.moveTo(null, { x: 150, y: 499 });
      expect(await tick()).toBeGreaterThan(down);

      drag.release();
      expect(await tick(160)).toBe(0);
    });
  });
});

/** A position of Tauri's drag and drop events, in physical pixels. */
const at = (x: number, y: number) => ({ x, y }) as unknown as PhysicalPosition;

/** Emits an event of Tauri's drag and drop handler. */
function emit(payload: DragDropEvent) {
  act(() => {
    for (const handler of [...webview.handlers]) handler({ payload });
  });
}
const enter = (x: number, y: number): DragDropEvent => ({ type: "enter", paths: ["/Users/me/a.png"], position: at(x, y) });
const over = (x: number, y: number): DragDropEvent => ({ type: "over", position: at(x, y) });
const drop = (x: number, y: number, paths: string[]): DragDropEvent => ({ type: "drop", paths, position: at(x, y) });

/** A promise the test settles by hand. */
function deferred() {
  let resolve: () => void = () => {};
  const promise = new Promise<void>((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

describe("files dragged in from outside", () => {
  beforeEach(() => {
    vi.stubGlobal("devicePixelRatio", 2);
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("listens only while the view is shown", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    await waitFor(() => expect(webview.handlers.size).toBe(1));
    await user.click(screen.getByRole("button", { name: "Board" }));
    await waitFor(() => expect(webview.handlers.size).toBe(0));
  });

  it("highlights the folder under the pointer, in CSS pixels, or the tree for its empty space, and nothing elsewhere", async () => {
    renderView();
    const view = await tree();
    await waitFor(() => expect(webview.handlers.size).toBe(1));
    pointerIsOver(item("docs"));
    emit(enter(400, 200));
    expect(document.elementFromPoint).toHaveBeenLastCalledWith(200, 100);
    expect(highlighted()).toEqual(["docs"]);
    pointerIsOver(item("notes.txt"));
    emit(over(402, 210));
    expect(highlighted()).toEqual([]);
    pointerIsOver(view);
    emit(over(404, 220));
    expect(highlighted()).toEqual(["tree"]);
    pointerIsOver(item(".weaver"));
    emit(over(404, 230));
    expect(highlighted()).toEqual([]);
    pointerIsOver(item("docs"));
    emit(over(404, 240));
    expect(highlighted()).toEqual(["docs"]);
    emit({ type: "leave" });
    expect(highlighted()).toEqual([]);
    pointerIsOver(document.body);
    emit(over(10, 10));
    expect(highlighted()).toEqual([]);
  });

  it("expands a collapsed folder held over for a second and scrolls the tree near its edges, as a row drag does", async () => {
    renderView();
    const view = await tree();
    await waitFor(() => expect(webview.handlers.size).toBe(1));
    const scroll = placeTree(view, 50);
    vi.useFakeTimers();
    pointerIsOver(item("docs"));
    emit(enter(100, 440));
    await act(() => vi.advanceTimersByTimeAsync(999));
    expect(called("files_expand")).toEqual([]);
    await act(() => vi.advanceTimersByTimeAsync(1));
    expect(called("files_expand")).toEqual([{ projectId: PROJECT, path: "docs" }]);
    // Physical y 440 is 220 CSS pixels: far from both edges. Near the bottom edge the tree scrolls.
    expect(scroll.scrollTop).toBe(50);
    emit(over(100, 980));
    await act(() => vi.advanceTimersByTimeAsync(160));
    expect(scroll.scrollTop).toBeGreaterThan(50);
    emit({ type: "leave" });
    const stopped = scroll.scrollTop;
    await act(() => vi.advanceTimersByTimeAsync(160));
    expect(scroll.scrollTop).toBe(stopped);
  });

  it("copies a drop on a folder with a progress toast, then expands the folder and selects and reveals the first copied entry", async () => {
    const revealed = vi.spyOn(Element.prototype, "scrollIntoView");
    renderView();
    await tree();
    await waitFor(() => expect(webview.handlers.size).toBe(1));
    const gate = deferred();
    copyGate = gate.promise;
    copied = ["docs/a.png", "docs/b.png"];
    pointerIsOver(item("docs"));
    emit(drop(400, 200, ["/Users/me/b.png", "/Users/me/a.png"]));
    expect(toast.loading).toHaveBeenCalledWith("Copying 2 items…");
    expect(called("files_copy_in")).toEqual([{ projectId: PROJECT, target: "docs", paths: ["/Users/me/b.png", "/Users/me/a.png"] }]);
    expect(highlighted()).toEqual([]);
    // The rest of the view stays usable meanwhile.
    fireEvent.click(item("README.md"));
    expect(tabs.openFile).toHaveBeenCalledWith("README.md");
    expect(toast.dismiss).not.toHaveBeenCalled();
    listings.docs = [file("docs/a.png"), file("docs/b.png"), file("docs/guide.md")];
    gate.resolve();
    await waitFor(() => expect(toast.dismiss).toHaveBeenCalledWith("copy-toast"));
    await waitFor(() => expect(called("files_expand")).toEqual([{ projectId: PROJECT, path: "docs" }]));
    await waitFor(() => expect(names()).toContain("a.png"));
    expect(item("a.png")).toHaveAttribute("aria-selected", "true");
    await waitFor(() => expect(revealed.mock.contexts).toContain(item("a.png")));
    expect(toast.error).not.toHaveBeenCalled();
  });

  it("says `Copying 1 item…` for one item and copies to the root for a drop on the empty space", async () => {
    renderView();
    const view = await tree();
    await waitFor(() => expect(webview.handlers.size).toBe(1));
    copied = ["a.png"];
    pointerIsOver(view);
    emit(drop(400, 200, ["/Users/me/a.png"]));
    expect(toast.loading).toHaveBeenCalledWith("Copying 1 item…");
    await waitFor(() => expect(called("files_copy_in")).toEqual([{ projectId: PROJECT, target: "", paths: ["/Users/me/a.png"] }]));
    await waitFor(() => expect(toast.dismiss).toHaveBeenCalled());
    // The root is open already: nothing to expand.
    expect(called("files_expand")).toEqual([]);
  });

  it("does nothing in the File view for a drop outside the tree or on a file", async () => {
    renderView();
    await tree();
    await waitFor(() => expect(webview.handlers.size).toBe(1));
    pointerIsOver(document.body);
    emit(drop(10, 10, ["/Users/me/a.png"]));
    pointerIsOver(item("notes.txt"));
    emit(drop(10, 10, ["/Users/me/a.png"]));
    expect(called("files_copy_in")).toEqual([]);
    expect(toast.loading).not.toHaveBeenCalled();
    expect(highlighted()).toEqual([]);
  });

  it("turns the progress toast into the error toast with the reason of a refused drop, and of a failure", async () => {
    renderView();
    await tree();
    await waitFor(() => expect(webview.handlers.size).toBe(1));
    const refusals = [
      '"a.png" is already in the project: drag it inside the tree to move it.',
      'Two dropped items are both named "a.png".',
      "Another copy is still running in this window.",
      'A file or folder named "a.png" already exists here.',
      ".weaver is written by Weaver: read-only here.",
      'Could not copy "a.png": disk full. Nothing was copied.',
    ];
    for (const reason of refusals) {
      toast.error.mockClear();
      failWith.files_copy_in = reason;
      pointerIsOver(item("docs"));
      emit(drop(10, 10, ["/Users/me/a.png"]));
      await waitFor(() => expect(toast.error).toHaveBeenCalledWith(reason, { id: "copy-toast" }));
    }
    expect(toast.dismiss).not.toHaveBeenCalled();
    expect(called("files_expand")).toEqual([]);
    expect(names()).toEqual([".weaver", "docs", "src", "README.md", "notes.txt"]);
  });

  it("closes the toast and leaves the tree alone when the copy ends after the view was left", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    await waitFor(() => expect(webview.handlers.size).toBe(1));
    const gate = deferred();
    copyGate = gate.promise;
    copied = ["docs/a.png"];
    pointerIsOver(item("docs"));
    emit(drop(10, 10, ["/Users/me/a.png"]));
    await user.click(screen.getByRole("button", { name: "Board" }));
    await waitFor(() => expect(webview.handlers.size).toBe(0));
    gate.resolve();
    await waitFor(() => expect(toast.dismiss).toHaveBeenCalledWith("copy-toast"));
    expect(called("files_expand")).toEqual([]);
  });
});

describe("moving with Move to…", () => {
  it("opens Move to… with the folders of the project read one at a time, and moves into the chosen one", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    const menu = await openMenu(item("README.md"));
    await user.click(within(menu).getByRole("menuitem", { name: "Move to…" }));
    const dialog = await screen.findByRole("dialog", { name: 'Move "README.md" to…' });
    const folders = within(dialog).getByRole("tree", { name: "Folders of the project" });
    await waitFor(() => expect(within(folders).getByRole("treeitem", { name: "src" })).toBeInTheDocument());
    expect(called("files_folders")).toEqual([{ projectId: PROJECT, path: "" }]);
    expect(within(folders).getAllByRole("treeitem").map((row) => row.textContent)).toEqual(["Project folder", ".weaver", "docs", "src"]);
    // Everything but where it is, and .weaver, can be chosen.
    expect(within(folders).getByRole("treeitem", { name: "Project folder" })).toHaveAttribute("aria-disabled", "true");
    expect(within(folders).getByRole("treeitem", { name: ".weaver" })).toHaveAttribute("aria-disabled", "true");
    const move = within(dialog).getByRole("button", { name: "Move" });
    expect(move).toHaveAttribute("aria-disabled", "true");
    // Expanding reads that folder only.
    const src = within(folders).getByRole("treeitem", { name: "src" });
    src.focus();
    await user.keyboard("{ArrowRight}");
    await waitFor(() => expect(within(folders).getByRole("treeitem", { name: "deep" })).toBeInTheDocument());
    expect(called("files_folders")).toEqual([
      { projectId: PROJECT, path: "" },
      { projectId: PROJECT, path: "src" },
    ]);
    await user.keyboard("{ArrowRight}{Enter}");
    expect(within(folders).getByRole("treeitem", { name: "deep" })).toHaveAttribute("aria-selected", "true");
    expect(move).not.toHaveAttribute("aria-disabled");
    await user.click(move);
    await waitFor(() => expect(called("files_move")).toEqual([{ projectId: PROJECT, path: "README.md", target: "src/deep" }]));
    expect(tabs.moveUnder).toHaveBeenCalledWith("README.md", "src/deep/README.md");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("cancels Move to… without moving, and keeps the dialog when the move is refused", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    const menu = await openMenu(item("notes.txt"));
    await user.click(within(menu).getByRole("menuitem", { name: "Move to…" }));
    const dialog = await screen.findByRole("dialog");
    await user.click(await within(dialog).findByRole("treeitem", { name: "docs" }));
    failWith.files_move = "The agent is editing this file.";
    await user.click(within(dialog).getByRole("button", { name: "Move" }));
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith("The agent is editing this file."));
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(tabs.moveUnder).not.toHaveBeenCalled();
  });
});

describe("leaving the view", () => {
  it("closes the tabs and shows the other view at once when nothing is unsaved", async () => {
    const user = userEvent.setup();
    renderView();
    await tree();
    await user.click(screen.getByRole("button", { name: "Board" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByText("Board content")).toBeInTheDocument();
    expect(tabs.closeAll).toHaveBeenCalledTimes(1);
    expect(tabs.saveAll).not.toHaveBeenCalled();
  });

  it("asks Save changes to <n> files?, listing the paths, with Cancel last and focused", async () => {
    const user = userEvent.setup();
    tabs.unsavedPaths = ["src/main.ts", "notes.txt"];
    renderView();
    await tree();
    await user.click(screen.getByRole("button", { name: "Board" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Save changes to 2 files?" });
    expect(within(dialog).getAllByRole("listitem").map((entry) => entry.textContent)).toEqual(["src/main.ts", "notes.txt"]);
    const buttons = within(dialog).getAllByRole("button");
    expect(buttons.map((button) => button.textContent)).toEqual(["Save all", "Discard all", "Cancel"]);
    expect(buttons[1]).toHaveClass("text-destructive", "border-destructive-line");
    expect(buttons[2]).toHaveFocus();
    expect(screen.queryByText("Board content")).not.toBeInTheDocument();
    expect(tabs.closeAll).not.toHaveBeenCalled();
  });

  it("keeps the File view as it is with Cancel", async () => {
    const user = userEvent.setup();
    tabs.unsavedPaths = ["notes.txt"];
    renderView();
    await tree();
    await user.click(screen.getByRole("button", { name: "Board" }));
    expect(await screen.findByRole("alertdialog", { name: "Save changes to 1 file?" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.getByRole("tree", { name: "Project files" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Files" })).toHaveAttribute("aria-current", "page");
    expect(tabs.closeAll).not.toHaveBeenCalled();
    expect(tabs.saveAll).not.toHaveBeenCalled();
    expect(called("files_leave")).toEqual([]);
  });

  it("closes every tab without saving with Discard all, then shows the other view", async () => {
    const user = userEvent.setup();
    tabs.unsavedPaths = ["notes.txt"];
    renderView();
    await tree();
    await user.click(screen.getByRole("button", { name: "Board" }));
    await user.click(await screen.findByRole("button", { name: "Discard all" }));
    expect(tabs.saveAll).not.toHaveBeenCalled();
    expect(tabs.closeAll).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("Board content")).toBeInTheDocument();
    await waitFor(() => expect(called("files_leave")).toHaveLength(1));
  });

  it("saves every file with Save all and then shows the other view", async () => {
    const user = userEvent.setup();
    tabs.unsavedPaths = ["notes.txt", "src/main.ts"];
    renderView();
    await tree();
    await user.click(screen.getByRole("button", { name: "Board" }));
    await user.click(await screen.findByRole("button", { name: "Save all" }));
    await waitFor(() => expect(tabs.closeAll).toHaveBeenCalledTimes(1));
    expect(tabs.saveAll).toHaveBeenCalledTimes(1);
    expect(await screen.findByText("Board content")).toBeInTheDocument();
  });

  it("stays on the File view when a file cannot be saved: the other view is not shown and the tabs stay", async () => {
    const user = userEvent.setup();
    tabs.unsavedPaths = ["notes.txt", "src/main.ts"];
    tabs.saveAll.mockResolvedValue([{ path: "notes.txt", reason: "The file is read-only." }]);
    renderView();
    await tree();
    await user.click(screen.getByRole("button", { name: "Board" }));
    await user.click(await screen.findByRole("button", { name: "Save all" }));
    await waitFor(() => expect(tabs.saveAll).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(tabs.closeAll).not.toHaveBeenCalled();
    expect(screen.getByRole("tree", { name: "Project files" })).toBeInTheDocument();
    expect(screen.queryByText("Board content")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Files" })).toHaveAttribute("aria-current", "page");
    expect(called("files_leave")).toEqual([]);
  });
});
