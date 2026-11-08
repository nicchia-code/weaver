import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { SettingChange } from "@/bindings/SettingChange";
import type { SettingsSnapshot } from "@/bindings/SettingsSnapshot";
import type { SystemCertificatesStatus } from "@/bindings/SystemCertificatesStatus";
import { SystemCertificatesSetting } from "@/components/settings-view";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SettingsProvider } from "@/hooks/use-settings";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const DESCRIPTION =
  "omp trusts the certificates your system trusts, for proxies that inspect HTTPS. Replaces NODE_EXTRA_CA_CERTS, SSL_CERT_FILE and similar variables.";
const TODAY = new Date(2026, 10, 2, 7, 5).toISOString();
const OTHER_DAY = new Date(2026, 10, 1, 7, 5).toISOString();

const certificates = (patch: Partial<SystemCertificatesStatus> = {}): SystemCertificatesStatus => ({
  revision: 1,
  phase: "off",
  automatic: false,
  count: null,
  writtenAt: null,
  error: null,
  turnOnNotice: false,
  failureBanner: false, failureReason: null,
  ...patch,
});

const ready = (patch: Partial<SystemCertificatesStatus> = {}) => certificates({ phase: "ready", count: 142, writtenAt: TODAY, ...patch });

function settings(revision: number, enabled: boolean): SettingsSnapshot {
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
      systemCertificates: { enabled },
    },
    issues: [],
    fileError: null,
    schema: [
      {
        key: "systemCertificates.enabled",
        title: "System Certificates",
        description: DESCRIPTION,
        advice: null,
        kind: "boolean",
        defaultValue: "false",
        choices: [],
      },
    ],
  };
}

let current: SettingsSnapshot;
let backendStatus: SystemCertificatesStatus;
let saved: SettingChange[];
let refreshes: number;
let failWrites: string | null;
let refreshResult: SystemCertificatesStatus | null;
let handlers: Record<string, ((payload: never) => void)[]>;

function install(enabled: boolean, status: SystemCertificatesStatus) {
  current = settings(1, enabled);
  backendStatus = status;
  saved = [];
  refreshes = 0;
  failWrites = null;
  refreshResult = null;
  handlers = {};
  vi.mocked(on).mockImplementation(((name: string, handler: (payload: never) => void) => {
    (handlers[name] ??= []).push(handler);
    return Promise.resolve(() => {});
  }) as unknown as typeof on);
  vi.mocked(call).mockImplementation(((name: string, args?: { change?: SettingChange }) => {
    switch (name) {
      case "settings_get":
        return Promise.resolve(current);
      case "system_certificates_status":
        return Promise.resolve(backendStatus);
      case "system_certificates_refresh":
        refreshes += 1;
        return Promise.resolve(refreshResult ?? backendStatus);
      case "settings_set": {
        saved.push(args!.change!);
        if (failWrites) return Promise.reject({ code: "internal", message: failWrites });
        current = settings(current.revision + 1, args!.change!.value === "true");
        return Promise.resolve(current);
      }
      default:
        return Promise.reject(new Error(`unexpected command ${name}`));
    }
  }) as unknown as typeof call);
}

/** The text of the status line (not the copies assistive technology and the tooltip hold of a reason). */
const line = (text: string) => screen.getByText(text, { selector: "p span" });

const push = <N extends string>(name: N, payload: unknown) => act(() => handlers[name]!.forEach((handler) => handler(payload as never)));

async function show(places = 1) {
  render(
    <TooltipProvider>
      <SettingsProvider>
        {Array.from({ length: places }, (_, index) => (
          <SystemCertificatesSetting key={index} place="settings" />
        ))}
      </SettingsProvider>
    </TooltipProvider>,
  );
  await screen.findAllByRole("switch", { name: "System Certificates" });
}

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date(2026, 10, 2, 12, 0));
  install(false, certificates());
});

afterEach(() => vi.useRealTimers());

describe("the System Certificates switch", () => {
  it("shows the title and the description of the schema, never the key, and nothing else while it is off", async () => {
    await show();
    expect(screen.getByRole("switch", { name: "System Certificates" })).not.toBeChecked();
    expect(screen.getByText(DESCRIPTION)).toBeInTheDocument();
    expect(screen.queryByText("systemCertificates.enabled")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Refresh system certificates" })).not.toBeInTheDocument();
    expect(screen.queryByText(/certificates, exported/)).not.toBeInTheDocument();
  });

  it("saves at once, without confirmation, and shows the value in force", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    await show();
    await user.click(screen.getByRole("switch", { name: "System Certificates" }));
    await vi.waitFor(() => expect(saved).toEqual([{ key: "systemCertificates.enabled", value: "true" }]));
    expect(await screen.findByRole("switch", { name: "System Certificates" })).toBeChecked();
    await user.click(screen.getByRole("switch", { name: "System Certificates" }));
    await vi.waitFor(() => expect(saved.at(-1)).toEqual({ key: "systemCertificates.enabled", value: "false" }));
    expect(screen.getByRole("switch", { name: "System Certificates" })).not.toBeChecked();
  });

  it("is reached with Tab and turned on and off with Space", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    await show();
    await user.tab();
    expect(screen.getByRole("switch", { name: "System Certificates" })).toHaveFocus();
    await user.keyboard(" ");
    await vi.waitFor(() => expect(saved).toEqual([{ key: "systemCertificates.enabled", value: "true" }]));
    await vi.waitFor(() => expect(screen.getByRole("switch", { name: "System Certificates" })).toBeChecked());
    await user.keyboard(" ");
    await vi.waitFor(() => expect(saved.at(-1)).toEqual({ key: "systemCertificates.enabled", value: "false" }));
  });

  it("says why a write failed under the field and keeps the stored value", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    await show();
    failWrites = "the settings file is read-only.";
    await user.click(screen.getByRole("switch", { name: "System Certificates" }));
    expect(await screen.findByText("Could not save System Certificates: the settings file is read-only.")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "System Certificates" })).not.toBeChecked();
    expect(screen.getByRole("switch", { name: "System Certificates" })).toHaveAccessibleDescription(
      expect.stringContaining("Could not save System Certificates"),
    );
  });

  it("shows a change from another window at once", async () => {
    await show();
    push("settings-changed", settings(5, true));
    expect(screen.getByRole("switch", { name: "System Certificates" })).toBeChecked();
    push("settings-changed", settings(6, false));
    expect(screen.getByRole("switch", { name: "System Certificates" })).not.toBeChecked();
  });

  it("shows the same value and state in every place, and a change in one shows in all", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    await show(2);
    const [first, second] = screen.getAllByRole("switch", { name: "System Certificates" });
    await user.click(first!);
    await vi.waitFor(() => expect(second).toBeChecked());
    expect(first).toBeChecked();
    push("system-certificates-changed", ready({ revision: 2 }));
    expect(screen.getAllByText("142 certificates, exported 07:05.")).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: "Refresh system certificates" })).toHaveLength(2);
  });
});

describe("the status line", () => {
  it("asks for the status after subscribing to the event, and follows the events", async () => {
    install(true, ready());
    await show();
    expect((await screen.findAllByText("142 certificates, exported 07:05."))).toHaveLength(1);
    const events = vi.mocked(on).mock.calls.findIndex(([name]) => name === "system-certificates-changed");
    const asked = vi.mocked(call).mock.calls.findIndex(([name]) => name === "system_certificates_status");
    expect(events).toBeGreaterThanOrEqual(0);
    expect(vi.mocked(on).mock.invocationCallOrder[events]).toBeLessThan(vi.mocked(call).mock.invocationCallOrder[asked]!);
    push("system-certificates-changed", certificates({ revision: 3, phase: "exporting" }));
    expect(line("Exporting system certificates…")).toBeInTheDocument();
    // An older revision never replaces a newer one.
    push("system-certificates-changed", ready({ revision: 2, count: 7 }));
    expect(line("Exporting system certificates…")).toBeInTheDocument();
  });

  it.each([
    ["exporting", certificates({ phase: "exporting" }), "Exporting system certificates…"],
    ["ready today", ready(), "142 certificates, exported 07:05."],
    ["ready on another day", ready({ writtenAt: OTHER_DAY }), "142 certificates, exported 1 Nov 07:05."],
    [
      "failed with an earlier bundle today",
      ready({ phase: "failed", error: "could not write the bundle: disk full" }),
      "Could not export the system certificates (could not write the bundle: disk full). Using the ones exported 07:05.",
    ],
    [
      "failed with an earlier bundle on another day",
      ready({ phase: "failed", writtenAt: OTHER_DAY, error: "the export did not finish within 30 s" }),
      "Could not export the system certificates (the export did not finish within 30 s). Using the ones exported 1 Nov 07:05.",
    ],
    [
      "failed without a bundle",
      certificates({ phase: "failed", error: "no certificate authority found" }),
      "Could not export the system certificates (no certificate authority found). The agent gets no certificates from Weaver.",
    ],
  ])("shows the line when %s", async (_case, status, text) => {
    install(true, status);
    await show();
    expect(await screen.findByText(text, { selector: "p span" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Refresh system certificates" })).toHaveTextContent("Refresh");
  });

  it("has an icon for each case: a still Loader under reduced motion, a CircleCheck, a TriangleAlert", async () => {
    install(true, certificates({ phase: "exporting" }));
    await show();
    const row = (await screen.findByText("Exporting system certificates…", { selector: "p span" })).closest("p")!;
    const loader = row.querySelector("svg")!;
    expect(loader).toHaveClass("lucide-loader");
    expect(loader).toHaveClass("animate-spin", "motion-reduce:animate-none");
    push("system-certificates-changed", ready({ revision: 2 }));
    expect(screen.getByText("142 certificates, exported 07:05.").closest("p")!.querySelector("svg")).toHaveClass("lucide-circle-check");
    push("system-certificates-changed", ready({ revision: 3, phase: "failed", error: "no certificate authority found" }));
    expect(screen.getByText(/^Could not export/).closest("p")!.querySelector("svg")).toHaveClass("lucide-triangle-alert");
  });

  it("is the description of the switch and a polite live region", async () => {
    install(true, ready());
    await show();
    const toggle = screen.getByRole("switch", { name: "System Certificates" });
    const text = await screen.findByText("142 certificates, exported 07:05.");
    expect(toggle).toHaveAccessibleDescription(`${DESCRIPTION} 142 certificates, exported 07:05. Refresh`);
    expect(text.closest("[aria-live]")).toHaveAttribute("aria-live", "polite");
    expect(toggle.getAttribute("aria-describedby")!.split(" ")).toContain(text.closest("[aria-live]")!.id);
  });

  it("appears when the switch is turned on and goes when it is turned off", async () => {
    await show();
    expect(screen.queryByText(/certificates, exported/)).not.toBeInTheDocument();
    push("settings-changed", settings(2, true));
    push("system-certificates-changed", ready({ revision: 2 }));
    expect(screen.getByText("142 certificates, exported 07:05.")).toBeInTheDocument();
    push("settings-changed", settings(3, false));
    expect(screen.queryByText(/certificates, exported/)).not.toBeInTheDocument();
  });
});

describe("Refresh", () => {
  it("asks the backend to export again and shows the progress in the line", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    install(true, ready());
    await show();
    refreshResult = certificates({ revision: 2, phase: "exporting", count: 142, writtenAt: TODAY });
    await user.click(await screen.findByRole("button", { name: "Refresh system certificates" }));
    expect(refreshes).toBe(1);
    expect(await screen.findByText("Exporting system certificates…", { selector: "p span" })).toBeInTheDocument();
  });

  it("is reached with Tab and used with Enter and Space", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    install(true, ready());
    await show();
    const button = await screen.findByRole("button", { name: "Refresh system certificates" });
    await user.tab();
    await user.tab();
    expect(button).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(refreshes).toBe(1);
    await user.keyboard(" ");
    expect(refreshes).toBe(2);
  });

  it("is disabled while an export runs, with the tooltip, and ignores every activation", async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    install(true, certificates({ phase: "exporting" }));
    await show();
    const button = await screen.findByRole("button", { name: "Refresh system certificates" });
    expect(button).toHaveAttribute("aria-disabled", "true");
    await user.click(button);
    await user.keyboard("{Enter}");
    expect(refreshes).toBe(0);
    act(() => button.focus());
    expect(button).toHaveFocus();
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Exporting system certificates…");
    expect(button).toHaveAccessibleDescription(expect.stringContaining("Exporting system certificates…"));
    push("system-certificates-changed", ready({ revision: 2 }));
    expect(button).not.toHaveAttribute("aria-disabled");
    expect(button).toHaveFocus();
  });

  it("wraps under the text below about 300 px: the line is a wrapping row and the text can shrink", async () => {
    install(true, ready());
    await show();
    const text = (await screen.findByText("142 certificates, exported 07:05.")).closest("p")!;
    expect(text.parentElement).toHaveClass("flex-wrap");
    expect(text).toHaveClass("min-w-0", "flex-1");
  });
});
