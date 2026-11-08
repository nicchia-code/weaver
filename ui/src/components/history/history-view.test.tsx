import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent, { type UserEvent } from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { HistoryEntry } from "@/bindings/HistoryEntry";
import type { HistorySnapshot } from "@/bindings/HistorySnapshot";
import type { HistoryTask } from "@/bindings/HistoryTask";
import { BoardActionsContext } from "@/components/board/board-ui";
import { groupByDay, inPeriod, localMidnight, periodTotals } from "@/components/history/history-model";
import { HistoryView } from "@/components/history/history-view";
import { ProjectContext } from "@/components/project-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const PROJECT = "0123456789abcdef";

/** Monday, 2 November 2026, 12:00, local time: the tests never depend on the time zone of the machine. */
const NOW = new Date(2026, 10, 2, 12, 0).getTime();
const local = (month: number, day: number, hour: number, minute: number) => new Date(2026, month - 1, day, hour, minute);
const instant = (date: Date) => date.toISOString();

function task(id: string, title: string, patch: Partial<HistoryTask> = {}): HistoryTask {
  return { id, title, label: null, finished: instant(local(11, 2, 8, 5)), refs: [], body: "", ...patch };
}

function entry(id: string, title: string, updated: Date, patch: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    id,
    title,
    summary: `Summary of ${title}`,
    text: "",
    created: instant(updated),
    updated: instant(updated),
    tasks: [task(`${id.slice(0, 4)}task1`, "A Task")],
    ...patch,
  };
}

let current: HistorySnapshot;
let handlers: Map<string, (payload: unknown) => void>;

function install(entries: HistoryEntry[], revision = 1) {
  current = { projectId: PROJECT, revision, entries };
  handlers = new Map();
  vi.mocked(on).mockImplementation(((name: string, handler: (payload: unknown) => void) => {
    handlers.set(name, handler);
    return Promise.resolve(() => {});
  }) as unknown as typeof on);
  vi.mocked(call).mockImplementation(((name: string) => {
    if (name === "history_snapshot") return Promise.resolve(current);
    return Promise.reject(new Error(`unexpected command ${name}`));
  }) as unknown as typeof call);
}

async function show() {
  render(
    <TooltipProvider>
      <ProjectContext.Provider value={{ projectId: PROJECT, showView: vi.fn(), openJob: vi.fn(), openedJob: null, openedJobRequest: 0 }}>
        <HistoryView now={() => NOW} />
      </ProjectContext.Provider>
    </TooltipProvider>,
  );
  await screen.findByRole("combobox", { name: "Period" });
}

async function choosePeriod(user: UserEvent, name: string) {
  await user.click(screen.getByRole("combobox", { name: "Period" }));
  await user.click(await screen.findByRole("option", { name }));
}

const group = (name: string) => screen.getByRole("region", { name });
const totalsOf = () => {
  const boxes = [...document.querySelectorAll("dl > div")];
  return Object.fromEntries(boxes.map((box) => [box.querySelector("dt")?.textContent, box.querySelector("dd")?.textContent]));
};
const titles = () => screen.queryAllByRole("button").map((button) => button.textContent ?? "");

beforeEach(() => {
  vi.mocked(call).mockReset();
  vi.mocked(on).mockReset();
});

describe("the periods", () => {
  it("run from the local midnight of their first day to now: today, 6 days ago, 29 days ago", () => {
    const at = (month: number, day: number, hour: number, minute: number) => local(month, day, hour, minute).getTime();
    expect(inPeriod(at(11, 2, 0, 0), "today", NOW)).toBe(true);
    expect(inPeriod(at(11, 1, 23, 59), "today", NOW)).toBe(false);

    expect(inPeriod(at(10, 27, 0, 0), "last-7-days", NOW)).toBe(true);
    expect(inPeriod(at(10, 26, 23, 59), "last-7-days", NOW)).toBe(false);

    expect(inPeriod(at(10, 4, 0, 0), "last-30-days", NOW)).toBe(true);
    expect(inPeriod(at(10, 3, 23, 59), "last-30-days", NOW)).toBe(false);

    expect(inPeriod(at(1, 1, 0, 0), "all", NOW)).toBe(true);
  });

  it("show an entry dated in the future only under All", () => {
    const future = NOW + 60 * 60 * 1000;
    for (const period of ["today", "last-7-days", "last-30-days"] as const) expect(inPeriod(future, period, NOW)).toBe(false);
    expect(inPeriod(future, "all", NOW)).toBe(true);
    expect(inPeriod(NOW, "today", NOW)).toBe(true);
  });

  it("count whole local days across a change of clock", () => {
    // The midnight of a day is the start of that local day whatever its length in hours.
    expect(new Date(localMidnight(NOW)).getHours()).toBe(0);
    expect(new Date(localMidnight(NOW, 6)).getDate()).toBe(27);
  });
});

describe("the grouping", () => {
  it("names Today, Yesterday and then the weekday, each with its date and its count, keeping the order of the backend", () => {
    const entries = [
      entry("aaaaaaaa", "One", local(11, 2, 11, 0)),
      entry("bbbbbbbb", "Two", local(11, 2, 9, 30)),
      entry("cccccccc", "Three", local(11, 1, 18, 0)),
      entry("dddddddd", "Four", local(10, 30, 10, 0)),
    ];
    const groups = groupByDay(entries, "all", NOW);
    expect(groups.map((g) => [g.name, g.date, g.count])).toEqual([
      ["Today", "Monday, 2 November 2026", "· 2 entries"],
      ["Yesterday", "Sunday, 1 November 2026", "· 1 entry"],
      ["Friday", "30 October 2026", "· 1 entry"],
    ]);
    expect(groups[0]?.entries.map((e) => e.title)).toEqual(["One", "Two"]);
  });
});

describe("the totals of a period", () => {
  const totals = (entries: HistoryEntry[], period: "today" | "all" = "all") => periodTotals(groupByDay(entries, period, NOW), NOW);

  it("count the entries, the Tasks they cite and give the date of the newest", () => {
    const entries = [
      entry("aaaaaaaa", "One", local(11, 2, 11, 5)),
      entry("bbbbbbbb", "Two", local(11, 1, 18, 0), { tasks: [task("bbbb0001", "A"), task("bbbb0002", "B")] }),
    ];
    expect(totals(entries)).toEqual({ delivered: 2, tasksDone: 3, lastDelivery: "Today, 11:05" });
    expect(totals(entries.slice(1))).toEqual({ delivered: 1, tasksDone: 2, lastDelivery: "Yesterday, 18:00" });
  });

  it("write the date out for an older newest entry, and give nothing for an empty period", () => {
    expect(totals([entry("aaaaaaaa", "Old", local(10, 30, 10, 0))]).lastDelivery).toBe("Friday, 30 October 2026");
    expect(totals([entry("aaaaaaaa", "Old", local(10, 30, 10, 0))], "today")).toEqual({ delivered: 0, tasksDone: 0, lastDelivery: null });
  });

  it("take the newest by date, not by position", () => {
    const entries = [entry("aaaaaaaa", "Early", local(11, 2, 8, 0)), entry("bbbbbbbb", "Late", local(11, 2, 10, 45))];
    expect(totals(entries).lastDelivery).toBe("Today, 10:45");
  });
});

describe("HistoryView", () => {
  it("has the view header with the title History, its subtitle and the Period filter on its right", async () => {
    install([entry("aaaaaaaa", "Checkout works", local(11, 2, 9, 30))]);
    await show();
    const header = screen.getByRole("heading", { level: 1, name: "History" }).closest("header")!;
    expect(within(header).getByText("Everything that shipped, grouped as it was delivered.")).toBeInTheDocument();
    expect(within(header).getByRole("combobox", { name: "Period" })).toBeInTheDocument();
  });

  it("offers Today, Last 7 days, Last 30 days and All time", async () => {
    const user = userEvent.setup();
    install([]);
    await show();
    await user.click(screen.getByRole("combobox", { name: "Period" }));
    expect((await screen.findAllByRole("option")).map((option) => option.textContent)).toEqual(["Today", "Last 7 days", "Last 30 days", "All time"]);
  });

  it("shows three totals computed on the entries of the period, and a dash for an empty one", async () => {
    const user = userEvent.setup();
    install([
      entry("aaaaaaaa", "Now", local(11, 2, 11, 5)),
      entry("bbbbbbbb", "Yesterday", local(11, 1, 18, 0), { tasks: [task("bbbb0001", "A"), task("bbbb0002", "B")] }),
      entry("cccccccc", "Long ago", local(8, 15, 12, 0)),
    ]);
    await show();
    expect(totalsOf()).toEqual({ Delivered: "2", "Tasks done": "3", "Last delivery": "Today, 11:05" });

    await choosePeriod(user, "All time");
    expect(totalsOf()).toEqual({ Delivered: "3", "Tasks done": "4", "Last delivery": "Today, 11:05" });

    await choosePeriod(user, "Today");
    expect(totalsOf()).toEqual({ Delivered: "1", "Tasks done": "1", "Last delivery": "Today, 11:05" });
  });

  it("shows a dash as the last delivery when the period is empty", async () => {
    install([entry("aaaaaaaa", "Old", local(9, 1, 12, 0))]);
    await show();
    expect(totalsOf()).toEqual({ Delivered: "0", "Tasks done": "0", "Last delivery": "—" });
  });

  it("groups the entries by local day under Today, Yesterday and the weekday, each with its date and count, and shows the time and the Tasks on every row", async () => {
    install([
      entry("aaaaaaaa", "Checkout works", local(11, 2, 9, 30)),
      entry("bbbbbbbb", "Cart totals", local(11, 2, 8, 15), { tasks: [task("bbbb0001", "One"), task("bbbb0002", "Two")] }),
      entry("cccccccc", "Search", local(11, 1, 18, 0)),
      entry("dddddddd", "Emails", local(10, 30, 10, 0)),
    ]);
    await show();

    const today = group("Today Monday, 2 November 2026 · 2 entries");
    expect(within(today).getAllByRole("listitem")).toHaveLength(2);
    const yesterday = group("Yesterday Sunday, 1 November 2026 · 1 entry");
    const friday = group("Friday 30 October 2026 · 1 entry");

    expect(within(today).getByText("09:30")).toBeInTheDocument();
    expect(within(today).getByText("08:15")).toBeInTheDocument();
    expect(within(today).getByText("2 tasks")).toBeInTheDocument();
    expect(within(today).getByText("1 task")).toBeInTheDocument();
    expect(within(yesterday).getByText("18:00")).toBeInTheDocument();
    expect(within(friday).getByText("10:00")).toBeInTheDocument();
  });

  it("shows Last 7 days first and filters by the period chosen, entries in the future only under All", async () => {
    const user = userEvent.setup();
    install([
      entry("aaaaaaaa", "Now", local(11, 2, 11, 0)),
      entry("bbbbbbbb", "Soon", new Date(NOW + 2 * 60 * 60 * 1000)),
      entry("cccccccc", "Yesterday", local(11, 1, 18, 0)),
      entry("dddddddd", "Edge of the week", local(10, 27, 0, 0)),
      entry("eeeeeeee", "Just outside the week", local(10, 26, 23, 59)),
      entry("ffffffff", "Edge of the month", local(10, 4, 0, 0)),
      entry("gggggggg", "Long ago", local(8, 15, 12, 0)),
    ]);
    await show();
    expect(screen.getByRole("combobox", { name: "Period" })).toHaveTextContent("Last 7 days");
    expect(titles().join("|")).toMatch(/Now.*Yesterday.*Edge of the week/);
    expect(screen.queryByText("Soon")).not.toBeInTheDocument();
    expect(screen.queryByText("Just outside the week")).not.toBeInTheDocument();

    await choosePeriod(user, "Today");
    expect(screen.getByText("Now")).toBeInTheDocument();
    expect(screen.queryByText("Yesterday")).not.toBeInTheDocument();

    await choosePeriod(user, "Last 30 days");
    expect(screen.getByText("Just outside the week")).toBeInTheDocument();
    expect(screen.getByText("Edge of the month")).toBeInTheDocument();
    expect(screen.queryByText("Long ago")).not.toBeInTheDocument();
    expect(screen.queryByText("Soon")).not.toBeInTheDocument();

    await choosePeriod(user, "All time");
    expect(screen.getByText("Long ago")).toBeInTheDocument();
    expect(screen.getByText("Soon")).toBeInTheDocument();
  });

  it("says so when the period has no entry", async () => {
    const user = userEvent.setup();
    install([entry("aaaaaaaa", "Old", local(9, 1, 12, 0))]);
    await show();
    expect(screen.getByText("Nothing done in this period")).toBeInTheDocument();
    await choosePeriod(user, "All time");
    expect(screen.queryByText("Nothing done in this period")).not.toBeInTheDocument();
  });

  it("gives the row the title, the summary on at most two lines, the time and the number of tasks, and the whole summary as tooltip", async () => {
    const user = userEvent.setup();
    const summary = "A very long summary that the row has to cut but that the tooltip shows whole, to the last word.";
    install([entry("aaaaaaaa", "Checkout works", local(11, 2, 9, 30), { summary })]);
    await show();
    const row = screen.getByRole("button", { name: /Checkout works/ });
    expect(row).toHaveTextContent(summary);
    expect(within(row).getByText(summary)).toHaveClass("line-clamp-2");
    expect(within(row).getByText("09:30")).toHaveClass("font-mono");
    expect(within(row).getByText("1 task")).toBeInTheDocument();
    await user.hover(row);
    expect(await screen.findByRole("tooltip")).toHaveTextContent(summary);
  });

  it("opens an entry in a Dialog with its title, date, summary, rendered text and its Tasks from the copies, in the order cited", async () => {
    const user = userEvent.setup();
    install([
      entry("aaaaaaaa", "Checkout works", local(11, 2, 9, 30), {
        summary: "Orders can be paid",
        text: "The **checkout** now works.",
        tasks: [
          task("bbbbbbbb", "Pay with a card", { label: "Checkout", finished: instant(local(11, 2, 8, 5)), body: "Card payments **succeed**.", refs: ["order/Paid", "requirements/ui#order"] }),
          task("cccccccc", "Receipt", { finished: instant(local(11, 1, 17, 40)) }),
        ],
      }),
    ]);
    await show();
    await user.click(screen.getByRole("button", { name: /Checkout works/ }));

    const dialog = await screen.findByRole("dialog", { name: "Checkout works" });
    expect(within(dialog).getByText("Monday, 2 November 2026, 09:30")).toBeInTheDocument();
    expect(within(dialog).getByText("Orders can be paid")).toBeInTheDocument();
    expect(within(dialog).getByText("checkout", { selector: "strong" })).toBeInTheDocument();

    const rows = within(dialog).getAllByRole("listitem").filter((item) => item.querySelector("button[aria-expanded]"));
    expect(rows).toHaveLength(2);
    expect(rows[0]).toHaveTextContent("bbbbbbbb");
    expect(rows[0]).toHaveTextContent("Pay with a card");
    expect(rows[0]).toHaveTextContent("Checkout");
    expect(within(rows[0] as HTMLElement).getByText("Checkout")).toHaveClass("border");
    expect(rows[0]).toHaveTextContent("Finished 08:05");
    expect(rows[1]).toHaveTextContent("cccccccc");
    expect(rows[1]).toHaveTextContent("Receipt");
    expect(rows[1]).toHaveTextContent(/Finished 1 Nov 17:40/);

    // A row expands to the rendered body and the refs; nothing in it can be edited.
    const expander = within(rows[0] as HTMLElement).getByRole("button");
    expect(expander).toHaveAttribute("aria-expanded", "false");
    expect(within(rows[0] as HTMLElement).queryByText("Card payments", { exact: false })).not.toBeVisible();
    await user.click(expander);
    expect(expander).toHaveAttribute("aria-expanded", "true");
    expect(within(rows[0] as HTMLElement).getByText("succeed", { selector: "strong" })).toBeVisible();
    expect(within(rows[0] as HTMLElement).getByText("order/Paid")).toBeVisible();
    expect(within(dialog).queryByRole("textbox")).not.toBeInTheDocument();

    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: /Checkout works/ })).toHaveFocus();
  });

  it("asks the backend again when a project-changed event names the history slice, and not for other slices", async () => {
    install([entry("aaaaaaaa", "First", local(11, 2, 9, 30))]);
    await show();
    const asked = () => vi.mocked(call).mock.calls.filter(([name]) => name === "history_snapshot").length;
    expect(asked()).toBe(1);

    await act(async () => handlers.get("project-changed")?.({ projectId: PROJECT, revision: 2, slices: ["board"] }));
    expect(asked()).toBe(1);

    current = { projectId: PROJECT, revision: 3, entries: [entry("bbbbbbbb", "Second", local(11, 2, 10, 0)), ...current.entries] };
    await act(async () => handlers.get("project-changed")?.({ projectId: PROJECT, revision: 3, slices: ["history"] }));
    expect(await screen.findByText("Second")).toBeInTheDocument();
    expect(asked()).toBe(2);
  });
});

describe("Ask more on a History entry", () => {
  function open(askMore = vi.fn().mockResolvedValue("askaskas")) {
    install([entry("eeeeeeee", "Checkout works", local(11, 2, 9, 30), { tasks: [task("bbbbbbbb", "Pay with a card", { body: "Card payments." })] })]);
    render(
      <TooltipProvider>
        <ProjectContext.Provider value={{ projectId: PROJECT, showView: vi.fn(), openJob: vi.fn(), openedJob: null, openedJobRequest: 0 }}>
          <BoardActionsContext.Provider value={{ askMore }}>
            <HistoryView now={() => NOW} />
          </BoardActionsContext.Provider>
        </ProjectContext.Provider>
      </TooltipProvider>,
    );
    return askMore;
  }

  it("is in the footer of the entry's dialog, with its tooltip and accessible name, and sends only the entry", async () => {
    const user = userEvent.setup();
    const askMore = open();
    await screen.findByRole("combobox", { name: "Period" });
    await user.click(screen.getByRole("button", { name: /Checkout works/ }));
    const dialog = await screen.findByRole("dialog", { name: "Checkout works" });
    const button = within(dialog).getByRole("button", { name: "Ask more about eeeeeeee" });
    expect(button).toHaveTextContent("Ask more");
    expect(button).toBeEnabled();
    await user.hover(button);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Ask an agent about this History entry");
    await user.click(button);
    expect(askMore).toHaveBeenCalledWith({ kind: "history", entry: "eeeeeeee" });
    // The dialog stays as it was.
    expect(screen.getByRole("dialog", { name: "Checkout works" })).toBeInTheDocument();
  });

  it("is not on the Tasks of the entry, collapsed or expanded", async () => {
    const user = userEvent.setup();
    open();
    await screen.findByRole("combobox", { name: "Period" });
    await user.click(screen.getByRole("button", { name: /Checkout works/ }));
    const dialog = await screen.findByRole("dialog", { name: "Checkout works" });
    await user.click(within(dialog).getByRole("button", { name: /Pay with a card/ }));
    expect(within(dialog).getByText("Card payments.")).toBeVisible();
    expect(within(dialog).queryByRole("button", { name: "Ask more about bbbbbbbb" })).toBeNull();
    expect(within(dialog).getAllByRole("button", { name: /^Ask more/ })).toHaveLength(1);
  });
});
