import { render, screen, waitFor, within, act } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { AskConversation } from "@/components/ask/ask-conversation";
import { expectOneGeometry, pressWhileComposing } from "@/components/assets/tag-box-checks";

import type { AskRow } from "@/bindings/AskRow";
import type { Entry } from "@/bindings/Entry";
import type { JobRow } from "@/bindings/JobRow";
import type { QuestionRoundView } from "@/bindings/QuestionRoundView";
import { AskViewSwitch } from "@/components/ask/ask-view-switch";
import { mergeRows } from "@/components/ask/ask-model";
import { AsksProvider, useAsks } from "@/components/ask/use-asks";
import { FormDialog } from "@/components/dialogs";
import { JobsView } from "@/components/jobs/jobs-view";
import { JobsProvider } from "@/components/jobs/use-jobs";
import { ProjectContext } from "@/components/project-context";
import { QuestionRoundLayer } from "@/components/questions/question-overlay";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ViewShownContext } from "@/components/view-shown";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));
vi.mock("@/hooks/use-omp", () => ({ useOmp: () => ({ status: null }) }));

const PROJECT = "0123456789abcdef";

const ASSETS = vi.hoisted(() => [
  { name: "logo.png", size: 1 },
  { name: "spec.pdf", size: 1 },
]);
vi.mock("@/components/assets/use-assets", () => ({ useAssets: () => ({ assets: ASSETS, loaded: true, apply: () => {} }) }));

function job(id: string, state: JobRow["state"], patch: Partial<JobRow> = {}): JobRow {
  return { id, kind: "run", name: `Run ${id}`, state, reason: null, created: "2026-11-02T07:00:00.000Z", started: null, ended: null, note: null, target: null, request: null, requestSent: false, ...patch };
}

function ask(id: string, name: string, patch: Partial<AskRow> = {}): AskRow {
  return { id, projectId: PROJECT, name, kind: "task", opened: "2026-11-02T08:00:00.000Z", status: "idle", starting: false, round: null, ...patch };
}

describe("the order of the Jobs list with Ask more conversations", () => {
  it("puts working conversations with the jobs in the same state and idle ones after the suspended jobs and before the finished ones", () => {
    const rows = mergeRows(
      [job("jobwait1", "waiting"), job("jobrun01", "running", { started: "2026-11-02T07:30:00.000Z" }), job("jobsusp1", "suspended"), job("jobdone1", "done")],
      [
        ask("askidle1", "Ask T1", { opened: "2026-11-02T07:00:00.000Z" }),
        ask("askidle2", "Ask T2", { opened: "2026-11-02T09:00:00.000Z" }),
        ask("askrun01", "Ask T3", { status: "running", opened: "2026-11-02T08:00:00.000Z" }),
        ask("askwait1", "Ask T4", { status: "waiting" }),
      ],
    );
    expect(rows.map((row) => (row.type === "job" ? row.job.id : row.ask.id))).toEqual([
      "askwait1",
      "jobwait1",
      "askrun01",
      "jobrun01",
      "jobsusp1",
      "askidle2",
      "askidle1",
      "jobdone1",
    ]);
  });
});

interface Mounted {
  closed: string[];
  sent: { askId: string; text: string }[];
  emit: (event: string, payload: unknown) => void;
}

function mount(asks: AskRow[], entries: Record<string, Entry[]> = {}, jobs: JobRow[] = [], round: QuestionRoundView | null = null): Mounted {
  const handlers = new Map<string, (payload: never) => void>();
  const mounted: Mounted = {
    closed: [],
    sent: [],
    emit: (event, payload) => act(() => handlers.get(event)?.(payload as never)),
  };
  vi.mocked(on).mockImplementation((async (name: string, handler: unknown) => {
    handlers.set(name, handler as (payload: never) => void);
    return () => {};
  }) as never);
  vi.mocked(call).mockImplementation((async (name: string, args?: unknown) => {
    const a = args as { askId?: string; text?: string };
    switch (name) {
      case "jobs_snapshot":
        return { projectId: PROJECT, jobs, ompReady: true };
      case "ask_list":
        return { projectId: PROJECT, asks };
      case "ask_snapshot": {
        const row = asks.find((candidate) => candidate.id === a.askId)!;
        return { row, seq: 0, entries: entries[row.id] ?? [], arrivals: [] };
      }
      case "ask_close":
        mounted.closed.push(a.askId!);
        return null;
      case "ask_send_message":
        mounted.sent.push({ askId: a.askId!, text: a.text! });
        return null;
      case "question_round_get":
        return round;
      default:
        return null;
    }
  }) as never);
  render(
    <TooltipProvider>
      <ProjectContext.Provider value={{ projectId: PROJECT, showView: vi.fn(), openJob: vi.fn(), openedJob: null, openedJobRequest: 0 }}>
        <JobsProvider projectId={PROJECT}>
          <AsksProvider projectId={PROJECT}>
            <QuestionRoundLayer projectId={PROJECT}>
              <JobsView />
            </QuestionRoundLayer>
          </AsksProvider>
        </JobsProvider>
      </ProjectContext.Provider>
    </TooltipProvider>,
  );
  return mounted;
}

describe("the rows of Ask more conversations", () => {
  beforeEach(() => vi.clearAllMocks());

  it("show the name in semibold, the kind `ask`, the badge, the opening time and no running time or note", async () => {
    mount([ask("askidle1", "Ask T3"), ask("askrun01", "Ask P2", { status: "running" }), ask("askwait1", "Ask D4", { status: "waiting" })]);
    const list = await screen.findByRole("navigation", { name: "Jobs" });
    const rows = await within(list).findAllByRole("listitem");
    const byName = (name: string) => rows.find((row) => row.textContent?.includes(name))!;
    expect(within(byName("Ask T3")).getByText("Ask T3")).toHaveClass("font-semibold");
    expect(within(byName("Ask T3")).getByText("ask")).toHaveClass("text-muted-foreground");
    expect(within(byName("Ask T3")).getByText("idle")).toBeInTheDocument();
    expect(within(byName("Ask T3")).queryByRole("img", { hidden: true })).toBeNull();
    expect(within(byName("Ask P2")).getByText("running")).toBeInTheDocument();
    expect(within(byName("Ask D4")).getByText("waiting")).toBeInTheDocument();
    expect(byName("Ask T3").textContent).not.toMatch(/\d+s\b/);
  });

  it("have an X in every state: it closes an idle conversation at once", async () => {
    const user = userEvent.setup();
    const mounted = mount([ask("askidle1", "Ask T3")]);
    const close = await screen.findByRole("button", { name: "Close Ask T3" });
    await user.hover(close);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Close");
    await user.click(close);
    expect(mounted.closed).toEqual(["askidle1"]);
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("confirm closing during a turn, with the focus on Cancel, and Cancel closes nothing", async () => {
    const user = userEvent.setup();
    const mounted = mount([ask("askrun01", "Ask T3", { status: "running" })]);
    await user.click(await screen.findByRole("button", { name: "Close Ask T3" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Close Ask T3?" });
    expect(dialog).toHaveTextContent("The agent's turn stops and the conversation ends.");
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(mounted.closed).toEqual([]);
    await user.click(screen.getByRole("button", { name: "Close Ask T3" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Close" }));
    expect(mounted.closed).toEqual(["askrun01"]);
  });

  it("follow the events of the backend: a row opens, changes and leaves", async () => {
    const mounted = mount([]);
    await screen.findByText("No jobs yet");
    mounted.emit("ask-changed", ask("askidle1", "Ask T3"));
    const list = await screen.findByRole("navigation", { name: "Jobs" });
    expect(await within(list).findByText("idle")).toBeInTheDocument();
    mounted.emit("ask-changed", ask("askidle1", "Ask T3", { status: "running" }));
    expect(await within(list).findByText("running")).toBeInTheDocument();
    mounted.emit("ask-closed", { projectId: PROJECT, id: "askidle1" });
    expect(await screen.findByText("No jobs yet")).toBeInTheDocument();
  });

  it("count in `All` and `Active` by what the agent does, and an idle one shows only under `All`", async () => {
    const user = userEvent.setup();
    mount(
      [ask("askidle1", "Ask T3"), ask("askrun01", "Ask P2", { status: "running" }), ask("askwait1", "Ask D4", { status: "waiting" })],
      {},
      [job("jobrun01", "running"), job("jobdone1", "done")],
    );
    const list = await screen.findByRole("navigation", { name: "Jobs" });
    const shown = () => within(list).queryAllByRole("listitem").map((row) => row.querySelector("span.font-semibold")?.textContent);
    expect(await screen.findByRole("radio", { name: "All 5" })).toBeChecked();
    expect(screen.getByRole("radio", { name: "Active 3" })).toBeInTheDocument();
    expect(screen.getByRole("radio", { name: "Ended 1" })).toBeInTheDocument();
    expect(shown()).toContain("Ask T3");
    await user.click(screen.getByRole("radio", { name: "Active 3" }));
    expect(shown().sort()).toEqual(["Ask D4", "Ask P2", "Run jobrun01"]);
    await user.click(screen.getByRole("radio", { name: "Ended 1" }));
    expect(shown()).toEqual(["Run jobdone1"]);
  });
});

describe("the conversation of an Ask more conversation", () => {
  beforeEach(() => vi.clearAllMocks());

  it.each([
    ["proposal", "Ask about the Proposal…"],
    ["task", "Ask about the Task…"],
    ["drift", "Ask about the Drift…"],
    ["history", "Ask about the History entry…"],
    ["requirements", "Ask about the Requirements…"],
    ["question", "Ask about the question…"],
  ] as const)("is empty with only the message box and the placeholder of a %s", async (kind, placeholder) => {
    mount([ask("askidle1", "Ask T3", { kind })]);
    const box = await screen.findByRole("combobox", { name: "Message the agent" });
    expect(box).toHaveAttribute("placeholder", placeholder);
    expect(screen.queryByRole("button", { name: "Stop the agent" })).toBeNull();
    expect(screen.queryByText(/Weaver/)).toBeNull();
  });

  it("sends the message trimmed, and after the first message the placeholder is the one of a job", async () => {
    const user = userEvent.setup();
    const mounted = mount([ask("askidle1", "Ask T3")], {
      askidle1: [{ kind: "developer", message: 1, text: "First", delivery: { status: "delivered" } }],
    });
    const box = await screen.findByRole("combobox", { name: "Message the agent" });
    await waitFor(() => expect(box).toHaveAttribute("placeholder", "Message the agent…"));
    await user.type(box, "  Why?  {Enter}");
    expect(mounted.sent).toEqual([{ askId: "askidle1", text: "Why?" }]);
  });

  it("says `Starting Weaver…` under a message sent while the agent starts, and shows `Stop the agent` during a turn without a dialog", async () => {
    const user = userEvent.setup();
    mount([ask("askrun01", "Ask T3", { status: "running", starting: true })], {
      askrun01: [{ kind: "developer", message: 1, text: "Hello", delivery: { status: "queued" } }],
    });
    expect(await screen.findByText("Starting Weaver…")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Stop the agent" }));
    expect(vi.mocked(call)).toHaveBeenCalledWith("ask_stop", { askId: "askrun01" });
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("shows `Back to questions` only while the round it was opened from is open, and reopens that round", async () => {
    const user = userEvent.setup();
    const round = {
      id: 7,
      projectId: PROJECT,
      origin: { kind: "agent", jobName: "Scan" },
      questions: [],
      position: { kind: "question", index: 0 },
      minimized: true,
      waiting: 0,
    } as unknown as QuestionRoundView;
    mount([ask("askidle1", "Ask question", { kind: "question", round: 7 })], {}, [], round);
    const back = await screen.findByRole("button", { name: "Back to questions" });
    await user.click(back);
    expect(vi.mocked(call)).toHaveBeenCalledWith("question_round_act", { projectId: PROJECT, roundId: 7, action: { kind: "minimize", minimized: false } });
  });

  it("has no `Back to questions` when the round is not the one that is open", async () => {
    mount([ask("askidle1", "Ask question", { kind: "question", round: 7 })], {}, [], null);
    await screen.findByRole("combobox", { name: "Message the agent" });
    expect(screen.queryByRole("button", { name: "Back to questions" })).toBeNull();
  });
});

describe("the header of an Ask more conversation", () => {
  beforeEach(() => vi.clearAllMocks());

  it.each([
    ["idle", "idle"],
    ["running", "running"],
    ["waiting", "waiting"],
  ] as const)("is in the header band, with the name and the %s badge, and no started line or buttons", async (status, badge) => {
    mount([ask("askrun01", "Ask T3", { status })]);
    const heading = await screen.findByRole("heading", { level: 2, name: "Ask T3" });
    const band = screen.getByRole("heading", { level: 1, name: "Jobs" }).closest("header")!;
    expect(band).toContainElement(heading);
    expect(heading).toHaveClass("text-[17px]", "font-semibold");
    expect(heading.parentElement).toHaveTextContent(badge);
    expect(within(heading.parentElement!).queryByRole("button")).toBeNull();
    expect(band).not.toHaveTextContent(/Started/);
    expect(within(screen.getByRole("region", { name: "Conversation" })).queryByRole("heading")).toBeNull();
  });

  it("shows `Back to questions` at the top of the conversation, in its column", async () => {
    const round = {
      id: 7,
      projectId: PROJECT,
      origin: { kind: "agent", jobName: "Scan" },
      questions: [],
      position: { kind: "question", index: 0 },
      minimized: true,
      waiting: 0,
    } as unknown as QuestionRoundView;
    mount([ask("askidle1", "Ask question", { kind: "question", round: 7 })], {}, [], round);
    const back = await screen.findByRole("button", { name: "Back to questions" });
    const box = screen.getByRole("combobox", { name: "Message the agent" });
    expect(back.parentElement).toHaveClass("max-w-[860px]");
    expect(back.compareDocumentPosition(box) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("renders the first message of the developer as in Chat, in a column of at most 860 px", async () => {
    mount([ask("askidle1", "Ask T3")], {
      askidle1: [{ kind: "developer", message: 1, text: "First", delivery: { status: "delivered" } }],
    });
    const bubble = await screen.findByText("First");
    expect(bubble).toHaveClass("max-w-[560px]", "bg-paper");
    expect(bubble.closest(".max-w-\\[860px\\]")).toHaveClass("mx-auto");
    expect(screen.getByRole("combobox", { name: "Message the agent" }).closest(".rounded-2xl")).toHaveClass("max-w-[860px]");
  });
});

describe("opening a conversation", () => {
  beforeEach(() => vi.clearAllMocks());

  function Opener() {
    const { askMore } = useAsks();
    return <button onClick={() => void askMore({ kind: "task", id: "aaaaaaaa" })}>open</button>;
  }

  function setup(open: () => string) {
    vi.mocked(on).mockResolvedValue(() => {});
    vi.mocked(call).mockImplementation((async (name: string) => {
      if (name === "ask_list") return { projectId: PROJECT, asks: [] };
      if (name === "ask_open") return open();
      return null;
    }) as never);
    const showView = vi.fn();
    render(
      <ProjectContext.Provider value={{ projectId: PROJECT, showView, openJob: vi.fn(), openedJob: null, openedJobRequest: 0 }}>
        <AsksProvider projectId={PROJECT}>
          <AskViewSwitch />
          <Opener />
        </AsksProvider>
      </ProjectContext.Provider>,
    );
    return showView;
  }

  it("sends the kind and the identity of the thing and shows the Jobs view", async () => {
    const user = userEvent.setup();
    const showView = setup(() => "askaskas");
    await user.click(screen.getByRole("button", { name: "open" }));
    expect(vi.mocked(call)).toHaveBeenCalledWith("ask_open", { projectId: PROJECT, target: { kind: "task", id: "aaaaaaaa" } });
    await waitFor(() => expect(showView).toHaveBeenCalledWith("jobs"));
  });

  it("opens a new conversation at every press, also on the same thing", async () => {
    const user = userEvent.setup();
    let count = 0;
    const showView = setup(() => `ask${(count += 1)}`);
    await user.click(screen.getByRole("button", { name: "open" }));
    await user.click(screen.getByRole("button", { name: "open" }));
    await waitFor(() => expect(showView).toHaveBeenCalledTimes(2));
  });

  it("changes nothing when the thing no longer exists", async () => {
    const user = userEvent.setup();
    const showView = setup(() => {
      throw { code: "refused", message: "gone" };
    });
    await user.click(screen.getByRole("button", { name: "open" }));
    await waitFor(() => expect(vi.mocked(call)).toHaveBeenCalledWith("ask_open", expect.anything()));
    expect(showView).not.toHaveBeenCalled();
  });
});

describe("a dialog of a view the window does not show", () => {
  const dialog = (
    <FormDialog open onOpenChange={() => {}} title="Details">
      <p>Body</p>
    </FormDialog>
  );

  it("hides with its view and shows again when the view does", () => {
    const { rerender } = render(<ViewShownContext.Provider value={false}>{dialog}</ViewShownContext.Provider>);
    expect(screen.queryByRole("dialog")).toBeNull();
    rerender(<ViewShownContext.Provider value>{dialog}</ViewShownContext.Provider>);
    expect(screen.getByRole("dialog", { name: "Details" })).toBeInTheDocument();
  });
});

describe("the focus of a conversation just opened", () => {
  it("goes to its message box", async () => {
    vi.mocked(on).mockResolvedValue(() => {});
    vi.mocked(call).mockImplementation((async (name: string) => {
      if (name === "ask_snapshot") return { row: ask("askidle1", "Ask T3"), seq: 0, entries: [], arrivals: [] };
      return null;
    }) as never);
    render(
      <TooltipProvider>
        <AskConversation ask={ask("askidle1", "Ask T3")} focus={1} />
      </TooltipProvider>,
    );
    await waitFor(() => expect(screen.getByRole("combobox", { name: "Message the agent" })).toHaveFocus());
  });
});

describe("the @ tags in the box of an Ask more conversation", () => {
  it("opens the list of assets, inserts the chosen one, does not send with the list open, and colours the recognised tags", async () => {
    const user = userEvent.setup();
    const mounted = mount([ask("askidle1", "Ask T3")]);
    const box = await screen.findByRole("combobox", { name: "Message the agent" });
    await user.type(box, "See @sp");
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["spec.pdf"]);
    await user.keyboard("{Tab}");
    expect(mounted.sent).toEqual([]);
    expect(box).toHaveValue("See @spec.pdf ");
    expect(document.querySelector("[data-tag='spec.pdf']")?.textContent).toBe("@spec.pdf");
    await user.keyboard("{Enter}");
    expect(mounted.sent).toEqual([{ askId: "askidle1", text: "See @spec.pdf" }]);
  });

  it("gives the textarea and the mirror the same metrics", async () => {
    mount([ask("askidle1", "Ask T3")]);
    expectOneGeometry(await screen.findByRole("combobox", { name: "Message the agent" }));
  });

  it("leaves Enter and Tab of a composition to the input method: nothing is sent, picked or moved", async () => {
    const user = userEvent.setup();
    const mounted = mount([ask("askidle1", "Ask T3")]);
    const box = await screen.findByRole("combobox", { name: "Message the agent" });
    await user.type(box, "Why");
    pressWhileComposing(box, "Enter");
    expect(mounted.sent).toEqual([]);
    await user.type(box, " @sp");
    pressWhileComposing(box, "Enter");
    pressWhileComposing(box, "Tab");
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["spec.pdf"]);
    expect(box).toHaveValue("Why @sp");
    expect(box).toHaveFocus();
    expect(mounted.sent).toEqual([]);
  });
});
