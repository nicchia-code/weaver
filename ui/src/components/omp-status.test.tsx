import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { OmpModel } from "@/bindings/OmpModel";
import type { OmpPhase } from "@/bindings/OmpPhase";
import type { OmpStatus } from "@/bindings/OmpStatus";
import type { SettingField } from "@/bindings/SettingField";
import type { SettingsSnapshot } from "@/bindings/SettingsSnapshot";
import type { SystemCertificatesStatus } from "@/bindings/SystemCertificatesStatus";
import { ConnectedSettingsView, modelCatalogOf, OmpSetup, OmpStatusBox, Providers, providersShown } from "@/components/omp-status";
import { TooltipProvider } from "@/components/ui/tooltip";
import { OmpProvider } from "@/hooks/use-omp";
import { SettingsProvider } from "@/hooks/use-settings";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const OFF: SystemCertificatesStatus = { revision: 1, phase: "off", automatic: false, count: null, writtenAt: null, error: null, turnOnNotice: false, failureBanner: false, failureReason: null };

const PROVIDERS = [
  ["openai-codex", "OpenAI Codex", "ChatGPT"],
  ["anthropic", "Anthropic", "Claude Pro or Max"],
  ["github-copilot", "GitHub Copilot", "Copilot subscription"],
  ["google-gemini-cli", "Gemini CLI", "Google account"],
  ["google-antigravity", "Antigravity", "Google account"],
  ["cursor", "Cursor", "Cursor account"],
  ["xai-oauth", "xAI", "Grok account"],
] as const;

const model = (provider: string, selector: string): OmpModel => ({ provider, selector, name: null, thinkingLevels: ["low", "high"] });

function status(state: OmpPhase, patch: Partial<OmpStatus> = {}): OmpStatus {
  const models = patch.models ?? [];
  return {
    revision: 1,
    state,
    ompPath: "",
    version: state === "checking" || state === "missing" ? null : "18.15.0",
    minVersion: "18.15.0",
    models,
    providers: PROVIDERS.map(([id, name, plan]) => ({
      id,
      name,
      plan,
      connected: state === "checking" || state === "missing" ? null : models.some((m) => m.provider === id),
    })),
    reason: null,
    updateTarget: null,
    updateFailure: null,
    loginProvider: null,
    terminalProblem: null,
    logout: null,
    waitingForExport: false,
    installerFailure: null,
    ...patch,
  };
}

const anthropic = model("anthropic", "anthropic/claude-sonnet-5");
const ready = (patch: Partial<OmpStatus> = {}) => status("ready", { models: [anthropic, model("anthropic", "anthropic/claude-haiku-5")], ...patch });

function renderBox(s: OmpStatus | null) {
  return render(
    <TooltipProvider>
      <OmpStatusBox status={s} />
    </TooltipProvider>,
  );
}

function renderProviders(s: OmpStatus) {
  const view = (value: OmpStatus) => (
    <TooltipProvider>
      <Providers status={value} />
    </TooltipProvider>
  );
  const result = render(view(s));
  return { ...result, update: (value: OmpStatus) => result.rerender(view(value)) };
}

const row = (name: string) => screen.getByRole("listitem", { name });

beforeEach(() => {
  vi.mocked(call).mockReset();
  vi.mocked(call).mockResolvedValue(null as never);
  vi.mocked(on).mockImplementation((() => Promise.resolve(() => {})) as unknown as typeof on);
});

describe("omp status box", () => {
  const cases: Array<[string, OmpStatus | null, string, string | null]> = [
    ["Checking", status("checking"), "Checking omp…", null],
    ["no answer yet", null, "Checking omp…", null],
    ["Missing without a reason", status("missing"), "omp is not installed.", "Install omp"],
    ["Missing with a reason", status("missing", { reason: "omp not found at /gone/omp" }), "omp not found at /gone/omp", "Install omp"],
    [
      "Missing after a failed update of an old omp",
      status("missing", { reason: "omp 18.0.0 is older than 18.15.0 and could not be updated (exit code 1)." }),
      "omp 18.0.0 is older than 18.15.0 and could not be updated (exit code 1).",
      "Install omp",
    ],
    ["Installing", status("installing"), "Installing omp in the terminal…", "Open installer again"],
    ["Updating with a version", status("updating", { updateTarget: "18.16.0" }), "Updating omp to 18.16.0…", null],
    ["Updating without one", status("updating"), "Updating omp…", null],
    ["Not connected", status("notConnected"), "omp 18.15.0 has no model provider. Log in to one below.", null],
    ["Logging in", status("loggingIn", { loginProvider: "anthropic" }), "Waiting for the login to Anthropic in the terminal…", null],
    ["Ready", ready(), "omp 18.15.0 is ready with 2 models.", null],
    ["Ready with one model", status("ready", { models: [anthropic] }), "omp 18.15.0 is ready with 1 model.", null],
    [
      "Ready after a failed update",
      ready({ updateFailure: "timed out" }),
      "Could not update omp (timed out). omp 18.15.0 is ready with 2 models.",
      null,
    ],
  ];

  it.each(cases)("%s shows its text and its action", (_name, value, text, action) => {
    renderBox(value);
    const box = screen.getByRole("status", { name: "omp status" });
    expect(box).toHaveTextContent(text);
    if (action) expect(within(box).getByRole("button", { name: action })).toBeInTheDocument();
    else expect(within(box).queryByRole("button")).not.toBeInTheDocument();
  });

  it("says that it exports the certificates first only during that wait", () => {
    const { rerender } = renderBox(status("checking", { waitingForExport: true }));
    const box = screen.getByRole("status", { name: "omp status" });
    expect(box).toHaveTextContent("Checking omp…");
    expect(box).toHaveTextContent("Exporting system certificates first…");
    rerender(
      <TooltipProvider>
        <OmpStatusBox status={status("checking")} />
      </TooltipProvider>,
    );
    expect(box).toHaveTextContent("Checking omp…");
    expect(box).not.toHaveTextContent("Exporting system certificates first…");
  });

  const explanation =
    "omp could not verify a server's certificate; a corporate proxy that inspects HTTPS traffic is the usual cause, and turning on System Certificates in Weaver's settings may fix it";

  it("ends a proxy explanation with a full stop and leaves the other reasons as they are", () => {
    const { rerender } = renderBox(status("missing", { reason: explanation }));
    const box = screen.getByRole("status", { name: "omp status" });
    expect(box).toHaveTextContent(`${explanation}.`);
    rerender(
      <TooltipProvider>
        <OmpStatusBox status={status("missing", { reason: "omp received an HTTP 407 error; a corporate proxy that inspects HTTPS traffic may be blocking the request" })} />
      </TooltipProvider>,
    );
    expect(box.textContent).toContain("may be blocking the request.");
    rerender(
      <TooltipProvider>
        <OmpStatusBox status={status("missing", { reason: "omp exited with code 3" })} />
      </TooltipProvider>,
    );
    expect(box.textContent).toContain("omp exited with code 3");
    expect(box.textContent).not.toContain("omp exited with code 3.");
  });

  it("shows the text of a failed installer with its action, and the last reason under it", () => {
    const text =
      "The omp installer failed (exit code 22); see the terminal. A corporate proxy that inspects HTTPS traffic is a common cause. Turning on System Certificates in Weaver's settings may fix it.";
    const { rerender } = renderBox(status("missing", { installerFailure: text }));
    const box = screen.getByRole("status", { name: "omp status" });
    expect(box).toHaveTextContent(text);
    expect(box).not.toHaveTextContent("omp is not installed.");
    expect(within(box).getByRole("button", { name: "Install omp" })).toBeInTheDocument();
    rerender(
      <TooltipProvider>
        <OmpStatusBox status={status("missing", { installerFailure: text, reason: "omp not found at /gone/omp" })} />
      </TooltipProvider>,
    );
    expect(box).toHaveTextContent(text);
    expect(box).toHaveTextContent("omp not found at /gone/omp");
  });

  it.each([
    "The omp installer failed (exit code 90): the download of omp was blocked on both of its download sites, probably by a corporate proxy that filters downloads. See the terminal.",
    "The omp installer failed (exit code 91): omp could not verify a server's certificate; a corporate proxy that inspects HTTPS traffic is the usual cause. See the terminal. Turning on System Certificates in Weaver's settings may fix it.",
    "The omp installer failed (exit code 92): the downloaded omp was incomplete or altered, so it was not installed; a proxy may have changed it. See the terminal.",
    "The omp installer failed (exit code 93): the downloaded omp does not start. See the terminal.",
  ])("shows the text of a failed installer by cause: %s", (text) => {
    renderBox(status("missing", { installerFailure: text }));
    const box = screen.getByRole("status", { name: "omp status" });
    expect(box).toHaveTextContent(text);
    expect(box).not.toHaveTextContent("omp is not installed.");
    expect(within(box).getByRole("button", { name: "Install omp" })).toBeInTheDocument();
  });

  it("is a polite live region", () => {
    renderBox(status("checking"));
    expect(screen.getByRole("status", { name: "omp status" })).toHaveAttribute("aria-live", "polite");
  });

  it("marks each State with an icon that is hidden from assistive technology, never colour alone", () => {
    const { container, rerender } = renderBox(status("missing"));
    expect(container.querySelector("svg.lucide-triangle-alert")).toHaveAttribute("aria-hidden", "true");
    rerender(
      <TooltipProvider>
        <OmpStatusBox status={ready()} />
      </TooltipProvider>,
    );
    expect(container.querySelector("svg.lucide-circle-check")).toHaveAttribute("aria-hidden", "true");
    expect(screen.getByRole("status", { name: "omp status" })).toHaveTextContent("ready");
  });

  it("keeps the Loader still under reduced motion", () => {
    const { container } = renderBox(status("checking"));
    const loader = container.querySelector("svg.lucide-loader");
    expect(loader).toHaveClass("animate-spin");
    expect(loader).toHaveClass("motion-reduce:animate-none");
  });

  it("tells, while Installing or Logging in, to finish in the terminal", () => {
    const { rerender } = renderBox(status("installing"));
    const line = "Finish it in the terminal window. Weaver checks again every few seconds.";
    expect(screen.getByText(line)).toBeInTheDocument();
    rerender(
      <TooltipProvider>
        <OmpStatusBox status={status("loggingIn", { loginProvider: "cursor" })} />
      </TooltipProvider>,
    );
    expect(screen.getByText(line)).toBeInTheDocument();
    rerender(
      <TooltipProvider>
        <OmpStatusBox status={ready()} />
      </TooltipProvider>,
    );
    expect(screen.queryByText(line)).not.toBeInTheDocument();
  });

  it("Install omp and Open installer again ask the backend for the installer", async () => {
    const user = userEvent.setup();
    const { rerender } = renderBox(status("missing"));
    await user.click(screen.getByRole("button", { name: "Install omp" }));
    expect(call).toHaveBeenCalledWith("omp_install");
    rerender(
      <TooltipProvider>
        <OmpStatusBox status={status("installing")} />
      </TooltipProvider>,
    );
    await user.click(screen.getByRole("button", { name: "Open installer again" }));
    expect(call).toHaveBeenCalledTimes(2);
  });

  it("is reachable and usable with the keyboard alone", async () => {
    const user = userEvent.setup();
    renderBox(status("missing"));
    await user.tab();
    expect(screen.getByRole("button", { name: "Install omp" })).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(call).toHaveBeenCalledWith("omp_install");
  });

  it("shows when the terminal cannot be opened, with the command to copy", async () => {
    const user = userEvent.setup();
    renderBox(status("installing", { terminalProblem: { reason: "no terminal emulator was found", command: "curl -fsSL https://omp.sh/install | sh" } }));
    expect(screen.getByText("Could not open a terminal: no terminal emulator was found.")).toBeInTheDocument();
    expect(screen.getByText("curl -fsSL https://omp.sh/install | sh").tagName).toBe("CODE");
    await user.click(screen.getByRole("button", { name: "Copy" }));
    expect(await navigator.clipboard.readText()).toBe("curl -fsSL https://omp.sh/install | sh");
    expect(screen.getByRole("button", { name: "Copied" })).toBeInTheDocument();
  });
});

describe("Providers", () => {
  it("lists the providers of the fixed list in order, each with its plan and its state", () => {
    renderProviders(ready());
    expect(screen.getByRole("heading", { name: "Providers" })).toBeInTheDocument();
    const names = screen.getAllByRole("listitem").map((item) => item.querySelector("span")?.textContent);
    expect(names).toEqual(PROVIDERS.map(([, name]) => name));
    expect(within(row("Anthropic")).getByText("Claude Pro or Max")).toBeInTheDocument();
    expect(within(row("Anthropic")).getByText("Connected")).toBeInTheDocument();
    expect(within(row("GitHub Copilot")).getByText("Not connected")).toBeInTheDocument();
  });

  it("leaves the state out of a row while omp has not answered", () => {
    renderProviders(status("checking", { logout: { provider: "anthropic", phase: "running", reason: null } }));
    const anthropicRow = row("Anthropic");
    expect(within(anthropicRow).queryByText("Connected")).not.toBeInTheDocument();
    expect(within(anthropicRow).queryByText("Not connected")).not.toBeInTheDocument();
  });

  it("offers Log in on every row and Log in again and Log out only on connected ones", () => {
    renderProviders(ready());
    expect(within(row("Anthropic")).getByRole("button", { name: "Log in again" })).toBeInTheDocument();
    expect(within(row("Anthropic")).getByRole("button", { name: "Log out" })).toBeInTheDocument();
    for (const name of ["OpenAI Codex", "GitHub Copilot", "Cursor", "xAI"]) {
      expect(within(row(name)).getByRole("button", { name: "Log in" })).toBeInTheDocument();
      expect(within(row(name)).queryByRole("button", { name: "Log out" })).not.toBeInTheDocument();
    }
  });

  it("opens the login of the provider of the row, Cursor included", async () => {
    const user = userEvent.setup();
    renderProviders(status("notConnected"));
    await user.click(within(row("Cursor")).getByRole("button", { name: "Log in" }));
    expect(call).toHaveBeenCalledWith("omp_log_in", { provider: "cursor" });
    await user.click(within(row("Anthropic")).getByRole("button", { name: "Log in" }));
    expect(call).toHaveBeenLastCalledWith("omp_log_in", { provider: "anthropic" });
  });

  it("explains Log in in a tooltip on keyboard focus", async () => {
    const user = userEvent.setup();
    renderProviders(status("notConnected"));
    await user.tab();
    expect(within(row("OpenAI Codex")).getByRole("button", { name: "Log in" })).toHaveFocus();
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Open a terminal that logs omp in to OpenAI Codex");
  });

  describe("Log out", () => {
    async function openDialog() {
      const user = userEvent.setup();
      const view = renderProviders(ready());
      const trigger = within(row("Anthropic")).getByRole("button", { name: "Log out" });
      await user.click(trigger);
      return { user, trigger, ...view };
    }

    it("asks first, with the focus on Cancel", async () => {
      await openDialog();
      const dialog = await screen.findByRole("alertdialog", { name: "Log out of Anthropic?" });
      expect(dialog).toHaveTextContent(
        "omp forgets the accounts of Anthropic it stored. Credentials in environment variables, .env files or models.yml stay.",
      );
      expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
      expect(call).not.toHaveBeenCalled();
    });

    it("Esc closes it without logging out and gives the focus back to Log out", async () => {
      const { user, trigger } = await openDialog();
      await screen.findByRole("alertdialog");
      await user.keyboard("{Escape}");
      expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
      expect(call).not.toHaveBeenCalled();
      expect(trigger).toHaveFocus();
    });

    it("Enter on Cancel, the button that starts focused, changes nothing", async () => {
      const { user, trigger } = await openDialog();
      await screen.findByRole("alertdialog");
      await user.keyboard("{Enter}");
      expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
      expect(call).not.toHaveBeenCalled();
      expect(trigger).toHaveFocus();
    });

    it("logs out with the keyboard: Tab to Log out, Enter", async () => {
      const { user } = await openDialog();
      const dialog = await screen.findByRole("alertdialog");
      await user.tab();
      expect(within(dialog).getByRole("button", { name: "Log out" })).toHaveFocus();
      await user.keyboard("{Enter}");
      expect(call).toHaveBeenCalledWith("omp_log_out", { provider: "anthropic" });
      expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    });

    it("shows the row busy while it runs, then gives the focus to the row's Log in", async () => {
      const { user, update } = await openDialog();
      const dialog = await screen.findByRole("alertdialog");
      await user.click(within(dialog).getByRole("button", { name: "Log out" }));
      await act(async () => {});

      const running = ready({ logout: { provider: "anthropic", phase: "running", reason: null } });
      update({ ...running, state: "checking" });
      const busy = row("Anthropic");
      expect(busy).toHaveAttribute("aria-busy", "true");
      for (const button of within(busy).getAllByRole("button")) expect(button).toBeDisabled();
      expect(busy.querySelector("svg.lucide-loader")).toHaveClass("animate-spin");
      // Only the row of the provider is busy.
      expect(row("Cursor")).toHaveAttribute("aria-busy", "false");

      update({ ...running, state: "checking", logout: { provider: "anthropic", phase: "verifying", reason: null } });
      expect(row("Anthropic")).toHaveAttribute("aria-busy", "true");

      // The provider is gone: nothing is said and the focus is on the row's Log in.
      update(status("ready", { models: [model("openai-codex", "openai-codex/gpt-6")] }));
      const after = row("Anthropic");
      expect(after).toHaveAttribute("aria-busy", "false");
      expect(within(after).getByRole("button", { name: "Log in" })).toHaveFocus();
      expect(within(after).queryByText(/Could not log out|still connected/)).not.toBeInTheDocument();
    });

    it("says when other credentials keep the provider connected", () => {
      const { update } = renderProviders(ready());
      update(ready({ logout: { provider: "anthropic", phase: "stillConnected", reason: null } }));
      expect(
        within(row("Anthropic")).getByText(
          "Anthropic is still connected through another credential (environment variable, .env file or models.yml). Remove it there to disconnect.",
        ),
      ).toBeInTheDocument();
      expect(within(row("Anthropic")).getByRole("button", { name: "Log in again" })).toBeEnabled();
    });

    it("says why omp could not log out", () => {
      const { update } = renderProviders(ready());
      update(ready({ logout: { provider: "anthropic", phase: "failed", reason: "timed out" } }));
      expect(within(row("Anthropic")).getByText("Could not log out of Anthropic: timed out.")).toBeInTheDocument();
    });

    it("says why the backend refused", async () => {
      vi.mocked(call).mockRejectedValueOnce({ code: "invalid-input", message: "unknown provider" } as never);
      const { user } = await openDialog();
      const dialog = await screen.findByRole("alertdialog");
      await user.click(within(dialog).getByRole("button", { name: "Log out" }));
      expect(await screen.findByText("Could not log out of Anthropic: unknown provider.")).toBeInTheDocument();
    });
  });
});

describe("what is shown when", () => {
  it("shows Providers when omp answers and while a log out checks them again", () => {
    for (const state of ["notConnected", "loggingIn", "ready"] as const) expect(providersShown(status(state))).toBe(true);
    for (const state of ["checking", "missing", "installing", "updating"] as const) expect(providersShown(status(state))).toBe(false);
    expect(providersShown(null)).toBe(false);
    expect(providersShown(status("checking", { logout: { provider: "anthropic", phase: "verifying", reason: null } }))).toBe(true);
  });

  it("lists the models of a Ready omp, with their thinking levels, and says why there are none otherwise", () => {
    expect(modelCatalogOf(ready())).toEqual({
      status: "listed",
      models: [
        { selector: "anthropic/claude-sonnet-5", name: undefined, thinkingLevels: ["low", "high"] },
        { selector: "anthropic/claude-haiku-5", name: undefined, thinkingLevels: ["low", "high"] },
      ],
    });
    expect(modelCatalogOf(status("missing", { reason: "omp not found at /gone/omp" }))).toEqual({
      status: "unavailable",
      reason: "omp not found at /gone/omp",
    });
    expect(modelCatalogOf(null).status).toBe("unavailable");
    expect(modelCatalogOf(status("notConnected")).status).toBe("unavailable");
  });
});

describe("OmpProvider", () => {
  let push: (value: OmpStatus) => void;

  beforeEach(() => {
    vi.mocked(on).mockImplementation(((_name: string, handler: (value: OmpStatus) => void) => {
      push = handler;
      return Promise.resolve(() => {});
    }) as unknown as typeof on);
  });

  const mount = () =>
    render(
      <TooltipProvider>
        <OmpProvider>
          <OmpSetup providers="when-needed" systemCertificates={false} />
        </OmpProvider>
      </TooltipProvider>,
    );

  it("fetches the status and applies the events, the highest revision winning", async () => {
    vi.mocked(call).mockImplementation(((name: string) =>
      name === "omp_status" ? Promise.resolve(status("checking", { revision: 2 })) : Promise.resolve(null)) as never);
    mount();
    expect(await screen.findByRole("status", { name: "omp status" })).toHaveTextContent("Checking omp…");
    act(() => push(ready({ revision: 4 })));
    expect(screen.getByRole("status", { name: "omp status" })).toHaveTextContent("omp 18.15.0 is ready with 2 models.");
    act(() => push(status("missing", { revision: 3 })));
    expect(screen.getByRole("status", { name: "omp status" })).toHaveTextContent("ready");
  });

  it("shows Providers where room is short only while omp has no provider or a login runs", async () => {
    vi.mocked(call).mockImplementation(((name: string) =>
      name === "omp_status" ? Promise.resolve(ready({ revision: 1 })) : Promise.resolve(null)) as never);
    mount();
    await screen.findByText(/is ready with/);
    expect(screen.queryByRole("heading", { name: "Providers" })).not.toBeInTheDocument();
    act(() => push(status("notConnected", { revision: 2 })));
    expect(screen.getByRole("heading", { name: "Providers" })).toBeInTheDocument();
  });
});

const field = (key: SettingField["key"], title: string, kind: SettingField["kind"], defaultValue = "", choices: SettingField["choices"] = []): SettingField => ({
  key,
  title,
  description: `About ${title}.`,
  advice: null,
  kind,
  defaultValue,
  choices,
});

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
    field("ompPath", "omp path", "text"),
    field("models.analysis", "Analysis", "model"),
    field("models.scan", "Scan", "model"),
    field("models.execution", "Execution", "model"),
    field("maxParallelAgents", "Max parallel sub-agents", "integer", "20"),
    field("language", "Language", "text"),
    field("theme", "Theme", "choice", "auto", [{ value: "auto", label: "Auto" }]),
    field("editor.preset", "Editor", "choice", "vscode", [{ value: "vscode", label: "VS Code" }]),
    field("editor.command", "Custom command", "text"),
    field("permissions", "Permissions", "choice", "yolo", [{ value: "yolo", label: "Run without asking" }]),
    field("systemCertificates.enabled", "System Certificates", "boolean", "false"),
  ],
};

describe("ConnectedSettingsView", () => {
  function mount(value: OmpStatus) {
    vi.mocked(call).mockImplementation(((name: string) =>
      Promise.resolve(
        name === "omp_status" ? value : name === "settings_get" ? settings : name === "system_certificates_status" ? OFF : null,
      )) as never);
    render(
      <TooltipProvider>
        <SettingsProvider>
          <OmpProvider>
            <ConnectedSettingsView />
          </OmpProvider>
        </SettingsProvider>
      </TooltipProvider>,
    );
  }

  it("puts the status box and Providers in the omp group", async () => {
    mount(ready());
    expect(await screen.findByRole("status", { name: "omp status" })).toHaveTextContent("omp 18.15.0 is ready with 2 models.");
    expect(screen.getByRole("heading", { name: "Providers" })).toBeInTheDocument();
  });

  it("says in the Models group why omp lists no model", async () => {
    mount(status("missing", { reason: "omp not found at /gone/omp" }));
    const box = await screen.findByRole("status", { name: "omp status" });
    expect(box).toHaveTextContent("omp not found at /gone/omp");
    expect(screen.queryByRole("heading", { name: "Providers" })).not.toBeInTheDocument();
    expect(screen.getAllByText("omp not found at /gone/omp")).toHaveLength(2);
  });
});

describe("System Certificates places", () => {
  let enabled: boolean;

  function mount(children: ReactNode) {
    enabled = false;
    vi.mocked(call).mockImplementation(((name: string, args?: { change?: { value: string } }) => {
      if (name === "omp_status") return Promise.resolve(ready());
      if (name === "system_certificates_status") return Promise.resolve(OFF);
      if (name === "settings_set") {
        enabled = args?.change?.value === "true";
        return Promise.resolve({ ...settings, revision: 2, settings: { ...settings.settings, systemCertificates: { enabled } } });
      }
      return Promise.resolve(settings);
    }) as never);
    render(
      <TooltipProvider>
        <SettingsProvider>
          <OmpProvider>{children}</OmpProvider>
        </SettingsProvider>
      </TooltipProvider>,
    );
  }

  it("shows the switch under the box where the box shows outside Settings (Get Started, welcome window)", async () => {
    mount(<OmpSetup providers="when-needed" />);
    const box = await screen.findByRole("status", { name: "omp status" });
    const toggle = await screen.findByRole("switch", { name: "System Certificates" });
    expect(box.compareDocumentPosition(toggle) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("shows it once in Settings, in the omp group and not under the box", async () => {
    mount(<ConnectedSettingsView />);
    await screen.findByRole("status", { name: "omp status" });
    expect(await screen.findAllByRole("switch", { name: "System Certificates" })).toHaveLength(1);
    expect(within(screen.getByRole("region", { name: "omp" })).getByRole("switch", { name: "System Certificates" })).toBeInTheDocument();
  });

  it("shows the same value in every place, and a change in one shows in all", async () => {
    const user = userEvent.setup();
    mount(
      <>
        <OmpSetup providers="when-needed" />
        <ConnectedSettingsView />
      </>,
    );
    const switches = await screen.findAllByRole("switch", { name: "System Certificates" });
    expect(switches).toHaveLength(2);
    for (const toggle of switches) expect(toggle).not.toBeChecked();
    await user.click(switches[0]!);
    await vi.waitFor(() => {
      for (const toggle of screen.getAllByRole("switch", { name: "System Certificates" })) expect(toggle).toBeChecked();
    });
    expect(enabled).toBe(true);
  });
});
