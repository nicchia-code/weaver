import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { OmpStatus } from "@/bindings/OmpStatus";
import type { UsageLimitStatus } from "@/bindings/UsageLimitStatus";
import type { UsageStatus } from "@/bindings/UsageStatus";
import type { UsageSummary as Summary } from "@/bindings/UsageSummary";
import { TooltipProvider } from "@/components/ui/tooltip";
import { UsageSummary, summaryText, summaryView } from "@/components/usage-summary";

const usage = vi.hoisted(() => ({ current: null as UsageStatus | null }));
vi.mock("@/hooks/use-usage", () => ({ useUsageStatus: () => usage.current }));

const omp = vi.hoisted(() => ({ current: null as OmpStatus | null }));
vi.mock("@/hooks/use-omp", () => ({ useOmp: () => ({ status: omp.current }) }));

const NOW = new Date(2026, 10, 2, 7, 0).getTime();
const HOUR = 3_600_000;
const MIN = 60_000;

const ompState = (state: OmpStatus["state"]) => ({ state, models: [], providers: [], version: "18.16.0" }) as unknown as OmpStatus;
const summary = (patch: Partial<Summary> = {}): Summary => ({ label: "Claude 5 Hour", usedFraction: 0.17, status: "ok", resetsAt: NOW + 2 * HOUR + 10 * MIN, ...patch });
const statusOf = (patch: Partial<UsageStatus> = {}): UsageStatus => ({ revision: 1, phase: "idle", failure: null, usage: null, summary: null, ...patch });
const emptyUsage = { fetchedAt: null, providers: [] };
const withLimits = (patch: Partial<Summary> = {}) => statusOf({ usage: emptyUsage, summary: summary(patch) });

beforeEach(() => {
  usage.current = null;
  omp.current = ompState("ready");
  vi.spyOn(Date, "now").mockReturnValue(NOW);
});

function renderSummary(compact = false, onOpen: (origin: HTMLElement) => void = () => {}) {
  return render(
    <TooltipProvider>
      <UsageSummary compact={compact} onOpen={onOpen} />
    </TooltipProvider>,
  );
}

const button = () => screen.getByRole("button");

describe("the box", () => {
  it("is one button on Surface with a Line ring, radius 12 px and padding 12 px, named after its text", () => {
    usage.current = withLimits();
    renderSummary();
    const box = button();
    expect(box).toHaveAccessibleName("Usage: 17% used, Claude 5 Hour · Resets in 2 h 10 min");
    expect(box).toHaveClass("bg-surface", "shadow-ring", "rounded-xl", "p-3");
    expect(screen.getAllByRole("button")).toHaveLength(1);
  });

  it("calls onOpen with the button, which is where the focus comes back when Usage closes", async () => {
    usage.current = withLimits();
    const onOpen = vi.fn();
    renderSummary(false, onOpen);
    await userEvent.setup().click(button());
    expect(onOpen).toHaveBeenCalledWith(button());
  });
});

describe("the limit closest to being used up", () => {
  it("shows Usage and the used percentage, a 6 px bar of that fraction, and the limit's name with its reset", () => {
    usage.current = withLimits();
    renderSummary();
    expect(screen.getByText("Usage")).toHaveClass("text-xs", "font-medium");
    expect(screen.getByText("17%")).toBeInTheDocument();
    const bar = button().querySelector("[data-usage-bar]") as HTMLElement;
    expect(bar).toHaveStyle({ width: "17%" });
    expect(bar.parentElement).toHaveClass("h-1.5", "bg-line-soft");
    const detail = screen.getByText("Claude 5 Hour · Resets in 2 h 10 min");
    expect(detail).toHaveClass("text-xs", "text-muted-foreground");
  });

  it.each<[UsageLimitStatus, string, string]>([
    ["ok", "bg-ink-2", ""],
    ["almostUsedUp", "bg-peach", "almost used up"],
    ["usedUp", "bg-rose", "used up"],
  ])("draws the bar of a limit that is %s in %s and says the state in words", (status, tone, words) => {
    usage.current = withLimits({ status, usedFraction: 0.9 });
    renderSummary();
    const bar = button().querySelector("[data-usage-bar]") as HTMLElement;
    expect(bar).toHaveClass(tone);
    if (words) expect(button().getAttribute("aria-label")).toContain(words);
    else expect(button().getAttribute("aria-label")).not.toContain("used up");
  });

  it("shows no reset when omp gives none, and a date when it is a day away or more", () => {
    usage.current = withLimits({ resetsAt: null });
    const { unmount } = renderSummary();
    expect(screen.getByText("Claude 5 Hour")).toBeInTheDocument();
    unmount();
    usage.current = withLimits({ resetsAt: NOW + 3 * 24 * HOUR });
    renderSummary();
    expect(screen.getByText(/^Claude 5 Hour · Resets on /)).toBeInTheDocument();
  });

  it("shows neither a percentage nor a bar when the used fraction is unknown", () => {
    usage.current = withLimits({ usedFraction: null });
    renderSummary();
    expect(screen.queryByText(/%/)).not.toBeInTheDocument();
    expect(button().querySelector("[data-usage-bar]")).toBeNull();
    expect(button()).toHaveAccessibleName("Usage: Claude 5 Hour · Resets in 2 h 10 min");
  });

  it("keeps the limits read earlier shown while a reading runs, after a failure and while the agent is not ready", () => {
    const read = withLimits({ usedFraction: 0.42 });
    usage.current = { ...read, phase: "reading" };
    const reading = renderSummary();
    expect(screen.getByText("42%")).toBeInTheDocument();
    reading.unmount();
    usage.current = { ...read, phase: "failed", failure: "timed out" };
    const failed = renderSummary();
    expect(screen.getByText("42%")).toBeInTheDocument();
    failed.unmount();
    omp.current = ompState("missing");
    usage.current = read;
    renderSummary();
    expect(screen.getByText("42%")).toBeInTheDocument();
  });
});

describe("without a limit to show", () => {
  const cases: Array<[string, UsageStatus | null, OmpStatus["state"] | null, string]> = [
    ["nothing read yet", null, "ready", "Open to read usage"],
    ["a first reading", statusOf({ phase: "reading" }), "ready", "Reading usage…"],
    ["no limit in the connected providers", statusOf({ usage: emptyUsage }), "ready", "No usage limits"],
    ["the agent is not ready", null, "missing", "Usage needs the agent"],
    ["the agent is not ready while reading", statusOf({ phase: "reading" }), "installing", "Usage needs the agent"],
    ["a failure and no limits ever read", statusOf({ phase: "failed", failure: "timed out" }), "ready", "Could not read usage"],
    ["a status of omp not known yet", null, null, "Open to read usage"],
  ];
  it.each(cases)("says what is going on: %s", (_name, status, state, text) => {
    usage.current = status;
    omp.current = state === null ? null : ompState(state);
    renderSummary();
    expect(button()).toHaveAccessibleName(`Usage: ${text}`);
    expect(screen.getByText(text)).toHaveClass("text-xs", "text-muted-foreground");
    expect(screen.queryByText(/%/)).not.toBeInTheDocument();
    expect(button().querySelector("[data-usage-bar]")).toBeNull();
  });
});

describe("with icons only", () => {
  it("is a Gauge button whose tooltip and accessible name carry the text of the summary", async () => {
    usage.current = withLimits({ status: "almostUsedUp", usedFraction: 0.85 });
    renderSummary(true);
    const gauge = button();
    expect(gauge.querySelector("svg")).toHaveClass("lucide-gauge");
    expect(gauge.textContent).toBe("");
    const name = "Usage: 85% used, almost used up, Claude 5 Hour · Resets in 2 h 10 min";
    expect(gauge).toHaveAccessibleName(name);
    await userEvent.setup().hover(gauge);
    expect(await screen.findByRole("tooltip")).toHaveTextContent(name);
  });

  it("opens Usage like the box", async () => {
    const onOpen = vi.fn();
    renderSummary(true, onOpen);
    await userEvent.setup().click(button());
    expect(onOpen).toHaveBeenCalledWith(button());
  });
});

describe("the text of the summary", () => {
  it("puts the used percentage, the state in words and the limit with its reset in one text", () => {
    expect(summaryText(summaryView(withLimits(), true, NOW))).toBe("17% used, Claude 5 Hour · Resets in 2 h 10 min");
    expect(summaryText(summaryView(withLimits({ status: "usedUp", usedFraction: 1 }), true, NOW))).toBe("100% used, used up, Claude 5 Hour · Resets in 2 h 10 min");
  });

  it("rounds the percentage to a whole number", () => {
    expect(summaryView(withLimits({ usedFraction: 0.1749 }), true, NOW)).toMatchObject({ kind: "limit", percent: 17 });
    expect(summaryView(withLimits({ usedFraction: 0.176 }), true, NOW)).toMatchObject({ kind: "limit", percent: 18 });
  });
});
