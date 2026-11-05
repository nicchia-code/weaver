import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BoardCard } from "@/bindings/BoardCard";
import type { BoardSnapshot } from "@/bindings/BoardSnapshot";
import type { DriftView } from "@/bindings/DriftView";
import { BoardProvider } from "@/components/board/board-state";
import { BoardActionsContext, type BoardActions } from "@/components/board/board-ui";
import { BoardView } from "@/components/board/board-view";
import { attachedProposal, driftView } from "@/components/board/drift-fixtures";
import { proposalView } from "@/components/board/proposal-fixtures";
import { ProjectContext } from "@/components/project-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const PROJECT = "0123456789abcdef";

function drift(view: Partial<DriftView> = {}, agentFinishing = false): BoardCard {
  return {
    kind: "drift",
    head: { id: "dddddddd", shortNumber: "D3", title: "The code ships twice", body: "", created: "2026-11-02T07:00:00.000Z", agentFinishing },
    origin: { id: "tttttttt", shortNumber: "T1", title: "Ship" },
    view: driftView({
      refs: ["order/Paid"],
      happens: "It ships twice.",
      should: "It ships once.",
      options: [
        { title: "Deduplicate", text: "Keep one shipment." },
        { title: "Lock", text: "Lock the order." },
      ],
      whereInCode: "`src/ship.rs:42`",
      places: [{ file: "src/ship.rs", line: 42 }],
      ...view,
    }),
  };
}

let snapshot: BoardSnapshot;
const decisions: { name: string; args: unknown }[] = [];

async function show(cards: BoardCard[], patch: Partial<BoardSnapshot> = {}, actions: BoardActions = {}) {
  decisions.length = 0;
  snapshot = { projectId: PROJECT, revision: 1, backlog: cards, active: [], ghosts: [], gathered: [], runAllReasons: [], doneSinceLastScan: 0, warnings: [], ...patch };
  vi.mocked(on).mockImplementation((() => Promise.resolve(() => {})) as unknown as typeof on);
  vi.mocked(call).mockImplementation(((name: string, args: unknown) => {
    if (name === "board_snapshot") return Promise.resolve(snapshot);
    if (name === "open_in_editor") return Promise.resolve(null);
    decisions.push({ name, args });
    return Promise.resolve(null);
  }) as unknown as typeof call);
  render(
    <TooltipProvider>
      <ProjectContext.Provider value={{ projectId: PROJECT, showView: vi.fn(), openJob: vi.fn(), openedJob: null }}>
        <BoardActionsContext.Provider value={{ focusSteps: () => 1, viewOn: vi.fn(), ...actions }}>
          <BoardProvider projectId={PROJECT}>
            <BoardView />
          </BoardProvider>
        </BoardActionsContext.Provider>
      </ProjectContext.Provider>
    </TooltipProvider>,
  );
  await screen.findByRole("region", { name: "Backlog" });
}

const card = () => screen.getByRole("region", { name: "Backlog" }).querySelector<HTMLElement>("[data-card-id]")!;

function reasonsOf(button: HTMLElement): string[] {
  const own = (button.getAttribute("aria-describedby") ?? "").split(" ")[0] ?? "";
  return [...(document.getElementById(own)?.querySelectorAll(":scope > .block") ?? [])].map((line) => line.textContent ?? "");
}

beforeEach(() => {
  vi.mocked(call).mockReset();
  vi.mocked(on).mockReset();
});

describe("the card of a Drift", () => {
  it("has a red left border, the id, the title, Resolve and Dismiss Drift", async () => {
    await show([drift()]);
    const shown = within(card());
    expect(card().className).toContain("border-l-red-600");
    expect(shown.getByRole("img", { name: "Short number (temporary) D3" })).toBeInTheDocument();
    expect(shown.getByRole("button", { name: "The code ships twice" })).toBeInTheDocument();
    expect(shown.getByRole("button", { name: "Resolve" })).not.toHaveAttribute("aria-disabled");
    expect(shown.getByRole("button", { name: "Dismiss Drift" })).not.toHaveAttribute("aria-disabled");
  });

  it("shows recurring, resolving and prepared, in that order, each with its tooltip", async () => {
    const user = userEvent.setup();
    await show([drift({ previous: { id: "eeeeeeee", shortNumber: "D1", title: "Old" }, resolving: true, prepared: true })]);
    const badges = within(card()).getAllByText(/^(recurring|resolving|prepared)$/);
    expect(badges.map((badge) => badge.textContent)).toEqual(["recurring", "resolving", "prepared"]);
    const tips = ["Repeats Drift D1: Old", "An agent is resolving this Drift", "Resolve asks the prepared question at once"];
    for (const [index, tip] of tips.entries()) {
      await user.unhover(badges[0]!);
      act(() => badges[index]!.focus());
      expect((await screen.findAllByText(tip)).length).toBeGreaterThan(0);
    }
  });

  it("shows `merge` after recurring, resolving and prepared, as an outline badge with its tooltip", async () => {
    const user = userEvent.setup();
    await show([drift({ previous: { id: "eeeeeeee", shortNumber: "D1", title: "Old" }, resolving: true, prepared: true, merge: true })]);
    const badges = within(card()).getAllByText(/^(recurring|resolving|prepared|merge)$/);
    expect(badges.map((badge) => badge.textContent)).toEqual(["recurring", "resolving", "prepared", "merge"]);
    await user.unhover(badges[0]!);
    act(() => badges[3]!.focus());
    expect((await screen.findAllByText("Found by the scan after a merge")).length).toBeGreaterThan(0);
  });

  it("has no merge badge on a Drift that does not come from a merge", async () => {
    await show([drift()]);
    expect(within(card()).queryByText("merge")).not.toBeInTheDocument();
  });

  it("lists `A merge is in progress` on Resolve third, after `A run is in progress`", async () => {
    await show([drift({ resolveReasons: ["A run is in progress", "A merge is in progress", "Being refined"] })]);
    expect(reasonsOf(within(card()).getByRole("button", { name: "Resolve" }))).toEqual(["A run is in progress", "A merge is in progress", "Being refined"]);
  });

  it("lists the reasons Resolve is disabled in order, the agent first, and ignores a click", async () => {
    const user = userEvent.setup();
    const reasons = ["A run is in progress", "This Drift is being resolved", "Another resolution is being written", "Proposal P4 resolving Drifts awaits a decision"];
    await show([drift({ resolveReasons: reasons }, true)]);
    const resolve = within(card()).getByRole("button", { name: "Resolve" });
    expect(resolve).toHaveAttribute("aria-disabled", "true");
    expect(reasonsOf(resolve)).toEqual(["The agent is still finishing", ...reasons]);
    await user.click(resolve);
    expect(decisions).toEqual([]);
  });

  it("resolves with the ids of the project and the Drift", async () => {
    await show([drift()]);
    await userEvent.setup().click(within(card()).getByRole("button", { name: "Resolve" }));
    expect(decisions).toEqual([{ name: "resolve_drift", args: { projectId: PROJECT, driftId: "dddddddd" } }]);
  });

  it("confirms Dismiss Drift with its sentence, Cancel changes nothing, and lists its reasons", async () => {
    const user = userEvent.setup();
    await show([drift()]);
    await user.click(within(card()).getByRole("button", { name: "Dismiss Drift" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Dismiss D3? Nothing changes in the code or in the Model." });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(decisions).toEqual([]);
    await user.click(within(card()).getByRole("button", { name: "Dismiss Drift" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Dismiss" }));
    expect(decisions).toEqual([{ name: "dismiss_drift", args: { projectId: PROJECT, driftId: "dddddddd" } }]);
  });

  it("disables Dismiss Drift with its reasons in order", async () => {
    await show([drift({ dismissReasons: ["This Drift is in a resolution"] }, true)]);
    const dismiss = within(card()).getByRole("button", { name: "Dismiss Drift" });
    expect(dismiss).toHaveAttribute("aria-disabled", "true");
    expect(reasonsOf(dismiss)).toEqual(["The agent is still finishing", "This Drift is in a resolution"]);
  });
});

describe("the details of a Drift", () => {
  async function open(card: BoardCard, patch: Partial<BoardSnapshot> = {}, actions: BoardActions = {}) {
    const user = userEvent.setup();
    await show([card], patch, actions);
    await user.click(within(screen.getByRole("region", { name: "Backlog" })).getByRole("button", { name: "The code ships twice" }));
    return { user, dialog: await screen.findByRole("dialog") };
  }

  it("has `Ask more` after Resolve and Dismiss Drift, never on the card, and sends the Drift identity", async () => {
    const askMore = vi.fn().mockResolvedValue("askaskas");
    const { user, dialog } = await open(drift(), {}, { askMore });
    const names = within(dialog)
      .getAllByRole("button")
      .map((b) => b.getAttribute("aria-label") ?? b.textContent);
    expect(names.indexOf("Dismiss Drift")).toBeGreaterThan(names.indexOf("Resolve"));
    expect(names.indexOf("Ask more about D3")).toBeGreaterThan(names.indexOf("Dismiss Drift"));
    const button = within(dialog).getByRole("button", { name: "Ask more about D3" });
    expect(button).toBeEnabled();
    expect(within(screen.getByRole("region", { name: "Backlog", hidden: true })).queryByRole("button", { name: /Ask more/, hidden: true })).toBeNull();
    await user.click(button);
    expect(askMore).toHaveBeenCalledWith({ kind: "drift", id: "dddddddd" });
  });

  it("shows chips, the two sides, numbered options with the first recommended, and where in the code", async () => {
    const { user, dialog } = await open(drift());
    expect(within(dialog).getByRole("list", { name: "References" })).toHaveTextContent("order/Paid");
    expect(within(dialog).getByText("What happens")).toBeInTheDocument();
    expect(within(dialog).getByText("It ships once.")).toBeInTheDocument();
    const options = within(within(dialog).getByRole("region", { name: "Fix options" })).getAllByRole("listitem");
    expect(options[0]).toHaveTextContent("Deduplicate");
    expect(options[0]).toHaveTextContent("recommended");
    expect(options[1]).not.toHaveTextContent("recommended");
    expect(within(dialog).getAllByText("src/ship.rs:42").length).toBeGreaterThan(0);
    await user.click(within(dialog).getByRole("button", { name: "Open in editor" }));
    expect(vi.mocked(call)).toHaveBeenCalledWith("open_in_editor", { file: "src/ship.rs", line: 42, projectId: PROJECT });
    expect(dialog).toHaveTextContent("Found while working on T1 Ship");
  });

  it("fixes the code with the recommended option preselected, the chosen one and the note", async () => {
    const { user, dialog } = await open(drift());
    expect(within(dialog).getByRole("radio", { name: "Deduplicate" })).toBeChecked();
    await user.click(within(dialog).getByRole("radio", { name: "Lock" }));
    await user.type(within(dialog).getByRole("textbox", { name: "Note" }), "Mind the lock");
    await user.click(within(dialog).getByRole("button", { name: "Fix the code" }));
    expect(decisions).toEqual([{ name: "fix_drift_code", args: { projectId: PROJECT, driftId: "dddddddd", option: 2, note: "Mind the lock" } }]);
  });

  it("puts the agent first on Fix the code", async () => {
    const { dialog } = await open(drift({}, true));
    const fix = within(dialog).getByRole("button", { name: "Fix the code" });
    expect(fix).toHaveAttribute("aria-disabled", "true");
    expect(reasonsOf(fix)).toEqual(["The agent is still finishing"]);
  });

  it("on a Drift from a merge marks Accept Proposal as the recommended one, preselects no option and keeps Fix the code disabled until one is chosen", async () => {
    const { user, dialog } = await open(drift({ merge: true, attached: attachedProposal() }));
    const options = within(within(dialog).getByRole("region", { name: "Fix options" })).getAllByRole("listitem");
    expect(options[0]).toHaveTextContent("Deduplicate");
    expect(options.map((option) => option.textContent)).toEqual(["DeduplicateKeep one shipment.", "LockLock the order."]);
    expect(within(within(dialog).getByRole("region", { name: "Fix options" })).queryByText("recommended")).not.toBeInTheDocument();
    const accept = within(dialog).getByRole("button", { name: "Accept Proposal" });
    expect(accept.parentElement).toHaveTextContent("recommended");
    expect(within(dialog).getAllByText("recommended")).toHaveLength(1);

    expect(within(dialog).getAllByRole("radio").every((radio) => !(radio as HTMLInputElement).checked && radio.getAttribute("aria-checked") !== "true")).toBe(true);
    const fix = within(dialog).getByRole("button", { name: "Fix the code" });
    expect(fix).toHaveAttribute("aria-disabled", "true");
    await user.click(fix);
    expect(decisions).toEqual([]);
    await user.click(within(dialog).getByRole("radio", { name: "Lock" }));
    expect(within(dialog).getByRole("button", { name: "Fix the code" })).not.toHaveAttribute("aria-disabled");
    await user.click(within(dialog).getByRole("button", { name: "Fix the code" }));
    expect(decisions).toEqual([{ name: "fix_drift_code", args: { projectId: PROJECT, driftId: "dddddddd", option: 2, note: null } }]);
  });

  it("shows the merge badge, with its tooltip, in the details of a Drift from a merge", async () => {
    const { dialog } = await open(drift({ merge: true }));
    const badge = within(dialog).getByText("merge");
    act(() => badge.focus());
    expect((await screen.findAllByText("Found by the scan after a merge")).length).toBeGreaterThan(0);
  });

  it("replaces the options by a sentence without Fix the code, and Accept Proposal lists the Proposal's reasons", async () => {
    const attached = attachedProposal({
      body: "The Model says what the code does.",
      files: { added: [], changed: ["model/shop/order.md"], removed: [] },
      acceptReasons: ["A run is in progress", "Not approved yet"],
    });
    const { user, dialog } = await open(drift({ options: [], attached }));
    expect(within(dialog).getByText("No code change can respect both the Model and the Requirements.")).toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "Fix the code" })).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("radio")).not.toBeInTheDocument();
    expect(dialog).toHaveTextContent("Paid orders");
    expect(dialog).toHaveTextContent("Open");
    expect(dialog).not.toHaveTextContent("stale");
    expect(dialog).toHaveTextContent("~ model/shop/order.md");
    const accept = within(dialog).getByRole("button", { name: "Accept Proposal" });
    expect(reasonsOf(accept)).toEqual(attached.acceptReasons);
    await user.click(accept);
    expect(decisions).toEqual([]);
  });

  it("accepts the Proposal when it can be approved, and repeats Resolve and Dismiss Drift in the footer", async () => {
    const { user, dialog } = await open(drift({ attached: attachedProposal() }));
    expect(within(dialog).queryByRole("button", { name: "Refine" })).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Accept Proposal" }));
    expect(decisions).toEqual([{ name: "accept_drift_proposal", args: { projectId: PROJECT, driftId: "dddddddd" } }]);
    expect(within(dialog).getAllByRole("button", { name: "Resolve" }).length).toBeGreaterThan(0);
    expect(within(dialog).getAllByRole("button", { name: "Dismiss Drift" }).length).toBeGreaterThan(0);
  });

  it("replaces Accept Proposal by Refine on a stale Open Proposal, shows the stale badge, and refines without deciding", async () => {
    const attached = attachedProposal({
      stale: true,
      acceptReasons: ["Stale: `model/shop/order.md` changed since the Proposal started"],
      refine: { reasons: [], tooltip: "Refine on the current Model", refines: true },
    });
    const { user, dialog } = await open(drift({ attached }));
    expect(dialog).toHaveTextContent("stale");
    expect(within(dialog).queryByRole("button", { name: "Accept Proposal" })).not.toBeInTheDocument();
    expect(within(dialog).queryByRole("button", { name: "Discard" })).not.toBeInTheDocument();
    const refine = within(dialog).getByRole("button", { name: "Refine" });
    expect(refine).not.toHaveAttribute("aria-disabled");
    act(() => refine.focus());
    expect((await screen.findAllByText("Refine on the current Model")).length).toBeGreaterThan(0);
    await user.click(refine);
    expect(decisions).toEqual([{ name: "refine_drift_proposal", args: { projectId: PROJECT, driftId: "dddddddd" } }]);
  });

  it("disables Refine only for the agent still finishing", async () => {
    const attached = attachedProposal({
      stale: true,
      acceptReasons: ["A run is in progress"],
      refine: { reasons: ["The agent is still finishing"], tooltip: "Refine on the current Model", refines: true },
    });
    const { user, dialog } = await open(drift({ attached }));
    const refine = within(dialog).getByRole("button", { name: "Refine" });
    expect(refine).toHaveAttribute("aria-disabled", "true");
    expect(reasonsOf(refine)).toEqual(["The agent is still finishing"]);
    await user.click(refine);
    expect(decisions).toEqual([]);
  });

  it("puts the agent that filed the Drift first on Refine", async () => {
    const attached = attachedProposal({ stale: true, refine: { reasons: [], tooltip: "Refine on the current Model", refines: true } });
    const { dialog } = await open(drift({ attached }, true));
    expect(reasonsOf(within(dialog).getByRole("button", { name: "Refine" }))).toEqual(["The agent is still finishing"]);
  });

  it("says `Refinement interrupted` and has Refine (Continue refining) and a confirmed Discard instead of Accept Proposal", async () => {
    const attached = attachedProposal({
      state: "Refinement interrupted",
      acceptReasons: ["The attached Proposal is still a Draft"],
      refine: { reasons: [], tooltip: "Continue refining", refines: true },
      discardReasons: [],
    });
    const { user, dialog } = await open(drift({ attached }));
    expect(dialog).toHaveTextContent("Refinement interrupted");
    expect(within(dialog).queryByRole("button", { name: "Accept Proposal" })).not.toBeInTheDocument();
    const refine = within(dialog).getByRole("button", { name: "Refine" });
    act(() => refine.focus());
    expect((await screen.findAllByText("Continue refining")).length).toBeGreaterThan(0);
    await user.click(refine);
    expect(decisions).toEqual([{ name: "refine_drift_proposal", args: { projectId: PROJECT, driftId: "dddddddd" } }]);

    decisions.length = 0;
    await user.click(within(dialog).getByRole("button", { name: "Discard" }));
    const confirm = await screen.findByRole("alertdialog", { name: "Discard P2?" });
    await user.click(within(confirm).getByRole("button", { name: "Cancel" }));
    expect(decisions).toEqual([]);
    await user.click(within(dialog).getByRole("button", { name: "Discard" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Discard" }));
    expect(decisions).toEqual([{ name: "discard_drift_proposal", args: { projectId: PROJECT, driftId: "dddddddd" } }]);
  });

  it("shows `Refining`, and while it lasts Accept Proposal, Fix the code, Resolve and Dismiss Drift say `Being refined`", async () => {
    const attached = attachedProposal({ state: "Refining", stale: true, acceptReasons: ["Being refined"] });
    const { user, dialog } = await open(
      drift({
        attached,
        fixReasons: ["Being refined"],
        resolveReasons: ["Being refined"],
        dismissReasons: ["Being refined"],
      }),
    );
    expect(dialog).toHaveTextContent("Refining");
    expect(within(dialog).queryByRole("button", { name: "Refine" })).not.toBeInTheDocument();
    for (const name of ["Accept Proposal", "Fix the code"]) {
      const button = within(dialog).getByRole("button", { name });
      expect(button).toHaveAttribute("aria-disabled", "true");
      expect(reasonsOf(button)).toEqual(["Being refined"]);
    }
    for (const name of ["Resolve", "Dismiss Drift"]) {
      for (const button of within(dialog).getAllByRole("button", { name })) {
        expect(reasonsOf(button)).toEqual(["Being refined"]);
      }
    }
    await user.click(within(dialog).getByRole("button", { name: "Fix the code" }));
    expect(decisions).toEqual([]);
  });

  it("opens the details of a Drift gathered into a resolution from its row", async () => {
    const user = userEvent.setup();
    const gathered = drift();
    const resolution: BoardCard = {
      kind: "resolution",
      head: { id: "rrrrrrrr", shortNumber: "P4", title: "Resolve: The code ships twice", body: "", created: "2026-11-02T07:00:00.000Z", agentFinishing: false },
      standing: "open",
      drifts: [{ drift: { id: "dddddddd", shortNumber: "D3" }, decision: "dismissed", option: null, note: null }],
      view: proposalView(),
    };
    await show([resolution], { gathered: [gathered] });
    await user.click(within(card()).getByRole("button", { name: /D3/ }));
    const dialog = await screen.findByRole("dialog");
    expect(dialog).toHaveTextContent("It ships twice.");
    await waitFor(() => expect(within(dialog).getByRole("list", { name: "References" })).toBeInTheDocument());
  });
});
