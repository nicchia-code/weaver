import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ProjectStatus } from "@/bindings/ProjectStatus";
import { GetStarted, initialiseFailure } from "@/components/project/get-started";
import { ProjectGate } from "@/components/project/project-gate";
import { ProjectStatusProvider } from "@/components/project/use-project-status";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const PROJECT = "0123456789abcdef";
const FILES = [".agents/skills/weaver-setup/SKILL.md", ".omp/agents/weaver-analysis.md", ".omp/WATCHDOG.yml"];

function status(patch: Partial<ProjectStatus> = {}): ProjectStatus {
  return {
    projectId: PROJECT,
    revision: 1,
    state: "not-initialised",
    files: FILES,
    folderHasFiles: false,
    solo: { mode: "off" },
    atomicCommits: { enabled: false },
    check: { opening: 0, outcome: { kind: "none" } },
    ...patch,
  };
}

let push: (status: ProjectStatus) => void;
let initialise: (force: boolean) => Promise<unknown>;
/** What the next `project_status` differs in (the state of Solo mode, for instance). */
let patch: Partial<ProjectStatus>;

beforeEach(() => {
  vi.mocked(call).mockReset();
  vi.mocked(on).mockImplementation(((_name: string, handler: (s: ProjectStatus) => void) => {
    push = handler;
    return Promise.resolve(() => {});
  }) as unknown as typeof on);
  patch = {};
  initialise = () => Promise.resolve({ conflicts: [] });
  vi.mocked(call).mockImplementation(((name: string, args: { force: boolean }) => {
    if (name === "project_status") return Promise.resolve(status(patch));
    if (name === "initialise_project") return initialise(args.force);
    return Promise.reject(new Error(`unexpected command ${name}`));
  }) as unknown as typeof call);
});

function renderGetStarted() {
  return render(
    <ProjectStatusProvider projectId={PROJECT}>
      <GetStarted />
    </ProjectStatusProvider>,
  );
}

describe("Get Started", () => {
  it("explains the folder, lists the agent files and offers Initialise", async () => {
    renderGetStarted();
    expect(await screen.findByRole("heading", { name: "Set up Weaver in this folder" })).toBeInTheDocument();
    expect(
      screen.getByText(
        "Weaver will create a .weaver folder that describes this project. It holds no secrets and is meant to be committed, like the agent files it installs.",
      ),
    ).toBeInTheDocument();
    const files = within(screen.getByRole("list", { name: "Agent files" })).getAllByRole("listitem");
    expect(files.map((item) => item.textContent)).toEqual(FILES);
    expect(screen.getByRole("button", { name: "Initialise" })).toBeEnabled();
  });

  it("initialises and asks the backend for the new status", async () => {
    const user = userEvent.setup();
    renderGetStarted();
    await user.click(await screen.findByRole("button", { name: "Initialise" }));
    expect(call).toHaveBeenCalledWith("initialise_project", { projectId: PROJECT, force: false, solo: "off" });
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("asks before overwriting agent files, with Cancel focused, and changes nothing on Cancel", async () => {
    const user = userEvent.setup();
    initialise = () => Promise.resolve({ conflicts: [".omp/WATCHDOG.yml", ".agents/skills/weaver-setup/SKILL.md"] });
    renderGetStarted();
    const button = await screen.findByRole("button", { name: "Initialise" });
    await user.click(button);
    const dialog = await screen.findByRole("alertdialog", { name: "Some agent files already exist" });
    expect(within(dialog).getAllByRole("listitem").map((item) => item.textContent)).toEqual([
      ".omp/WATCHDOG.yml",
      ".agents/skills/weaver-setup/SKILL.md",
    ]);
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(vi.mocked(call).mock.calls.filter(([name]) => name === "initialise_project")).toHaveLength(1);
    expect(button).toHaveFocus();
  });

  it("overwrites and initialises when the developer confirms", async () => {
    const user = userEvent.setup();
    initialise = (force) => Promise.resolve({ conflicts: force ? [] : [".omp/WATCHDOG.yml"] });
    renderGetStarted();
    await user.click(await screen.findByRole("button", { name: "Initialise" }));
    await user.click(await screen.findByRole("button", { name: "Overwrite and initialise" }));
    expect(call).toHaveBeenCalledWith("initialise_project", { projectId: PROJECT, force: true, solo: "off" });
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("shows `Could not initialise: <reason>.` under the button and keeps the button for a retry", async () => {
    const user = userEvent.setup();
    initialise = () => Promise.reject({ code: "refused", message: "the folder is read-only" });
    renderGetStarted();
    await user.click(await screen.findByRole("button", { name: "Initialise" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not initialise: the folder is read-only.");
    expect(screen.getByRole("button", { name: "Initialise" })).toBeEnabled();
    expect(initialiseFailure("no space left.")).toBe("Could not initialise: no space left.");
  });

  describe("Solo mode", () => {
    const NOTE = "Applies only to this working copy: other clones start with it off.";
    const WARNING = "Weaver's files are not versioned anywhere: losing this working copy loses them.";
    const PLAIN = "Weaver will create a .weaver folder that describes this project. It holds no secrets and is meant to be committed, like the agent files it installs.";
    const WITH_SOLO =
      "Weaver will create a .weaver folder that describes this project. With Solo mode its files stay only in this working copy and never go into a commit.";

    it("sits between the list of agent files and Initialise, chosen on Off, with its note as its description", async () => {
      renderGetStarted();
      const control = await screen.findByRole("radiogroup", { name: "Solo mode" });
      const files = screen.getByRole("list", { name: "Agent files" });
      const button = screen.getByRole("button", { name: "Initialise" });
      expect(files.compareDocumentPosition(control) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
      expect(control.compareDocumentPosition(button) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
      expect(within(control).getAllByRole("radio").map((radio) => radio.textContent)).toEqual([
        "Off",
        "Whole folders",
        "Weaver's files only",
      ]);
      expect(screen.getByRole("radio", { name: "Off" })).toBeChecked();
      expect(control).toHaveAccessibleDescription(NOTE);
      expect(screen.getByText(PLAIN)).toBeInTheDocument();
      expect(screen.queryByText(WARNING)).not.toBeInTheDocument();
    });

    it.each([
      ["Whole folders", "folders"],
      ["Weaver's files only", "files"],
    ])("with `%s` the text follows the choice and the warning shows under the control", async (label, mode) => {
      const user = userEvent.setup();
      renderGetStarted();
      await user.click(await screen.findByRole("radio", { name: label }));
      expect(screen.getByRole("radio", { name: label })).toBeChecked();
      expect(screen.getByText(WITH_SOLO)).toBeInTheDocument();
      expect(screen.queryByText(PLAIN)).not.toBeInTheDocument();
      const warning = screen.getByText(WARNING);
      expect(warning.closest("[aria-live]")).toHaveAttribute("aria-live", "polite");
      expect(warning).toHaveClass("text-peach-ink");
      expect(screen.getByRole("radiogroup", { name: "Solo mode" })).toHaveAccessibleDescription(`${NOTE} ${WARNING}`);

      await user.click(screen.getByRole("button", { name: "Initialise" }));
      expect(call).toHaveBeenCalledWith("initialise_project", { projectId: PROJECT, force: false, solo: mode });

      // Back to Off: the first text and no warning.
      await user.click(screen.getByRole("radio", { name: "Off" }));
      expect(screen.getByText(PLAIN)).toBeInTheDocument();
      expect(screen.queryByText(WARNING)).not.toBeInTheDocument();
    });

    it("sends the choice with `Overwrite and initialise` too", async () => {
      const user = userEvent.setup();
      initialise = (force) => Promise.resolve({ conflicts: force ? [] : [".omp/WATCHDOG.yml"] });
      renderGetStarted();
      await user.click(await screen.findByRole("radio", { name: "Weaver's files only" }));
      await user.click(screen.getByRole("button", { name: "Initialise" }));
      expect(call).toHaveBeenCalledWith("initialise_project", { projectId: PROJECT, force: false, solo: "files" });
      await user.click(await screen.findByRole("button", { name: "Overwrite and initialise" }));
      expect(call).toHaveBeenCalledWith("initialise_project", { projectId: PROJECT, force: true, solo: "files" });
    });

    it("keeps the choice only in the screen: nothing is sent before Initialise", async () => {
      const user = userEvent.setup();
      renderGetStarted();
      await user.click(await screen.findByRole("radio", { name: "Whole folders" }));
      expect(vi.mocked(call).mock.calls.map(([name]) => name)).toEqual(["project_status"]);
    });

    it("shows a Solo mode failure as `Could not initialise: <reason>.` and keeps the button and the choice for a retry", async () => {
      const user = userEvent.setup();
      initialise = () => Promise.reject({ code: "refused", message: "this folder is not in a git repository" });
      renderGetStarted();
      await user.click(await screen.findByRole("radio", { name: "Whole folders" }));
      await user.click(screen.getByRole("button", { name: "Initialise" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("Could not initialise: this folder is not in a git repository.");
      expect(screen.getByRole("button", { name: "Initialise" })).toBeEnabled();
      expect(screen.getByRole("radio", { name: "Whole folders" })).toBeChecked();
    });

    it.each([
      ["git was not found", "Solo mode needs git: git was not found."],
      ["this folder is not in a git repository", "Solo mode needs git: this folder is not in a git repository."],
    ])("without git (%s) it is disabled on Off, says why, and Initialise sends off", async (reason, text) => {
      const user = userEvent.setup();
      patch = { solo: { mode: "off", unavailable: reason } };
      renderGetStarted();
      const control = await screen.findByRole("radiogroup", { name: "Solo mode" });
      for (const radio of within(control).getAllByRole("radio")) expect(radio).toBeDisabled();
      expect(screen.getByRole("radio", { name: "Off" })).toBeChecked();
      expect(screen.getByText(text)).toBeInTheDocument();
      expect(control).toHaveAccessibleDescription(`${NOTE} ${text}`);
      await user.click(screen.getByRole("button", { name: "Initialise" }));
      expect(call).toHaveBeenCalledWith("initialise_project", { projectId: PROJECT, force: false, solo: "off" });
    });

    it("takes the arrow keys and Enter, and is disabled while the project is created", async () => {
      const user = userEvent.setup();
      let finish!: (result: unknown) => void;
      initialise = () =>
        new Promise((resolve) => {
          finish = resolve;
        });
      renderGetStarted();
      await screen.findByRole("radiogroup", { name: "Solo mode" });
      screen.getByRole("radio", { name: "Off" }).focus();
      await user.keyboard("{ArrowRight}{Enter}");
      expect(screen.getByRole("radio", { name: "Whole folders" })).toBeChecked();
      await user.click(screen.getByRole("button", { name: "Initialise" }));
      await vi.waitFor(() => expect(screen.getByRole("radio", { name: "Off" })).toBeDisabled());
      await act(async () => finish({ conflicts: [] }));
      await vi.waitFor(() => expect(screen.getByRole("radio", { name: "Off" })).toBeEnabled());
    });
  });
});

describe("ProjectGate", () => {
  it("shows Get Started in place of a view until the project is initialised, then the view", async () => {
    render(
      <ProjectStatusProvider projectId={PROJECT}>
        <ProjectGate view="board">
          <p>The Board</p>
        </ProjectGate>
      </ProjectStatusProvider>,
    );
    expect(await screen.findByRole("heading", { name: "Set up Weaver in this folder" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Set up Weaver in this folder" })).toHaveAttribute("id", "view-board");
    expect(screen.queryByText("The Board")).not.toBeInTheDocument();
    // An older status than the one shown is dropped.
    act(() => push(status({ revision: 0, state: "has-model" })));
    expect(screen.queryByText("The Board")).not.toBeInTheDocument();
    act(() => push(status({ revision: 2, state: "empty-model" })));
    expect(screen.getByText("The Board")).toBeInTheDocument();
    act(() => push(status({ revision: 3, state: "not-initialised" })));
    expect(screen.getByRole("heading", { name: "Set up Weaver in this folder" })).toBeInTheDocument();
  });

  it("shows only the view header while the status is not known", async () => {
    vi.mocked(call).mockImplementation((() => new Promise(() => {})) as unknown as typeof call);
    render(
      <ProjectStatusProvider projectId={PROJECT}>
        <ProjectGate view="board">
          <p>The Board</p>
        </ProjectGate>
      </ProjectStatusProvider>,
    );
    expect(screen.getByRole("heading", { level: 1, name: "Board" })).toBeInTheDocument();
    expect(screen.queryByText("The Board")).not.toBeInTheDocument();
  });

  it("shows the view as it is when the status cannot be read", async () => {
    vi.mocked(call).mockImplementation((() => Promise.reject(new Error("closed"))) as unknown as typeof call);
    render(
      <ProjectStatusProvider projectId={PROJECT}>
        <ProjectGate view="board">
          <p>The Board</p>
        </ProjectGate>
      </ProjectStatusProvider>,
    );
    expect(await screen.findByText("The Board")).toBeInTheDocument();
  });
});
