import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ComponentProps, ReactElement } from "react";

import type { OmpStatus } from "@/bindings/OmpStatus";
import type { ProviderUsage } from "@/bindings/ProviderUsage";
import type { Usage } from "@/bindings/Usage";
import type { UsageLimit } from "@/bindings/UsageLimit";
import type { UsageNoLimits } from "@/bindings/UsageNoLimits";
import type { UsageStatus } from "@/bindings/UsageStatus";
import { UsageDialog, formatLimitValue, formatReset } from "@/components/usage-dialog";
import { UsageProvider } from "@/hooks/use-usage";
import { call, on } from "@/lib/backend";
import { formatDate, formatTime } from "@/lib/format";

vi.mock("@/lib/backend", () => ({ call: vi.fn(), on: vi.fn() }));

const omp = vi.hoisted(() => ({ current: null as OmpStatus | null }));
vi.mock("@/hooks/use-omp", () => ({ useOmp: () => ({ status: omp.current }) }));

const READ_AT = new Date(2026, 10, 2, 7, 5).getTime();

const ompStatus = (state: OmpStatus["state"], patch: Partial<OmpStatus> = {}) => ({ state, models: [], providers: [], version: "18.16.0", ...patch }) as unknown as OmpStatus;
const READY = ompStatus("ready", { models: [{ provider: "anthropic", selector: "anthropic/a", name: null, thinkingLevels: [] }] });

const limit = (patch: Partial<UsageLimit> = {}): UsageLimit => ({
  label: "Claude 5 Hour",
  resetsAt: null,
  used: 17,
  limit: 100,
  unit: "percent",
  usedFraction: 0.17,
  status: "ok",
  ...patch,
});
const provider = (id: string, name: string, limits: UsageLimit[], noLimits: UsageNoLimits | null = null): ProviderUsage => ({ id, name, limits, noLimits });
const usageOf = (providers: ProviderUsage[], fetchedAt: number | null = READ_AT): Usage => ({ fetchedAt, providers });
const statusOf = (patch: Partial<UsageStatus>): UsageStatus => ({ revision: 1, phase: "idle", failure: null, usage: null, summary: null, ...patch });

/** How many readings the dialog asked the backend for (the provider also asks for the status, which reads nothing). */
const reads = () => vi.mocked(call).mock.calls.filter(([name]) => name === "omp_usage").length;
const withUsage = (node: ReactElement) => <UsageProvider>{node}</UsageProvider>;

let handlers: Record<string, (payload: never) => void>;
const push = (status: UsageStatus) => act(() => handlers["usage-changed"]?.(status as never));

beforeEach(() => {
  handlers = {};
  omp.current = READY;
  vi.mocked(on).mockImplementation(((name: string, handler: (payload: never) => void) => {
    handlers[name] = handler;
    return Promise.resolve(() => {});
  }) as unknown as typeof on);
  // The reading never ends by itself: the tests send the statuses the backend would emit.
  vi.mocked(call).mockImplementation((() => new Promise(() => {})) as unknown as typeof call);
});

function renderDialog(props: Partial<ComponentProps<typeof UsageDialog>> = {}) {
  const onClose = vi.fn();
  const onOpenSettings = vi.fn();
  const view = render(withUsage(<UsageDialog open onClose={onClose} onOpenSettings={onOpenSettings} {...props} />));
  return { ...view, onClose, onOpenSettings };
}

const dialog = () => screen.getByRole("dialog", { name: "Usage" });

describe("closed", () => {
  it("shows nothing and reads nothing", () => {
    render(withUsage(<UsageDialog open={false} onClose={() => {}} onOpenSettings={() => {}} />));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(reads()).toBe(0);
  });
});

describe("waiting for the agent", () => {
  const cases: Array<[string, OmpStatus | null, string]> = [
    ["missing", ompStatus("missing"), "The agent is not installed."],
    ["installing", ompStatus("installing"), "Installing the agent in the terminal…"],
    ["updating", ompStatus("updating", { updateTarget: "18.17.0" }), "Updating the agent to 18.17.0…"],
    ["logging in", ompStatus("loggingIn", { loginProvider: null }), "Waiting for the login to the provider in the terminal…"],
    ["not connected", ompStatus("notConnected"), "The agent has no model provider. Log in to one below."],
    ["checking", ompStatus("checking"), "Checking the agent…"],
    ["no answer yet", null, "Checking the agent…"],
  ];
  it.each(cases)("says why the agent is not ready when it is %s, with the text of its status box, and reads nothing", (_name, status, text) => {
    omp.current = status;
    renderDialog();
    expect(within(dialog()).getByText(text)).toBeInTheDocument();
    expect(within(dialog()).getByRole("button", { name: "Open Settings" })).toBeInTheDocument();
    expect(reads()).toBe(0);
    expect(within(dialog()).queryByText("Reading usage…")).not.toBeInTheDocument();
  });

  it("closes the dialog and shows Settings with Open Settings", async () => {
    omp.current = ompStatus("missing");
    const { onOpenSettings, onClose } = renderDialog();
    await userEvent.setup().click(screen.getByRole("button", { name: "Open Settings" }));
    expect(onOpenSettings).toHaveBeenCalledTimes(1);
    expect(onClose).not.toHaveBeenCalled();
  });

  it("starts reading by itself when the agent becomes ready, without closing and opening the dialog", async () => {
    omp.current = ompStatus("updating");
    const { rerender, onClose, onOpenSettings } = renderDialog();
    expect(reads()).toBe(0);
    omp.current = READY;
    rerender(withUsage(<UsageDialog open onClose={onClose} onOpenSettings={onOpenSettings} />));
    expect(call).toHaveBeenCalledWith("omp_usage");
    expect(reads()).toBe(1);
    expect(await screen.findByText("Reading usage…")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open Settings" })).not.toBeInTheDocument();
  });
});

describe("reading", () => {
  it("asks for the limits at each opening and shows a centred Loader and Reading usage… without earlier limits", () => {
    renderDialog();
    expect(call).toHaveBeenCalledWith("omp_usage");
    expect(reads()).toBe(1);
    const text = within(dialog()).getByText("Reading usage…");
    expect(text).toHaveClass("justify-center");
    expect(text.querySelector("svg")).toHaveClass("animate-spin", "motion-reduce:animate-none");
    expect(dialog().querySelector("[aria-busy]")).toHaveAttribute("aria-busy", "true");
    expect(within(dialog()).queryByText(/Updated at/)).not.toBeInTheDocument();
  });

  it("keeps the limits read last, marked as being updated, while a reading runs", () => {
    renderDialog();
    push(statusOf({ revision: 2, phase: "reading", usage: usageOf([provider("anthropic", "Anthropic", [limit()])]) }));
    expect(within(dialog()).getByRole("progressbar", { name: "Claude 5 Hour: 17% used" })).toBeInTheDocument();
    const updating = within(dialog()).getByText("Updating…");
    expect(updating.querySelector("svg")).toHaveClass("animate-spin", "motion-reduce:animate-none");
    expect(within(dialog()).getByText(`Updated at ${formatTime(READ_AT)}`)).toBeInTheDocument();
    expect(within(dialog()).queryByText("Reading usage…")).not.toBeInTheDocument();
    expect(dialog().querySelector("[aria-busy]")).toHaveAttribute("aria-busy", "true");
  });

  it("is no longer busy once the limits are shown", () => {
    renderDialog();
    push(statusOf({ revision: 2, phase: "idle", usage: usageOf([provider("anthropic", "Anthropic", [limit()])]) }));
    expect(within(dialog()).queryByText("Updating…")).not.toBeInTheDocument();
    expect(dialog().querySelector("[aria-busy]")).toHaveAttribute("aria-busy", "false");
  });
});

describe("shown", () => {
  it("has a block per connected provider, in the order it is given, with its bold short name", () => {
    renderDialog();
    push(
      statusOf({
        revision: 2,
        usage: usageOf([
          provider("anthropic", "Anthropic", [limit(), limit({ label: "Claude 7 Day", used: 29, usedFraction: 0.29 })]),
          provider("github-copilot", "GitHub Copilot", [limit({ label: "Premium Requests", used: 3437, limit: 100000, usedFraction: 0.03437, unit: "requests" })]),
          provider("cursor", "Cursor", [limit({ label: "Personal Usage", used: 0.5, limit: 1000, usedFraction: 0.0005, unit: "usd" })]),
        ]),
      }),
    );
    const names = within(dialog())
      .getAllByRole("heading", { level: 3 })
      .map((heading) => heading.textContent);
    expect(names).toEqual(["Anthropic", "GitHub Copilot", "Cursor"]);
    expect(within(dialog()).getByRole("heading", { name: "Anthropic" })).toHaveClass("font-bold");
    const anthropic = within(dialog()).getByRole("region", { name: "Anthropic" });
    expect(within(anthropic).getAllByRole("listitem").map((row) => row.textContent)).toEqual(["Claude 5 Hour17% used", "Claude 7 Day29% used"]);
    expect(within(dialog()).getByRole("progressbar", { name: "Premium Requests: 3,437 / 100,000 requests" })).toBeInTheDocument();
    expect(within(dialog()).getByRole("progressbar", { name: "Personal Usage: $0.50 / $1,000.00" })).toBeInTheDocument();
    expect(within(dialog()).getByText(`Updated at ${formatTime(READ_AT)}`)).toBeInTheDocument();
    expect(within(dialog()).queryByText("No usage limits to show.")).not.toBeInTheDocument();
  });

  it("fills the bar with the used fraction", () => {
    renderDialog();
    push(statusOf({ revision: 2, usage: usageOf([provider("anthropic", "Anthropic", [limit({ usedFraction: 0.42, used: 42 })])]) }));
    const bar = within(dialog()).getByRole("progressbar", { name: "Claude 5 Hour: 42% used" });
    expect(bar).toHaveAttribute("aria-valuenow", "42");
    expect(bar.firstElementChild).toHaveStyle({ transform: "translateX(-58%)" });
  });

  it("shows the reason of a connected provider without limits", () => {
    renderDialog();
    push(
      statusOf({
        revision: 2,
        usage: usageOf([
          provider("anthropic", "Anthropic", [limit()]),
          provider("google-gemini-cli", "Gemini CLI", [], "noUsageData"),
          provider("xai-oauth", "xAI", [], "loginExpired"),
          provider("cursor", "Cursor", [], "notReported"),
        ]),
      }),
    );
    expect(within(within(dialog()).getByRole("region", { name: "Gemini CLI" })).getByText("No usage data")).toBeInTheDocument();
    expect(within(within(dialog()).getByRole("region", { name: "xAI" })).getByText("Login expired")).toBeInTheDocument();
    expect(within(within(dialog()).getByRole("region", { name: "Cursor" })).getByText("Not reported by the agent")).toBeInTheDocument();
    expect(within(dialog()).queryByText("No usage limits to show.")).not.toBeInTheDocument();
  });

  it("says that there are no limits above the blocks when no provider has any", () => {
    renderDialog();
    push(statusOf({ revision: 2, usage: usageOf([provider("cursor", "Cursor", [], "notReported")], null) }));
    const none = within(dialog()).getByText("No usage limits to show.");
    expect(none).toHaveClass("text-muted-foreground");
    expect(none.compareDocumentPosition(within(dialog()).getByRole("region", { name: "Cursor" })) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(dialog()).queryByText(/Updated at/)).not.toBeInTheDocument();
  });

  it("writes almost used up in amber and used up in red, shade 600 in light and 400 in dark, and the others neutral", () => {
    renderDialog();
    push(
      statusOf({
        revision: 2,
        usage: usageOf([
          provider("anthropic", "Anthropic", [
            limit({ label: "Fine", used: 10, usedFraction: 0.1, status: "ok" }),
            limit({ label: "Nearly", used: 85, usedFraction: 0.85, status: "almostUsedUp" }),
            limit({ label: "Gone", used: 100, usedFraction: 1, status: "usedUp" }),
          ]),
        ]),
      }),
    );
    const row = (name: string) => within(dialog()).getByText(name).closest("li") as HTMLElement;
    const bar = (name: string) => within(row(name)).getByRole("progressbar").firstElementChild;

    expect(within(row("Nearly")).getByText("85% used")).toHaveClass("text-peach-ink");
    expect(within(row("Nearly")).getByText("Almost used up")).toHaveClass("text-peach-ink");
    expect(bar("Nearly")).toHaveClass("bg-peach");

    expect(within(row("Gone")).getByText("100% used")).toHaveClass("text-rose-ink");
    expect(within(row("Gone")).getByText("Used up")).toHaveClass("text-rose-ink");
    expect(bar("Gone")).toHaveClass("bg-rose");

    expect(within(row("Fine")).queryByText("Almost used up")).not.toBeInTheDocument();
    expect(within(row("Fine")).queryByText("Used up")).not.toBeInTheDocument();
    expect(within(row("Fine")).getByText("10% used").className).not.toMatch(/-(amber|red)-/);
    expect(bar("Fine")?.className).not.toMatch(/-(amber|red)-/);
  });

  it("writes when a limit resets: in a duration under 24 hours, on a date otherwise, and nothing without a time", () => {
    renderDialog();
    const now = Date.now();
    const in90Minutes = now + 90 * 60_000 + 5_000;
    const in3Days = now + 3 * 24 * 60 * 60_000;
    push(
      statusOf({
        revision: 2,
        usage: usageOf([
          provider("anthropic", "Anthropic", [
            limit({ label: "Soon", resetsAt: in90Minutes }),
            limit({ label: "Later", resetsAt: in3Days }),
            limit({ label: "Never", resetsAt: null }),
          ]),
        ]),
      }),
    );
    expect(within(dialog()).getByText("Resets in 1 h 30 min")).toBeInTheDocument();
    expect(within(dialog()).getByText(`Resets on ${formatDate(in3Days)}`)).toBeInTheDocument();
    expect(within(dialog()).getAllByText(/^Resets /)).toHaveLength(2);
  });
});

describe("failed", () => {
  const earlier = usageOf([provider("anthropic", "Anthropic", [limit()])]);

  it("says why, offers Retry, and keeps the earlier limits below with their time", async () => {
    renderDialog();
    push(statusOf({ revision: 3, phase: "failed", failure: "timed out", usage: earlier }));
    const alert = within(dialog()).getByRole("alert");
    expect(alert).toHaveTextContent("Could not read usage: timed out.");
    const retry = within(alert).getByRole("button", { name: "Retry" });
    expect(retry.querySelector("svg")).toHaveClass("lucide-rotate-cw");
    expect(within(dialog()).getByRole("progressbar", { name: "Claude 5 Hour: 17% used" })).toBeInTheDocument();
    expect(within(dialog()).getByText(`Updated at ${formatTime(READ_AT)}`)).toBeInTheDocument();
    expect(alert.compareDocumentPosition(within(dialog()).getByRole("region", { name: "Anthropic" })) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("keeps a reason that ends with a full stop to one", () => {
    renderDialog();
    push(statusOf({ revision: 3, phase: "failed", failure: "the agent did not return valid usage." }));
    expect(within(dialog()).getByRole("alert")).toHaveTextContent("Could not read usage: the agent did not return valid usage.");
    expect(within(dialog()).getByRole("alert").textContent).not.toContain("..");
    expect(within(dialog()).queryByText("No usage limits to show.")).not.toBeInTheDocument();
  });

  it("reads again with Retry and shows Reading, not the failure, until the backend answers", async () => {
    renderDialog();
    push(statusOf({ revision: 3, phase: "failed", failure: "timed out", usage: earlier }));
    vi.mocked(call).mockClear();
    await userEvent.setup().click(within(dialog()).getByRole("button", { name: "Retry" }));
    expect(call).toHaveBeenCalledWith("omp_usage");
    expect(within(dialog()).queryByRole("alert")).not.toBeInTheDocument();
    expect(within(dialog()).getByText("Updating…")).toBeInTheDocument();
    expect(within(dialog()).getByRole("progressbar", { name: "Claude 5 Hour: 17% used" })).toBeInTheDocument();

    push(statusOf({ revision: 4, phase: "reading", usage: earlier }));
    push(statusOf({ revision: 5, phase: "idle", usage: usageOf([provider("anthropic", "Anthropic", [limit({ used: 30, usedFraction: 0.3 })])]) }));
    expect(within(dialog()).getByRole("progressbar", { name: "Claude 5 Hour: 30% used" })).toBeInTheDocument();
    expect(within(dialog()).queryByText("Updating…")).not.toBeInTheDocument();
  });

  it("ignores a status older than the one it holds", () => {
    renderDialog();
    push(statusOf({ revision: 5, phase: "idle", usage: earlier }));
    push(statusOf({ revision: 4, phase: "failed", failure: "old" }));
    expect(within(dialog()).queryByRole("alert")).not.toBeInTheDocument();
  });

  it("goes to Waiting for the agent when the agent is no longer ready", () => {
    const { rerender, onClose, onOpenSettings } = renderDialog();
    push(statusOf({ revision: 3, phase: "failed", failure: "timed out", usage: earlier }));
    omp.current = ompStatus("missing");
    rerender(withUsage(<UsageDialog open onClose={onClose} onOpenSettings={onOpenSettings} />));
    expect(within(dialog()).getByText("The agent is not installed.")).toBeInTheDocument();
    expect(within(dialog()).queryByRole("alert")).not.toBeInTheDocument();
  });
});

describe("closing", () => {
  it("closes with Esc, with the X and with a click outside", async () => {
    const user = userEvent.setup();
    const { onClose } = renderDialog();
    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(1);
    await user.click(within(dialog()).getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledTimes(2);
    await user.click(dialog().parentElement as HTMLElement);
    expect(onClose).toHaveBeenCalledTimes(3);
  });

  it("is at most 480 px wide, has no Refresh button and reads once per opening", () => {
    renderDialog();
    expect(dialog()).toHaveClass("max-w-[480px]");
    expect(within(dialog()).queryByRole("button", { name: /refresh/i })).not.toBeInTheDocument();
    expect(reads()).toBe(1);
  });

  it("reads again each time it is opened, keeping the limits read last meanwhile", () => {
    const { rerender, onClose, onOpenSettings } = renderDialog();
    push(statusOf({ revision: 2, usage: usageOf([provider("anthropic", "Anthropic", [limit()])]) }));
    rerender(withUsage(<UsageDialog open={false} onClose={onClose} onOpenSettings={onOpenSettings} />));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    rerender(withUsage(<UsageDialog open onClose={onClose} onOpenSettings={onOpenSettings} />));
    expect(reads()).toBe(2);
    // The limits are the ones every window holds: they stay, marked as being updated.
    expect(within(dialog()).getByText("Updating…")).toBeInTheDocument();
    expect(within(dialog()).getByRole("progressbar", { name: "Claude 5 Hour: 17% used" })).toBeInTheDocument();
  });

  it("shows the reading a timer started while it is open, keeping the limits shown", () => {
    renderDialog();
    push(statusOf({ revision: 2, usage: usageOf([provider("anthropic", "Anthropic", [limit()])]) }));
    expect(within(dialog()).queryByText("Updating…")).not.toBeInTheDocument();
    push(statusOf({ revision: 3, phase: "reading", usage: usageOf([provider("anthropic", "Anthropic", [limit()])]) }));
    expect(within(dialog()).getByText("Updating…")).toBeInTheDocument();
    expect(within(dialog()).getByRole("progressbar", { name: "Claude 5 Hour: 17% used" })).toBeInTheDocument();
    // A failed one in the background is shown too, with Retry.
    push(statusOf({ revision: 4, phase: "failed", failure: "timed out", usage: usageOf([provider("anthropic", "Anthropic", [limit()])]) }));
    expect(within(dialog()).getByRole("alert")).toHaveTextContent("Could not read usage: timed out.");
    push(statusOf({ revision: 5, phase: "reading", usage: usageOf([provider("anthropic", "Anthropic", [limit()])]) }));
    expect(within(dialog()).queryByRole("alert")).not.toBeInTheDocument();
    expect(within(dialog()).getByText("Updating…")).toBeInTheDocument();
  });
});

describe("values by unit", () => {
  const value = (patch: Partial<UsageLimit>) => formatLimitValue(limit(patch));

  it("writes each unit as the requirements say", () => {
    expect(value({ unit: "percent", used: 17 })).toBe("17% used");
    expect(value({ unit: "requests", used: 3437, limit: 100000, usedFraction: 0.03437 })).toBe("3,437 / 100,000 requests");
    expect(value({ unit: "tokens", used: 1200, limit: 5000, usedFraction: 0.24 })).toBe("1,200 / 5,000 tokens");
    expect(value({ unit: "credits", used: 3, limit: 10, usedFraction: 0.3 })).toBe("3 / 10 credits");
    expect(value({ unit: "minutes", used: 12, limit: 60, usedFraction: 0.2 })).toBe("12 / 60 min");
    expect(value({ unit: "usd", used: 0.5, limit: 1000, usedFraction: 0.0005 })).toBe("$0.50 / $1,000.00");
  });

  it("writes only the used amount without a limit", () => {
    expect(value({ unit: "requests", used: 5, limit: null, usedFraction: 0.5 })).toBe("5 requests");
    expect(value({ unit: "usd", used: 2.5, limit: null, usedFraction: 0.1 })).toBe("$2.50");
    expect(value({ unit: "tokens", used: 1234567, limit: null, usedFraction: 0.1 })).toBe("1,234,567 tokens");
  });

  it("writes Usage unknown, with no bar, when the used fraction is not known", () => {
    expect(value({ unit: "requests", used: 5, limit: null, usedFraction: null })).toBe("Usage unknown");
    renderDialog();
    push(statusOf({ revision: 2, usage: usageOf([provider("anthropic", "Anthropic", [limit({ label: "Mystery", unit: "requests", used: 5, limit: null, usedFraction: null })])]) }));
    expect(within(dialog()).getByText("Usage unknown")).toBeInTheDocument();
    expect(within(dialog()).queryByRole("progressbar")).not.toBeInTheDocument();
  });

  it("writes a reset under 24 hours as a duration and later as a date", () => {
    const now = new Date(2026, 10, 2, 7, 0).getTime();
    expect(formatReset(null, now)).toBeNull();
    expect(formatReset(now + 20_000, now)).toBe("Resets in less than a minute");
    expect(formatReset(now + 45 * 60_000, now)).toBe("Resets in 45 min");
    expect(formatReset(now + 23 * 3_600_000 + 59 * 60_000, now)).toBe("Resets in 23 h 59 min");
    expect(formatReset(now + 24 * 3_600_000, now)).toBe(`Resets on ${formatDate(now + 24 * 3_600_000)}`);
    expect(formatReset(new Date(2026, 11, 1).getTime(), now)).toBe("Resets on Tuesday, 1 December 2026");
  });
});
