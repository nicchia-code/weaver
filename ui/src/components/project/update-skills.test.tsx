import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { toast } from "sonner";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ProjectStatus } from "@/bindings/ProjectStatus";
import type { SkillsReport } from "@/bindings/SkillsReport";
import { reportView, skillsFailure } from "@/components/project/skills-report";
import { OpeningCheckToast, SkillsBanner, UpdateSkills } from "@/components/project/update-skills";
import { ProjectStatusProvider } from "@/components/project/use-project-status";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));
vi.mock("sonner", () => ({ toast: vi.fn() }));

const PROJECT = "0123456789abcdef";

function status(patch: Partial<ProjectStatus> = {}): ProjectStatus {
  return {
    projectId: PROJECT,
    revision: 1,
    state: "has-model",
    files: [],
    folderHasFiles: false,
    solo: { mode: "off" },
    atomicCommits: { enabled: false },
    check: { opening: 1, outcome: { kind: "none" } },
    ...patch,
  };
}

const generated = { mode: "fragment", budget: 100, limit: 4000, switched: null } as const;
const unchanged: SkillsReport = { changes: [], areas: [], generated };
const changed: SkillsReport = {
  changes: [
    { path: ".omp/WATCHDOG.yml", action: "rewritten" },
    { path: ".agents/skills/weaver-req-ui/SKILL.md", action: "created" },
  ],
  areas: ["ui"],
  generated,
};

let current: ProjectStatus;
let update: () => Promise<unknown>;
let push: (status: ProjectStatus) => void;

beforeEach(() => {
  vi.mocked(toast).mockReset();
  current = status();
  update = () => Promise.resolve(unchanged);
  vi.mocked(on).mockImplementation(((_name: string, handler: (s: ProjectStatus) => void) => {
    push = handler;
    return Promise.resolve(() => {});
  }) as unknown as typeof on);
  vi.mocked(call).mockReset();
  vi.mocked(call).mockImplementation(((name: string) => {
    if (name === "project_status") return Promise.resolve(current);
    if (name === "update_skills") return update();
    return Promise.reject(new Error(`unexpected command ${name}`));
  }) as unknown as typeof call);
});

const inProject = (...children: React.ReactNode[]) =>
  render(<ProjectStatusProvider projectId={PROJECT}>{children}</ProjectStatusProvider>);

describe("the report of Update skills", () => {
  it("words each outcome", () => {
    expect(reportView(unchanged)).toEqual({ kind: "unchanged", text: "Everything was already up to date." });
    const view = reportView(changed);
    expect(view.text).toBe("Updated 2 files:");
    expect(view.kind === "changed" && view.files).toEqual([
      { path: ".omp/WATCHDOG.yml", action: "rewritten" },
      { path: ".agents/skills/weaver-req-ui/SKILL.md", action: "created" },
    ]);
    expect(skillsFailure("disk full.")).toBe("Could not update the agent files: disk full.");
  });
});

describe("Update skills in Settings", () => {
  it("runs, spins and disables the button meanwhile, and leaves the report under it", async () => {
    const user = userEvent.setup();
    let finish: (report: SkillsReport) => void = () => {};
    update = () => new Promise((resolve) => (finish = resolve));
    inProject(<UpdateSkills />);
    const button = screen.getByRole("button", { name: "Update skills" });
    await user.click(button);
    expect(button).toBeDisabled();
    expect(button.querySelector("svg")).toHaveClass("motion-reduce:animate-none");
    await act(async () => finish(changed));
    expect(screen.getByRole("button", { name: "Update skills" })).toBeEnabled();
    expect(screen.getByText("Updated 2 files:")).toBeInTheDocument();
    expect(screen.getByText(".omp/WATCHDOG.yml")).toBeInTheDocument();
    expect(screen.getByText("created")).toBeInTheDocument();
  });

  it("says when everything was up to date, and replaces the report at the next run", async () => {
    const user = userEvent.setup();
    inProject(<UpdateSkills />);
    await user.click(screen.getByRole("button", { name: "Update skills" }));
    expect(await screen.findByText("Everything was already up to date.")).toBeInTheDocument();
    update = () => Promise.resolve(changed);
    await user.click(screen.getByRole("button", { name: "Update skills" }));
    expect(await screen.findByText("Updated 2 files:")).toBeInTheDocument();
    expect(screen.queryByText("Everything was already up to date.")).not.toBeInTheDocument();
  });

  it("shows the error in the error colour", async () => {
    const user = userEvent.setup();
    update = () => Promise.reject({ code: "refused", message: "permission denied" });
    inProject(<UpdateSkills />);
    await user.click(screen.getByRole("button", { name: "Update skills" }));
    expect(await screen.findByText("Could not update the agent files: permission denied.")).toHaveClass("text-rose-ink");
  });
});

describe("the check when the window opens", () => {
  it("says once that it updated the agent files", async () => {
    current = status({ check: { opening: 1, outcome: { kind: "updated", files: 2 } } });
    inProject(<OpeningCheckToast />);
    await act(async () => {});
    expect(toast).toHaveBeenCalledTimes(1);
    expect(toast).toHaveBeenCalledWith("Updated the agent files to this version of Weaver.");
    act(() => push({ ...current, revision: 2 }));
    expect(toast).toHaveBeenCalledTimes(1);
  });

  it("says nothing when nothing was changed", async () => {
    inProject(<OpeningCheckToast />, <SkillsBanner />);
    await act(async () => {});
    expect(toast).not.toHaveBeenCalled();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("shows a banner with Retry that runs Update skills until it succeeds", async () => {
    const user = userEvent.setup();
    current = status({ check: { opening: 1, outcome: { kind: "failed", reason: "permission denied" } } });
    inProject(<SkillsBanner />);
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not update the agent files: permission denied.");
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(call).toHaveBeenCalledWith("update_skills", { projectId: PROJECT });
    act(() => push(status({ revision: 2, check: { opening: 1, outcome: { kind: "none" } } })));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
