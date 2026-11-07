import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactNode } from "react";

import type { BoardCard } from "@/bindings/BoardCard";
import type { BoardSnapshot } from "@/bindings/BoardSnapshot";
import type { JobRow } from "@/bindings/JobRow";
import type { ViewSnapshot } from "@/bindings/ViewSnapshot";
import { BoardProvider } from "@/components/board/board-state";
import { BoardActionsContext, type BoardActions } from "@/components/board/board-ui";
import { BoardView } from "@/components/board/board-view";
import { JobsProvider } from "@/components/jobs/use-jobs";
import { ProjectContext } from "@/components/project-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";
import { driftView } from "@/components/board/drift-fixtures";
import { useViews, ViewsProvider } from "@/components/views/views-state";
import { snapshot as viewSnapshot } from "@/components/views/view-fixtures";
import type { ViewId } from "@/views";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const PROJECT = "0123456789abcdef";

function head(id: string, shortNumber: string | null, title: string, body = "") {
  return { id, shortNumber, title, body, created: "2026-11-02T07:00:00.000Z", agentFinishing: false };
}

type TaskCard = Extract<BoardCard, { kind: "task" }>;

function task(id: string, shortNumber: string | null, title: string, patch: Partial<TaskCard> = {}): TaskCard {
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

function snapshot(patch: Partial<BoardSnapshot>): BoardSnapshot {
  return {
    projectId: PROJECT,
    revision: 1,
    backlog: [],
    active: [],
    ghosts: [],
    gathered: [],
    runAllReasons: [],
    doneSinceLastScan: 0,
    warnings: [],
    ...patch,
  };
}

interface Extras {
  /** The jobs of the project: wraps the Board in a `JobsProvider`. */
  jobs?: JobRow[];
  /** The views' snapshot: wraps the Board in a `ViewsProvider`, whose own `viewOn` and `focusSteps` replace the actions given. */
  views?: ViewSnapshot;
  openJob?: (jobId: string) => void;
  showView?: (view: string) => void;
  /** Rendered beside the Board, inside the providers. */
  probe?: ReactNode;
}

function job(id: string, name: string, patch: Partial<JobRow> = {}): JobRow {
  return { id, kind: "run", name, state: "running", reason: null, created: "2026-11-02T07:00:00.000Z", started: null, ended: null, note: null, target: null, request: null, requestSent: false, ...patch };
}

/** Tells the subject of the focus the Requirements were last opened on. */
function FocusProbe() {
  const { focus } = useViews();
  return <output aria-label="Focus subject" data-subject={JSON.stringify(focus.requirements?.subject ?? null)} />;
}

async function show(board: BoardSnapshot, actions: BoardActions = {}, extras: Extras = {}) {
  vi.mocked(on).mockImplementation((() => Promise.resolve(() => {})) as unknown as typeof on);
  vi.mocked(call).mockImplementation(((name: string) => {
    if (name === "board_snapshot") return Promise.resolve(board);
    if (name === "jobs_snapshot") return Promise.resolve({ projectId: PROJECT, jobs: extras.jobs ?? [], ompReady: true });
    if (name === "view_snapshot") return Promise.resolve(extras.views ?? viewSnapshot());
    if (name === "delete_task") return Promise.resolve(null);
    return Promise.reject(new Error(`unexpected command ${name}`));
  }) as unknown as typeof call);
  let tree = (
    <>
      <BoardProvider projectId={PROJECT}>
        <BoardView />
      </BoardProvider>
      {extras.probe}
    </>
  );
  if (extras.jobs) tree = <JobsProvider projectId={PROJECT}>{tree}</JobsProvider>;
  if (extras.views) tree = <ViewsProvider projectId={PROJECT}>{tree}</ViewsProvider>;
  render(
    <TooltipProvider>
      <ProjectContext.Provider
        value={{ projectId: PROJECT, showView: (extras.showView ?? vi.fn()) as (view: ViewId) => void, openJob: extras.openJob ?? vi.fn(), openedJob: null, openedJobRequest: 0 }}
      >
        <BoardActionsContext.Provider value={actions}>{tree}</BoardActionsContext.Provider>
      </ProjectContext.Provider>
    </TooltipProvider>,
  );
  await screen.findByRole("region", { name: "Backlog" });
}

const active = () => screen.getByRole("region", { name: "Active" });
const backlog = () => screen.getByRole("region", { name: "Backlog" });

/** The reasons a guarded control lists, one per line, as its accessible description holds them. */
function reasonsOf(button: HTMLElement): string[] {
  const own = (button.getAttribute("aria-describedby") ?? "").split(" ")[0] ?? "";
  const lines = [...(document.getElementById(own)?.querySelectorAll(":scope > .block") ?? [])];
  return lines.map((line) => line.textContent ?? "");
}

beforeEach(() => {
  vi.mocked(call).mockReset();
  vi.mocked(on).mockReset();
});

describe("the card of a Task", () => {
  it("shows the status as a pill with its name: To do neutral with a hollow ring, In progress and Agent working in Sky", async () => {
    await show(
      snapshot({
        active: [
          task("aaaaaaaa", "T1", "First"),
          task("bbbbbbbb", "T2", "Second", { doing: true }),
          task("cccccccc", "T3", "Third", { doing: true, heldBy: "jjjjjjjj" }),
        ],
      }),
    );
    const pill = (id: string) => within(active().querySelector<HTMLElement>(`[data-card-id="${id}"]`)!).getByRole("img", { name: /To do|In progress|Agent working/ });
    expect(pill("aaaaaaaa")).toHaveAttribute("aria-label", "To do");
    expect(pill("aaaaaaaa")).toHaveClass("bg-muted");
    expect(pill("aaaaaaaa").firstElementChild).toHaveClass("border");
    expect(pill("bbbbbbbb")).toHaveAttribute("aria-label", "In progress");
    expect(pill("bbbbbbbb")).toHaveClass("bg-sky-tint");
    expect(pill("bbbbbbbb").firstElementChild).toHaveClass("bg-sky");
    expect(pill("cccccccc")).toHaveAttribute("aria-label", "Agent working");
    expect(pill("cccccccc")).toHaveClass("bg-sky-tint");
  });

  it("tells the status in a tooltip", async () => {
    const user = userEvent.setup();
    await show(snapshot({ active: [task("aaaaaaaa", "T1", "First")] }));
    await user.hover(within(active()).getByRole("img", { name: "To do" }));
    expect(await screen.findByRole("tooltip")).toHaveTextContent("To do");
  });

  it("puts the id, the label in Muted and the status in one row, then the title and the first paragraph of the body in plain text", async () => {
    const card = task("aaaaaaaa", "T1", "Simplify with omp", { label: "omp ACP" });
    const body = "The worker starts `omp acp`\nas a **child** process.\n\nA second paragraph that does not show.";
    await show(snapshot({ active: [{ ...card, head: { ...card.head, body } } as BoardCard] }));
    const frame = active().querySelector<HTMLElement>('[data-card-id="aaaaaaaa"]')!;
    const row = frame.firstElementChild as HTMLElement;
    expect(within(row).getByRole("img", { name: "Short number (temporary) T1" })).toBeInTheDocument();
    expect(within(row).getByText("omp ACP")).toHaveClass("text-muted-foreground");
    expect(within(row).getByRole("img", { name: "To do" })).toBeInTheDocument();
    expect(within(frame).getByRole("heading", { level: 3 })).toHaveClass("text-[15px]", "font-semibold");
    const summary = within(frame).getByText("The worker starts omp acp as a child process.");
    expect(summary).toHaveClass("line-clamp-3", "text-ink-2");
    expect(frame).not.toHaveTextContent("second paragraph");
  });

  it("shows no summary for a Task without text, and no phase of the work", async () => {
    await show(snapshot({ active: [task("aaaaaaaa", "T1", "First")] }));
    const frame = active().querySelector<HTMLElement>('[data-card-id="aaaaaaaa"]')!;
    expect(frame.querySelector("p")).toBeNull();
    expect(within(frame).queryByRole("list", { name: "Progress" })).not.toBeInTheDocument();
  });

  it("ends with a footer after a rule: `View requirements` and `Run`, and no `Open job` while no job holds it", async () => {
    await show(snapshot({ active: [task("aaaaaaaa", "T1", "First", { refs: ["order/Paid"] })] }), { viewOn: vi.fn(), focusSteps: () => 1 });
    const footer = active().querySelector<HTMLElement>("[data-card-controls]")!;
    expect(footer).toHaveClass("border-t");
    expect(within(footer).getAllByRole("button").map((button) => button.textContent)).toEqual(["View requirements", "Run"]);
    expect(within(active()).queryByRole("button", { name: "Open job" })).not.toBeInTheDocument();
  });
});

describe("the card of a Task a job holds", () => {
  const held = (patch: Partial<TaskCard> = {}) => task("cccccccc", "T3", "Held", { doing: true, heldBy: "jjjjjjjj", ...patch });
  const frame = () => active().querySelector<HTMLElement>('[data-card-id="cccccccc"]')!;

  it("shows in the footer the name of that job, `View requirements` and `Open job` instead of `Run`", async () => {
    await show(snapshot({ active: [held({ refs: ["order/Paid"] })] }), { viewOn: vi.fn(), focusSteps: () => 1 }, { jobs: [job("jjjjjjjj", "Run T3")] });
    const footer = frame().querySelector<HTMLElement>("[data-card-controls]")!;
    expect(await within(footer).findByText("Run T3")).toBeInTheDocument();
    expect(within(footer).getAllByRole("button").map((button) => button.textContent)).toEqual(["View requirements", "Open job"]);
    expect(within(footer).queryByRole("button", { name: "Run" })).not.toBeInTheDocument();
  });

  it("`Open job` shows Jobs with that job selected", async () => {
    const user = userEvent.setup();
    const openJob = vi.fn();
    await show(snapshot({ active: [held()] }), {}, { jobs: [job("jjjjjjjj", "Run T3")], openJob });
    await user.click(within(frame()).getByRole("button", { name: "Open job" }));
    expect(openJob).toHaveBeenCalledWith("jjjjjjjj");
  });

  it("says `Agent working · <running time of that job>`, which ticks every second, and keeps the rotating border", async () => {
    vi.useFakeTimers({ toFake: ["Date", "setInterval", "clearInterval"] });
    try {
      vi.setSystemTime(new Date("2026-11-02T08:00:00.000Z"));
      const started = new Date("2026-11-02T07:57:44.000Z").toISOString();
      await show(snapshot({ active: [held()] }), {}, { jobs: [job("jjjjjjjj", "Run T3", { started })] });
      const pill = () => within(frame()).getByRole("img", { name: /^Agent working/ });
      await waitFor(() => expect(pill()).toHaveAttribute("aria-label", "Agent working · 2m 16s"));
      expect(frame()).toHaveClass("task-held");
      await act(async () => {
        vi.advanceTimersByTime(1000);
      });
      expect(pill()).toHaveAttribute("aria-label", "Agent working · 2m 17s");
    } finally {
      vi.useRealTimers();
    }
  });

  it("says only `Agent working` while the job has no session yet, and still names the job's Task", async () => {
    await show(snapshot({ active: [held()] }), {}, { jobs: [job("jjjjjjjj", "Run T3", { state: "queued" })] });
    expect(within(frame()).getByRole("img", { name: "Agent working" })).toBeInTheDocument();
  });
});

describe("the file chips of a Task card", () => {
  it("shows one chip per Requirements fragment and Specs field the refs name, in monospace, each opening the single-ref focus of the first ref it stands for", async () => {
    const user = userEvent.setup();
    const showView = vi.fn();
    await show(
      snapshot({ active: [task("aaaaaaaa", "T1", "First", { technical: true, refs: ["order", "requirements/orders#order", "specs#languages"] })] }),
      {},
      { views: viewSnapshot(), showView },
    );
    const frame = active().querySelector<HTMLElement>("[data-card-id]")!;
    const checkout = await within(frame).findByRole("button", { name: "View requirements: checkout.md" });
    expect(checkout).toHaveClass("font-mono");
    expect(checkout).toHaveTextContent("checkout.md");
    const specs = within(frame).getByRole("button", { name: "View specs: specs.yaml" });
    expect(within(frame).getByText("technical")).toBeInTheDocument();
    await user.click(checkout);
    expect(showView).toHaveBeenLastCalledWith("requirements");
    await user.click(specs);
    expect(showView).toHaveBeenLastCalledWith("specs");
  });

  it("opens the single-ref focus of the first reference the chip stands for", async () => {
    const user = userEvent.setup();
    const probe = <FocusProbe />;
    await show(snapshot({ active: [task("aaaaaaaa", "T1", "First", { refs: ["order/Paid", "order", "requirements/orders#order"] })] }), {}, { views: viewSnapshot(), probe });
    await user.click(await within(active()).findByRole("button", { name: "View requirements: checkout.md" }));
    await waitFor(() => expect(screen.getByRole("status", { name: "Focus subject" }).getAttribute("data-subject")).toBe(JSON.stringify({ kind: "ref", ref: "order" })));
  });

  it("shows no chip for a Task that names no file, nor without the views' snapshot", async () => {
    await show(snapshot({ active: [task("aaaaaaaa", "T1", "First", { refs: ["order/Paid"] }), task("bbbbbbbb", "T2", "Second", { refs: ["order"] })] }), {}, { views: viewSnapshot() });
    await within(active()).findByRole("button", { name: "View requirements: checkout.md" });
    const first = active().querySelector<HTMLElement>('[data-card-id="aaaaaaaa"]')!;
    expect(within(first).queryByRole("button", { name: /^View (requirements|specs): / })).not.toBeInTheDocument();
  });

  it("shows no chip when no provider holds the Requirements", async () => {
    await show(snapshot({ active: [task("aaaaaaaa", "T1", "First", { refs: ["order"] })] }));
    expect(within(active()).queryByRole("button", { name: /^View (requirements|specs): / })).not.toBeInTheDocument();
  });
});

describe("the badges and the border of a Task card", () => {

  it("shows technical, blocked and waiting badges, and the label badge, with their names", async () => {
    await show(
      snapshot({
        active: [
          task("aaaaaaaa", "T1", "First", {
            technical: true,
            label: "Checkout",
            blockedBy: [
              { id: "d1111111", shortNumber: "D3", title: "Total is wrong" },
              { id: "d2222222", shortNumber: "D7", title: "Tax is wrong" },
            ],
            waitingFor: [{ id: "ffffffff", shortNumber: "T4", title: "Fix the total" }],
          }),
        ],
      }),
    );
    const card = within(active());
    expect(card.getByText("technical")).toBeInTheDocument();
    expect(card.getByRole("button", { name: "Blocked by Drift D3, D7" })).toHaveTextContent("blocked");
    expect(card.getByRole("button", { name: "Waiting for Task T4" })).toHaveTextContent("waiting T4");
    expect(card.getByText("Checkout")).toBeInTheDocument();
  });

  it("has no coloured border for the label, and the rotating border only while held", async () => {
    await show(
      snapshot({
        active: [
          task("aaaaaaaa", "T1", "Labelled", { label: "Checkout" }),
          task("bbbbbbbb", "T2", "Plain"),
          task("cccccccc", "T3", "Held", { doing: true, heldBy: "jjjjjjjj" }),
        ],
      }),
    );
    const frame = (id: string) => active().querySelector<HTMLElement>(`[data-card-id="${id}"]`)!;
    for (const id of ["aaaaaaaa", "bbbbbbbb", "cccccccc"]) expect(frame(id).className).not.toMatch(/border-l-/);
    expect(within(frame("aaaaaaaa")).getByText("Checkout")).toHaveClass("text-muted-foreground");
    expect(within(frame("aaaaaaaa")).getByText("Checkout").className).not.toMatch(/-ink|border-(sky|sage|peach|lavender|rose)/);
    expect(frame("cccccccc")).toHaveClass("task-held");
    expect(frame("aaaaaaaa")).not.toHaveClass("task-held");
    expect(frame("bbbbbbbb")).not.toHaveClass("task-held");
  });

  it("opens a popover from `blocked` listing the Drifts, and a row focuses that Drift's card", async () => {
    const user = userEvent.setup();
    await show(
      snapshot({
        backlog: [drift("d1111111", "D3", "Total is wrong")],
        active: [task("aaaaaaaa", "T1", "First", { blockedBy: [{ id: "d1111111", shortNumber: "D3", title: "Total is wrong" }] })],
      }),
    );
    await user.click(within(active()).getByRole("button", { name: "Blocked by Drift D3" }));
    const popover = await screen.findByRole("dialog", { name: "Blocked by" });
    await user.click(within(popover).getByRole("button", { name: /D3\s*Total is wrong/ }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Blocked by" })).not.toBeInTheDocument());
    const title = within(backlog()).getByRole("button", { name: "Total is wrong" });
    await waitFor(() => expect(title).toHaveFocus());
  });

  it("opens a popover from `waiting` listing the fix Task", async () => {
    const user = userEvent.setup();
    await show(
      snapshot({
        active: [
          task("aaaaaaaa", "T1", "First", { waitingFor: [{ id: "ffffffff", shortNumber: "T4", title: "Fix the total" }] }),
          task("ffffffff", "T4", "Fix the total"),
        ],
      }),
    );
    await user.click(within(active()).getByRole("button", { name: "Waiting for Task T4" }));
    const popover = await screen.findByRole("dialog", { name: "Waiting for" });
    expect(within(popover).getByRole("button", { name: /T4\s*Fix the total/ })).toBeInTheDocument();
    await user.click(within(popover).getByRole("button", { name: /T4\s*Fix the total/ }));
    await waitFor(() => expect(within(active()).getAllByRole("button", { name: "Fix the total" })[0]).toHaveFocus());
  });
});

describe("Run", () => {
  it("lists every reason it is disabled, in order, and does nothing while disabled", async () => {
    const user = userEvent.setup();
    const runTask = vi.fn();
    const reasons = ["The agent is still finishing", "Drifts are being resolved", "A run is in progress", "Blocked by Drift D3", "Waiting for Task T4"];
    await show(snapshot({ active: [task("aaaaaaaa", "T1", "First", { runReasons: reasons })] }), { runTask });
    const run = within(active()).getByRole("button", { name: "Run" });
    expect(run).toHaveAttribute("aria-disabled", "true");
    expect(reasonsOf(run)).toEqual(reasons);
    await user.click(run);
    expect(runTask).not.toHaveBeenCalled();
  });

  it("runs the Task when it has no reason", async () => {
    const user = userEvent.setup();
    const runTask = vi.fn();
    await show(snapshot({ active: [task("aaaaaaaa", "T1", "First")] }), { runTask });
    const run = within(active()).getByRole("button", { name: "Run" });
    expect(run).not.toHaveAttribute("aria-disabled");
    await user.click(run);
    expect(runTask).toHaveBeenCalledWith("aaaaaaaa");
  });

  it("shows `View requirements` only when its focus has a step, and never `View on diagram`", async () => {
    const user = userEvent.setup();
    const viewOn = vi.fn();
    const focusSteps = vi.fn((view: string) => (view === "requirements" ? 2 : 0));
    await show(snapshot({ active: [task("aaaaaaaa", "T1", "First", { refs: ["order/Paid"] })] }), { viewOn, focusSteps });
    expect(within(active()).queryByRole("button", { name: "View on diagram" })).not.toBeInTheDocument();
    await user.click(within(active()).getByRole("button", { name: "View requirements" }));
    expect(viewOn).toHaveBeenCalledWith("requirements", ["order/Paid"], { kind: "task", id: "aaaaaaaa", number: "T1", title: "First" });
  });

  it("has no `View requirements` when its focus has no step", async () => {
    await show(snapshot({ active: [task("aaaaaaaa", "T1", "First", { refs: ["order/Paid"] })] }), { viewOn: vi.fn(), focusSteps: () => 0 });
    expect(within(active()).queryByRole("button", { name: "View requirements" })).not.toBeInTheDocument();
  });
});

describe("the details of a Task", () => {
  const details = async (card: TaskCard, actions: BoardActions = {}) => {
    const user = userEvent.setup();
    await show(snapshot({ active: [card] }), actions);
    await user.click(within(active()).getByRole("button", { name: card.head.title }));
    return { user, dialog: await screen.findByRole("dialog") };
  };

  it("has `Ask more` after Run and before Delete, never on the card itself, always enabled, and sends the Task identity", async () => {
    const askMore = vi.fn().mockResolvedValue("askaskas");
    const { user, dialog } = await details(task("aaaaaaaa", "T1", "First", { runReasons: ["The agent is still finishing"] }), { askMore });
    const names = within(dialog)
      .getAllByRole("button")
      .map((b) => b.getAttribute("aria-label") ?? b.textContent);
    expect(names.indexOf("Run")).toBeGreaterThanOrEqual(0);
    expect(names.indexOf("Ask more about T1")).toBe(names.indexOf("Run") + 1);
    expect(names.indexOf("Delete")).toBe(names.indexOf("Ask more about T1") + 1);
    const button = within(dialog).getByRole("button", { name: "Ask more about T1" });
    expect(button).toBeEnabled();
    expect(button).toHaveTextContent("Ask more");
    expect(within(screen.getByRole("region", { name: "Active", hidden: true })).queryByRole("button", { name: /Ask more/, hidden: true })).toBeNull();
    await user.click(button);
    expect(askMore).toHaveBeenCalledWith({ kind: "task", id: "aaaaaaaa" });
  });

  it("shows the status, the references as chips and the text", async () => {
    const card = task("aaaaaaaa", "T1", "First", { refs: ["order/Paid", "user"], label: "Checkout" });
    const withBody = { ...card, head: { ...card.head, body: "Carry out the **cart**." } } as TaskCard;
    const { dialog } = await details(withBody);
    expect(dialog).toHaveTextContent("To do");
    expect(dialog).toHaveTextContent("Carry out the cart.");
    expect(within(dialog).getByRole("list", { name: "References" })).toHaveTextContent("order/Paid");
    expect(within(dialog).getByRole("list", { name: "References" })).toHaveTextContent("user");
  });

  it("a chip focuses the first of Requirements and Specs where the reference has a step", async () => {
    const viewOn = vi.fn();
    const { user, dialog } = await details(task("aaaaaaaa", "T1", "First", { refs: ["order/Paid", "user"] }), {
      viewOn,
      focusSteps: (view) => (view === "specs" ? 1 : 0),
    });
    await user.click(within(dialog).getByRole("button", { name: "View specs: user" }));
    expect(viewOn).toHaveBeenCalledWith("specs", ["user"]);
  });

  it("a chip opens Requirements first when both views have a step", async () => {
    const viewOn = vi.fn();
    const { user, dialog } = await details(task("aaaaaaaa", "T1", "First", { refs: ["order/Paid", "user"] }), { viewOn, focusSteps: () => 1 });
    await user.click(within(dialog).getByRole("button", { name: "View requirements: user" }));
    expect(viewOn).toHaveBeenCalledWith("requirements", ["user"]);
    expect(within(dialog).queryByRole("button", { name: /View on diagram/ })).not.toBeInTheDocument();
  });

  it("a chip is plain text when no view has a step for it", async () => {
    const { dialog } = await details(task("aaaaaaaa", "T1", "First", { refs: ["user"] }), { viewOn: vi.fn(), focusSteps: () => 0 });
    expect(within(within(dialog).getByRole("list", { name: "References" })).queryByRole("button")).not.toBeInTheDocument();
  });

  it("has Delete only in To do, lists its reasons in order, and confirms with `Delete T1 for good?`", async () => {
    const reasons = ["The agent is still finishing", "Origin of open Drift D3", "Fix Task of Drift D5, whose code is to be fixed"];
    const { user, dialog } = await details(task("aaaaaaaa", "T1", "First", { deleteReasons: reasons }));
    const remove = within(dialog).getByRole("button", { name: "Delete" });
    expect(remove).toHaveAttribute("aria-disabled", "true");
    expect(reasonsOf(remove)).toEqual(reasons);
    await user.click(remove);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("confirms Delete and sends the decision", async () => {
    const { user, dialog } = await details(task("aaaaaaaa", "T1", "First"));
    await user.click(within(dialog).getByRole("button", { name: "Delete" }));
    const confirm = await screen.findByRole("alertdialog");
    expect(within(confirm).getByText("Delete T1 for good?")).toBeInTheDocument();
    expect(within(confirm).getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.click(within(confirm).getByRole("button", { name: "Delete" }));
    expect(vi.mocked(call)).toHaveBeenCalledWith("delete_task", { projectId: PROJECT, taskId: "aaaaaaaa" });
  });

  it("has no Delete once the Task is in progress, and shows `Run` with the agent's reason", async () => {
    const { dialog } = await details(task("aaaaaaaa", "T1", "First", { doing: true, heldBy: "jjjjjjjj", runReasons: ["The agent is working on it"] }));
    expect(dialog).toHaveTextContent("Agent working");
    expect(within(dialog).queryByRole("button", { name: "Delete" })).not.toBeInTheDocument();
    expect(reasonsOf(within(dialog).getByRole("button", { name: "Run" }))).toEqual(["The agent is working on it"]);
  });
});
