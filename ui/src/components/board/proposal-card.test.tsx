import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BoardCard } from "@/bindings/BoardCard";
import type { BoardSnapshot } from "@/bindings/BoardSnapshot";
import type { OmpStatus } from "@/bindings/OmpStatus";
import type { ProposalView } from "@/bindings/ProposalView";
import { BoardProvider } from "@/components/board/board-state";
import { BoardActionsContext, type BoardActions } from "@/components/board/board-ui";
import { BoardView } from "@/components/board/board-view";
import { proposalView } from "@/components/board/proposal-fixtures";
import { ProjectContext } from "@/components/project-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const OMP: { status: Partial<OmpStatus> } = {
  status: {
    state: "ready",
    models: [
      { provider: "anthropic", selector: "anthropic/opus", name: "Claude Opus", thinkingLevels: [] },
      { provider: "openai", selector: "openai/mini", name: null, thinkingLevels: [] },
    ],
  },
};
vi.mock("@/hooks/use-omp", () => ({ useOmp: () => OMP }));

const PROJECT = "0123456789abcdef";

function head(id: string, shortNumber: string | null, title: string, body = "") {
  return { id, shortNumber, title, body, created: "2026-11-02T07:00:00.000Z", agentFinishing: false };
}

type ProposalCard = Extract<BoardCard, { kind: "proposal" }>;

function proposal(patch: Partial<ProposalCard> = {}, view: Partial<ProposalView> = {}): BoardCard {
  return {
    kind: "proposal",
    head: head("pppppppp", "P1", "Cancel an order", "Why the order is cancelled.\n"),
    standing: "open",
    proposalKind: "change",
    view: proposalView(view),
    ...patch,
  };
}

function snapshot(backlog: BoardCard[]): BoardSnapshot {
  return { projectId: PROJECT, revision: 1, backlog, active: [], ghosts: [], gathered: [], runAllReasons: [], doneSinceLastScan: 0, warnings: [] };
}

const showView = vi.fn();
const decisions: { name: string; args: unknown }[] = [];
const handlers = new Map<string, (event: unknown) => void>();
let cards: BoardCard[] = [];
let revision = 1;

async function show(card: BoardCard, actions: BoardActions = {}, fail?: { code: string; message: string }) {
  decisions.length = 0;
  cards = [card];
  revision = 1;
  vi.mocked(on).mockImplementation(((name: string, handler: (event: unknown) => void) => {
    handlers.set(name, handler);
    return Promise.resolve(() => {});
  }) as unknown as typeof on);
  vi.mocked(call).mockImplementation(((name: string, args: unknown) => {
    if (name === "board_snapshot") return Promise.resolve({ ...snapshot(cards), revision });
    decisions.push({ name, args });
    return fail ? Promise.reject(fail) : Promise.resolve(null);
  }) as unknown as typeof call);
  render(
    <TooltipProvider>
      <ProjectContext.Provider value={{ projectId: PROJECT, showView, openJob: vi.fn(), openedJob: null }}>
        <BoardActionsContext.Provider value={actions}>
          <BoardProvider projectId={PROJECT}>
            <BoardView />
          </BoardProvider>
        </BoardActionsContext.Provider>
      </ProjectContext.Provider>
    </TooltipProvider>,
  );
  await screen.findByRole("region", { name: "Backlog" });
}

const backlog = () => screen.getByRole("region", { name: "Backlog" });
const card = () => backlog().querySelector<HTMLElement>("[data-card-id]")!;

/** The reasons a guarded control lists, one per line, as its accessible description holds them. */
function reasonsOf(button: HTMLElement): string[] {
  const own = (button.getAttribute("aria-describedby") ?? "").split(" ")[0] ?? "";
  const lines = [...(document.getElementById(own)?.querySelectorAll(":scope > .block") ?? [])];
  return lines.map((line) => line.textContent ?? "");
}

beforeEach(() => {
  vi.mocked(call).mockReset();
  vi.mocked(on).mockReset();
  showView.mockReset();
});

describe("the card of a Proposal", () => {
  it("shows the id, the title, no kind mark for a change and an outline mark for a setup or an import", async () => {
    await show(proposal());
    const shown = within(card());
    expect(shown.getByRole("img", { name: "Short number (temporary) P1" })).toBeInTheDocument();
    expect(shown.getByRole("button", { name: "Cancel an order" })).toBeInTheDocument();
    expect(shown.queryByText("setup")).not.toBeInTheDocument();
    expect(shown.queryByText("import")).not.toBeInTheDocument();
  });

  it.each(["setup", "import"] as const)("marks a %s with an outline badge", async (kind) => {
    await show(proposal({ proposalKind: kind }));
    expect(within(card()).getByText(kind)).toBeInTheDocument();
  });

  it("lists the badges in order: stale, refining, blocked, the chosen model", async () => {
    await show(
      proposal({ standing: "refining" }, { stale: true, blocked: ["Stale: `model/x.md` changed since the Proposal started", "Proposal P2 is not approved yet"], model: "anthropic/opus" }),
    );
    const badges = within(card())
      .getAllByText(/^(stale|refining|refinement interrupted|blocked|anthropic\/opus)$/)
      .map((badge) => badge.textContent);
    expect(badges).toEqual(["stale", "refining", "blocked", "anthropic/opus"]);
  });

  it("opens the reasons of `blocked` in a popover, each on its own line", async () => {
    const user = userEvent.setup();
    const reasons = ["Stale: `model/x.md` changed since the Proposal started", "Proposal P2 is not approved yet", "The label is missing"];
    await show(proposal({}, { blocked: reasons }));
    await user.click(within(card()).getByRole("button", { name: "Blocked: 3 reasons" }));
    const popover = await screen.findByRole("dialog", { name: "Blocked by" });
    expect(within(popover).getAllByRole("listitem").map((item) => item.textContent)).toEqual(reasons);
  });

  it("says `refinement interrupted` and has only Refine and Discard", async () => {
    const user = userEvent.setup();
    await show(proposal({ standing: "interrupted" }, { carriesTasks: true, creates: 1, label: "Orders", approve: { reasons: [], tooltip: "Continue refining, then approve", refines: true } }));
    const shown = within(card());
    expect(shown.getByText("refinement interrupted")).toBeInTheDocument();
    expect(shown.getByRole("button", { name: "Refine" })).not.toHaveAttribute("aria-disabled");
    expect(shown.queryByRole("button", { name: "Approve" })).not.toBeInTheDocument();
    expect(shown.queryByRole("button", { name: "Model" })).not.toBeInTheDocument();
    expect(shown.getByRole("button", { name: "Discard" })).toBeInTheDocument();
    expect(shown.queryByRole("button", { name: "Reject" })).not.toBeInTheDocument();
    expect(shown.queryByRole("button", { name: "More ways to reject" })).not.toBeInTheDocument();
    await user.hover(shown.getByRole("button", { name: "Refine" }));
    expect((await screen.findAllByText("Continue refining, then approve")).length).toBeGreaterThan(0);
  });

  it("has the controls in order, the model control last at the right edge, with the Settings icon", async () => {
    await show(proposal({}, { carriesTasks: true, creates: 1, label: "Orders", scopeRefs: ["order/Paid"] }), { viewOn: vi.fn(), focusSteps: () => 1 });
    const controls = card().querySelector<HTMLElement>("[data-card-controls]")!;
    const names = within(controls)
      .getAllByRole("button")
      .map((b) => b.getAttribute("aria-label") ?? b.textContent);
    expect(names).toEqual(["Approve", "Reject", "More ways to reject", "View requirements", "Model"]);
    const model = within(controls).getByRole("button", { name: "Model" });
    expect(model).toHaveClass("ml-auto");
    expect(model.querySelector("svg")).toHaveClass("lucide-settings");
    expect(within(controls).getByRole("button", { name: "Approve" })).not.toHaveClass("ml-auto");
  });

  it("keeps the icon `Cpu` on the badge of the chosen model", async () => {
    await show(proposal({}, { carriesTasks: true, creates: 1, label: "Orders", model: "openai/mini" }));
    const badge = within(card()).getByText("openai/mini");
    expect(badge.querySelector("svg")).toHaveClass("lucide-cpu");
  });

  it("confirms Discard before it discards, and a cancel changes nothing", async () => {
    const user = userEvent.setup();
    await show(proposal({ standing: "interrupted" }, { approve: { reasons: [], tooltip: "Continue refining, then approve", refines: true } }));
    await user.click(within(card()).getByRole("button", { name: "Discard" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Discard P1?" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(decisions).toEqual([]);
    await user.click(within(card()).getByRole("button", { name: "Discard" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Discard" }));
    expect(decisions).toEqual([{ name: "discard_proposal", args: { projectId: PROJECT, proposalId: "pppppppp" } }]);
  });
});

describe("the files and Tasks lines", () => {
  it("counts the files with `+a ~c −r`, and the Tasks with the label as a badge", async () => {
    await show(
      proposal(
        {},
        {
          files: { added: ["model/a.md"], changed: ["model/b.md", "specs.yaml"], removed: ["model/c.md"] },
          carriesTasks: true,
          creates: 3,
          label: "Orders",
        },
      ),
    );
    const shown = within(card());
    expect(shown.getByText(/4 files/)).toBeInTheDocument();
    expect(shown.getByText("+1 ~2 −1")).toBeInTheDocument();
    expect(shown.getByText("Creates 3 Tasks")).toBeInTheDocument();
    expect(shown.getByText("Orders")).toBeInTheDocument();
  });

  it("omits the lines that have nothing to say, and the Tasks line for an import", async () => {
    await show(proposal({ proposalKind: "import" }, { files: { added: ["model/a.md"], changed: [], removed: [] } }));
    const shown = within(card());
    expect(shown.getByText(/1 file/)).toBeInTheDocument();
    expect(shown.queryByText(/Creates/)).not.toBeInTheDocument();
    expect(shown.queryByRole("button", { name: "Model" })).not.toBeInTheDocument();
  });
});

describe("Approve", () => {
  it("approves with the ids of the project and the Proposal", async () => {
    const user = userEvent.setup();
    await show(proposal());
    await user.click(within(card()).getByRole("button", { name: "Approve" }));
    expect(decisions).toEqual([{ name: "approve_proposal", args: { projectId: PROJECT, proposalId: "pppppppp" } }]);
  });

  it("lists every reason it is disabled in order and ignores a click", async () => {
    const user = userEvent.setup();
    const reasons = ["The agent is still finishing", "A run is in progress", "Proposal P2 is not approved yet"];
    await show(proposal({}, { approve: { reasons, tooltip: null, refines: false } }));
    const approve = within(card()).getByRole("button", { name: "Approve" });
    expect(approve).toHaveAttribute("aria-disabled", "true");
    expect(reasonsOf(approve)).toEqual(reasons);
    await user.click(approve);
    expect(decisions).toEqual([]);
  });

  it("is a yellow `Refine`, enabled, with its tooltip, when only staleness blocks the Proposal", async () => {
    const user = userEvent.setup();
    await show(proposal({}, { stale: true, blocked: ["Stale: `model/x.md` changed since the Proposal started"], approve: { reasons: [], tooltip: "Refine on the current Model, then approve", refines: true } }));
    expect(within(card()).queryByRole("button", { name: "Approve" })).not.toBeInTheDocument();
    const refine = within(card()).getByRole("button", { name: "Refine" });
    expect(refine).not.toHaveAttribute("aria-disabled");
    expect(refine).toHaveClass("bg-yellow-500", "dark:bg-yellow-400", "text-zinc-950");
    expect(refine).not.toHaveClass("bg-primary");
    expect(refine.querySelector("svg")).toHaveClass("lucide-sparkles");
    await user.hover(refine);
    expect((await screen.findAllByText("Refine on the current Model, then approve")).length).toBeGreaterThan(0);
    await user.click(refine);
    expect(decisions).toEqual([{ name: "approve_proposal", args: { projectId: PROJECT, proposalId: "pppppppp" } }]);
  });

  it("stays a blue `Approve` with a check icon when a click does not refine", async () => {
    await show(proposal());
    const approve = within(card()).getByRole("button", { name: "Approve" });
    expect(approve).toHaveClass("bg-primary");
    expect(approve).not.toHaveClass("bg-yellow-500");
    expect(approve.querySelector("svg")).toHaveClass("lucide-check");
  });

  it("shows a decision refused when applied as a notice and asks for the Board again", async () => {
    const user = userEvent.setup();
    await show(proposal(), {}, { code: "refused", message: "Could not approve P1: A run is in progress." });
    await user.click(within(card()).getByRole("button", { name: "Approve" }));
    expect(await screen.findByText("Could not approve P1: A run is in progress.")).toBeInTheDocument();
    await waitFor(() => expect(vi.mocked(call).mock.calls.filter(([name]) => name === "board_snapshot").length).toBeGreaterThan(1));
  });
});

describe("Reject and Rework with feedback", () => {
  it("opens a Dialog with an optional Reason: Reject sends it, a blank one sends nothing", async () => {
    const user = userEvent.setup();
    await show(proposal());
    await user.click(within(card()).getByRole("button", { name: "Reject" }));
    const dialog = await screen.findByRole("dialog", { name: "Reject P1" });
    await user.type(within(dialog).getByRole("textbox", { name: "Reason" }), "Too big");
    await user.click(within(dialog).getByRole("button", { name: "Reject" }));
    expect(decisions).toEqual([{ name: "reject_proposal", args: { projectId: PROJECT, proposalId: "pppppppp", feedback: "Too big", rework: false } }]);

    decisions.length = 0;
    await user.click(within(card()).getByRole("button", { name: "Reject" }));
    await user.click(within(await screen.findByRole("dialog", { name: "Reject P1" })).getByRole("button", { name: "Reject" }));
    expect(decisions).toEqual([{ name: "reject_proposal", args: { projectId: PROJECT, proposalId: "pppppppp", feedback: null, rework: false } }]);
  });

  it("changes nothing when the dialog is cancelled", async () => {
    const user = userEvent.setup();
    await show(proposal());
    await user.click(within(card()).getByRole("button", { name: "Reject" }));
    await user.click(within(await screen.findByRole("dialog", { name: "Reject P1" })).getByRole("button", { name: "Cancel" }));
    expect(decisions).toEqual([]);
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Reject P1" })).not.toBeInTheDocument());
  });

  it("has `Rework with feedback…` in the menu of the split button, and a Dialog with Feedback and Rework", async () => {
    const user = userEvent.setup();
    await show(proposal());
    await user.click(within(card()).getByRole("button", { name: "More ways to reject" }));
    await user.click(await screen.findByRole("menuitem", { name: "Rework with feedback…" }));
    const dialog = await screen.findByRole("dialog", { name: "Rework P1 with feedback" });
    await user.type(within(dialog).getByRole("textbox", { name: "Feedback" }), "Split it in two");
    await user.click(within(dialog).getByRole("button", { name: "Rework" }));
    expect(decisions).toEqual([{ name: "reject_proposal", args: { projectId: PROJECT, proposalId: "pppppppp", feedback: "Split it in two", rework: true } }]);
  });

  it("asks `Rework without feedback?` for a blank feedback before it reworks", async () => {
    const user = userEvent.setup();
    await show(proposal());
    await user.click(within(card()).getByRole("button", { name: "More ways to reject" }));
    await user.click(await screen.findByRole("menuitem", { name: "Rework with feedback…" }));
    await user.click(within(await screen.findByRole("dialog", { name: "Rework P1 with feedback" })).getByRole("button", { name: "Rework" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Rework without feedback?" });
    expect(decisions).toEqual([]);
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    expect(decisions).toEqual([]);

    await user.click(within(card()).getByRole("button", { name: "More ways to reject" }));
    await user.click(await screen.findByRole("menuitem", { name: "Rework with feedback…" }));
    await user.click(within(await screen.findByRole("dialog", { name: "Rework P1 with feedback" })).getByRole("button", { name: "Rework" }));
    await user.click(within(await screen.findByRole("alertdialog", { name: "Rework without feedback?" })).getByRole("button", { name: "Rework" }));
    expect(decisions).toEqual([{ name: "reject_proposal", args: { projectId: PROJECT, proposalId: "pppppppp", feedback: null, rework: true } }]);
  });

  it("has no Rework once the Proposal is itself a rework", async () => {
    await show(proposal({}, { canRework: false }));
    const shown = within(card());
    expect(shown.getByRole("button", { name: "Reject" })).toBeInTheDocument();
    expect(shown.queryByRole("button", { name: "More ways to reject" })).not.toBeInTheDocument();
  });

  it("is disabled only by the agent still finishing or by being refined, and the menu stays shut", async () => {
    const user = userEvent.setup();
    const reasons = ["The agent is still finishing", "Being refined"];
    await show(proposal({ standing: "refining" }, { rejectReasons: reasons, approve: { reasons: [...reasons, "Stale: `x` changed"], tooltip: null, refines: false } }));
    const shown = within(card());
    const reject = shown.getByRole("button", { name: "Reject" });
    expect(reject).toHaveAttribute("aria-disabled", "true");
    expect(reasonsOf(reject)).toEqual(reasons);
    const arrow = shown.getByRole("button", { name: "More ways to reject" });
    expect(arrow).toHaveAttribute("aria-disabled", "true");
    await user.click(arrow);
    expect(screen.queryByRole("menuitem")).not.toBeInTheDocument();
    await user.click(reject);
    expect(screen.queryByRole("dialog", { name: "Reject P1" })).not.toBeInTheDocument();
    expect(reasonsOf(shown.getByRole("button", { name: "Approve" }))).toEqual([...reasons, "Stale: `x` changed"]);
  });
});

describe("the model control", () => {
  it("exists only when the Proposal carries Tasks, and lists the Settings models first", async () => {
    const user = userEvent.setup();
    await show(proposal({}, { carriesTasks: true, creates: 1, label: "Orders" }));
    await user.click(within(card()).getByRole("button", { name: "Model" }));
    const popover = await screen.findByRole("dialog", { name: "Model" });
    await user.click(within(popover).getByRole("combobox", { name: "Model" }));
    const options = within(await screen.findByRole("listbox")).getAllByRole("option").map((o) => o.textContent);
    expect(options).toEqual(["Use the Settings models", "anthropic/opus", "openai/mini"]);
  });

  it("applies a choice at once and goes back to the Settings models with the first item", async () => {
    const user = userEvent.setup();
    await show(proposal({}, { carriesTasks: true, creates: 1, label: "Orders" }));
    await user.click(within(card()).getByRole("button", { name: "Model" }));
    await user.click(within(await screen.findByRole("dialog", { name: "Model" })).getByRole("combobox", { name: "Model" }));
    await user.click(await screen.findByRole("option", { name: "openai/mini" }));
    expect(decisions).toEqual([{ name: "choose_proposal_model", args: { projectId: PROJECT, proposalId: "pppppppp", model: "openai/mini" } }]);
  });

  it("shows the chosen model as a badge with its icon and can go back to the Settings models", async () => {
    const user = userEvent.setup();
    await show(proposal({}, { carriesTasks: true, creates: 1, label: "Orders", model: "openai/mini" }));
    expect(within(card()).getByText("openai/mini")).toBeInTheDocument();
    await user.click(within(card()).getByRole("button", { name: "Model" }));
    await user.click(within(await screen.findByRole("dialog", { name: "Model" })).getByRole("combobox", { name: "Model" }));
    await user.click(await screen.findByRole("option", { name: "Use the Settings models" }));
    expect(decisions).toEqual([{ name: "choose_proposal_model", args: { projectId: PROJECT, proposalId: "pppppppp", model: null } }]);
  });
});

describe("View requirements", () => {
  it("exists only when its focus has at least one step, and there is no `View on diagram`", async () => {
    const viewOn = vi.fn();
    const focusSteps = vi.fn((view: string) => (view === "requirements" ? 2 : 0));
    await show(proposal({}, { scopeRefs: ["order/Paid"] }), { viewOn, focusSteps });
    const user = userEvent.setup();
    expect(within(card()).queryByRole("button", { name: "View on diagram" })).not.toBeInTheDocument();
    await user.click(within(card()).getByRole("button", { name: "View requirements" }));
    expect(viewOn).toHaveBeenCalledWith("requirements", ["order/Paid"], { kind: "proposal", id: "pppppppp", number: "P1", title: expect.any(String) });
  });

  it("is absent when its focus has no step", async () => {
    await show(proposal({}, { scopeRefs: ["order/Paid"] }), { viewOn: vi.fn(), focusSteps: () => 0 });
    expect(within(card()).queryByRole("button", { name: "View requirements" })).not.toBeInTheDocument();
  });
});

describe("the details dialog", () => {
  async function open(card: BoardCard, extra: BoardActions = {}) {
    const user = userEvent.setup();
    await show(card, { focusSteps: () => 1, viewOn: vi.fn(), ...extra });
    await user.click(within(backlog().querySelector<HTMLElement>("[data-card-id]")!).getByRole("button", { name: card.head.title }));
    return { user, dialog: await screen.findByRole("dialog", { name: /Cancel an order/ }) };
  }

  it("has `Ask more` after the controls of the card and sends the Proposal identity, never on the card itself", async () => {
    const askMore = vi.fn().mockResolvedValue("askaskas");
    const { user, dialog } = await open(proposal({ head: head("pppppppp", "P1", "Cancel an order", "") }, {}), { askMore });
    const names = within(dialog)
      .getAllByRole("button")
      .map((b) => b.getAttribute("aria-label") ?? b.textContent);
    expect(names.indexOf("Ask more about P1")).toBeGreaterThan(names.indexOf("Reject"));
    const button = within(dialog).getByRole("button", { name: "Ask more about P1" });
    expect(button).toBeEnabled();
    expect(within(screen.getByRole("region", { name: "Backlog", hidden: true })).queryByRole("button", { name: /Ask more/, hidden: true })).toBeNull();
    await user.click(button);
    expect(askMore).toHaveBeenCalledWith({ kind: "proposal", id: "pppppppp" });
  });

  it("shows the id, the title, the kind, the badges, the text, what it requires, its files and its Tasks", async () => {
    const { dialog } = await open(
      proposal(
        { proposalKind: "setup", head: head("pppppppp", "P1", "Cancel an order", "Why the order is cancelled.\n") },
        {
          stale: true,
          blocked: ["Proposal P2 is not approved yet"],
          carriesTasks: true,
          creates: 2,
          label: "Orders",
          files: { added: ["model/a.md"], changed: ["model/b.md"], removed: ["model/c.md"] },
          requires: [{ id: "qqqqqqqq", shortNumber: "P2", title: "Carts", state: "Open", problem: "is not approved yet" }],
          tasks: [
            { id: "tttttttt", title: "Cancel the order", body: "Set it to Cancelled.\n" },
            { id: "uuuuuuuu", title: "Wire the queue", body: "" },
          ],
          removalTasks: [{ id: "removal:order/Gone", title: "Remove code of `order/Gone`", body: "" }],
          scopeRefs: ["order/Paid"],
        },
      ),
    );
    const shown = within(dialog);
    expect(shown.getByRole("img", { name: "Short number (temporary) P1" })).toBeInTheDocument();
    expect(shown.getByText("Why the order is cancelled.")).toBeInTheDocument();
    expect(shown.getAllByText("setup").length).toBeGreaterThan(0);
    expect(shown.getByText("stale")).toBeInTheDocument();
    expect(shown.getByRole("button", { name: "Blocked: 1 reason" })).toBeInTheDocument();
    const requires = within(shown.getByRole("region", { name: "Requires" }));
    expect(requires.getByText("Carts")).toBeInTheDocument();
    expect(requires.getByText("Open")).toBeInTheDocument();
    const files = shown.getByRole("region", { name: "Files" });
    expect(within(within(files).getByRole("group", { name: "Added" })).getByText("model/a.md")).toBeInTheDocument();
    expect(within(within(files).getByRole("group", { name: "Changed" })).getByText("model/b.md")).toBeInTheDocument();
    expect(within(within(files).getByRole("group", { name: "Removed" })).getByText("model/c.md")).toBeInTheDocument();
    const tasks = within(shown.getByRole("region", { name: "Tasks it creates" }));
    expect(tasks.getAllByRole("button").map((b) => b.textContent)).toEqual(["Cancel the order", "Wire the queue"]);
    expect(shown.queryByText("technical")).not.toBeInTheDocument();
    expect(shown.queryByText("order/Paid")).not.toBeInTheDocument();
    expect(shown.queryByRole("list", { name: /References of/ })).not.toBeInTheDocument();
    const removal = within(shown.getByRole("region", { name: "Code-removal Tasks" }));
    const gone = removal.getByText("order/Gone");
    expect(gone.tagName).toBe("CODE");
    expect(removal.getByRole("listitem").textContent).toBe("Remove code of order/Gone");
    expect(removal.queryByRole("button")).not.toBeInTheDocument();
  });

  it("shows `Refine` in its footer when a click refines", async () => {
    const { dialog } = await open(proposal({}, { stale: true, approve: { reasons: [], tooltip: "Refine on the current Model, then approve", refines: true } }));
    expect(within(dialog).getByRole("button", { name: "Refine" })).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "Approve" })).not.toBeInTheDocument();
  });

  describe("a Task it creates", () => {
    async function openWithTasks() {
      return open(
        proposal(
          {},
          {
            carriesTasks: true,
            creates: 3,
            label: "Orders",
            tasks: [
              { id: "tttttttt", title: "Cancel the order", body: "Set the order to **Cancelled**.\n" },
              { id: "uuuuuuuu", title: "Wire the queue", body: "  \n" },
              { id: "vvvvvvvv", title: "Wire the queue", body: "The other one.\n" },
            ],
          },
        ),
      );
    }

    it("opens a nested Dialog over the details dialog with its title and its rendered text, and Close returns the focus to the row", async () => {
      const { user, dialog } = await openWithTasks();
      const row = within(dialog).getByRole("button", { name: "Cancel the order" });
      await user.click(row);
      const nested = await screen.findByRole("dialog", { name: "Cancel the order" });
      expect(within(nested).getByRole("heading", { name: "Cancel the order" })).toBeInTheDocument();
      expect(within(nested).getByText("Cancelled").tagName).toBe("STRONG");
      expect(within(nested).queryByText("technical")).not.toBeInTheDocument();
      expect(within(nested).queryByText("No description.")).not.toBeInTheDocument();

      await user.click(within(nested).getByRole("button", { name: "Close" }));
      await waitFor(() => expect(screen.queryByRole("dialog", { name: "Cancel the order" })).not.toBeInTheDocument());
      expect(screen.getByRole("dialog", { name: /Cancel an order/ })).toBeInTheDocument();
      expect(row).toHaveFocus();
    });

    it("opens with the keyboard, and Esc closes only the nested Dialog and returns the focus to the row", async () => {
      const { user, dialog } = await openWithTasks();
      const row = within(dialog).getByRole("button", { name: "Cancel the order" });
      row.focus();
      await user.keyboard("{Enter}");
      await screen.findByRole("dialog", { name: "Cancel the order" });
      await user.keyboard("{Escape}");
      await waitFor(() => expect(screen.queryByRole("dialog", { name: "Cancel the order" })).not.toBeInTheDocument());
      expect(screen.getByRole("dialog", { name: /Cancel an order/ })).toBeInTheDocument();
      expect(row).toHaveFocus();
    });

    it("says `No description.` for a blank text and tells apart Tasks with the same title", async () => {
      const { user, dialog } = await openWithTasks();
      const rows = within(dialog).getAllByRole("button", { name: "Wire the queue" });
      expect(rows).toHaveLength(2);
      await user.click(rows[0]!);
      expect(within(await screen.findByRole("dialog", { name: "Wire the queue" })).getByText("No description.")).toBeInTheDocument();
      await user.keyboard("{Escape}");
      await waitFor(() => expect(screen.queryByRole("dialog", { name: "Wire the queue" })).not.toBeInTheDocument());
      await user.click(rows[1]!);
      expect(within(await screen.findByRole("dialog", { name: "Wire the queue" })).getByText("The other one.")).toBeInTheDocument();
    });

    it("keeps the details dialog open when it closes, and a second Esc closes that one", async () => {
      const { user, dialog } = await openWithTasks();
      await user.click(within(dialog).getByRole("button", { name: "Cancel the order" }));
      await screen.findByRole("dialog", { name: "Cancel the order" });
      await user.keyboard("{Escape}");
      await waitFor(() => expect(screen.queryByRole("dialog", { name: "Cancel the order" })).not.toBeInTheDocument());
      expect(screen.getByRole("dialog", { name: /Cancel an order/ })).toBeInTheDocument();
      await user.keyboard("{Escape}");
      await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    });

    it("closes with the details dialog", async () => {
      const { user, dialog } = await openWithTasks();
      await user.click(within(dialog).getByRole("button", { name: "Cancel the order" }));
      await screen.findByRole("dialog", { name: "Cancel the order" });
      // The Proposal leaves the Board: the details dialog closes by itself.
      cards = [];
      revision = 2;
      await act(async () => handlers.get("project-changed")?.({ projectId: PROJECT, revision: 2, slices: ["board"] }));
      await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    });
  });

  it("ends with the controls of the card in their order, the model control last, then `Ask more`", async () => {
    const { dialog } = await open(proposal({}, { carriesTasks: true, creates: 1, label: "Orders", scopeRefs: ["order/Paid"] }), {
      askMore: vi.fn(),
      viewOn: vi.fn(),
      focusSteps: () => 1,
    });
    const names = within(dialog)
      .getAllByRole("button")
      .map((b) => b.getAttribute("aria-label") ?? b.textContent)
      .filter((name) => ["Approve", "Reject", "More ways to reject", "View requirements", "Model", "Ask more about P1"].includes(name ?? ""));
    expect(names).toEqual(["Approve", "Reject", "More ways to reject", "View requirements", "Model", "Ask more about P1"]);
  });
});

describe("the card of a resolution", () => {
  const resolution: BoardCard = {
    kind: "resolution",
    head: head("rrrrrrrr", "P4", "Resolve: Cart total"),
    standing: "open",
    drifts: [
      { drift: { id: "dddddddd", shortNumber: "D1" }, decision: "fix-code", option: 2, note: "Mind the lock" },
      { drift: { id: "eeeeeeee", shortNumber: "D2" }, decision: "proposal", option: null, note: null },
      { drift: { id: "ffffffff", shortNumber: null }, decision: "dismissed", option: null, note: null },
    ],
    view: proposalView(),
  };

  it("has a yellow border and tint, the mark `Resolution` and one row per Drift", async () => {
    await show(resolution);
    const shown = within(card());
    expect(card().className).toContain("border-l-yellow-600");
    expect(card().className).toContain("bg-yellow-500/10");
    expect(shown.getByText("Resolution")).toBeInTheDocument();
    expect(shown.getByText("Fix the code: option 2")).toBeInTheDocument();
    expect(shown.getByText(/Mind the lock/)).toBeInTheDocument();
    expect(shown.getByText("Change the Model")).toBeInTheDocument();
    expect(shown.getByText("Let it be")).toBeInTheDocument();
    expect(shown.getByText("ffffffff")).toBeInTheDocument();
    // Same controls as any Proposal; no Tasks line.
    expect(shown.getByRole("button", { name: "Approve" })).toBeInTheDocument();
    expect(shown.queryByText(/Creates/)).not.toBeInTheDocument();
  });
});
