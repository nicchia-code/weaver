import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { SettingsSnapshot } from "@/bindings/SettingsSnapshot";
import type { StartupFailure } from "@/bindings/StartupFailure";
import type { WelcomeState } from "@/bindings/WelcomeState";
import { TooltipProvider } from "@/components/ui/tooltip";
import { WelcomeWindow } from "@/components/welcome-window";
import { OmpProvider } from "@/hooks/use-omp";
import { SettingsProvider } from "@/hooks/use-settings";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const alpha = { path: "/work/alpha", name: "alpha", opened: "2026-11-02T07:00:00.000Z", exists: true };
const gone = { path: "/work/gone", name: "gone", opened: "2026-11-01T07:00:00.000Z", exists: false };

const state = (recent: WelcomeState["recent"]): WelcomeState => ({ name: "Weaver", version: "0.1.0", recent, failure: null });

const settings: SettingsSnapshot = {
  revision: 1,
  settings: {
    ompPath: "",
    models: { analysis: "", scan: "", execution: "" },
    maxParallelAgents: 20,
    language: "",
    theme: "auto",
    editor: { preset: "vscode", command: "" },
    permissions: "yolo",
    systemCertificates: { enabled: false },
  },
  issues: [],
  fileError: null,
  schema: [
    {
      key: "systemCertificates.enabled",
      title: "System Certificates",
      description: "About System Certificates.",
      advice: null,
      kind: "boolean",
      defaultValue: "false",
      choices: [],
    },
  ],
};

let answers: Record<string, (args?: unknown) => unknown>;

function render_() {
  return render(
    <TooltipProvider>
      <SettingsProvider>
        <OmpProvider>
          <WelcomeWindow />
        </OmpProvider>
      </SettingsProvider>
    </TooltipProvider>,
  );
}

beforeEach(() => {
  answers = {
    welcome_state: () => state([alpha, gone]),
    startup_failures: () => [],
    update_status: () => ({ available: null, downloading: false }),
    omp_status: () => new Promise(() => {}),
    settings_get: () => settings,
    system_certificates_status: () => ({ revision: 1, phase: "off", automatic: false, count: null, writtenAt: null, error: null, turnOnNotice: false, failureBanner: false, failureReason: null }),
  };
  vi.mocked(on).mockImplementation((() => Promise.resolve(() => {})) as unknown as typeof on);
  vi.mocked(call).mockImplementation(((name: string, args?: unknown) => {
    const answer = answers[name];
    return answer ? Promise.resolve().then(() => answer(args)) : Promise.resolve(null);
  }) as never);
});

describe("welcome window", () => {
  it("shows the name and the version of the app", async () => {
    render_();
    expect(await screen.findByText("Version 0.1.0")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "Weaver" })).toBeInTheDocument();
  });

  it("shows the logo above the app name, decorative, on a Surface panel over Paper", async () => {
    const { container } = render_();
    const heading = await screen.findByRole("heading", { level: 1, name: "Weaver" });
    const logo = container.querySelector("svg[viewBox='0 0 30 30']")!;
    expect(logo).toHaveAttribute("aria-hidden", "true");
    expect(logo.compareDocumentPosition(heading) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    // No lucide mark stands in for the logo.
    expect(container.querySelector(".lucide-blocks")).toBeNull();
    const panel = heading.closest("main")!.parentElement!;
    expect(panel).toHaveClass("bg-surface", "rounded-xl");
    expect(panel.parentElement).toHaveClass("bg-paper", "p-2");
  });

  it("shows the System Certificates notice and the failure banner without Open Settings", async () => {
    answers.system_certificates_status = () => ({
      revision: 2,
      phase: "failed",
      automatic: true,
      count: null,
      writtenAt: null,
      error: "no certificate authority found",
      turnOnNotice: true,
      failureBanner: true,
      failureReason: "no certificate authority found",
    });
    render_();
    const notice = await screen.findByRole("status", { name: "System Certificates turned on" });
    const failure = screen.getByRole("alert", { name: "System Certificates export failed" });
    for (const banner of [notice, failure]) {
      expect(within(banner).getByRole("button", { name: "Close" })).toBeInTheDocument();
      expect(within(banner).queryByRole("button", { name: "Open Settings" })).not.toBeInTheDocument();
    }
  });

  it("lists the recent projects with the folder name and the full path, most recent first", async () => {
    render_();
    const open = await screen.findByRole("button", { name: /alpha/ });
    expect(open).toHaveTextContent("/work/alpha");
    const items = screen.getAllByRole("listitem").map((item) => item.textContent);
    expect(items[0]).toContain("alpha");
    expect(items[1]).toContain("gone");
  });

  it("says when there is no recent project", async () => {
    answers.welcome_state = () => state([]);
    render_();
    expect(await screen.findByText("No recent projects. Open a folder to start.")).toBeInTheDocument();
  });

  it("opens a recent project", async () => {
    const user = userEvent.setup();
    render_();
    await user.click(await screen.findByRole("button", { name: /alpha/ }));
    expect(call).toHaveBeenCalledWith("open_project", { path: "/work/alpha" });
  });

  it("marks a folder that no longer exists and removes it from the list", async () => {
    const user = userEvent.setup();
    answers.remove_recent = () => state([alpha]);
    render_();
    const item = (await screen.findByText("gone")).closest("li")!;
    expect(within(item).getByText("Not found")).toBeInTheDocument();
    expect(within(item).queryByRole("button", { name: /^gone/ })).not.toBeInTheDocument();
    await user.click(within(item).getByRole("button", { name: "Remove gone from the recent projects" }));
    expect(call).toHaveBeenCalledWith("remove_recent", { path: "/work/gone" });
    await vi.waitFor(() => expect(screen.queryByText("gone")).not.toBeInTheDocument());
  });

  it("Open Folder asks for the system folder picker", async () => {
    const user = userEvent.setup();
    render_();
    await user.click(await screen.findByRole("button", { name: "Open Folder" }));
    expect(call).toHaveBeenCalledWith("open_folder");
  });

  it("is usable with the keyboard alone: Open Folder first, then the recent projects", async () => {
    const user = userEvent.setup();
    render_();
    await screen.findByText("Version 0.1.0");
    await user.tab();
    expect(screen.getByRole("button", { name: "Open Folder" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: /alpha/ })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(call).toHaveBeenCalledWith("open_project", { path: "/work/alpha" });
  });

  it("says why a project did not open", async () => {
    const user = userEvent.setup();
    answers.open_project = () => Promise.reject({ code: "invalid-input", message: "the folder does not exist" });
    render_();
    await user.click(await screen.findByRole("button", { name: /alpha/ }));
    expect(await screen.findByText("alpha could not open: the folder does not exist")).toBeInTheDocument();
  });

  it("asks again when the window comes forward", async () => {
    render_();
    await screen.findByText("Version 0.1.0");
    answers.welcome_state = () => state([]);
    act(() => {
      window.dispatchEvent(new Event("focus"));
    });
    expect(await screen.findByText("No recent projects. Open a folder to start.")).toBeInTheDocument();
  });

  it("follows a change of the recent projects made from the File menu at once", async () => {
    let changed: ((recent: WelcomeState["recent"]) => void) | undefined;
    vi.mocked(on).mockImplementation(((name: string, listener: (recent: WelcomeState["recent"]) => void) => {
      if (name === "recent-changed") changed = listener;
      return Promise.resolve(() => {});
    }) as never);
    render_();
    expect(await screen.findByText("gone")).toBeInTheDocument();
    expect(changed).toBeDefined();

    act(() => changed?.([]));
    expect(await screen.findByText("No recent projects. Open a folder to start.")).toBeInTheDocument();

    act(() => changed?.([alpha]));
    expect(await screen.findByRole("button", { name: /alpha/ })).toBeInTheDocument();
  });

  it("says why an opening chosen from the File menu failed, with the texts of its own actions", async () => {
    type Failed = { name: string | null; reason: string };
    let failed: ((failure: Failed) => void) | undefined;
    vi.mocked(on).mockImplementation(((name: string, listener: (failure: Failed) => void) => {
      if (name === "menu-open-failed") failed = listener;
      return Promise.resolve(() => {});
    }) as never);
    render_();
    await screen.findByText("Version 0.1.0");
    expect(failed).toBeDefined();

    act(() => failed?.({ name: "alpha", reason: "the folder does not exist" }));
    expect(await screen.findByText("alpha could not open: the folder does not exist")).toBeInTheDocument();

    act(() => failed?.({ name: null, reason: "no display" }));
    expect(await screen.findByText("The folder could not be opened: no display")).toBeInTheDocument();
  });

  it("shows an opening that failed while no window had the focus, handed over with the state", async () => {
    answers.welcome_state = () => ({ ...state([alpha]), failure: { name: "gone", reason: "the folder does not exist" } });
    render_();
    expect(await screen.findByText("gone could not open: the folder does not exist")).toBeInTheDocument();
  });

  it("shows the status of the agent without naming omp", async () => {
    answers.omp_status = () => ({
      revision: 1,
      state: "missing",
      ompPath: "",
      version: null,
      minVersion: "18.15.0",
      models: [],
      providers: [],
      reason: null,
      updateTarget: null,
      updateFailure: null,
      loginProvider: null,
      terminalProblem: null,
      logout: null,
      waitingForExport: false,
      installerFailure: null,
    });
    render_();
    expect(await screen.findByRole("button", { name: "Install the agent" })).toBeInTheDocument();
    expect(screen.getByText("The agent is not installed.")).toBeInTheDocument();
    expect(document.body.textContent).not.toMatch(/\bomp\b/);
  });

  it("shows the System Certificates switch under the status box", async () => {
    render_();
    expect(await screen.findByRole("switch", { name: "System Certificates" })).not.toBeChecked();
  });

  it("tells a project of the last session that could not open", async () => {
    const failure: StartupFailure = { projectId: "aaaa", projectName: "Acme", step: "project window", reason: "no display" };
    answers.startup_failures = () => [failure];
    render_();
    expect(await screen.findByText("Acme could not open: no display")).toBeInTheDocument();
  });
});
