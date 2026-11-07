import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BoardCard } from "@/bindings/BoardCard";
import type { BoardSnapshot } from "@/bindings/BoardSnapshot";
import type { JobRow } from "@/bindings/JobRow";
import type { MergeAnswer } from "@/bindings/MergeAnswer";
import type { MergeBranches } from "@/bindings/MergeBranches";
import type { MergeStatus } from "@/bindings/MergeStatus";
import type { ProjectStatus } from "@/bindings/ProjectStatus";
import { BoardProvider } from "@/components/board/board-state";
import { BoardActionsContext, type BoardActions } from "@/components/board/board-ui";
import { BoardView } from "@/components/board/board-view";
import { JobsProvider } from "@/components/jobs/use-jobs";
import { MergeProvider } from "@/components/merge/use-merge";
import { ProjectContext } from "@/components/project-context";
import { ProjectStatusProvider } from "@/components/project/use-project-status";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const PROJECT = "0123456789abcdef";

const AVAILABLE: MergeStatus = { available: true, current: "main" };
const BRANCHES: MergeBranches = { current: "main", local: ["dev", "feature/cart"], remote: ["origin/dev", "origin/release"], tracked: [] };

function conflicts(count: number): MergeStatus {
  return { available: false, reason: "A merge is in progress", current: "main", inProgress: { branch: "dev", current: "main", conflicts: count } };
}

function statusOf(revision: number, merge?: MergeStatus): ProjectStatus {
  return {
    projectId: PROJECT,
    revision,
    state: "has-model",
    files: [],
    folderHasFiles: true,
    check: { opening: 0, outcome: "none" },
    solo: { mode: "on" },
    atomicCommits: { enabled: false },
    ...(merge ? { merge } : {}),
  } as unknown as ProjectStatus;
}

function board(patch: Partial<BoardSnapshot> = {}): BoardSnapshot {
  return { projectId: PROJECT, revision: 1, backlog: [], active: [], ghosts: [], gathered: [], runAllReasons: [], doneSinceLastScan: 0, warnings: [], ...patch };
}

function task(runReasons: string[]): BoardCard {
  return {
    kind: "task",
    head: { id: "tttttttt", shortNumber: "T1", title: "Write the cart", body: "", created: "2026-11-02T07:00:00.000Z", agentFinishing: false },
    doing: false,
    heldBy: null,
    technical: false,
    refs: [],
    label: null,
    blockedBy: [],
    waitingFor: [],
    runReasons,
    deleteReasons: [],
  } as BoardCard;
}

function mergeJob(state: JobRow["state"]): JobRow {
  return { id: "jjjjjjjj", kind: "merge", name: "Merge conflicts", state, reason: null, created: "2026-11-05T10:00:00.000Z", started: null, ended: null, note: null, target: null, request: null, requestSent: false };
}

let calls: Array<[string, unknown]>;
let handlers: Map<string, (payload: unknown) => void>;
let status: ProjectStatus;
let snapshot: BoardSnapshot;
let jobs: JobRow[];
let branches: MergeBranches | { code: string; message: string };
let started: (() => Promise<MergeAnswer>) | null;
let aborted: MergeAnswer | { code: string; message: string };
let resolved: MergeStatus | { code: string; message: string };

function install(merge?: MergeStatus) {
  calls = [];
  handlers = new Map();
  status = statusOf(1, merge);
  snapshot = board();
  jobs = [];
  branches = BRANCHES;
  started = null;
  aborted = { merge: AVAILABLE };
  resolved = conflicts(2);
  vi.mocked(on).mockImplementation(((name: string, handler: (payload: unknown) => void) => {
    handlers.set(name, handler);
    return Promise.resolve(() => {});
  }) as unknown as typeof on);
  vi.mocked(call).mockImplementation(((name: string, args: unknown) => {
    calls.push([name, args]);
    switch (name) {
      case "project_status":
        return Promise.resolve(status);
      case "board_snapshot":
        return Promise.resolve(snapshot);
      case "jobs_snapshot":
        return Promise.resolve({ projectId: PROJECT, jobs, ompReady: true });
      case "merge_refresh":
        return Promise.resolve(status.merge ?? null);
      case "merge_branches":
        return "code" in branches ? Promise.reject(branches) : Promise.resolve(branches);
      case "merge_start":
        return started ? started() : Promise.resolve({ merge: AVAILABLE });
      case "merge_abort":
        return "code" in aborted ? Promise.reject(aborted) : Promise.resolve(aborted);
      case "resolve_merge_conflicts":
        return "code" in resolved ? Promise.reject(resolved) : Promise.resolve(resolved);
      default:
        return Promise.reject(new Error(`unexpected command ${name}`));
    }
  }) as unknown as typeof call);
}

async function show(actions: BoardActions = {}) {
  render(
    <TooltipProvider>
      <ProjectContext.Provider value={{ projectId: PROJECT, showView: vi.fn(), openJob: vi.fn(), openedJob: null, openedJobRequest: 0 }}>
        <BoardActionsContext.Provider value={actions}>
          <ProjectStatusProvider projectId={PROJECT}>
            <BoardProvider projectId={PROJECT}>
              <JobsProvider projectId={PROJECT}>
                <MergeProvider projectId={PROJECT}>
                  <BoardView />
                </MergeProvider>
              </JobsProvider>
            </BoardProvider>
          </ProjectStatusProvider>
        </BoardActionsContext.Provider>
      </ProjectContext.Provider>
    </TooltipProvider>,
  );
  await screen.findByRole("region", { name: "Backlog" });
  // Wait for the project status and the jobs, which come in their own time.
  await waitFor(() => expect(calls.map(([name]) => name)).toContain("jobs_snapshot"));
}

/** The backend sends a new status of the project. */
async function publish(next: ProjectStatus) {
  status = next;
  await act(async () => {
    handlers.get("project-state")?.(next);
  });
}

/** The reasons a guarded control lists, one per line, as its accessible description holds them. */
function reasonsOf(button: HTMLElement): string[] {
  const own = (button.getAttribute("aria-describedby") ?? "").split(" ")[0] ?? "";
  return [...(document.getElementById(own)?.querySelectorAll(":scope > .block") ?? [])].map((line) => line.textContent ?? "");
}

const header = () => screen.getByRole("heading", { level: 1, name: "Board" }).closest("header")!;
const mergeButton = () => within(header()).findByRole("button", { name: "Merge" });

beforeEach(() => {
  install();
});

describe("the Merge button", () => {
  it("is absent while Solo mode is off", async () => {
    await show();
    expect(within(header()).queryByRole("button", { name: "Merge" })).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Merge in progress" })).not.toBeInTheDocument();
  });

  it("sits to the left of Scan System in the header and is enabled when git allows it", async () => {
    install(AVAILABLE);
    await show();
    const button = await mergeButton();
    expect(button).not.toHaveAttribute("aria-disabled");
    const buttons = within(header()).getAllByRole("button").map((b) => b.textContent);
    expect(buttons.slice(0, 2)).toEqual(["Merge", "Scan System"]);
    expect(buttons[2]).toMatch(/^Run all/);
    await userEvent.setup().hover(button);
    expect((await screen.findAllByText("Merge another branch into main"))[0]).toBeInTheDocument();
  });

  it.each([
    "The agent is working",
    "A merge is in progress",
    "Merge needs git: git was not found",
    "Git is in the middle of a merge, rebase or cherry-pick",
    "Not on a branch",
    "Commit or stash your changes first",
  ])("is disabled with `%s`, the backend's reason, as its tooltip and description", async (reason) => {
    install({ available: false, reason });
    await show();
    const button = await mergeButton();
    expect(button).toHaveAttribute("aria-disabled", "true");
    expect(reasonsOf(button)).toEqual([reason]);
    await userEvent.setup().click(button);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(calls.some(([name]) => name === "merge_branches")).toBe(false);
  });

  it("asks the backend again each time the Board is shown and follows the status it sends", async () => {
    install({ available: false, reason: "The agent is working" });
    await show();
    expect(calls.filter(([name]) => name === "merge_refresh")).toHaveLength(1);
    expect(reasonsOf(await mergeButton())).toEqual(["The agent is working"]);
    await publish(statusOf(2, AVAILABLE));
    await waitFor(() => expect(within(header()).getByRole("button", { name: "Merge" })).not.toHaveAttribute("aria-disabled"));
  });
});

describe("the dialog", () => {
  beforeEach(() => install(AVAILABLE));

  async function open() {
    await show();
    const user = userEvent.setup();
    await user.click(await mergeButton());
    return { user, dialog: await screen.findByRole("dialog", { name: "Merge a branch" }) };
  }

  it("lists the local branches and then the remote ones under their headings, and filters them", async () => {
    const { user, dialog } = await open();
    const search = await within(dialog).findByRole("combobox", { name: "Search branches" });
    expect(search).toHaveFocus();
    expect(within(dialog).getAllByRole("group").map((group) => group.textContent)).toEqual(["Localdevfeature/cart", "Remoteorigin/devorigin/release"]);
    await user.type(search, "REL");
    expect(within(dialog).getAllByRole("option").map((option) => option.textContent)).toEqual(["origin/release"]);
    expect(within(dialog).queryByText("Local")).not.toBeInTheDocument();
    await user.clear(search);
    await user.type(search, "zzz");
    expect(within(dialog).queryAllByRole("option")).toHaveLength(0);
    expect(calls.filter(([name]) => name === "merge_branches")).toHaveLength(1);
  });

  it("moves with the arrows and chooses with Enter, then asks for the confirmation naming both branches", async () => {
    const { user, dialog } = await open();
    const search = await within(dialog).findByRole("combobox", { name: "Search branches" });
    const options = within(dialog).getAllByRole("option");
    expect(options[0]).toHaveAttribute("aria-selected", "true");
    expect(search).toHaveAttribute("aria-activedescendant", options[0]!.id);
    await user.keyboard("{ArrowDown}");
    expect(within(dialog).getAllByRole("option")[1]).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{Enter}");
    expect(await within(dialog).findByText("Merge feature/cart into main?")).toBeInTheDocument();
    expect(within(dialog).queryByRole("alert")).not.toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Merge" })).toBeEnabled();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toBeEnabled();
    expect(calls.some(([name]) => name === "merge_start")).toBe(false);
  });

  it("chooses with a click and reminds of the Weaver files git still tracks", async () => {
    branches = { ...BRANCHES, tracked: [".weaver/tasks/a.md", ".weaver/model/b.md", ".weaver/specs.yaml"] };
    const { user, dialog } = await open();
    await user.click(await within(dialog).findByRole("option", { name: "origin/dev" }));
    expect(within(dialog).getByText("Merge origin/dev into main?")).toBeInTheDocument();
    expect(within(dialog).getByRole("alert")).toHaveTextContent("Git still tracks 3 of Weaver's files: their changes go into the merge commit.");
  });

  it("says `No other branch` when git knows no other branch", async () => {
    branches = { current: "main", local: [], remote: [], tracked: [] };
    const { dialog } = await open();
    expect(await within(dialog).findByText("No other branch")).toBeInTheDocument();
    expect(within(dialog).queryAllByRole("option")).toHaveLength(0);
  });

  it("shows a spinner and disables both buttons while git works, and closes when git answers", async () => {
    let answer: (value: MergeAnswer) => void = () => {};
    started = () => new Promise<MergeAnswer>((resolve) => (answer = resolve));
    const { user, dialog } = await open();
    await user.click(await within(dialog).findByRole("option", { name: "dev" }));
    await user.click(within(dialog).getByRole("button", { name: "Merge" }));
    expect(calls).toContainEqual(["merge_start", { projectId: PROJECT, branch: "dev" }]);
    const merging = within(dialog).getByRole("button", { name: "Merge" });
    expect(merging).toBeDisabled();
    expect(merging.querySelector("svg.animate-spin")).not.toBeNull();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toBeDisabled();
    await user.keyboard("{Escape}");
    expect(screen.getByRole("dialog", { name: "Merge a branch" })).toBeInTheDocument();

    await act(async () => answer({ merge: conflicts(2) }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    // The answer shows at once, before the backend's next status.
    expect(await screen.findByRole("region", { name: "Merge in progress" })).toBeInTheDocument();
  });

  it("closes on Cancel without starting anything", async () => {
    const { user, dialog } = await open();
    await user.click(await within(dialog).findByRole("option", { name: "dev" }));
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(calls.some(([name]) => name === "merge_start")).toBe(false);
  });

  it("shows the reason when git cannot list the branches", async () => {
    branches = { code: "refused", message: "git did not answer within 10 s" };
    await show();
    await userEvent.setup().click(await mergeButton());
    expect(await within(await screen.findByRole("dialog")).findByRole("alert")).toHaveTextContent("git did not answer within 10 s");
  });
});

describe("the notices", () => {
  beforeEach(() => install(AVAILABLE));
  async function merge(notice: { text: string; paths?: string[] }) {
    started = () => Promise.resolve({ merge: AVAILABLE, notice });
    await show();
    const user = userEvent.setup();
    await user.click(await mergeButton());
    const dialog = await screen.findByRole("dialog");
    await user.click(await within(dialog).findByRole("option", { name: "dev" }));
    await user.click(within(dialog).getByRole("button", { name: "Merge" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    return user;
  }

  it.each([
    "main is already up to date with dev.",
    "Could not merge dev: error: Your local changes would be overwritten.",
    "Could not merge dev: git did not finish the merge within 2 minutes.",
  ])("shows `%s` at the top of the Board with a close button", async (text) => {
    const user = await merge({ text });
    const alert = (await screen.findAllByRole("alert")).find((a) => a.textContent?.includes(text))!;
    expect(alert).toBeInTheDocument();
    await user.click(within(alert).getByRole("button", { name: "Close notice" }));
    expect(screen.queryByText(text)).not.toBeInTheDocument();
  });

  it("lists the files Solo mode keeps out of git in monospace", async () => {
    await merge({ text: "Could not merge dev: it would overwrite files that Solo mode keeps out of git.", paths: [".weaver/solo/solo.json", ".weaver/merge/merge.json"] });
    const alert = (await screen.findAllByRole("alert")).find((a) => a.textContent?.includes("Solo mode keeps out of git"))!;
    const items = within(alert).getAllByRole("listitem");
    expect(items.map((item) => item.textContent)).toEqual([".weaver/solo/solo.json", ".weaver/merge/merge.json"]);
    expect(items[0]!.closest("ul")).toHaveClass("font-mono");
  });

  it("shows a merge the backend forgot, which comes as a notice event", async () => {
    install(AVAILABLE);
    await show();
    await act(async () => {
      handlers.get("notice")?.({ projectId: PROJECT, id: "n1", text: "The merge of dev ended without a commit: Weaver forgot it and did not scan." });
    });
    expect(await screen.findByText("The merge of dev ended without a commit: Weaver forgot it and did not scan.")).toBeInTheDocument();
  });
});

describe("the banner", () => {
  it("is a region named `Merge in progress`, above the notices, with the number of files with conflicts", async () => {
    install(conflicts(3));
    await show();
    const banner = await screen.findByRole("region", { name: "Merge in progress" });
    expect(banner).toHaveTextContent("Merging dev into main: 3 files with conflicts. Resolve them here or by hand, then commit the merge in git.");
    expect(banner.querySelector("[aria-live='polite']")).toHaveTextContent("Merging dev into main: 3 files");
    expect(within(banner).queryByRole("button", { name: "Close" })).not.toBeInTheDocument();
    const board = screen.getByRole("heading", { level: 1, name: "Board" }).closest("div")!;
    expect(board.contains(banner)).toBe(true);
  });

  it("changes its text when no conflicts are left and then has no Resolve with the agent", async () => {
    install(conflicts(2));
    await show();
    const banner = await screen.findByRole("region", { name: "Merge in progress" });
    expect(within(banner).getByRole("button", { name: "Resolve with the agent" })).toBeInTheDocument();
    await publish(statusOf(2, conflicts(0)));
    await waitFor(() => expect(banner).toHaveTextContent("Merging dev into main: no conflicts left. Commit the merge in git to finish."));
    expect(within(banner).queryByRole("button", { name: "Resolve with the agent" })).not.toBeInTheDocument();
    expect(within(banner).getByRole("button", { name: "Abort merge" })).toBeInTheDocument();
  });

  it("disappears when the merge ends", async () => {
    install(conflicts(2));
    await show();
    await screen.findByRole("region", { name: "Merge in progress" });
    await publish(statusOf(2, AVAILABLE));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Merge in progress" })).not.toBeInTheDocument());
  });

  it("starts the agent with `Resolve with the agent`", async () => {
    install(conflicts(2));
    await show();
    const banner = await screen.findByRole("region", { name: "Merge in progress" });
    await userEvent.setup().click(within(banner).getByRole("button", { name: "Resolve with the agent" }));
    expect(calls).toContainEqual(["resolve_merge_conflicts", { projectId: PROJECT }]);
  });

  it("says why when the backend refuses the agent", async () => {
    install(conflicts(2));
    resolved = { code: "refused", message: "The agent is resolving the conflicts" };
    await show();
    const banner = await screen.findByRole("region", { name: "Merge in progress" });
    await userEvent.setup().click(within(banner).getByRole("button", { name: "Resolve with the agent" }));
    expect(await screen.findByText("The agent is resolving the conflicts")).toBeInTheDocument();
  });

  it.each(["queued", "running", "waiting", "suspended"] as const)("waits with its two actions disabled while the Merge conflicts job is %s", async (state) => {
    install(conflicts(2));
    jobs = [mergeJob(state)];
    await show();
    const banner = await screen.findByRole("region", { name: "Merge in progress" });
    await waitFor(() => expect(within(banner).getByRole("button", { name: "Abort merge" })).toHaveAttribute("aria-disabled", "true"));
    for (const name of ["Resolve with the agent", "Abort merge"]) {
      const button = within(banner).getByRole("button", { name });
      expect(reasonsOf(button)).toEqual(["The agent is resolving the conflicts"]);
    }
    await userEvent.setup().click(within(banner).getByRole("button", { name: "Resolve with the agent" }));
    expect(calls.some(([name]) => name === "resolve_merge_conflicts")).toBe(false);
  });

  it("acts again once the job has ended", async () => {
    install(conflicts(2));
    jobs = [mergeJob("done")];
    await show();
    const banner = await screen.findByRole("region", { name: "Merge in progress" });
    expect(within(banner).getByRole("button", { name: "Abort merge" })).not.toHaveAttribute("aria-disabled");
  });
});

describe("Abort merge", () => {
  async function ask() {
    install(conflicts(2));
    await show();
    const user = userEvent.setup();
    const banner = await screen.findByRole("region", { name: "Merge in progress" });
    await user.click(within(banner).getByRole("button", { name: "Abort merge" }));
    return { user, dialog: await screen.findByRole("alertdialog") };
  }

  it("confirms first, with the focus on Cancel, and Cancel changes nothing", async () => {
    const { user, dialog } = await ask();
    expect(within(dialog).getByText("Abort the merge of dev? Every change of the merge, conflict resolutions included, is lost.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(calls.some(([name]) => name === "merge_abort")).toBe(false);
  });

  it("aborts once confirmed, and the answer ends the banner", async () => {
    const { user, dialog } = await ask();
    await user.click(within(dialog).getByRole("button", { name: "Abort merge" }));
    expect(calls).toContainEqual(["merge_abort", { projectId: PROJECT }]);
    await waitFor(() => expect(screen.queryByRole("region", { name: "Merge in progress" })).not.toBeInTheDocument());
  });

  it("keeps the merge and shows git's error when it cannot abort", async () => {
    install(conflicts(2));
    aborted = { merge: conflicts(2), notice: { text: "Could not abort the merge: fatal: There is no merge to abort." } };
    await show();
    const user = userEvent.setup();
    const banner = await screen.findByRole("region", { name: "Merge in progress" });
    await user.click(within(banner).getByRole("button", { name: "Abort merge" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Abort merge" }));
    expect(await screen.findByText("Could not abort the merge: fatal: There is no merge to abort.")).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Merge in progress" })).toBeInTheDocument();
  });
});

describe("the other controls during a merge", () => {
  it("disables Scan System with `A merge is in progress` and enables it again afterwards", async () => {
    const scanSystem = vi.fn();
    install(conflicts(2));
    await show({ scanSystem });
    const user = userEvent.setup();
    const scan = await within(header()).findByRole("button", { name: "Scan System" });
    await waitFor(() => expect(scan).toHaveAttribute("aria-disabled", "true"));
    expect(reasonsOf(scan)).toEqual(["A merge is in progress"]);
    await user.click(scan);
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    await publish(statusOf(2, AVAILABLE));
    await waitFor(() => expect(scan).not.toHaveAttribute("aria-disabled"));
  });

  it("shows `A merge is in progress` first on Run all and on Run, as the backend counts them", async () => {
    install(conflicts(2));
    snapshot = board({ runAllReasons: ["A merge is in progress", "Drifts are being resolved"], active: [task(["The agent is still finishing", "A merge is in progress", "Drifts are being resolved"])] });
    await show();
    const runAll = await screen.findByRole("button", { name: /^Run all/ });
    expect(reasonsOf(runAll)).toEqual(["A merge is in progress", "Drifts are being resolved"]);
    const run = await screen.findByRole("button", { name: "Run" });
    expect(reasonsOf(run)).toEqual(["The agent is still finishing", "A merge is in progress", "Drifts are being resolved"]);
  });
});
