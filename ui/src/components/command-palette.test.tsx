import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { CommandPalette } from "@/components/command-palette";
import { ProjectContext, WindowControlsContext, type WindowControls } from "@/components/project-context";
import { ProjectWindow } from "@/components/project-window";
import { useWindowControls } from "@/components/project-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn((name: string) => Promise.resolve(name === "startup_failures" ? [] : null)),
  on: vi.fn(() => Promise.resolve(() => {})),
  isAppError: () => false,
}));
vi.mock("sonner", () => ({ toast: { error: vi.fn() } }));
vi.mock("@/hooks/use-omp", () => ({ useOmp: () => ({ status: null }) }));

const data = vi.hoisted(() => ({
  runAllReasons: [] as string[],
  inProgress: false,
  runAll: vi.fn(),
  newRequest: vi.fn(),
  openEditor: vi.fn(),
}));
const card = (id: string, shortNumber: string, title: string) => ({ kind: "proposal", head: { id, shortNumber, title } });
const draft = (id: string, title: string) => ({ kind: "request", id, title, created: "2026-11-02T07:00:00", empty: false, returned: false });
vi.mock("@/components/board/board-state", () => ({
  useBoard: () => ({
    snapshot: {
      runAllReasons: data.runAllReasons,
      backlog: [card("p1", "P1", "Add a cart"), draft("r1", "Dark mode Cart theme"), draft("r2", "Untitled request"), ...Array.from({ length: 9 }, (_, index) => draft(`m${index}`, `Mobile ${index}`))],
      active: [card("t1", "T1", "Cart total task")],
      // Ghosts are not cards: the palette never lists them.
      ghosts: [{ id: "g1", jobId: "j1", kind: "proposal", title: "Cart ghost", phase: "preparing", note: null }],
    },
  }),
}));
vi.mock("@/components/requests/use-requests", () => ({ useRequests: () => ({ openEditor: data.openEditor }) }));
vi.mock("@/components/board/board-ui", () => ({ useBoardActions: () => ({ runAll: data.runAll, newRequest: data.newRequest }) }));
vi.mock("@/components/merge/use-merge", () => ({ useMerge: () => ({ merge: { inProgress: data.inProgress } }) }));
vi.mock("@/components/history/use-history", () => ({
  useHistory: () => ({ snapshot: { entries: [{ id: "h1", title: "Cart added", updated: "2026-11-02T07:00:00.000Z" }] } }),
}));
vi.mock("@/components/views/views-state", () => ({
  useViews: () => ({ snapshot: { data: { requirements: [{ id: "ui", title: "UI", fragments: [{ name: "views", title: "Views", sections: [{ title: "cart" }] }] }] } } }),
}));
vi.mock("@/components/project-switcher", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/components/project-switcher")>()),
  useProjectList: () => ({
    list: { open: [{ id: "cur", path: "/work/alpha", name: "alpha", working: 0 }, { id: "b", path: "/work/beta", name: "beta", working: 1 }], recent: [{ path: "/old/gone", name: "gone", opened: "", exists: false }] },
    reload: () => {},
  }),
}));

const PROJECT = { projectId: "cur", showView: vi.fn(), openJob: vi.fn(), openedJob: null, openedJobRequest: 0 };
const controls = (patch: Partial<WindowControls> = {}): WindowControls => ({
  openUsage: vi.fn(),
  palette: { open: true, origin: null, show: vi.fn(), close: vi.fn() },
  reveal: vi.fn(),
  revealed: null,
  revealHandled: vi.fn(),
  ...patch,
});

function renderPalette(value: WindowControls) {
  return render(
    <ProjectContext.Provider value={PROJECT}>
      <WindowControlsContext.Provider value={value}>
        <CommandPalette />
      </WindowControlsContext.Provider>
    </ProjectContext.Provider>,
  );
}
const groups = () => screen.getAllByRole("group").map((group) => group.textContent?.slice(0, 5));

beforeEach(() => {
  data.runAllReasons = [];
  data.inProgress = false;
  for (const mock of [data.runAll, data.newRequest, data.openEditor, PROJECT.showView]) mock.mockClear();
});

describe("the palette", () => {
  it("is a Dialog with a field `Search or run…` and, with the field empty, only Views, Actions and Projects", () => {
    renderPalette(controls());
    expect(screen.getByRole("dialog")).toHaveClass("max-w-[560px]");
    expect(screen.getByRole("combobox", { name: "Search or run…" })).toHaveFocus();
    expect(groups()).toEqual(["Views", "Actio", "Proje"]);
    expect(screen.getByRole("option", { name: "Scan System" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Open Folder…" })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /Add a cart/ })).not.toBeInTheDocument();
  });

  it("shows Cards, Requirements and History once something is typed, in that order, matching case-insensitively", async () => {
    renderPalette(controls());
    await userEvent.setup().type(screen.getByRole("combobox"), "CART");
    expect(groups()).toEqual(["Cards", "Requi", "Histo"]);
    expect(screen.getByRole("option", { name: /Add a cart/ })).toBeInTheDocument();
    expect(within(screen.getByRole("group", { name: "Requirements" })).getByText("UI · Views")).toHaveClass("text-muted-foreground");
    expect(screen.getByRole("option", { name: /Cart added/ })).toBeInTheDocument();
  });

  it("finds a card by its short number", async () => {
    renderPalette(controls());
    await userEvent.setup().type(screen.getByRole("combobox"), "t1");
    expect(screen.getAllByRole("option").map((row) => row.textContent)).toEqual(["Cart total taskT1"]);
  });

  it("goes to a view, a card, a section and an entry, and closes at every choice", async () => {
    const value = controls();
    renderPalette(value);
    const user = userEvent.setup();
    await user.click(screen.getByRole("option", { name: "Specs" }));
    expect(PROJECT.showView).toHaveBeenCalledWith("specs");
    expect(value.palette.close).toHaveBeenCalledTimes(1);
    await user.type(screen.getByRole("combobox"), "cart");
    await user.click(screen.getByRole("option", { name: /Add a cart/ }));
    expect(value.reveal).toHaveBeenCalledWith({ kind: "card", id: "p1" });
    await user.click(screen.getByRole("option", { name: /^cart/ }));
    expect(value.reveal).toHaveBeenCalledWith({ kind: "section", key: "ui#cart" });
    await user.click(screen.getByRole("option", { name: /Cart added/ }));
    expect(value.reveal).toHaveBeenCalledWith({ kind: "history", id: "h1" });
    expect(value.palette.close).toHaveBeenCalledTimes(4);
  });

  it("lists the drafts of requests among the Cards with `Request` in place of the short number, never the ghosts, and finds them by title", async () => {
    renderPalette(controls());
    const user = userEvent.setup();
    await user.type(screen.getByRole("combobox"), "CART");
    const cards = within(screen.getByRole("group", { name: "Cards" })).getAllByRole("option");
    expect(cards.map((row) => row.textContent)).toEqual(["Cart total taskT1", "Add a cartP1", "Dark mode Cart themeRequest"]);
    expect(screen.queryByRole("option", { name: /Cart ghost/ })).not.toBeInTheDocument();
    expect(within(screen.getByRole("option", { name: /Dark mode/ })).getByText("Request")).toHaveClass("text-muted-foreground");
    await user.clear(screen.getByRole("combobox"));
    await user.type(screen.getByRole("combobox"), "untitled");
    expect(screen.getAllByRole("option").map((row) => row.textContent)).toEqual(["Untitled requestRequest"]);
  });

  it("shows at most 8 rows of Cards, drafts included, those that start with the text first", async () => {
    renderPalette(controls());
    await userEvent.setup().type(screen.getByRole("combobox"), "mobile");
    const cards = within(screen.getByRole("group", { name: "Cards" })).getAllByRole("option");
    expect(cards).toHaveLength(8);
    expect(cards[0]).toHaveTextContent("Mobile 0Request");
  });

  it("opens a draft in the request editor on the Board, not in the details dialog, and closes", async () => {
    const value = controls();
    renderPalette(value);
    const user = userEvent.setup();
    await user.type(screen.getByRole("combobox"), "dark");
    await user.click(screen.getByRole("option", { name: /Dark mode/ }));
    expect(PROJECT.showView).toHaveBeenCalledWith("board");
    expect(data.openEditor).toHaveBeenCalledWith("r1");
    expect(value.reveal).not.toHaveBeenCalled();
    expect(value.palette.close).toHaveBeenCalledTimes(1);
  });

  it("keeps the order of the actions and runs the Board header's `New Request` for its action", async () => {
    renderPalette(controls());
    const actions = within(screen.getByRole("group", { name: "Actions" })).getAllByRole("option");
    expect(actions.map((row) => row.textContent)).toEqual(["Scan System", "Run all", "New Request", "Open Folder…", "Usage"]);
    expect(screen.queryByRole("option", { name: "New Work Item" })).not.toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole("option", { name: "New Request" }));
    expect(PROJECT.showView).toHaveBeenCalledWith("board");
    expect(data.newRequest).toHaveBeenCalledTimes(1);
  });

  it("runs an action as its control does: Scan System asks the Board to confirm, Run all runs, Usage opens over the origin", async () => {
    const origin = document.createElement("button");
    const value = controls({ palette: { open: true, origin, show: vi.fn(), close: vi.fn() } });
    renderPalette(value);
    const user = userEvent.setup();
    await user.click(screen.getByRole("option", { name: "Scan System" }));
    expect(value.reveal).toHaveBeenCalledWith({ kind: "scan" });
    await user.click(screen.getByRole("option", { name: "Run all" }));
    expect(data.runAll).toHaveBeenCalledTimes(1);
    await user.click(screen.getByRole("option", { name: "Usage" }));
    expect(value.openUsage).toHaveBeenCalledWith(origin);
    await user.click(screen.getByRole("option", { name: "Open Folder…" }));
    expect(call).toHaveBeenCalledWith("open_folder_from_window");
  });

  it("shows the reasons of a disabled action in Muted and cannot choose it", async () => {
    data.runAllReasons = ["No Task is in To do"];
    data.inProgress = true;
    const value = controls();
    renderPalette(value);
    const user = userEvent.setup();
    const run = screen.getByRole("option", { name: /Run all/ });
    expect(run).toHaveAttribute("aria-disabled", "true");
    expect(within(run).getByText("No Task is in To do")).toHaveClass("text-muted-foreground");
    expect(within(screen.getByRole("option", { name: /Scan System/ })).getByText("A merge is in progress")).toBeInTheDocument();
    await user.click(run);
    await user.click(screen.getByRole("option", { name: /Scan System/ }));
    expect(data.runAll).not.toHaveBeenCalled();
    expect(value.reveal).not.toHaveBeenCalled();
    expect(value.palette.close).not.toHaveBeenCalled();
  });

  it("opens another project as from the switcher, and only closes for the current one; a missing folder cannot be chosen", async () => {
    const value = controls();
    renderPalette(value);
    const user = userEvent.setup();
    await user.click(screen.getByRole("option", { name: /beta/ }));
    expect(call).toHaveBeenCalledWith("open_project", { path: "/work/beta" });
    vi.mocked(call).mockClear();
    await user.click(screen.getByRole("option", { name: /alpha/ }));
    expect(call).not.toHaveBeenCalled();
    expect(screen.getByRole("option", { name: /gone — Not found/ })).toHaveAttribute("aria-disabled", "true");
  });

  it("renders nothing while closed", () => {
    renderPalette(controls({ palette: { open: false, origin: null, show: vi.fn(), close: vi.fn() } }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});

function Probe() {
  const { palette } = useWindowControls();
  return <p>{palette.open ? "palette open" : "palette closed"}</p>;
}

describe("the shortcut", () => {
  const mount = () =>
    render(
      <TooltipProvider>
        <ProjectWindow
          projectId="0123456789abcdef"
          views={{ board: <textarea aria-label="Notes" /> }}
          around={(children) => (
            <>
              <Probe />
              {children}
            </>
          )}
        />
      </TooltipProvider>,
    );

  it("opens the palette with Ctrl+K even with the focus in a text field, prevents the default, and closes it when pressed again", () => {
    mount();
    const field = screen.getByRole("textbox", { name: "Notes" });
    field.focus();
    const press = () => {
      const event = new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true, cancelable: true });
      act(() => {
        field.dispatchEvent(event);
      });
      return event;
    };
    const first = press();
    expect(first.defaultPrevented).toBe(true);
    expect(screen.getByText("palette open")).toBeInTheDocument();
    press();
    expect(screen.getByText("palette closed")).toBeInTheDocument();
  });

  it("takes the key before the field does", () => {
    mount();
    const field = screen.getByRole("textbox", { name: "Notes" });
    const reached = vi.fn();
    field.addEventListener("keydown", reached);
    fireEvent.keyDown(field, { key: "k", ctrlKey: true });
    expect(reached).not.toHaveBeenCalled();
  });

  it("ignores K alone and other modifiers, and opens with the Search or run… button", async () => {
    mount();
    fireEvent.keyDown(document.body, { key: "k" });
    fireEvent.keyDown(document.body, { key: "k", ctrlKey: true, shiftKey: true });
    expect(screen.getByText("palette closed")).toBeInTheDocument();
    await userEvent.setup().click(screen.getByRole("button", { name: "Search or run…" }));
    expect(screen.getByText("palette open")).toBeInTheDocument();
  });
});
