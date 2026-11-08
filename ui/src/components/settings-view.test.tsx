import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { SettingChange } from "@/bindings/SettingChange";
import type { SettingField } from "@/bindings/SettingField";
import type { AtomicCommitsStatus } from "@/bindings/AtomicCommitsStatus";
import type { SettingsSnapshot } from "@/bindings/SettingsSnapshot";
import type { SystemCertificatesStatus } from "@/bindings/SystemCertificatesStatus";
import type { ProjectStatus } from "@/bindings/ProjectStatus";
import type { SoloChange } from "@/bindings/SoloChange";
import type { SoloMode } from "@/bindings/SoloMode";
import { SettingsView, type ModelCatalog } from "@/components/settings-view";
import { SoloModeSetting } from "@/components/project/solo-mode";
import { AtomicCommitsSetting } from "@/components/project/atomic-commits";
import { ProjectStatusProvider } from "@/components/project/use-project-status";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SettingsProvider } from "@/hooks/use-settings";
import { ViewShownContext } from "@/components/view-shown";
import { call, on } from "@/lib/backend";

const OFF: SystemCertificatesStatus = { revision: 1, phase: "off", automatic: false, count: null, writtenAt: null, error: null, turnOnNotice: false, failureBanner: false, failureReason: null };

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const schema: SettingField[] = [
  { key: "ompPath", title: "omp path", description: "Where omp is.", advice: null, kind: "text", defaultValue: "", choices: [] },
  { key: "models.analysis", title: "Analysis", description: "Designs Proposals.", advice: "Your most capable model.", kind: "model", defaultValue: "", choices: [] },
  { key: "models.scan", title: "Scan", description: "Compares the Model.", advice: "A fast, light model.", kind: "model", defaultValue: "", choices: [] },
  { key: "models.execution", title: "Execution", description: "Carries out Tasks.", advice: "A mid-range model.", kind: "model", defaultValue: "", choices: [] },
  { key: "maxParallelAgents", title: "Max parallel sub-agents", description: "How many at once.", advice: null, kind: "integer", defaultValue: "20", choices: [] },
  { key: "language", title: "Language of the agent's texts", description: "Empty means English.", advice: null, kind: "text", defaultValue: "", choices: [] },
  {
    key: "theme",
    title: "Theme",
    description: "Auto follows the system.",
    advice: null,
    kind: "choice",
    defaultValue: "auto",
    choices: [
      { value: "auto", label: "Auto" },
      { value: "light", label: "Light" },
      { value: "dark", label: "Dark" },
    ],
  },
  {
    key: "editor.preset",
    title: "Editor",
    description: "Opens a file at a line.",
    advice: null,
    kind: "choice",
    defaultValue: "vscode",
    choices: [
      { value: "vscode", label: "VS Code" },
      { value: "custom", label: "Custom command" },
    ],
  },
  { key: "editor.command", title: "Custom command", description: "Needs {file}.", advice: null, kind: "text", defaultValue: "", choices: [] },
  {
    key: "permissions",
    title: "Permissions",
    description: "Whether omp asks first.",
    advice: null,
    kind: "choice",
    defaultValue: "yolo",
    choices: [
      { value: "yolo", label: "Run without asking" },
      { value: "ask", label: "Ask before acting" },
    ],
  },
  {
    key: "systemCertificates.enabled",
    title: "System Certificates",
    description:
      "omp trusts the certificates your system trusts, for proxies that inspect HTTPS. Replaces NODE_EXTRA_CA_CERTS, SSL_CERT_FILE and similar variables.",
    advice: null,
    kind: "boolean",
    defaultValue: "false",
    choices: [],
  },
];

function snapshot(revision = 1, patch: Partial<SettingsSnapshot["settings"]> = {}, rest: Partial<SettingsSnapshot> = {}): SettingsSnapshot {
  return {
    revision,
    settings: {
      ompPath: "",
      models: { analysis: "", scan: "", execution: "" },
      maxParallelAgents: 20,
      language: "",
      theme: "auto",
      editor: { preset: "vscode", command: "" },
      permissions: "yolo",
      systemCertificates: { enabled: false },
      ...patch,
    },
    issues: [],
    fileError: null,
    schema,
    ...rest,
  };
}

let current: SettingsSnapshot;
let saved: SettingChange[];
let failWrites: string | null;
let push: (next: SettingsSnapshot) => void;
let pushStatus: (next: ProjectStatus) => void;
let project: ProjectStatus;
let trackedFiles: () => Promise<string[]>;
let changeSolo: (args: { mode: SoloMode }) => Promise<SoloChange>;
let changeAtomic: (args: { enabled: boolean }) => Promise<AtomicCommitsStatus>;

const PROJECT = "0123456789abcdef";

function projectStatus(patch: Partial<ProjectStatus> = {}): ProjectStatus {
  return {
    projectId: PROJECT,
    revision: 1,
    state: "has-model",
    files: [],
    folderHasFiles: false,
    check: { opening: 1, outcome: { kind: "none" } },
    solo: { mode: "off" },
    atomicCommits: { enabled: false },
    ...patch,
  };
}

/** A backend that validates the two rules the tests exercise and stores what it accepts. */
function install(initial: SettingsSnapshot) {
  current = initial;
  saved = [];
  failWrites = null;
  project = projectStatus();
  trackedFiles = () => Promise.resolve([]);
  // The backend of a change that works: the project's mode becomes the one asked for.
  changeSolo = ({ mode }) => {
    project = { ...project, revision: project.revision + 1, solo: { mode } };
    return Promise.resolve({ solo: { mode }, tracked: [] });
  };
  changeAtomic = ({ enabled }) => {
    project = { ...project, revision: project.revision + 1, atomicCommits: { enabled } };
    return Promise.resolve({ enabled });
  };
  vi.mocked(on).mockImplementation(((name: string, handler: (next: never) => void) => {
    if (name === "settings-changed") push = handler as unknown as typeof push;
    if (name === "project-state") pushStatus = handler as unknown as typeof pushStatus;
    return Promise.resolve(() => {});
  }) as unknown as typeof on);
  vi.mocked(call).mockImplementation(((name: string, args: { change?: SettingChange }) => {
    if (name === "settings_get") return Promise.resolve(current);
    if (name === "system_certificates_status") return Promise.resolve(OFF);
    if (name === "project_status") return Promise.resolve(project);
    if (name === "solo_tracked_files") return trackedFiles();
    if (name === "set_solo_mode") return changeSolo(args as unknown as { mode: SoloMode });
    if (name === "check_atomic_commits") return Promise.resolve(project.atomicCommits);
    if (name === "set_atomic_commits") return changeAtomic(args as unknown as { enabled: boolean });
    const change = args.change!;
    saved.push(change);
    if (change.key === "maxParallelAgents" && !/^[1-9][0-9]*$/.test(change.value)) {
      return Promise.reject({ code: "invalid-input", message: "Enter a whole number of at least 1." });
    }
    if (change.key === "editor.command" && !change.value.includes("{file}")) {
      return Promise.reject({ code: "invalid-input", message: "The command must contain {file}." });
    }
    if (failWrites) return Promise.reject({ code: "internal", message: failWrites });
    const s = structuredClone(current.settings);
    switch (change.key) {
      case "maxParallelAgents":
        s.maxParallelAgents = Number(change.value);
        break;
      case "language":
        s.language = change.value;
        break;
      case "theme":
        s.theme = change.value as typeof s.theme;
        break;
      case "permissions":
        s.permissions = change.value as typeof s.permissions;
        break;
      case "systemCertificates.enabled":
        s.systemCertificates.enabled = change.value === "true";
        break;
      case "editor.preset":
        s.editor.preset = change.value as typeof s.editor.preset;
        break;
      case "editor.command":
        s.editor.command = change.value;
        break;
      case "models.analysis":
        s.models.analysis = change.value;
        break;
      default:
        break;
    }
    current = { ...current, revision: current.revision + 1, settings: s };
    return Promise.resolve(current);
  }) as unknown as typeof call);
}

async function show(props: Parameters<typeof SettingsView>[0] = {}) {
  render(
    <TooltipProvider>
      <SettingsProvider>
        <SettingsView {...props} />
      </SettingsProvider>
    </TooltipProvider>,
  );
  await screen.findByRole("heading", { name: "omp" });
}

beforeEach(() => install(snapshot()));

describe("settings view", () => {
  it("shows the groups in order and never a setting key", async () => {
    await show({ projectSlot: <p>Update skills box</p> });
    const headings = screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent);
    expect(headings).toEqual(["omp", "Models", "Agents", "Editor", "Appearance", "Project"]);
    expect(screen.getByText("Update skills box")).toBeInTheDocument();
    for (const key of ["ompPath", "maxParallelAgents", "models.analysis", "editor.preset"]) {
      expect(screen.queryByText(key)).not.toBeInTheDocument();
    }
  });

  it("starts with the view header, titled Settings, and its groups are 14 px semibold, never larger than the title", async () => {
    await show({ projectSlot: <p>Update skills box</p> });
    const title = screen.getByRole("heading", { level: 1, name: "Settings" });
    expect(title.closest("header")).toHaveClass("h-[76px]", "border-b");
    expect(title.closest("header")?.nextElementSibling).toContainElement(screen.getByRole("heading", { name: "omp" }));
    for (const group of screen.getAllByRole("heading", { level: 2 })) {
      expect(group).toHaveClass("text-sm", "font-semibold");
    }
  });

  it("shows the Project group only with its slot, and the omp slot first in its group", async () => {
    await show({ ompSlot: <p>omp status box</p> });
    expect(screen.queryByRole("heading", { name: "Project" })).not.toBeInTheDocument();
    const group = screen.getByRole("region", { name: "omp" });
    expect(group.firstElementChild?.nextElementSibling?.textContent).toBe("omp status box");
  });

  it("puts System Certificates last in the omp group, after Permissions, and never shows its key", async () => {
    await show({ ompSlot: <p>omp status box</p> });
    const group = screen.getByRole("region", { name: "omp" });
    const labels = Array.from(group.children).map((child) => child.textContent ?? "");
    expect(labels[1]).toBe("omp status box");
    expect(labels.findIndex((text) => text.startsWith("Permissions"))).toBeLessThan(labels.findIndex((text) => text.startsWith("System Certificates")));
    expect(labels.at(-1)).toMatch(/^System Certificates/);
    expect(screen.getByRole("switch", { name: "System Certificates" })).not.toBeChecked();
    expect(screen.queryByText("systemCertificates.enabled")).not.toBeInTheDocument();
  });

  it("shows titles, bold advice and descriptions from the schema", async () => {
    await show();
    expect(screen.getByLabelText("Analysis")).toBeInTheDocument();
    const advice = screen.getByText("Your most capable model.");
    expect(advice.tagName).toBe("STRONG");
    expect(screen.getByText("Empty means English.")).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Run without asking" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Auto" })).toBeChecked();
  });

  it("does not save an invalid value: the field keeps the text and says why", async () => {
    const user = userEvent.setup();
    await show();
    const field = screen.getByLabelText("Max parallel sub-agents");
    await user.clear(field);
    await user.type(field, "0");
    await user.tab();
    expect(await screen.findByText("Enter a whole number of at least 1.")).toBeInTheDocument();
    expect(field).toHaveValue("0");
    expect(field).toHaveAttribute("aria-invalid", "true");
    expect(current.settings.maxParallelAgents).toBe(20);
  });

  it("saves a number on Enter and clears the refusal", async () => {
    const user = userEvent.setup();
    await show();
    const field = screen.getByLabelText("Max parallel sub-agents");
    await user.clear(field);
    await user.type(field, "x{Enter}");
    expect(await screen.findByText("Enter a whole number of at least 1.")).toBeInTheDocument();
    await user.clear(field);
    await user.type(field, "8{Enter}");
    await vi.waitFor(() => expect(current.settings.maxParallelAgents).toBe(8));
    expect(screen.queryByText("Enter a whole number of at least 1.")).not.toBeInTheDocument();
    expect(field).toHaveValue("8");
  });

  it("saves text on blur and sends nothing when the value did not change", async () => {
    const user = userEvent.setup();
    await show();
    const field = screen.getByLabelText("Language of the agent's texts");
    await user.click(field);
    await user.tab();
    expect(saved).toEqual([]);
    await user.type(field, "  Italian ");
    await user.tab();
    await vi.waitFor(() => expect(saved).toEqual([{ key: "language", value: "Italian" }]));
  });

  it("shows a failed write under the field and puts the stored value back", async () => {
    const user = userEvent.setup();
    await show();
    failWrites = "cannot write settings.json (disk full)";
    const field = screen.getByLabelText("Language of the agent's texts");
    await user.type(field, "French");
    await user.tab();
    expect(
      await screen.findByText("Could not save Language of the agent's texts: cannot write settings.json (disk full)."),
    ).toBeInTheDocument();
    expect(field).toHaveValue("");
    expect(current.settings.language).toBe("");
  });

  it("saves a choice at once, with no confirmation", async () => {
    const user = userEvent.setup();
    await show();
    await user.click(screen.getByRole("radio", { name: "Dark" }));
    await vi.waitFor(() => expect(saved).toEqual([{ key: "theme", value: "dark" }]));
    expect(await screen.findByRole("radio", { name: "Dark" })).toBeChecked();
    await user.click(screen.getByRole("radio", { name: "Ask before acting" }));
    await vi.waitFor(() => expect(saved.at(-1)).toEqual({ key: "permissions", value: "ask" }));
  });

  it("shows a change from another window at once, except in the field with the focus", async () => {
    const user = userEvent.setup();
    await show();
    const language = screen.getByLabelText("Language of the agent's texts");
    await user.click(language);
    act(() => push(snapshot(5, { theme: "dark", language: "Italian", ompPath: "/opt/omp" })));
    expect(screen.getByRole("radio", { name: "Dark" })).toBeChecked();
    expect(screen.getByLabelText("omp path")).toHaveValue("/opt/omp");
    expect(language).toHaveValue("");
    await user.tab();
    expect(language).toHaveValue("Italian");
    expect(saved).toEqual([]);
  });

  it("keeps what the developer typed in a focused field when another window changes it", async () => {
    const user = userEvent.setup();
    await show();
    const language = screen.getByLabelText("Language of the agent's texts");
    await user.type(language, "Fr");
    act(() => push(snapshot(5, { language: "Italian" })));
    expect(language).toHaveValue("Fr");
  });

  it("ignores a snapshot older than the one it holds", async () => {
    await show();
    act(() => push(snapshot(7, { theme: "dark" })));
    act(() => push(snapshot(3, { theme: "light" })));
    expect(screen.getByRole("radio", { name: "Dark" })).toBeChecked();
  });

  it("shows the custom command only with the Custom command preset, and refuses one without {file}", async () => {
    const user = userEvent.setup();
    install(snapshot(1, { editor: { preset: "custom", command: "" } }));
    await show();
    const field = screen.getByLabelText("Custom command");
    await user.type(field, "myeditor");
    await user.tab();
    expect(await screen.findByText("The command must contain {file}.")).toBeInTheDocument();
    expect(field).toHaveValue("myeditor");
    await user.type(field, " {{file}:{{line}");
    await user.tab();
    await vi.waitFor(() => expect(current.settings.editor.command).toBe("myeditor {file}:{line}"));
  });

  it("hides the custom command with a preset", async () => {
    await show();
    expect(screen.queryByLabelText("Custom command")).not.toBeInTheDocument();
  });

  it("tells which key of the file was ignored and why, and shows an unreadable file", async () => {
    install(
      snapshot(1, {}, {
        issues: [{ key: "maxParallelAgents", reason: "Enter a whole number of at least 1." }],
        fileError: "expected value at line 1 column 1",
      }),
    );
    await show();
    expect(screen.getByText(/The value in the settings file was ignored.*Enter a whole number of at least 1\./)).toBeInTheDocument();
    expect(screen.getByText(/The settings file could not be used: expected value at line 1 column 1\. Every default is in force/)).toBeInTheDocument();
  });

  it("shows a failed load with Retry", async () => {
    const user = userEvent.setup();
    vi.mocked(call).mockRejectedValueOnce({ code: "internal", message: "disk gone" });
    vi.mocked(on).mockImplementation((() => Promise.resolve(() => {})) as unknown as typeof on);
    render(
      <SettingsProvider>
        <SettingsView />
      </SettingsProvider>,
    );
    expect(await screen.findByText("The settings could not be loaded: disk gone")).toBeInTheDocument();
    vi.mocked(call).mockResolvedValueOnce(snapshot());
    await user.click(screen.getByRole("button", { name: "Retry" }));
    expect(await screen.findByRole("heading", { name: "omp" })).toBeInTheDocument();
  });
});

describe("model roles", () => {
  const listed: ModelCatalog = {
    status: "listed",
    models: [
      { selector: "anthropic/opus", name: "Claude Opus", thinkingLevels: ["low", "high"] },
      { selector: "openai/mini", name: "openai/mini" },
    ],
  };

  it("offers `omp default` and the listed models, filtered while typing, and saves a choice at once", async () => {
    const user = userEvent.setup();
    await show({ modelCatalog: listed });
    const field = screen.getByLabelText("Analysis");
    await user.click(field);
    const options = within(await screen.findByRole("listbox")).getAllByRole("option");
    expect(options.map((o) => o.textContent)).toEqual(["omp default", "anthropic/opusClaude Opus", "openai/mini"]);
    await user.type(field, "opus");
    expect(within(screen.getByRole("listbox")).getAllByRole("option")).toHaveLength(1);
    await user.click(screen.getByRole("option", { name: /anthropic\/opus/ }));
    await vi.waitFor(() => expect(saved).toEqual([{ key: "models.analysis", value: "anthropic/opus" }]));
  });

  it("keeps free text that no list contains, saved on Enter", async () => {
    const user = userEvent.setup();
    await show({ modelCatalog: listed });
    await user.type(screen.getByLabelText("Analysis"), "my/custom-model{Enter}");
    await vi.waitFor(() => expect(saved).toEqual([{ key: "models.analysis", value: "my/custom-model" }]));
  });

  it("picks a suggestion with the arrows and Enter, and closes the list with Esc", async () => {
    const user = userEvent.setup();
    await show({ modelCatalog: listed });
    const field = screen.getByLabelText("Analysis");
    await user.click(field);
    await user.keyboard("{ArrowDown}{ArrowDown}{Enter}");
    await vi.waitFor(() => expect(saved).toEqual([{ key: "models.analysis", value: "anthropic/opus" }]));
    await user.keyboard("{ArrowDown}");
    expect(await screen.findByRole("listbox")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).not.toBeInTheDocument();
  });

  it("shows the thinking level Select only for a model with listed levels, and shows the model only in the field", async () => {
    install(snapshot(1, { models: { analysis: "anthropic/opus:high", scan: "openai/mini", execution: "" } }));
    await show({ modelCatalog: listed });
    expect(screen.getByLabelText("Analysis")).toHaveValue("anthropic/opus");
    const level = screen.getByRole("combobox", { name: "Analysis thinking level" });
    expect(level).toHaveTextContent("high");
    expect(screen.queryByRole("combobox", { name: "Scan thinking level" })).not.toBeInTheDocument();
    expect(screen.queryByRole("combobox", { name: "Execution thinking level" })).not.toBeInTheDocument();
  });

  it("treats a suffix omp does not list as part of the model name", async () => {
    install(snapshot(1, { models: { analysis: "anthropic/opus:turbo", scan: "", execution: "" } }));
    await show({ modelCatalog: listed });
    expect(screen.getByLabelText("Analysis")).toHaveValue("anthropic/opus:turbo");
    expect(screen.queryByRole("combobox", { name: "Analysis thinking level" })).not.toBeInTheDocument();
  });

  it("says why the list is unavailable, and still takes typing", async () => {
    const user = userEvent.setup();
    await show({ modelCatalog: { status: "unavailable", reason: "omp not found at /opt/omp" } });
    expect(screen.getByText("omp not found at /opt/omp")).toBeInTheDocument();
    await user.type(screen.getByLabelText("Scan"), "x/y");
    await user.tab();
    await vi.waitFor(() => expect(saved).toEqual([{ key: "models.scan", value: "x/y" }]));
  });
});

/** The Settings view with `Solo mode` first in its Project group, as the project window composes it. */
async function showSolo(visible = true) {
  const view = (shown: boolean) => (
    <TooltipProvider>
      <SettingsProvider>
        <ProjectStatusProvider projectId={PROJECT}>
          <ViewShownContext.Provider value={shown}>
            <SettingsView
              projectSlot={
                <>
                  <SoloModeSetting />
                  <p>Update skills box</p>
                </>
              }
            />
          </ViewShownContext.Provider>
        </ProjectStatusProvider>
      </SettingsProvider>
    </TooltipProvider>
  );
  const rendered = render(view(visible));
  await screen.findByRole("heading", { name: "omp" });
  await screen.findByRole("radiogroup", { name: "Solo mode" });
  return { show: (shown: boolean) => rendered.rerender(view(shown)) };
}

const NOTE = "Applies only to this working copy: other clones start with it off.";
const VERSIONED_NOWHERE = "Weaver's files are not versioned anywhere: losing this working copy loses them.";

describe("Solo mode in Settings", () => {
  it("is the first item of the Project group, before Update skills", async () => {
    await showSolo();
    const group = screen.getByRole("region", { name: "Project" });
    const control = within(group).getByRole("radiogroup", { name: "Solo mode" });
    expect(group.children[1]).toContainElement(control);
    expect(within(group).getByText("Solo mode")).toBeInTheDocument();
    expect(control.compareDocumentPosition(within(group).getByText("Update skills box")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("is a segmented control Off / Whole folders / Weaver's files only, a radio group, with the note as its description", async () => {
    await showSolo();
    const control = screen.getByRole("radiogroup", { name: "Solo mode" });
    expect(within(control).getAllByRole("radio").map((radio) => radio.textContent)).toEqual(["Off", "Whole folders", "Weaver's files only"]);
    expect(screen.getByRole("radio", { name: "Off" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Whole folders" })).not.toBeChecked();
    expect(control).toHaveAccessibleDescription(NOTE);
    expect(screen.getByText(NOTE)).toBeInTheDocument();
    for (const radio of within(control).getAllByRole("radio")) expect(radio).toBeEnabled();
    expect(screen.queryByText(VERSIONED_NOWHERE)).not.toBeInTheDocument();
    // Never asked for the tracked files while it is off.
    expect(call).not.toHaveBeenCalledWith("solo_tracked_files", expect.anything());
  });

  it("is disabled on Off in a folder that is not a project yet, and says where to choose it", async () => {
    project = projectStatus({ state: "not-initialised" });
    await showSolo();
    const control = screen.getByRole("radiogroup", { name: "Solo mode" });
    for (const radio of within(control).getAllByRole("radio")) expect(radio).toBeDisabled();
    expect(screen.getByRole("radio", { name: "Off" })).toBeChecked();
    expect(screen.getByText("Choose Solo mode when you initialise the project.")).toBeInTheDocument();
    expect(control).toHaveAccessibleDescription(`${NOTE} Choose Solo mode when you initialise the project.`);
  });

  it.each([
    ["git was not found", "Solo mode needs git: git was not found."],
    ["this folder is not in a git repository", "Solo mode needs git: this folder is not in a git repository."],
    ["git did not answer within 10 s", "Solo mode needs git: git did not answer within 10 s."],
  ])("is disabled on Off without git (%s) and gives the reason as its description", async (reason, text) => {
    project = projectStatus({ solo: { mode: "off", unavailable: reason } });
    await showSolo();
    const control = screen.getByRole("radiogroup", { name: "Solo mode" });
    for (const radio of within(control).getAllByRole("radio")) expect(radio).toBeDisabled();
    expect(screen.getByRole("radio", { name: "Off" })).toBeChecked();
    expect(screen.getByText(text)).toBeInTheDocument();
    expect(control).toHaveAccessibleDescription(`${NOTE} ${text}`);
  });

  it("is never stored as a setting: choosing writes no settings and asks the backend to change the mode", async () => {
    const user = userEvent.setup();
    await showSolo();
    await user.click(screen.getByRole("radio", { name: "Whole folders" }));
    await vi.waitFor(() => expect(call).toHaveBeenCalledWith("set_solo_mode", { projectId: PROJECT, mode: "folders" }));
    await vi.waitFor(() => expect(screen.getByRole("radio", { name: "Whole folders" })).toBeChecked());
    expect(saved).toEqual([]);
    expect(vi.mocked(call).mock.calls.some(([name]) => name === "settings_set")).toBe(false);
    expect(JSON.stringify(snapshot().schema)).not.toMatch(/solo/i);
  });

  it("changes at once, with no confirmation, is disabled while the change runs and warns once it is on", async () => {
    const user = userEvent.setup();
    let finish!: () => void;
    changeSolo = ({ mode }) =>
      new Promise((resolve) => {
        finish = () => {
          project = { ...project, revision: project.revision + 1, solo: { mode } };
          resolve({ solo: { mode }, tracked: [".weaver/model/system.md"] });
        };
      });
    await showSolo();
    await user.click(screen.getByRole("radio", { name: "Weaver's files only" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    await vi.waitFor(() => expect(screen.getByRole("radio", { name: "Off" })).toBeDisabled());
    expect(screen.getByRole("radio", { name: "Weaver's files only" })).toBeChecked();

    await act(async () => finish());
    await vi.waitFor(() => expect(screen.getByRole("radio", { name: "Off" })).toBeEnabled());
    expect(screen.getByRole("radio", { name: "Weaver's files only" })).toBeChecked();
    const warning = screen.getByText(VERSIONED_NOWHERE);
    expect(warning.closest("[aria-live]")).toHaveAttribute("aria-live", "polite");
    expect(screen.getByRole("radiogroup", { name: "Solo mode" })).toHaveAccessibleDescription(`${NOTE} ${VERSIONED_NOWHERE}`);
    // The answer of the change carries the tracked files.
    const region = screen.getByRole("region", { name: "Git still tracks 1 of Weaver's files: their changes still go into commits." });
    expect(within(region).getAllByRole("listitem").map((item) => item.textContent)).toEqual([".weaver/model/system.md"]);
  });

  it("turns off at once too, and the warning and the list go", async () => {
    const user = userEvent.setup();
    project = projectStatus({ solo: { mode: "folders" } });
    trackedFiles = () => Promise.resolve([".omp/WATCHDOG.yml"]);
    await showSolo();
    expect(await screen.findByRole("region", { name: /Git still tracks 1/ })).toBeInTheDocument();
    await user.click(screen.getByRole("radio", { name: "Off" }));
    await vi.waitFor(() => expect(call).toHaveBeenCalledWith("set_solo_mode", { projectId: PROJECT, mode: "off" }));
    await vi.waitFor(() => expect(screen.getByRole("radio", { name: "Off" })).toBeChecked());
    expect(screen.queryByText(VERSIONED_NOWHERE)).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: /Git still tracks/ })).not.toBeInTheDocument();
  });

  it("shows `Could not change Solo mode: <reason>.` in the error colour and puts back the project's value", async () => {
    const user = userEvent.setup();
    changeSolo = () => Promise.reject({ code: "refused", message: "git was not found" });
    await showSolo();
    await user.click(screen.getByRole("radio", { name: "Whole folders" }));
    const failure = await screen.findByRole("alert");
    expect(failure).toHaveTextContent("Could not change Solo mode: git was not found.");
    expect(failure).toHaveClass("text-rose-ink");
    await vi.waitFor(() => expect(screen.getByRole("radio", { name: "Off" })).toBeChecked());
    expect(screen.getByRole("radio", { name: "Whole folders" })).not.toBeChecked();
    expect(screen.getByRole("radio", { name: "Off" })).toBeEnabled();
    expect(screen.getByRole("radiogroup", { name: "Solo mode" })).toHaveAccessibleDescription(`${NOTE} Could not change Solo mode: git was not found.`);
    // The next choice clears it.
    changeSolo = ({ mode }) => {
      project = { ...project, revision: project.revision + 1, solo: { mode } };
      return Promise.resolve({ solo: { mode }, tracked: [] });
    };
    await user.click(screen.getByRole("radio", { name: "Whole folders" }));
    await vi.waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("works from the keyboard: focus, arrows move, Enter chooses", async () => {
    const user = userEvent.setup();
    await showSolo();
    screen.getByRole("radio", { name: "Off" }).focus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("radio", { name: "Whole folders" })).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByRole("radio", { name: "Weaver's files only" })).toHaveFocus();
    await user.keyboard("{Enter}");
    await vi.waitFor(() => expect(call).toHaveBeenCalledWith("set_solo_mode", { projectId: PROJECT, mode: "files" }));
    await vi.waitFor(() => expect(screen.getByRole("radio", { name: "Weaver's files only" })).toBeChecked());
  });

  it("warns when it is on but the block is not in place, in the warning colour, and stays usable", async () => {
    await showSolo();
    act(() => pushStatus(projectStatus({ revision: 2, solo: { mode: "folders", unavailable: "git was not found", problem: "git was not found" } })));
    const text = "Solo mode is on but cannot exclude Weaver's files: git was not found.";
    const line = await screen.findByText(text);
    expect(line).toHaveClass("text-peach-ink");
    expect(line.closest("[aria-live]")).toHaveAttribute("aria-live", "polite");
    for (const radio of screen.getAllByRole("radio", { name: /Off|Whole folders|Weaver's files only/ })) expect(radio).toBeEnabled();
    expect(screen.getByRole("radiogroup", { name: "Solo mode" })).toHaveAccessibleDescription(`${NOTE} ${VERSIONED_NOWHERE} ${text}`);

    act(() => pushStatus(projectStatus({ revision: 3, solo: { mode: "folders", unavailable: "this folder is not in a git repository", problem: "this folder is not in a git repository" } })));
    expect(await screen.findByText("Solo mode is on but cannot exclude Weaver's files: this folder is not in a git repository.")).toBeInTheDocument();

    act(() => pushStatus(projectStatus({ revision: 4, solo: { mode: "files", problem: "could not write /r/.git/info/exclude: Permission denied (os error 13)" } })));
    expect(
      await screen.findByText("Weaver's files may not be excluded from git: could not write /r/.git/info/exclude: Permission denied (os error 13)."),
    ).toBeInTheDocument();
    expect(screen.queryByText(/cannot exclude/)).not.toBeInTheDocument();

    // Put in place again at the next opening: the warning goes.
    act(() => pushStatus(projectStatus({ revision: 5, solo: { mode: "files" } })));
    await vi.waitFor(() => expect(screen.queryByText(/may not be excluded/)).not.toBeInTheDocument());
  });

  it("lists the tracked files in a labelled region of monospace paths that scrolls past 8 lines, with no action", async () => {
    const files = Array.from({ length: 10 }, (_, index) => `.weaver/model/file-${index}.md`);
    project = projectStatus({ solo: { mode: "folders" } });
    trackedFiles = () => Promise.resolve(files);
    await showSolo();
    const region = await screen.findByRole("region", { name: "Git still tracks 10 of Weaver's files: their changes still go into commits." });
    expect(region).toHaveClass("border-peach");
    const list = within(region).getByRole("list", { name: "Files git still tracks" });
    expect(list).toHaveClass("font-mono", "overflow-auto", "max-h-[12rem]");
    expect(within(list).getAllByRole("listitem").map((item) => item.textContent)).toEqual(files);
    expect(within(region).queryByRole("button")).not.toBeInTheDocument();
  });

  it("asks for the tracked files each time the view is shown, and the earlier list stays while it loads", async () => {
    project = projectStatus({ solo: { mode: "folders" } });
    let calls = 0;
    let release!: (files: string[]) => void;
    trackedFiles = () => {
      calls += 1;
      if (calls === 1) return Promise.resolve(["a.md"]);
      return new Promise((resolve) => {
        release = resolve;
      });
    };
    const view = await showSolo();
    await screen.findByRole("region", { name: /Git still tracks 1 of/ });
    expect(calls).toBe(1);

    view.show(false);
    expect(calls).toBe(1);
    view.show(true);
    await vi.waitFor(() => expect(calls).toBe(2));
    expect(screen.getByRole("region", { name: /Git still tracks 1 of/ })).toBeInTheDocument();
    await act(async () => release(["a.md", "b.md"]));
    expect(await screen.findByRole("region", { name: /Git still tracks 2 of/ })).toBeInTheDocument();
  });

  it("shows nothing about tracked files while the list is empty", async () => {
    project = projectStatus({ solo: { mode: "files" } });
    await showSolo();
    await vi.waitFor(() => expect(call).toHaveBeenCalledWith("solo_tracked_files", { projectId: PROJECT }));
    expect(screen.queryByRole("region", { name: /Git still tracks/ })).not.toBeInTheDocument();
    expect(screen.getByText(VERSIONED_NOWHERE)).toBeInTheDocument();
  });

  it("is disabled for every choice while a merge is in progress, with the reason under the note as its description", async () => {
    const user = userEvent.setup();
    const reason = "Finish or abort the merge in progress first.";
    project = projectStatus({
      solo: { mode: "folders" },
      merge: { available: false, reason: "A merge is in progress", inProgress: { branch: "dev", current: "main", conflicts: 2 } },
    });
    await showSolo();
    const control = screen.getByRole("radiogroup", { name: "Solo mode" });
    for (const radio of within(control).getAllByRole("radio")) expect(radio).toBeDisabled();
    expect(screen.getByRole("radio", { name: "Whole folders" })).toBeChecked();
    expect(screen.getByText(reason)).toBeInTheDocument();
    // The note comes first, then the reason, then the usual warning: all of them its description.
    expect(control).toHaveAccessibleDescription(`${NOTE} ${reason} ${VERSIONED_NOWHERE}`);
    expect(screen.getByText(reason).closest("[aria-live]")).toHaveAttribute("aria-live", "polite");

    // Neither the other mode nor turning it off is asked of the backend.
    await user.click(screen.getByRole("radio", { name: "Off" }));
    await user.click(screen.getByRole("radio", { name: "Weaver's files only" }));
    expect(call).not.toHaveBeenCalledWith("set_solo_mode", expect.anything());

    // The merge ends: the control is usable again and the reason goes.
    act(() => pushStatus(projectStatus({ revision: 2, solo: { mode: "folders" }, merge: { available: true, current: "main" } })));
    await vi.waitFor(() => expect(screen.getByRole("radio", { name: "Off" })).toBeEnabled());
    expect(screen.queryByText(reason)).not.toBeInTheDocument();
    expect(control).toHaveAccessibleDescription(`${NOTE} ${VERSIONED_NOWHERE}`);
  });
});

/** The Settings view with `Solo mode`, `Atomic commits` and then `Update skills`, as the project window composes it. */
async function showAtomic(visible = true) {
  const view = (shown: boolean) => (
    <TooltipProvider>
      <SettingsProvider>
        <ProjectStatusProvider projectId={PROJECT}>
          <ViewShownContext.Provider value={shown}>
            <SettingsView
              projectSlot={
                <>
                  <SoloModeSetting />
                  <AtomicCommitsSetting />
                  <p>Update skills box</p>
                </>
              }
            />
          </ViewShownContext.Provider>
        </ProjectStatusProvider>
      </SettingsProvider>
    </TooltipProvider>
  );
  const rendered = render(view(visible));
  await screen.findByRole("heading", { name: "omp" });
  await screen.findByRole("switch", { name: "Atomic commits" });
  return { show: (shown: boolean) => rendered.rerender(view(shown)) };
}

const ATOMIC_NOTE = "Applies only to this working copy: after each History entry, Weaver commits the code the agent wrote for it.";

describe("Atomic commits in Settings", () => {
  it("sits in the Project group between Solo mode and Update skills", async () => {
    await showAtomic();
    const group = screen.getByRole("region", { name: "Project" });
    const solo = within(group).getByRole("radiogroup", { name: "Solo mode" });
    const atomic = within(group).getByRole("switch", { name: "Atomic commits" });
    const skills = within(group).getByText("Update skills box");
    expect(solo.compareDocumentPosition(atomic) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(atomic.compareDocumentPosition(skills) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(group).getByText("Atomic commits").tagName).toBe("LABEL");
  });

  it("is a Switch, off by default and enabled, with the note as its description", async () => {
    await showAtomic();
    const control = screen.getByRole("switch", { name: "Atomic commits" });
    expect(control).not.toBeChecked();
    expect(control).toBeEnabled();
    expect(control).toHaveAccessibleDescription(ATOMIC_NOTE);
    expect(screen.getByText(ATOMIC_NOTE)).toBeInTheDocument();
  });

  it("asks the backend whether git is there each time the view is shown, and not while it is hidden", async () => {
    const view = await showAtomic(false);
    await vi.waitFor(() => expect(call).toHaveBeenCalledWith("project_status", { projectId: PROJECT }));
    expect(call).not.toHaveBeenCalledWith("check_atomic_commits", expect.anything());
    view.show(true);
    await vi.waitFor(() => expect(call).toHaveBeenCalledWith("check_atomic_commits", { projectId: PROJECT }));
    const asked = () => vi.mocked(call).mock.calls.filter(([name]) => name === "check_atomic_commits").length;
    const before = asked();
    view.show(false);
    view.show(true);
    await vi.waitFor(() => expect(asked()).toBe(before + 1));
  });

  it("is disabled and off in a folder that is not a project yet, and says what to do first", async () => {
    project = projectStatus({ state: "not-initialised", atomicCommits: { enabled: true } });
    await showAtomic();
    const control = screen.getByRole("switch", { name: "Atomic commits" });
    expect(control).toBeDisabled();
    expect(control).not.toBeChecked();
    expect(screen.getByText("Initialise the project to use Atomic commits.")).toBeInTheDocument();
    expect(control).toHaveAccessibleDescription(`${ATOMIC_NOTE} Initialise the project to use Atomic commits.`);
    expect(call).not.toHaveBeenCalledWith("check_atomic_commits", expect.anything());
  });

  it.each([
    ["git was not found", "Atomic commits needs git: git was not found."],
    ["this folder is not in a git repository", "Atomic commits needs git: this folder is not in a git repository."],
    ["git did not answer within 10 s", "Atomic commits needs git: git did not answer within 10 s."],
    ["git failed: fatal: bad object", "Atomic commits needs git: git failed: fatal: bad object."],
  ])("is disabled while off without git (%s) and gives the reason as its description", async (reason, text) => {
    project = projectStatus({ atomicCommits: { enabled: false, unavailable: reason } });
    await showAtomic();
    const control = screen.getByRole("switch", { name: "Atomic commits" });
    expect(control).toBeDisabled();
    expect(control).not.toBeChecked();
    expect(screen.getByText(text)).toBeInTheDocument();
    expect(control).toHaveAccessibleDescription(`${ATOMIC_NOTE} ${text}`);
  });

  it("stays usable while on without git, so that it can be turned off", async () => {
    const user = userEvent.setup();
    project = projectStatus({ atomicCommits: { enabled: true, unavailable: "git was not found" } });
    await showAtomic();
    const control = screen.getByRole("switch", { name: "Atomic commits" });
    expect(control).toBeChecked();
    expect(control).toBeEnabled();
    expect(screen.queryByText(/needs git/)).not.toBeInTheDocument();
    await user.click(control);
    await vi.waitFor(() => expect(call).toHaveBeenCalledWith("set_atomic_commits", { projectId: PROJECT, enabled: false }));
  });

  it("is never stored as a setting: turning it on asks the backend and writes no settings", async () => {
    const user = userEvent.setup();
    await showAtomic();
    await user.click(screen.getByRole("switch", { name: "Atomic commits" }));
    await vi.waitFor(() => expect(call).toHaveBeenCalledWith("set_atomic_commits", { projectId: PROJECT, enabled: true }));
    await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Atomic commits" })).toBeChecked());
    expect(saved).toEqual([]);
    expect(vi.mocked(call).mock.calls.some(([name]) => name === "settings_set")).toBe(false);
    expect(JSON.stringify(snapshot().schema)).not.toMatch(/atomic/i);
  });

  it("changes at once with no confirmation, and is disabled while the change runs", async () => {
    const user = userEvent.setup();
    let finish!: () => void;
    changeAtomic = ({ enabled }) =>
      new Promise((resolve) => {
        finish = () => {
          project = { ...project, revision: project.revision + 1, atomicCommits: { enabled } };
          resolve({ enabled });
        };
      });
    await showAtomic();
    await user.click(screen.getByRole("switch", { name: "Atomic commits" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Atomic commits" })).toBeDisabled());
    expect(screen.getByRole("switch", { name: "Atomic commits" })).toBeChecked();

    await act(async () => finish());
    await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Atomic commits" })).toBeEnabled());
    expect(screen.getByRole("switch", { name: "Atomic commits" })).toBeChecked();
  });

  it("turns off at once too", async () => {
    const user = userEvent.setup();
    project = projectStatus({ atomicCommits: { enabled: true } });
    await showAtomic();
    expect(screen.getByRole("switch", { name: "Atomic commits" })).toBeChecked();
    await user.click(screen.getByRole("switch", { name: "Atomic commits" }));
    await vi.waitFor(() => expect(call).toHaveBeenCalledWith("set_atomic_commits", { projectId: PROJECT, enabled: false }));
    await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Atomic commits" })).not.toBeChecked());
  });

  it("shows `Could not change Atomic commits: <reason>.` in the error colour and puts back the project's value", async () => {
    const user = userEvent.setup();
    changeAtomic = () => Promise.reject({ code: "refused", message: "could not write /r/.weaver/commits/commits.json: Permission denied (os error 13)" });
    await showAtomic();
    await user.click(screen.getByRole("switch", { name: "Atomic commits" }));
    const failure = await screen.findByRole("alert");
    const text = "Could not change Atomic commits: could not write /r/.weaver/commits/commits.json: Permission denied (os error 13).";
    expect(failure).toHaveTextContent(text);
    expect(failure).toHaveClass("text-rose-ink");
    await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Atomic commits" })).toBeEnabled());
    expect(screen.getByRole("switch", { name: "Atomic commits" })).not.toBeChecked();
    expect(screen.getByRole("switch", { name: "Atomic commits" })).toHaveAccessibleDescription(`${ATOMIC_NOTE} ${text}`);
    // The next change clears it.
    changeAtomic = ({ enabled }) => {
      project = { ...project, revision: project.revision + 1, atomicCommits: { enabled } };
      return Promise.resolve({ enabled });
    };
    await user.click(screen.getByRole("switch", { name: "Atomic commits" }));
    await vi.waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("follows the status another window or the backend sends", async () => {
    await showAtomic();
    act(() => pushStatus(projectStatus({ revision: 2, atomicCommits: { enabled: false, unavailable: "git was not found" } })));
    await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Atomic commits" })).toBeDisabled());
    expect(screen.getByText("Atomic commits needs git: git was not found.")).toBeInTheDocument();
    act(() => pushStatus(projectStatus({ revision: 3, atomicCommits: { enabled: true } })));
    await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Atomic commits" })).toBeChecked());
    expect(screen.getByRole("switch", { name: "Atomic commits" })).toBeEnabled();
    expect(screen.queryByText(/needs git/)).not.toBeInTheDocument();
  });

  it("works from the keyboard: Tab reaches it, Space turns it on", async () => {
    const user = userEvent.setup();
    await showAtomic();
    const control = screen.getByRole("switch", { name: "Atomic commits" });
    control.focus();
    expect(control).toHaveFocus();
    await user.keyboard(" ");
    await vi.waitFor(() => expect(call).toHaveBeenCalledWith("set_atomic_commits", { projectId: PROJECT, enabled: true }));
    await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Atomic commits" })).toBeChecked());
  });
});

describe("Atomic commits file that cannot be used", () => {
  it("counts as off and says which file and why, as part of the description of the switch", async () => {
    const diagnostic = ".weaver/commits/commits.json: `enabled` must be `true`";
    project = projectStatus({ atomicCommits: { enabled: false, diagnostic } });
    await showAtomic();
    const control = screen.getByRole("switch", { name: "Atomic commits" });
    const text = `The Atomic commits file counts as off: ${diagnostic}.`;
    expect(control).not.toBeChecked();
    expect(control).toBeEnabled();
    const line = screen.getByText(text);
    expect(line).toHaveClass("text-peach-ink");
    expect(line.closest("[aria-live]")).toHaveAttribute("aria-live", "polite");
    expect(control).toHaveAccessibleDescription(`${ATOMIC_NOTE} ${text}`);
    // The next change rewrites the file: the project's status then carries no diagnostic.
    const user = userEvent.setup();
    await user.click(control);
    await vi.waitFor(() => expect(screen.getByRole("switch", { name: "Atomic commits" })).toBeChecked());
    await vi.waitFor(() => expect(screen.queryByText(text)).not.toBeInTheDocument());
  });
});
