import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BoardCard } from "@/bindings/BoardCard";
import type { BoardSnapshot } from "@/bindings/BoardSnapshot";
import type { Ghost } from "@/bindings/Ghost";
import { BoardProvider } from "@/components/board/board-state";
import { BoardActionsContext, type BoardActions } from "@/components/board/board-ui";
import { BoardView } from "@/components/board/board-view";
import { ProjectContext } from "@/components/project-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";
import { proposalView } from "@/components/board/proposal-fixtures";
import { driftView } from "@/components/board/drift-fixtures";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const PROJECT = "0123456789abcdef";

function head(id: string, shortNumber: string | null, title: string, extra: { body?: string; agentFinishing?: boolean } = {}) {
  return { id, shortNumber, title, body: extra.body ?? "", created: "2026-11-02T07:00:00.000Z", agentFinishing: extra.agentFinishing ?? false };
}

function task(id: string, shortNumber: string, title: string, patch: Partial<Extract<BoardCard, { kind: "task" }>> = {}): Extract<BoardCard, { kind: "task" }> {
  return {
    kind: "task",
    head: head(id, shortNumber, title),
    doing: false,
    heldBy: null,
    technical: false,
    refs: [],
    label: null,
    blockedBy: [],
    waitingFor: [],
    runReasons: [],
    deleteReasons: [],
    ...patch,
  };
}

function drift(id: string, shortNumber: string, title: string): BoardCard {
  return { kind: "drift", head: head(id, shortNumber, title), origin: null, view: driftView() };
}

function proposal(id: string, shortNumber: string, title: string): BoardCard {
  return { kind: "proposal", head: head(id, shortNumber, title), standing: "open", proposalKind: "change", view: proposalView() };
}

function ghost(id: string, kind: Ghost["kind"], title: string, patch: Partial<Ghost> = {}): Ghost {
  return { id, jobId: `job-${id}`, kind, title, phase: "writing", note: null, ...patch };
}

function board(revision: number, patch: Partial<BoardSnapshot> = {}): BoardSnapshot {
  return {
    projectId: PROJECT,
    revision,
    backlog: [],
    active: [],
    ghosts: [],
    gathered: [],
    runAllReasons: ["No Task is in To do"],
    doneSinceLastScan: 0,
    warnings: [],
    ...patch,
  };
}

let current: BoardSnapshot;
let handlers: Map<string, (payload: unknown) => void>;
let deleteResult: { code: string; message: string } | null;

function install(initial: BoardSnapshot) {
  current = initial;
  handlers = new Map();
  deleteResult = null;
  vi.mocked(on).mockImplementation(((name: string, handler: (payload: unknown) => void) => {
    handlers.set(name, handler);
    return Promise.resolve(() => {});
  }) as unknown as typeof on);
  vi.mocked(call).mockImplementation(((name: string) => {
    if (name === "board_snapshot") return Promise.resolve(current);
    if (name === "delete_task") return deleteResult ? Promise.reject(deleteResult) : Promise.resolve(null);
    return Promise.reject(new Error(`unexpected command ${name}`));
  }) as unknown as typeof call);
}

/** The backend sends a new picture: a `project-changed` event, then the answer to the next fetch. */
async function publish(next: BoardSnapshot) {
  current = next;
  await act(async () => {
    handlers.get("project-changed")?.({ projectId: PROJECT, revision: next.revision, slices: ["board"] });
  });
}

async function show(actions: BoardActions = {}, openJob = vi.fn(), showView = vi.fn()) {
  render(
    <TooltipProvider>
      <ProjectContext.Provider value={{ projectId: PROJECT, showView, openJob, openedJob: null, openedJobRequest: 0 }}>
        <BoardActionsContext.Provider value={actions}>
          <BoardProvider projectId={PROJECT}>
            <BoardView />
          </BoardProvider>
        </BoardActionsContext.Provider>
      </ProjectContext.Provider>
    </TooltipProvider>,
  );
  await screen.findByRole("region", { name: "Backlog" });
  return { openJob, showView };
}

const backlog = () => screen.getByRole("region", { name: "Backlog" });
const active = () => screen.getByRole("region", { name: "Active" });

/** The reasons a guarded control lists, one per line, as its accessible description holds them. */
function reasonsOf(button: HTMLElement): string[] {
  // An open tooltip adds its own id after the control's own description, which is the first.
  const own = (button.getAttribute("aria-describedby") ?? "").split(" ")[0] ?? "";
  const lines = [...(document.getElementById(own)?.querySelectorAll(":scope > .block") ?? [])];
  return lines.map((line) => line.textContent ?? "");
}

beforeEach(() => {
  install(board(1));
});

describe("columns", () => {
  it("shows Backlog and Active with the number of their real cards, never counting ghosts", async () => {
    install(
      board(1, {
        backlog: [proposal("pppppppp", "P1", "Add a cart"), drift("dddddddd", "D1", "Cart total")],
        active: [task("tttttttt", "T1", "Write the cart")],
        ghosts: [ghost("g1", "proposal", "Coming proposal"), ghost("g2", "task", "Coming task")],
      }),
    );
    await show();
    const regions = screen.getAllByRole("region");
    expect(regions.map((region) => region.querySelector("h2")?.textContent)).toEqual(["Backlog2", "Active1"]);
    expect(within(backlog()).getByRole("button", { name: "Add a cart" })).toBeInTheDocument();
    expect(within(backlog()).getByRole("button", { name: "Cart total" })).toBeInTheDocument();
    // The ghosts sit in their target columns, after the cards.
    expect(within(backlog()).getByRole("button", { name: "Proposal being prepared by the agent: Coming proposal, Writing" })).toBeInTheDocument();
    expect(within(active()).getByRole("button", { name: "Task being prepared by the agent: Coming task, Writing" })).toBeInTheDocument();
  });

  it("says Nothing to decide and Nothing active, also above ghosts", async () => {
    await show();
    expect(within(backlog()).getByText("Nothing to decide")).toBeInTheDocument();
    expect(within(active()).getByText("Nothing active")).toBeInTheDocument();
  });

  it("keeps the empty text above a ghost and counts 0", async () => {
    install(board(1, { ghosts: [ghost("g1", "drift", "Coming drift", { phase: "preparing" })] }));
    await show();
    expect(backlog().querySelector("h2")).toHaveTextContent("Backlog0");
    const text = within(backlog()).getByText("Nothing to decide");
    const button = within(backlog()).getByRole("button", { name: "Drift being prepared by the agent: Coming drift, Preparing" });
    expect(text.compareDocumentPosition(button) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("keeps the backend's order of the cards", async () => {
    install(board(1, { active: [task("bbbbbbbb", "T2", "Second"), task("aaaaaaaa", "T1", "First")] }));
    await show();
    const titles = within(active()).getAllByRole("heading", { level: 3 }).map((h) => h.textContent);
    expect(titles).toEqual(["Second", "First"]);
  });
});

describe("ghosts", () => {
  it("is one button that opens the conversation of its job, with the job's note on one line", async () => {
    install(board(1, { ghosts: [ghost("g1", "task", "Coming task", { note: "Reading the Model", phase: "waiting-for-answers" })] }));
    const { openJob } = await show();
    const button = within(active()).getByRole("button", {
      name: "Task being prepared by the agent: Coming task, Waiting for your answers",
    });
    expect(button).toHaveTextContent("Reading the Model");
    const user = userEvent.setup();
    await user.click(button);
    expect(openJob).toHaveBeenCalledWith("job-g1");
  });

  it("shows still known texts and, under them, only placeholders that shimmer, shaped like the real card of its kind", async () => {
    install(
      board(1, {
        ghosts: [ghost("g1", "proposal", "Coming proposal", { note: "Reading the Model" }), ghost("g2", "task", "Coming task"), ghost("g3", "drift", "Coming drift")],
      }),
    );
    await show();
    const shapes = (name: RegExp) => {
      const button = screen.getByRole("button", { name });
      const placeholders = Array.from(button.querySelectorAll("[data-placeholder]"));
      expect(placeholders.every((element) => element.classList.contains("shimmer") && element.textContent === "")).toBe(true);
      // Nothing else in the ghost shimmers: the kind, title, phase and note are still.
      expect(button.querySelectorAll(".shimmer")).toHaveLength(placeholders.length);
      expect(button).toHaveTextContent("Coming");
      return placeholders.map((element) => (element.classList.contains("h-8") ? "button" : element.classList.contains("size-2") ? "dot" : "bar"));
    };
    expect(shapes(/Proposal being prepared/)).toEqual(["bar", "bar", "bar", "button", "button"]);
    expect(shapes(/Task being prepared/)).toEqual(["dot", "bar", "button"]);
    expect(shapes(/Drift being prepared/)).toEqual(["bar", "button", "button"]);
  });

  it("leaves in the same picture in which its card arrives", async () => {
    install(board(1, { ghosts: [ghost("g1", "task", "Coming task")] }));
    await show();
    await publish(board(2, { active: [task("tttttttt", "T1", "Coming task")] }));
    expect(within(active()).queryByRole("button", { name: /being prepared by the agent/ })).not.toBeInTheDocument();
    expect(within(active()).getByRole("button", { name: "Coming task" })).toBeInTheDocument();
  });
});

describe("Run all", () => {
  const runAllButton = () => screen.getByRole("button", { name: /^Run all/ });

  it("lists every reason it is disabled, in the order of the control, and ignores a click", async () => {
    const runAll = vi.fn();
    install(board(1, { runAllReasons: ["Drifts are being resolved", "A run is in progress", "No Task is in To do"] }));
    await show({ runAll });
    const button = runAllButton();
    expect(button).toHaveAttribute("aria-disabled", "true");
    expect(reasonsOf(button)).toEqual(["Drifts are being resolved", "A run is in progress", "No Task is in To do"]);
    await userEvent.setup().click(button);
    expect(runAll).not.toHaveBeenCalled();
  });

  it("shows the blocked reason alone and enables itself when the backend says so, without moving the focus", async () => {
    const runAll = vi.fn();
    install(board(1, { runAllReasons: ["All the Tasks in To do are blocked"] }));
    await show({ runAll });
    const user = userEvent.setup();
    const button = runAllButton();
    expect(reasonsOf(button)).toEqual(["All the Tasks in To do are blocked"]);
    button.focus();
    await publish(board(2, { runAllReasons: [] }));
    const same = runAllButton();
    expect(same).toBe(button);
    expect(same).toHaveFocus();
    expect(same).not.toHaveAttribute("aria-disabled");
    await user.click(same);
    expect(runAll).toHaveBeenCalledTimes(1);
  });

  it("is a primary button in the header, after Scan System, with the tooltip `Run all the Tasks in To do`", async () => {
    const user = userEvent.setup();
    install(board(1, { runAllReasons: [] }));
    await show();
    const header = screen.getByRole("heading", { level: 1, name: "Board" }).closest("header")!;
    const buttons = within(header).getAllByRole("button");
    expect(buttons.map((button) => button.textContent)).toEqual(["Scan System", "Run all"]);
    expect(buttons[1]).toHaveClass("bg-primary");
    expect(buttons[0]).not.toHaveClass("bg-primary");
    await user.hover(buttons[1]!);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Run all the Tasks in To do");
  });

  it("counts inside the button the Tasks in To do that are not blocked, and hides the count at 0", async () => {
    install(
      board(1, {
        runAllReasons: [],
        active: [
          task("aaaaaaaa", "T1", "Ready one"),
          task("bbbbbbbb", "T2", "Ready two"),
          task("cccccccc", "T3", "Blocked", { blockedBy: [{ id: "dddddddd", shortNumber: "D1", title: "A drift" }] }),
          task("eeeeeeee", "T4", "Waiting", { waitingFor: [{ id: "ffffffff", shortNumber: "T9", title: "A fix" }] }),
          task("gggggggg", "T5", "In progress", { doing: true }),
        ],
      }),
    );
    await show();
    expect(runAllButton()).toHaveTextContent("Run all2 ready");
    expect(runAllButton()).toHaveAccessibleName("Run all, 2 ready");
    await publish(board(2, { runAllReasons: [], active: [task("gggggggg", "T5", "In progress", { doing: true })] }));
    expect(runAllButton()).toHaveTextContent(/^Run all$/);
    expect(runAllButton()).toHaveAccessibleName("Run all");
  });

  it("has no footer in either column: no `Run all` below Active and no `Request` below Backlog", async () => {
    install(board(1, { runAllReasons: [], backlog: [proposal("pppppppp", "P1", "Add a cart")], active: [task("aaaaaaaa", "T1", "Ready")] }));
    await show();
    expect(within(active()).queryByRole("button", { name: /^Run all/ })).not.toBeInTheDocument();
    expect(within(backlog()).queryByRole("button", { name: "Request" })).not.toBeInTheDocument();
  });
});

describe("the lanes", () => {
  it("are lanes with a dot, the title and the number of real cards in their heading, and no Done column", async () => {
    install(board(1, { backlog: [proposal("pppppppp", "P1", "Add a cart")], active: [task("tttttttt", "T1", "Write the cart")], ghosts: [ghost("g1", "task", "Coming")] }));
    await show();
    expect(screen.getAllByRole("region").map((region) => region.querySelector("h2")?.textContent)).toEqual(["Backlog1", "Active1"]);
    expect(backlog()).toHaveClass("bg-lane", "rounded-[14px]");
    expect(active()).toHaveClass("bg-lane", "rounded-[14px]");
    expect(backlog().querySelector("h2")?.previousElementSibling).toHaveClass("size-2", "bg-lavender");
    expect(active().querySelector("h2")?.previousElementSibling).toHaveClass("size-2", "bg-sky");
    expect(screen.queryByRole("region", { name: "Done" })).not.toBeInTheDocument();
  });

  it("ends the Backlog heading with the icon-only `New Request`, always enabled, with its tooltip", async () => {
    const user = userEvent.setup();
    const newRequest = vi.fn();
    await show({ newRequest });
    const heading = backlog().querySelector("h2")!.parentElement!;
    const add = within(heading).getByRole("button", { name: "New Request" });
    expect(add).not.toHaveAttribute("aria-disabled");
    expect(add.textContent).toBe("");
    expect(heading.lastElementChild).toContainElement(add);
    await user.hover(add);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Write a request in Markdown and send it to an agent");
    await user.click(add);
    expect(newRequest).toHaveBeenCalledTimes(1);
    expect(within(active().querySelector("h2")!.parentElement!).queryByRole("button")).not.toBeInTheDocument();
  });

  it("shows in an empty Backlog the weave, what lands there, `Request` and `Ask in Chat`, also above ghosts", async () => {
    const user = userEvent.setup();
    const newRequest = vi.fn();
    install(board(1, { ghosts: [ghost("g1", "drift", "Coming drift", { phase: "preparing" })] }));
    const { showView } = await show({ newRequest });
    expect(backlog().querySelector("svg[data-weave]")).toHaveAttribute("aria-hidden", "true");
    expect(within(backlog()).getByText("Nothing to decide")).toHaveClass("text-sm", "font-semibold");
    expect(within(backlog()).getByText("New requests and Proposals from Chat land here for your go-ahead.")).toHaveClass("text-muted-foreground");
    const empty = within(backlog()).getByText("Nothing to decide");
    const ghostButton = within(backlog()).getByRole("button", { name: /Drift being prepared/ });
    expect(empty.compareDocumentPosition(ghostButton) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    const item = within(backlog()).getByRole("button", { name: "Request" });
    expect(item).toHaveClass("border");
    await user.click(item);
    expect(newRequest).toHaveBeenCalledTimes(1);
    await user.click(within(backlog()).getByRole("button", { name: "Ask in Chat" }));
    expect(showView).toHaveBeenCalledWith("general");
  });

  it("shows no empty Backlog text once a card is there", async () => {
    install(board(1, { backlog: [proposal("pppppppp", "P1", "Add a cart")] }));
    await show();
    expect(within(backlog()).queryByText("Nothing to decide")).not.toBeInTheDocument();
    expect(within(backlog()).queryByRole("button", { name: "Ask in Chat" })).not.toBeInTheDocument();
  });
});

describe("Scan System", () => {
  it("confirms first, and cancelling leaves the count and the work untouched", async () => {
    const scanSystem = vi.fn();
    install(board(1, { doneSinceLastScan: 3 }));
    await show({ scanSystem });
    expect(screen.getByText("3 done since last scan")).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Scan System" }));
    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText("Compare the whole System with the code?")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(scanSystem).not.toHaveBeenCalled();
    expect(screen.getByText("3 done since last scan")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Scan System" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Scan System" }));
    expect(scanSystem).toHaveBeenCalledTimes(1);
  });

  it("is secondary, with the icon before the text", async () => {
    await show();
    const button = screen.getByRole("button", { name: "Scan System" });
    expect(button).toHaveClass("border-input");
    expect(button.querySelector("svg")).not.toBeNull();
  });
});

describe("the view header", () => {
  const subtitle = (header: HTMLElement) => header.querySelector("p")?.textContent ?? null;
  const header = () => screen.getByRole("heading", { level: 1, name: "Board" }).closest("header")!;

  it("holds the title `Board` on the left, the subtitle in Muted under it, and Scan System and Run all on the right, in one row", async () => {
    install(board(1, { doneSinceLastScan: 18, active: [task("tttttttt", "T1", "Write the cart")] }));
    await show();
    expect(subtitle(header())).toBe("1 active · 18 done since last scan");
    expect(header().querySelector("p")).toHaveClass("text-muted-foreground");
    expect(within(header()).getByRole("button", { name: "Scan System" })).toBeInTheDocument();
    expect(document.querySelectorAll("header")).toHaveLength(1);
  });

  it("leaves out each part of the subtitle when it is 0, and the subtitle with them", async () => {
    install(board(1, { doneSinceLastScan: 2 }));
    await show();
    expect(subtitle(header())).toBe("2 done since last scan");
    await publish(board(2, { active: [task("tttttttt", "T1", "Write the cart")] }));
    expect(subtitle(header())).toBe("1 active");
    await publish(board(3));
    expect(subtitle(header())).toBeNull();
  });

  it("counts in `active` the real cards of Active, never ghosts", async () => {
    install(board(1, { active: [task("aaaaaaaa", "T1", "First"), task("bbbbbbbb", "T2", "Second", { doing: true })], ghosts: [ghost("g1", "task", "Coming")] }));
    await show();
    expect(subtitle(header())).toBe("2 active");
  });

  it("shows while the Board loads", async () => {
    vi.mocked(call).mockImplementation((() => new Promise(() => {})) as unknown as typeof call);
    render(
      <TooltipProvider>
        <ProjectContext.Provider value={{ projectId: PROJECT, showView: vi.fn(), openJob: vi.fn(), openedJob: null, openedJobRequest: 0 }}>
          <BoardActionsContext.Provider value={{}}>
            <BoardProvider projectId={PROJECT}>
              <BoardView />
            </BoardProvider>
          </BoardActionsContext.Provider>
        </ProjectContext.Provider>
      </TooltipProvider>,
    );
    expect(screen.getByRole("heading", { level: 1, name: "Board" })).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Loading the Board…");
  });
});

describe("details", () => {
  const body = "Carry out the **cart**.";

  function withTask(title = "Write the cart", patch: Partial<Extract<BoardCard, { kind: "task" }>> = {}): BoardCard {
    const base = task("tttttttt", "T4", title, patch);
    return { ...base, head: { ...base.head, body } } as BoardCard;
  }

  it("opens with a click on the card and with Enter on its title, and Esc closes it", async () => {
    install(board(1, { active: [withTask()] }));
    await show();
    const user = userEvent.setup();
    const title = within(active()).getByRole("button", { name: "Write the cart" });
    await user.click(title);
    expect(await screen.findByRole("dialog")).toHaveTextContent("Carry out the cart.");
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(title).toHaveFocus();

    title.focus();
    await user.keyboard("{Enter}");
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
  });

  it("updates in place with each picture, without moving the focus", async () => {
    install(board(1, { active: [withTask()] }));
    await show();
    const user = userEvent.setup();
    await user.click(within(active()).getByRole("button", { name: "Write the cart" }));
    const dialog = await screen.findByRole("dialog");
    const focused = document.activeElement;
    expect(dialog).toContainElement(focused as HTMLElement);

    await publish(board(2, { active: [withTask("Write the whole cart")] }));
    expect(await screen.findByRole("dialog")).toHaveTextContent("Write the whole cart");
    expect(document.activeElement).toBe(focused);
  });

  it("closes by itself when the card leaves the Board and the focus goes to its column", async () => {
    install(board(1, { active: [withTask()], backlog: [proposal("pppppppp", "P1", "Add a cart")] }));
    await show();
    const user = userEvent.setup();
    await user.click(within(active()).getByRole("button", { name: "Write the cart" }));
    await screen.findByRole("dialog");

    await publish(board(2, { backlog: [proposal("pppppppp", "P1", "Add a cart")] }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    await waitFor(() => expect(active()).toHaveFocus());

    // A card that comes back does not reopen what the developer no longer has open.
    await publish(board(3, { active: [withTask()] }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
});

describe("a refused decision", () => {
  it("shows a notice, writes nothing on the UI side and shows the Board as it holds now", async () => {
    const user = userEvent.setup();
    const finishing = { ...task("tttttttt", "T4", "Write the cart"), deleteReasons: ["The agent is still finishing"] } as BoardCard;
    // The button was enabled when the developer saw the Board; the agent works by the time it is applied.
    install(board(1, { active: [task("tttttttt", "T4", "Write the cart")] }));
    await show();
    deleteResult = { code: "refused", message: "Could not delete T4: the agent that filed it is still working." };
    current = board(2, { active: [finishing] });

    await user.click(within(active()).getByRole("button", { name: "Write the cart" }));
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Delete" }));
    const confirm = await screen.findByRole("alertdialog");
    expect(within(confirm).getByText("Delete T4 for good?")).toBeInTheDocument();
    await user.click(within(confirm).getByRole("button", { name: "Delete" }));

    expect(await screen.findByText("Could not delete T4: the agent that filed it is still working.")).toBeInTheDocument();
    expect(vi.mocked(call).mock.calls.filter(([name]) => name === "delete_task")).toEqual([
      ["delete_task", { projectId: PROJECT, taskId: "tttttttt" }],
    ]);
    // The card is still there, and Delete now lists why it is not available.
    await waitFor(() => {
      const delete_ = within(screen.getByRole("dialog")).getByRole("button", { name: "Delete" });
      expect(reasonsOf(delete_)).toEqual(["The agent is still finishing"]);
    });
    // The first Esc closes the tooltip the focused Delete shows, the second the dialog.
    await user.keyboard("{Escape}");
    if (screen.queryByRole("dialog")) await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(within(active()).getByRole("button", { name: "Write the cart" })).toBeInTheDocument();
  });

  it("closes the notice with its close button", async () => {
    await show();
    await act(async () => {
      handlers.get("notice")?.({ projectId: PROJECT, id: "n1", text: "P3 went back to Draft while it was being decided." });
      handlers.get("notice")?.({ projectId: PROJECT, id: "n1", text: "P3 went back to Draft while it was being decided." });
      handlers.get("notice")?.({ projectId: "another-project", id: "n2", text: "Not for this project." });
    });
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    await userEvent.setup().click(screen.getByRole("button", { name: "Close notice" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows each History entry Weaver did not commit as a notice with the reason, newest first, closed from the keyboard", async () => {
    const user = userEvent.setup();
    await show();
    await act(async () => {
      handlers.get("notice")?.({ projectId: PROJECT, id: "history-not-committed-hhhhhhhh", text: 'History entry "Cart" was not committed: the checks of the Run failed.' });
      handlers.get("notice")?.({ projectId: PROJECT, id: "history-not-committed-iiiiiiii", text: 'History entry "Orders" was not committed: HEAD is detached.' });
    });
    const alerts = screen.getAllByRole("alert");
    expect(alerts).toHaveLength(2);
    expect(within(alerts[0]!).getByText('History entry "Orders" was not committed: HEAD is detached.')).toBeInTheDocument();
    expect(within(alerts[1]!).getByText('History entry "Cart" was not committed: the checks of the Run failed.')).toBeInTheDocument();

    const close = within(alerts[0]!).getByRole("button", { name: "Close notice" });
    close.focus();
    await user.keyboard("{Enter}");
    const left = screen.getAllByRole("alert");
    expect(left).toHaveLength(1);
    expect(within(left[0]!).getByText('History entry "Cart" was not committed: the checks of the Run failed.')).toBeInTheDocument();
  });
});

describe("pictures", () => {
  it("drops an answer older than the one it shows", async () => {
    install(board(5, { active: [task("tttttttt", "T1", "Current")] }));
    await show();
    await publish(board(4, { active: [task("tttttttt", "T1", "Old")] }));
    expect(within(active()).getByRole("button", { name: "Current" })).toBeInTheDocument();
    expect(within(active()).queryByRole("button", { name: "Old" })).not.toBeInTheDocument();
  });

  it("asks again for a gap in the revisions, and ignores another project's events", async () => {
    await show();
    const asked = () => vi.mocked(call).mock.calls.filter(([name]) => name === "board_snapshot").length;
    expect(asked()).toBe(1);
    await act(async () => handlers.get("project-changed")?.({ projectId: PROJECT, revision: 2, slices: ["history"] }));
    expect(asked()).toBe(1);
    await act(async () => handlers.get("project-changed")?.({ projectId: PROJECT, revision: 5, slices: ["history"] }));
    expect(asked()).toBe(2);
    await act(async () => handlers.get("project-changed")?.({ projectId: "other", revision: 6, slices: ["board"] }));
    expect(asked()).toBe(2);
  });
});

describe("History warnings", () => {
  it("shows a warning above the columns for a cited Task that could not be recorded in History", async () => {
    const text = "Could not record abcd1234 in History: it has no `finished` instant. The Task stays until its copy can be made.";
    install(board(1, { warnings: [text] }));
    await show();
    expect(screen.getByRole("alert")).toHaveTextContent(text);
  });

  it("closes a warning with its close button, keeps it closed through Board updates, and shows a changed text", async () => {
    const first = "Could not record abcd1234 in History: it has no `finished` instant. The Task stays until its copy can be made.";
    const second = "Could not record efgh5678 in History: it has no `finished` instant. The Task stays until its copy can be made.";
    install(board(1, { warnings: [first, second] }));
    await show();
    expect(screen.getAllByRole("alert")).toHaveLength(2);
    const closeFirst = within(screen.getAllByRole("alert")[0]!).getByRole("button", { name: "Close" });
    await userEvent.setup().click(closeFirst);
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(screen.getByRole("alert")).toHaveTextContent(second);

    // The Board changes: the closed warning stays hidden, a warning with another text shows at once.
    const changed = "Could not record abcd1234 in History: the copy was refused. The Task stays until its copy can be made.";
    await publish(board(2, { warnings: [first, second, changed] }));
    expect(screen.getAllByRole("alert").map((alert) => alert.textContent)).toEqual([expect.stringContaining(second), expect.stringContaining(changed)]);
  });
});
