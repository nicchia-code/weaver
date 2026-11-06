import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BoardCard } from "@/bindings/BoardCard";
import type { JobRow } from "@/bindings/JobRow";
import type { ProjectStatus } from "@/bindings/ProjectStatus";
import { proposalView } from "@/components/board/proposal-fixtures";
import { driftView } from "@/components/board/drift-fixtures";
import { proposalOpen, SetupLayer, setupWorks } from "@/components/project/empty-model";
import { ProjectStatusProvider } from "@/components/project/use-project-status";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const mocks = vi.hoisted(() => ({
  jobs: [] as unknown[],
  backlog: [] as unknown[],
  newRequest: vi.fn(),
  omp: { state: "ready" } as { state: string } | null,
}));
vi.mock("@/components/jobs/use-jobs", () => ({ useJobs: () => ({ jobs: mocks.jobs }) }));
vi.mock("@/components/board/board-state", () => ({ useBoard: () => ({ snapshot: { backlog: mocks.backlog } }) }));
vi.mock("@/components/requests/use-requests", () => ({ useRequests: () => ({ newRequest: mocks.newRequest }) }));
vi.mock("@/hooks/use-omp", () => ({ useOmp: () => ({ status: mocks.omp }) }));
vi.mock("@/components/omp-status", () => ({ OmpSetup: () => <div role="status">omp is not ready</div> }));

const PROJECT = "0123456789abcdef";

const job = (kind: JobRow["kind"], state: JobRow["state"]): JobRow => ({
  id: "job00001",
  kind,
  name: "Setup",
  state,
  reason: null,
  created: "2026-11-02T07:00:00.000Z",
  started: null,
  ended: null,
  note: null,
  target: null,
});

const proposal = { kind: "proposal", head: { id: "pppppppp", shortNumber: "P1", title: "Setup", body: "", created: "", agentFinishing: false }, standing: "open", proposalKind: "setup", view: proposalView() } as BoardCard;

let state: ProjectStatus["state"];
let folderHasFiles: boolean;

beforeEach(() => {
  mocks.jobs = [];
  mocks.backlog = [];
  mocks.omp = { state: "ready" };
  mocks.newRequest.mockReset();
  state = "empty-model";
  folderHasFiles = false;
  vi.mocked(on).mockImplementation((() => Promise.resolve(() => {})) as unknown as typeof on);
  vi.mocked(call).mockImplementation(((name: string) => {
    if (name === "project_status") {
      return Promise.resolve({
        projectId: PROJECT,
        revision: 1,
        state,
        files: [],
        folderHasFiles,
        solo: { mode: "off" },
        atomicCommits: { enabled: false },
        check: { opening: 0, outcome: { kind: "none" } },
      });
    }
    if (name === "project_folder") return Promise.resolve([]);
    return Promise.reject(new Error(`unexpected command ${name}`));
  }) as unknown as typeof call);
});

function renderLayer() {
  return render(
    <TooltipProvider>
      <ProjectStatusProvider projectId={PROJECT}>
        <SetupLayer>
          <p>The Board</p>
        </SetupLayer>
      </ProjectStatusProvider>
    </TooltipProvider>,
  );
}

describe("the setup layer of an empty Model", () => {
  it("decides what hides the cards", () => {
    expect(setupWorks([job("setup", "running"), job("run", "running")])).toBe(true);
    expect(setupWorks([job("import", "waiting")])).toBe(true);
    expect(setupWorks([job("setup", "done"), job("import", "failed"), job("run", "running")])).toBe(false);
    expect(proposalOpen([proposal])).toBe(true);
    expect(proposalOpen([{ kind: "drift", head: proposal.head, origin: null, view: driftView() }])).toBe(false);
  });

  it("shows the two cards above the Board, side by side with their texts", async () => {
    renderLayer();
    expect(await screen.findByRole("button", { name: /Start from Scratch/ })).toHaveTextContent(
      "For an application still to be built: you describe it, an agent designs it.",
    );
    expect(screen.getByRole("button", { name: /Import from Code/ })).toHaveTextContent(
      "For a project that already has code: an agent describes it as Proposals.",
    );
    expect(screen.getByText("The Board")).toBeInTheDocument();
    expect(screen.queryByText(/Start from Scratch ignores/)).not.toBeInTheDocument();
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("opens a new setup request with Start from Scratch", async () => {
    renderLayer();
    await userEvent.setup().click(await screen.findByRole("button", { name: /Start from Scratch/ }));
    expect(mocks.newRequest).toHaveBeenCalledTimes(1);
  });

  it("warns that Start from Scratch ignores the files already in the folder", async () => {
    folderHasFiles = true;
    renderLayer();
    expect(
      await screen.findByText("Start from Scratch ignores the files already in this folder unless your request mentions them."),
    ).toBeInTheDocument();
  });

  it("opens the import panel with Import from Code", async () => {
    renderLayer();
    await userEvent.setup().click(await screen.findByRole("button", { name: /Import from Code/ }));
    expect(await screen.findByRole("button", { name: "Start import" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Start from Scratch/ })).not.toBeInTheDocument();
  });

  it("hides the cards while a setup or import job works, and while a Proposal is open", async () => {
    mocks.jobs = [job("setup", "running")];
    const { unmount } = renderLayer();
    expect(await screen.findByText("The Board")).toBeInTheDocument();
    await Promise.resolve();
    expect(screen.queryByRole("button", { name: /Start from Scratch/ })).not.toBeInTheDocument();
    unmount();
    mocks.jobs = [];
    mocks.backlog = [proposal];
    renderLayer();
    expect(await screen.findByText("The Board")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Import from Code/ })).not.toBeInTheDocument();
  });

  it("shows the omp status box above the cards while omp is not ready", async () => {
    mocks.omp = { state: "missing" };
    renderLayer();
    expect(await screen.findByRole("status")).toHaveTextContent("omp is not ready");
    expect(screen.getByRole("button", { name: /Start from Scratch/ })).toBeInTheDocument();
  });

  it("shows only the Board in a project that has a Model", async () => {
    state = "has-model";
    renderLayer();
    expect(await screen.findByText("The Board")).toBeInTheDocument();
    await Promise.resolve();
    expect(screen.queryByRole("button", { name: /Start from Scratch/ })).not.toBeInTheDocument();
  });
});
