import type { DragDropEvent } from "@tauri-apps/api/webview";
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { Asset } from "@/bindings/Asset";
import type { AssetsChanged } from "@/bindings/AssetsChanged";
import type { AssetsSnapshot } from "@/bindings/AssetsSnapshot";
import { LARGE_TEXT, SIZE_LIMIT } from "@/components/assets/asset-kinds";
import { RENAME_ONE } from "@/components/assets/asset-grid";
import { AssetsView } from "@/components/assets/assets-view";
import { AssetsProvider } from "@/components/assets/use-assets";
import { ProjectWindow } from "@/components/project-window";
import { TooltipProvider } from "@/components/ui/tooltip";
import type * as BackendModule from "@/lib/backend";
import { call, on } from "@/lib/backend";

const { toast, webview } = vi.hoisted(() => ({
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

const PROJECT = "0123456789abcdef";
const MB = 1024 * 1024;
const asset = (name: string, size = 100): Asset => ({ name, size });

const LIST = [asset("logo.png", Math.round(2.4 * MB)), asset("notes.txt", 12), asset("report.pdf", 2048), asset("Zebra.mov", 5)];

let generation: number;
let assets: Asset[];
let calls: [string, Record<string, unknown>][];
let failWith: Record<string, { code: string; message: string }>;
let pushEvent: (changed: AssetsChanged) => void;
/** What the next copy answers, and when. */
let copyAnswer: (args: Record<string, unknown>) => Promise<unknown>;
let deleteAnswer: (names: string[]) => { trashed: string[]; refused: { name: string; reason: string }[] };
/** Puts back what a test replaced of the layout jsdom does not do. */
let restoreLayout: (() => void) | null = null;

const snapshot = (list: Asset[] = assets): AssetsSnapshot => ({ generation: ++generation, assets: list });

function backend(name: string, args: Record<string, unknown>): Promise<unknown> {
  calls.push([name, args]);
  if (failWith[name]) return Promise.reject(failWith[name]);
  switch (name) {
    case "assets_list":
      return Promise.resolve({ generation, assets });
    case "assets_copy_in":
      return copyAnswer(args);
    case "assets_rename": {
      assets = assets.map((one) => (one.name === args.name ? { ...one, name: args.newName as string } : one));
      return Promise.resolve({ name: args.newName, snapshot: snapshot() });
    }
    case "assets_delete": {
      const answer = deleteAnswer(args.names as string[]);
      assets = assets.filter((one) => !answer.trashed.includes(one.name));
      return Promise.resolve({ ...answer, snapshot: snapshot() });
    }
    case "assets_drag_out":
      return Promise.resolve(null);
    case "project_status":
      return Promise.resolve({ state: "has-model" });
    default:
      return Promise.reject(new Error(`unexpected command ${name}`));
  }
}

const called = (name: string) => calls.filter(([n]) => n === name).map(([, args]) => args);

beforeEach(() => {
  generation = 1;
  assets = [...LIST];
  calls = [];
  failWith = {};
  copyAnswer = () => Promise.resolve({ copied: [], large: [], snapshot: snapshot() });
  deleteAnswer = (names) => ({ trashed: names, refused: [] });
  toast.error.mockReset();
  toast.warning.mockReset();
  toast.loading.mockReset().mockReturnValue("copy-toast");
  toast.dismiss.mockReset();
  webview.handlers.clear();
  document.elementFromPoint = vi.fn(() => null);
  vi.mocked(call).mockReset().mockImplementation(((name: string, args: Record<string, unknown>) => backend(name, args)) as unknown as typeof call);
  vi.mocked(on).mockImplementation(((name: string, handler: (changed: AssetsChanged) => void) => {
    if (name === "assets-changed") pushEvent = handler;
    return Promise.resolve(() => {});
  }) as unknown as typeof on);
});

afterEach(() => {
  restoreLayout?.();
  restoreLayout = null;
});

function renderView() {
  return render(
    <TooltipProvider>
      <ProjectWindow
        projectId={PROJECT}
        initialView="assets"
        views={{ assets: <AssetsView />, board: <p>Board content</p> }}
        around={(children) => <AssetsProvider projectId={PROJECT}>{children}</AssetsProvider>}
      />
    </TooltipProvider>,
  );
}

const grid = () => screen.findByRole("grid", { name: "Assets" });
// While a menu is open the rest of the page is hidden from assistive technologies, but the tiles are still there.
const tiles = () => screen.getAllByRole("gridcell", { hidden: true });
const names = () => tiles().map((tile) => tile.dataset.name);
const tile = (name: string) => {
  const found = tiles().find((one) => one.dataset.name === name);
  if (!found) throw new Error(`no tile ${name}`);
  return found;
};
const selected = () => tiles().filter((one) => one.getAttribute("aria-selected") === "true").map((one) => one.dataset.name);

const changed = (list: Asset[], at = ++generation) => act(() => pushEvent({ projectId: PROJECT, generation: at, assets: list }));

/** The events of the webview's drag and drop handler (positions in physical pixels). */
const emit = (payload: DragDropEvent) =>
  act(() => {
    for (const handler of webview.handlers) handler({ payload });
  });
const over = () => emit({ type: "over", position: { x: 20, y: 20 } } as DragDropEvent);
const drop = (paths: string[]) => emit({ type: "drop", paths, position: { x: 20, y: 20 } } as DragDropEvent);
/** The pointer is over the view (its heading is inside it). */
const pointerOverView = () => {
  document.elementFromPoint = vi.fn(() => screen.getByRole("heading", { name: "Assets" }));
};

const platform = (name: string) => vi.spyOn(window.navigator, "platform", "get").mockReturnValue(name);

describe("the Assets entry", () => {
  it("shows the heading and the assets in name order, each with the icon of its kind, nothing selected", async () => {
    renderView();
    await grid();
    expect(screen.getByRole("heading", { level: 1, name: "Assets" })).toBeInTheDocument();
    expect(names()).toEqual(["logo.png", "notes.txt", "report.pdf", "Zebra.mov"]);
    expect(selected()).toEqual([]);
    const icons = tiles().map((one) => one.querySelector("svg")?.getAttribute("class") ?? "");
    expect(icons[0]).toContain("lucide-file-image");
    expect(icons[1]).toContain("lucide-file-text");
    expect(icons[2]).toContain("lucide-file-text");
    expect(icons[3]).toContain("lucide-file-video");
    expect(screen.getByRole("grid")).toHaveAttribute("aria-multiselectable", "true");
    expect(called("assets_list")).toEqual([{ projectId: PROJECT }]);
  });

  it("gives each tile a tooltip with the full name and the size, at once on the keyboard focus", async () => {
    renderView();
    await grid();
    act(() => tile("logo.png").focus());
    expect(await screen.findByRole("tooltip")).toHaveTextContent("logo.png — 2.4 MB");
  });

  it("says how to add assets when there are none, with the file manager of the system", async () => {
    assets = [];
    platform("MacIntel");
    renderView();
    expect(await screen.findByText("No assets yet. Drag files here from the Finder or Explorer.")).toBeInTheDocument();
    expect(screen.queryByRole("grid")).not.toBeInTheDocument();
  });

  it("names File Explorer on Windows and the file manager on Linux", async () => {
    assets = [];
    platform("Win32");
    const first = renderView();
    expect(await screen.findByText("No assets yet. Drag files here from File Explorer.")).toBeInTheDocument();
    first.unmount();
    platform("Linux x86_64");
    renderView();
    expect(await screen.findByText("No assets yet. Drag files here from your file manager.")).toBeInTheDocument();
  });

  it("marks the assets larger than 10 MB, at the limit and not below, with the accessible name and tooltip of the mark", async () => {
    assets = [asset("exact.bin", SIZE_LIMIT), asset("over.bin", SIZE_LIMIT + 1), asset("small.txt", 3)];
    renderView();
    await grid();
    const marks = screen.getAllByRole("img", { name: LARGE_TEXT });
    expect(marks).toHaveLength(1);
    expect(tile("over.bin")).toContainElement(marks[0] as HTMLElement);
    expect(tile("exact.bin").querySelector('[role="img"]')).toBeNull();
    expect(marks[0]?.querySelector("svg")?.getAttribute("class")).toContain("lucide-triangle-alert");
    expect(marks[0]?.className).toContain("text-peach-ink");
    expect(LARGE_TEXT).toBe("Larger than 10 MB: it goes into the repository.");
  });

  it("follows the assets the backend tells, marks included, and an asset that is gone leaves the selection", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("notes.txt"));
    await user.keyboard("{Control>}");
    await user.click(tile("report.pdf"));
    await user.keyboard("{/Control}");
    expect(selected()).toEqual(["notes.txt", "report.pdf"]);

    changed([asset("logo.png", SIZE_LIMIT + 5), asset("report.pdf", 2048), asset("new.txt", 1)]);
    expect(names()).toEqual(["logo.png", "new.txt", "report.pdf"]);
    expect(selected()).toEqual(["report.pdf"]);
    expect(within(tile("logo.png")).getByRole("img", { name: LARGE_TEXT })).toBeInTheDocument();

    // A list older than the one held is ignored.
    changed([asset("old.txt")], 1);
    expect(names()).toEqual(["logo.png", "new.txt", "report.pdf"]);
  });

  it("forgets the selection when another view is chosen and shows the assets as they are when it comes back", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("logo.png"));
    expect(selected()).toEqual(["logo.png"]);
    await user.click(screen.getByRole("button", { name: "Board" }));
    expect(screen.getByText("Board content")).toBeInTheDocument();
    changed([...LIST, asset("while-away.txt")]);
    await user.click(screen.getByRole("button", { name: "Assets" }));
    expect(selected()).toEqual([]);
    expect(names()).toContain("while-away.txt");
  });
});

describe("selecting", () => {
  it("selects one tile with a click, and a click on the empty space clears the selection", async () => {
    const user = userEvent.setup();
    renderView();
    const area = await grid();
    await user.click(tile("notes.txt"));
    expect(selected()).toEqual(["notes.txt"]);
    await user.click(tile("report.pdf"));
    expect(selected()).toEqual(["report.pdf"]);
    await user.click(area);
    expect(selected()).toEqual([]);
  });

  it("adds and removes tiles with Ctrl+click, and selects the range with Shift+click", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("logo.png"));
    await user.keyboard("{Control>}");
    await user.click(tile("report.pdf"));
    expect(selected()).toEqual(["logo.png", "report.pdf"]);
    await user.click(tile("logo.png"));
    expect(selected()).toEqual(["report.pdf"]);
    await user.keyboard("{/Control}");
    await user.click(tile("logo.png"));
    await user.keyboard("{Shift>}");
    await user.click(tile("report.pdf"));
    await user.keyboard("{/Shift}");
    expect(selected()).toEqual(["logo.png", "notes.txt", "report.pdf"]);
  });

  it("uses Cmd+click on macOS, where Ctrl+click is a right click", async () => {
    platform("MacIntel");
    renderView();
    await grid();
    fireEvent.click(tile("logo.png"));
    fireEvent.click(tile("report.pdf"), { metaKey: true });
    expect(selected()).toEqual(["logo.png", "report.pdf"]);
    fireEvent.click(tile("notes.txt"), { ctrlKey: true });
    expect(selected()).toEqual(["notes.txt"]);
  });

  it("moves the active tile with the arrow keys, extends with Shift, and goes to the first and the last", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    act(() => tile("logo.png").focus());
    expect(selected()).toEqual([]);
    await user.keyboard("{ArrowRight}");
    expect(selected()).toEqual(["notes.txt"]);
    expect(tile("notes.txt")).toHaveFocus();
    await user.keyboard("{Shift>}{ArrowRight}{/Shift}");
    expect(selected()).toEqual(["notes.txt", "report.pdf"]);
    await user.keyboard("{End}");
    expect(selected()).toEqual(["Zebra.mov"]);
    await user.keyboard("{ArrowLeft}{Home}");
    expect(selected()).toEqual(["logo.png"]);
  });

  it("moves a row at a time with Up and Down, the row being as many tiles as fit the width", async () => {
    // Two tiles to a row: the first two share a top, the next two sit lower.
    const tops: Record<string, number> = { "logo.png": 0, "notes.txt": 0, "report.pdf": 100, "Zebra.mov": 100 };
    const original = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "offsetTop");
    Object.defineProperty(HTMLElement.prototype, "offsetTop", {
      configurable: true,
      get(this: HTMLElement) {
        return tops[this.dataset.name ?? ""] ?? 0;
      },
    });
    restoreLayout = () => {
      if (original) Object.defineProperty(HTMLElement.prototype, "offsetTop", original);
      else Reflect.deleteProperty(HTMLElement.prototype, "offsetTop");
    };
    const user = userEvent.setup();
    renderView();
    await grid();
    act(() => tile("notes.txt").focus());
    await user.keyboard("{ArrowDown}");
    expect(selected()).toEqual(["Zebra.mov"]);
    await user.keyboard("{ArrowUp}");
    expect(selected()).toEqual(["notes.txt"]);
    await user.keyboard("{Shift>}{ArrowDown}{/Shift}");
    expect(selected()).toEqual(["notes.txt", "report.pdf", "Zebra.mov"]);
  });

  it("selects every asset with Ctrl+A", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    act(() => tile("report.pdf").focus());
    await user.keyboard("{Control>}a{/Control}");
    expect(selected()).toEqual(["logo.png", "notes.txt", "report.pdf", "Zebra.mov"]);
  });

  it("selects the tile with Enter or Space and opens nothing, a double click included", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    act(() => tile("report.pdf").focus());
    await user.keyboard("{Enter}");
    expect(selected()).toEqual(["report.pdf"]);
    act(() => tile("notes.txt").focus());
    await user.keyboard(" ");
    expect(selected()).toEqual(["notes.txt"]);
    await user.dblClick(tile("logo.png"));
    expect(selected()).toEqual(["logo.png"]);
    // No command of the view ran for it: a click or a double click opens nothing.
    expect(calls.map(([name]) => name).filter((name) => name.startsWith("assets_") && name !== "assets_list")).toEqual([]);
  });

  it("keeps one tile in the tab order, the first until another is active", async () => {
    renderView();
    await grid();
    expect(tiles().map((one) => one.tabIndex)).toEqual([0, -1, -1, -1]);
    act(() => tile("report.pdf").focus());
    expect(tiles().map((one) => one.tabIndex)).toEqual([-1, -1, 0, -1]);
    expect(selected()).toEqual([]);
  });
});

describe("the menu of a tile", () => {
  const openMenu = async (target: HTMLElement) => {
    fireEvent.contextMenu(target);
    return screen.findByRole("menu");
  };

  it("has Rename and Delete and nothing else, acting on the selection", async () => {
    renderView();
    await grid();
    const menu = await openMenu(tile("notes.txt"));
    expect(within(menu).getAllByRole("menuitem").map((item) => item.textContent)).toEqual(["Rename", "Delete"]);
    expect(within(menu).getByRole("menuitem", { name: "Rename" })).not.toHaveAttribute("aria-disabled");
  });

  it("selects a tile outside the selection alone first, and keeps the selection when the tile is in it", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("logo.png"));
    await openMenu(tile("report.pdf"));
    expect(selected()).toEqual(["report.pdf"]);
    await user.keyboard("{Escape}");
    await user.keyboard("{Control>}");
    await user.click(tile("notes.txt"));
    await user.keyboard("{/Control}");
    expect(selected()).toEqual(["notes.txt", "report.pdf"]);
    await openMenu(tile("report.pdf"));
    expect(selected()).toEqual(["notes.txt", "report.pdf"]);
  });

  it("shows Rename disabled with the reason when more than one asset is selected", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("logo.png"));
    await user.keyboard("{Control>}");
    await user.click(tile("notes.txt"));
    await user.keyboard("{/Control}");
    const menu = await openMenu(tile("logo.png"));
    const rename = within(menu).getByRole("menuitem", { name: "Rename" });
    expect(rename).toHaveAttribute("aria-disabled", "true");
    expect(rename).toHaveAccessibleDescription(RENAME_ONE);
    await user.click(rename);
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("opens with the Menu key and with Shift+F10 on the active tile", async () => {
    renderView();
    await grid();
    act(() => tile("notes.txt").focus());
    fireEvent.keyDown(tile("notes.txt"), { key: "ContextMenu" });
    expect(await screen.findByRole("menu")).toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("menu")).not.toBeInTheDocument());
    act(() => tile("notes.txt").focus());
    fireEvent.keyDown(tile("notes.txt"), { key: "F10", shiftKey: true });
    expect(await screen.findByRole("menu")).toBeInTheDocument();
  });
});

describe("renaming", () => {
  const field = (name: string) => screen.getByRole("textbox", { name: `New name for ${name}` });

  it("replaces the name with a field that has it selected without its extension, and F2 starts it on the selected asset", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("notes.txt"));
    await user.keyboard("{F2}");
    const input = field("notes.txt") as HTMLInputElement;
    expect(input).toHaveValue("notes.txt");
    expect(input).toHaveFocus();
    expect([input.selectionStart, input.selectionEnd]).toEqual([0, 5]);
    expect(within(tile("notes.txt")).queryByText("notes.txt")).not.toBeInTheDocument();
  });

  it("does nothing on F2 with no asset or several assets selected", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    act(() => tile("notes.txt").focus());
    await user.keyboard("{F2}");
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    await user.keyboard("{Control>}a{/Control}{F2}");
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("renames on Enter, keeps the asset selected under its new name and sends the names, never paths", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("notes.txt"));
    await user.keyboard("{F2}");
    await user.clear(field("notes.txt"));
    await user.type(field("notes.txt"), "ideas.txt{Enter}");
    await waitFor(() => expect(names()).toContain("ideas.txt"));
    expect(called("assets_rename")).toEqual([{ projectId: PROJECT, name: "notes.txt", newName: "ideas.txt" }]);
    expect(names()).not.toContain("notes.txt");
    expect(selected()).toEqual(["ideas.txt"]);
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(toast.error).not.toHaveBeenCalled();
  });

  it("renames from the menu", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    fireEvent.contextMenu(tile("report.pdf"));
    await user.click(await screen.findByRole("menuitem", { name: "Rename" }));
    await user.clear(field("report.pdf"));
    await user.type(field("report.pdf"), "summary.pdf{Enter}");
    await waitFor(() => expect(names()).toContain("summary.pdf"));
    expect(called("assets_rename")).toEqual([{ projectId: PROJECT, name: "report.pdf", newName: "summary.pdf" }]);
  });

  it("shows the reason of a refusal in a toast, keeps the field for the name to be corrected and changes nothing", async () => {
    failWith.assets_rename = { code: "refused", message: 'An asset named "report.pdf" already exists.' };
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("notes.txt"));
    await user.keyboard("{F2}");
    await user.clear(field("notes.txt"));
    await user.type(field("notes.txt"), "report.pdf{Enter}");
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('An asset named "report.pdf" already exists.'));
    expect(field("notes.txt")).toHaveValue("report.pdf");
    expect(names()).toEqual(["logo.png", "notes.txt", "report.pdf", "Zebra.mov"]);
    // Corrected, it goes through.
    delete failWith.assets_rename;
    await user.clear(field("notes.txt"));
    await user.type(field("notes.txt"), "ideas.txt{Enter}");
    await waitFor(() => expect(names()).toContain("ideas.txt"));
  });

  it("cancels with Esc, and by leaving the field, without asking the backend", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("notes.txt"));
    await user.keyboard("{F2}");
    await user.type(field("notes.txt"), "{Escape}");
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(tile("notes.txt")).toHaveFocus();
    await user.keyboard("{F2}");
    await user.type(field("notes.txt"), "x");
    await user.click(tile("logo.png"));
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(called("assets_rename")).toEqual([]);
    expect(names()).toContain("notes.txt");
  });

  it("does not ask for a rename when the name is unchanged", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("notes.txt"));
    await user.keyboard("{F2}{Enter}");
    expect(called("assets_rename")).toEqual([]);
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });
});

describe("deleting", () => {
  it("asks first, by name for one asset, and sends it to the trash only when confirmed", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("notes.txt"));
    await user.keyboard("{Delete}");
    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText('Move "notes.txt" to the trash?')).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    expect(called("assets_delete")).toEqual([]);
    await user.click(within(dialog).getByRole("button", { name: "Move to Trash" }));
    await waitFor(() => expect(names()).toEqual(["logo.png", "report.pdf", "Zebra.mov"]));
    expect(called("assets_delete")).toEqual([{ projectId: PROJECT, names: ["notes.txt"] }]);
    expect(selected()).toEqual([]);
    expect(toast.error).not.toHaveBeenCalled();
  });

  it("lists the names of several assets in the question", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("logo.png"));
    await user.keyboard("{Shift>}");
    await user.click(tile("report.pdf"));
    await user.keyboard("{/Shift}{Delete}");
    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText("Move 3 assets to the trash?")).toBeInTheDocument();
    expect(within(dialog).getAllByRole("listitem").map((item) => item.textContent)).toEqual(["logo.png", "notes.txt", "report.pdf"]);
    await user.click(within(dialog).getByRole("button", { name: "Move to Trash" }));
    await waitFor(() => expect(names()).toEqual(["Zebra.mov"]));
    expect(called("assets_delete")).toEqual([{ projectId: PROJECT, names: ["logo.png", "notes.txt", "report.pdf"] }]);
  });

  it("changes nothing when cancelled, and the focus goes back to a tile", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("notes.txt"));
    await user.keyboard("{Delete}");
    await user.click(await screen.findByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(called("assets_delete")).toEqual([]);
    expect(selected()).toEqual(["notes.txt"]);
    expect(tiles()).toContain(document.activeElement);
  });

  it("deletes from the menu, and on macOS with Cmd+Backspace", async () => {
    platform("MacIntel");
    const user = userEvent.setup();
    renderView();
    await grid();
    fireEvent.contextMenu(tile("Zebra.mov"));
    await user.click(await screen.findByRole("menuitem", { name: "Delete" }));
    expect(await screen.findByText('Move "Zebra.mov" to the trash?')).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    act(() => tile("notes.txt").focus());
    await user.keyboard("{Enter}");
    await user.keyboard("{Meta>}{Backspace}{/Meta}");
    expect(await screen.findByText('Move "notes.txt" to the trash?')).toBeInTheDocument();
  });

  it("does nothing on Delete with nothing selected", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    act(() => tile("notes.txt").focus());
    await user.keyboard("{Delete}");
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("tells which asset the trash refused and why, and the others are gone", async () => {
    deleteAnswer = (requested) => ({
      trashed: requested.filter((name) => name !== "notes.txt"),
      refused: [{ name: "notes.txt", reason: "the trash is full" }],
    });
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("logo.png"));
    await user.keyboard("{Shift>}");
    await user.click(tile("notes.txt"));
    await user.keyboard("{/Shift}{Delete}");
    await user.click(await screen.findByRole("button", { name: "Move to Trash" }));
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('Could not move "notes.txt" to the trash: the trash is full'));
    expect(names()).toEqual(["notes.txt", "report.pdf", "Zebra.mov"]);
    expect(selected()).toEqual(["notes.txt"]);
  });

  it("counts the assets the trash refused when there are several, with the reasons", async () => {
    deleteAnswer = () => ({
      trashed: [],
      refused: [
        { name: "logo.png", reason: "no trash" },
        { name: "notes.txt", reason: "no trash" },
        { name: "report.pdf", reason: "read-only" },
      ],
    });
    const user = userEvent.setup();
    renderView();
    await grid();
    act(() => tile("logo.png").focus());
    await user.keyboard("{Control>}a{/Control}{Delete}");
    await user.click(await screen.findByRole("button", { name: "Move to Trash" }));
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith("Could not move 3 assets to the trash: no trash; read-only"));
    expect(names()).toHaveLength(4);
  });
});

describe("dropping files from outside", () => {
  it("copies them, selects the copied assets and scrolls to the first", async () => {
    pointerOverView();
    copyAnswer = () => Promise.resolve({ copied: ["b-new.png", "a-new.txt"], large: [], snapshot: snapshot([...LIST, asset("a-new.txt"), asset("b-new.png")]) });
    renderView();
    await grid();
    await drop(["/Users/me/b-new.png", "/Users/me/a-new.txt"]);
    await waitFor(() => expect(selected()).toEqual(["a-new.txt", "b-new.png"]));
    expect(called("assets_copy_in")).toEqual([{ projectId: PROJECT, paths: ["/Users/me/b-new.png", "/Users/me/a-new.txt"] }]);
    expect(names()).toEqual(["a-new.txt", "b-new.png", "logo.png", "notes.txt", "report.pdf", "Zebra.mov"]);
    expect(toast.warning).not.toHaveBeenCalled();
    expect(toast.error).not.toHaveBeenCalled();
  });

  it("says the copy is running while it runs, and the view stays usable", async () => {
    pointerOverView();
    let finish: () => void = () => {};
    copyAnswer = () =>
      new Promise((resolve) => {
        finish = () => resolve({ copied: ["a.txt"], large: [], snapshot: snapshot([...LIST, asset("a.txt")]) });
      });
    const user = userEvent.setup();
    renderView();
    await grid();
    await drop(["/x/a.txt", "/x/b.txt"]);
    await waitFor(() => expect(toast.loading).toHaveBeenCalledWith("Copying 2 files…"));
    await user.click(tile("notes.txt"));
    expect(selected()).toEqual(["notes.txt"]);
    expect(toast.dismiss).not.toHaveBeenCalled();
    await act(async () => finish());
    await waitFor(() => expect(toast.dismiss).toHaveBeenCalledWith("copy-toast"));
    expect(selected()).toEqual(["a.txt"]);
  });

  it("says Copying 1 file… for one file", async () => {
    pointerOverView();
    let finish: () => void = () => {};
    copyAnswer = () =>
      new Promise((resolve) => {
        finish = () => resolve({ copied: [], large: [], snapshot: snapshot() });
      });
    renderView();
    await grid();
    await drop(["/x/a.txt"]);
    await waitFor(() => expect(toast.loading).toHaveBeenCalledWith("Copying 1 file…"));
    await act(async () => finish());
  });

  it("warns about a copied file over the limit, naming it, and about several together", async () => {
    pointerOverView();
    copyAnswer = () => Promise.resolve({ copied: ["big.bin"], large: ["big.bin"], snapshot: snapshot([...LIST, asset("big.bin", SIZE_LIMIT + 1)]) });
    renderView();
    await grid();
    await drop(["/x/big.bin"]);
    await waitFor(() => expect(toast.warning).toHaveBeenCalledWith('"big.bin" is larger than 10 MB: it goes into the repository.'));
    expect(within(tile("big.bin")).getByRole("img", { name: LARGE_TEXT })).toBeInTheDocument();

    copyAnswer = () =>
      Promise.resolve({ copied: ["c.bin", "d.bin"], large: ["c.bin", "d.bin"], snapshot: snapshot([...assets, asset("c.bin", SIZE_LIMIT * 2), asset("d.bin", SIZE_LIMIT * 3)]) });
    await drop(["/x/c.bin", "/x/d.bin"]);
    await waitFor(() => expect(toast.warning).toHaveBeenCalledWith("2 copied files are larger than 10 MB: they go into the repository."));
  });

  it("shows the reason of a refusal in an error toast, and nothing changes", async () => {
    pointerOverView();
    failWith.assets_copy_in = { code: "refused", message: '"pictures" is a folder: only files can be assets.' };
    renderView();
    await grid();
    await drop(["/x/pictures"]);
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith('"pictures" is a folder: only files can be assets.'));
    expect(toast.loading).not.toHaveBeenCalled();
    expect(names()).toEqual(["logo.png", "notes.txt", "report.pdf", "Zebra.mov"]);
    expect(selected()).toEqual([]);
  });

  it("turns the loading toast into the error toast when a copy that was running fails", async () => {
    pointerOverView();
    let fail: () => void = () => {};
    copyAnswer = () =>
      new Promise((_, reject) => {
        fail = () => reject({ code: "refused", message: 'Could not copy "a.txt": the disk is full. Nothing was copied.' });
      });
    renderView();
    await grid();
    await drop(["/x/a.txt"]);
    await waitFor(() => expect(toast.loading).toHaveBeenCalled());
    await act(async () => fail());
    await waitFor(() =>
      expect(toast.error).toHaveBeenCalledWith('Could not copy "a.txt": the disk is full. Nothing was copied.', { id: "copy-toast" }),
    );
  });

  it("shows nothing for the project's own assets dropped back onto the view", async () => {
    pointerOverView();
    renderView();
    await grid();
    await drop(["/project/.weaver/assets/notes.txt"]);
    await waitFor(() => expect(called("assets_copy_in")).toHaveLength(1));
    // Give a toast the time it would need to appear.
    await new Promise((resolve) => setTimeout(resolve, 250));
    expect(toast.loading).not.toHaveBeenCalled();
    expect(toast.dismiss).not.toHaveBeenCalled();
    expect(toast.error).not.toHaveBeenCalled();
    expect(toast.warning).not.toHaveBeenCalled();
    expect(selected()).toEqual([]);
  });

  it("ignores a drop that is not on the view", async () => {
    renderView();
    await grid();
    await drop(["/x/a.txt"]);
    expect(called("assets_copy_in")).toEqual([]);
  });

  it("highlights the whole grid area with a dashed border while files are over it, and stops when they leave", async () => {
    pointerOverView();
    renderView();
    const area = (await grid()).closest("[data-drop-target], div.relative") as HTMLElement;
    expect(area).not.toHaveAttribute("data-drop-target");
    await over();
    expect(area).toHaveAttribute("data-drop-target");
    expect(area.querySelector(".border-dashed")).not.toBeNull();
    await emit({ type: "leave" } as DragDropEvent);
    expect(area).not.toHaveAttribute("data-drop-target");
    expect(area.querySelector(".border-dashed")).toBeNull();
    await over();
    await drop(["/x/a.txt"]);
    expect(area).not.toHaveAttribute("data-drop-target");
  });

  it("highlights nothing when the files are not over the view", async () => {
    renderView();
    const area = (await grid()).closest("div.relative") as HTMLElement;
    await over();
    expect(area).not.toHaveAttribute("data-drop-target");
  });

  it("listens to the drag and drop of the window only while the view is shown", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await waitFor(() => expect(webview.handlers.size).toBe(1));
    await user.click(screen.getByRole("button", { name: "Board" }));
    await waitFor(() => expect(webview.handlers.size).toBe(0));
    await user.click(screen.getByRole("button", { name: "Assets" }));
    await waitFor(() => expect(webview.handlers.size).toBe(1));
  });

  it("goes on to its end when the view is left, and the copied assets are not selected in a view that is not shown", async () => {
    pointerOverView();
    let finish: () => void = () => {};
    copyAnswer = () =>
      new Promise((resolve) => {
        finish = () => resolve({ copied: ["big.bin"], large: ["big.bin"], snapshot: snapshot([...LIST, asset("big.bin", SIZE_LIMIT + 1)]) });
      });
    const user = userEvent.setup();
    renderView();
    await grid();
    await drop(["/x/big.bin"]);
    await waitFor(() => expect(toast.loading).toHaveBeenCalled());
    await user.click(screen.getByRole("button", { name: "Board" }));
    await act(async () => finish());
    await waitFor(() => expect(toast.dismiss).toHaveBeenCalledWith("copy-toast"));
    expect(toast.warning).toHaveBeenCalledWith('"big.bin" is larger than 10 MB: it goes into the repository.');
    await user.click(screen.getByRole("button", { name: "Assets" }));
    expect(names()).toContain("big.bin");
    expect(selected()).toEqual([]);
  });
});

describe("dragging assets out", () => {
  const press = (target: HTMLElement, x: number, y: number) => fireEvent.pointerDown(target, { pointerId: 1, button: 0, clientX: x, clientY: y });
  const move = (target: HTMLElement, x: number, y: number) => fireEvent.pointerMove(target, { pointerId: 1, clientX: x, clientY: y });

  it("starts after 4 px with the selection when the tile is one of its tiles", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("logo.png"));
    await user.keyboard("{Control>}");
    await user.click(tile("report.pdf"));
    await user.keyboard("{/Control}");
    press(tile("report.pdf"), 10, 10);
    move(tile("report.pdf"), 12, 10);
    expect(called("assets_drag_out")).toEqual([]);
    move(tile("report.pdf"), 15, 10);
    expect(called("assets_drag_out")).toEqual([{ projectId: PROJECT, names: ["logo.png", "report.pdf"] }]);
    // The grid does not change.
    expect(names()).toEqual(["logo.png", "notes.txt", "report.pdf", "Zebra.mov"]);
    expect(selected()).toEqual(["logo.png", "report.pdf"]);
  });

  it("carries the tile alone when it is not selected, and starts once per press", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("logo.png"));
    press(tile("Zebra.mov"), 0, 0);
    move(tile("Zebra.mov"), 0, 6);
    move(tile("Zebra.mov"), 0, 20);
    expect(called("assets_drag_out")).toEqual([{ projectId: PROJECT, names: ["Zebra.mov"] }]);
  });

  it("stays a click when the pointer moves less than 4 px", async () => {
    renderView();
    await grid();
    press(tile("notes.txt"), 10, 10);
    move(tile("notes.txt"), 11, 12);
    fireEvent.pointerUp(tile("notes.txt"), { pointerId: 1, clientX: 11, clientY: 12 });
    fireEvent.click(tile("notes.txt"));
    expect(called("assets_drag_out")).toEqual([]);
    expect(selected()).toEqual(["notes.txt"]);
  });

  it("ignores the click that follows a drag, and a press that is not the primary button", async () => {
    const user = userEvent.setup();
    renderView();
    await grid();
    await user.click(tile("logo.png"));
    press(tile("notes.txt"), 0, 0);
    move(tile("notes.txt"), 10, 0);
    fireEvent.click(tile("notes.txt"));
    expect(selected()).toEqual(["logo.png"]);
    fireEvent.pointerDown(tile("report.pdf"), { pointerId: 2, button: 2, clientX: 0, clientY: 0 });
    fireEvent.pointerMove(tile("report.pdf"), { pointerId: 2, clientX: 30, clientY: 0 });
    expect(called("assets_drag_out")).toHaveLength(1);
  });

  it("shows the reason in a toast when the drag cannot start", async () => {
    failWith.assets_drag_out = { code: "internal", message: "The drag could not start: no window." };
    renderView();
    await grid();
    press(tile("notes.txt"), 0, 0);
    move(tile("notes.txt"), 20, 0);
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith("The drag could not start: no window."));
  });
});
