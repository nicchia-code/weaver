import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AgentNode } from "@/bindings/AgentNode";
import type { ConversationSnapshot } from "@/bindings/ConversationSnapshot";
import type { Entry } from "@/bindings/Entry";
import type { JobRow } from "@/bindings/JobRow";
import type { Step } from "@/bindings/Step";
import type { SubAgentConversation } from "@/bindings/SubAgentConversation";
import { expectOneGeometry, pressWhileComposing } from "@/components/assets/tag-box-checks";
import { clockText } from "@/components/general/chat-model";
import { JobsView } from "@/components/jobs/jobs-view";
import { startedLine, startedText } from "@/components/jobs/format";
import { JobsProvider } from "@/components/jobs/use-jobs";
import { ProjectContext } from "@/components/project-context";
import { RequestsProvider, useRequests } from "@/components/requests/use-requests";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";
import type { ViewId } from "@/views";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const PROJECT = "0123456789abcdef";

const ASSETS = vi.hoisted(() => [
  { name: "logo.png", size: 1 },
  { name: "spec.pdf", size: 1 },
]);
vi.mock("@/components/assets/use-assets", () => ({ useAssets: () => ({ assets: ASSETS, loaded: true, apply: () => {} }) }));

function job(id: string, state: JobRow["state"], patch: Partial<JobRow> = {}): JobRow {
  return { id, kind: "run", name: `Run ${id}`, state, reason: null, created: "2026-11-02T07:00:00.000Z", started: null, ended: null, note: null, target: null, request: null, requestSent: false, ...patch };
}

/** What a test may add around the view: the window's `showView`, and the requests of the window with a probe of the editor's state. */
interface Around {
  showView?: (view: ViewId) => void;
  requests?: boolean;
  /** The job a view asked the window to open. */
  openedJob?: string;
}

function EditorProbe() {
  const { editing, selected, origin } = useRequests();
  return <output aria-label="Editor">{editing ? `${selected} from ${origin.kind === "jobs" ? `jobs/${origin.jobId}` : "board"}` : "closed"}</output>;
}

/** What a test hands the backend as the conversation of a job; the instants the entries arrived are optional. */
type Snapshot = Omit<ConversationSnapshot, "arrivals"> & { arrivals?: number[] };

function mount(
  jobs: JobRow[],
  conversation: Snapshot,
  handlers: Record<string, (args: never) => unknown> = {},
  around: Around = {},
) {
  vi.mocked(on).mockResolvedValue(() => {});
  vi.mocked(call).mockImplementation(async (name: string, args?: unknown) => {
    if (name === "jobs_snapshot") return { projectId: PROJECT, jobs, ompReady: true };
    if (name === "job_conversation") return { arrivals: [], ...conversation };
    return (handlers[name]?.(args as never) ?? null) as null;
  });
  const view = (
    <JobsProvider projectId={PROJECT}>
      <JobsView />
    </JobsProvider>
  );
  render(
    <TooltipProvider>
      <ProjectContext.Provider value={{ projectId: PROJECT, showView: around.showView ?? vi.fn(), openJob: vi.fn(), openedJob: around.openedJob ?? null, openedJobRequest: around.openedJob ? 1 : 0 }}>
        {around.requests ? (
          <RequestsProvider projectId={PROJECT}>
            {view}
            <EditorProbe />
          </RequestsProvider>
        ) : (
          view
        )}
      </ProjectContext.Provider>
    </TooltipProvider>,
  );
}

describe("JobsView", () => {
  beforeEach(() => vi.clearAllMocks());

  it("says there are no jobs", async () => {
    mount([], { jobId: "x", seq: 0, entries: [], notes: [] });
    expect(await screen.findByText("No jobs yet")).toBeInTheDocument();
  });

  it("starts with one header band across both panels: the title and `Hide jobs list` over the list, the header of the selection over the conversation", async () => {
    mount([job("aaaaaaaa", "running")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] });
    const list = await screen.findByRole("navigation", { name: "Jobs" });
    const header = screen.getByRole("heading", { level: 1, name: "Jobs" }).closest("header")!;
    expect(header).toHaveClass("h-[76px]", "border-b");
    expect(within(header).getByRole("button", { name: "Hide jobs list" })).toBeInTheDocument();
    expect(within(header).getByRole("heading", { level: 2, name: "Run aaaaaaaa" })).toBeInTheDocument();
    expect(header.compareDocumentPosition(list) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(header.parentElement).toContainElement(screen.getByRole("region", { name: "Conversation" }));
    expect(header.contains(list)).toBe(false);
    // The list's rule goes on through the band, which has no other header inside the conversation.
    expect(screen.getByRole("heading", { level: 1, name: "Jobs" }).parentElement).toHaveClass("border-r", "border-line-soft");
    expect(within(screen.getByRole("region", { name: "Conversation" })).queryByRole("banner")).toBeNull();
  });

  it("puts 12 px of space above the first row inside the scrolling list, and none below the last", async () => {
    mount([job("aaaaaaaa", "running"), job("bbbbbbbb", "done")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] });
    const list = await screen.findByRole("navigation", { name: "Jobs" });
    const rows = within(list).getByRole("list");
    expect(list).toHaveClass("overflow-y-auto");
    expect(list.className).not.toMatch(/(^|\s)p[tby]?-/);
    expect(rows).toHaveClass("pt-3");
    expect(rows.className).not.toMatch(/(^|\s)(pb|py|p|my|mb)-/);
  });

  it("keeps the empty list text without the extra space", async () => {
    mount([], { jobId: "x", seq: 0, entries: [], notes: [] });
    const list = await screen.findByRole("navigation", { name: "Jobs" });
    expect(within(list).queryByRole("list")).toBeNull();
    expect(within(list).getByText("No jobs yet")).toHaveClass("p-4");
  });

  it("lists the jobs with their state badge and latest note, and shows the conversation of the first", async () => {
    mount([job("aaaaaaaa", "running", { note: "Reading the Model" }), job("bbbbbbbb", "done")], {
      jobId: "aaaaaaaa",
      seq: 1,
      notes: [],
      entries: [
        { kind: "instruction", text: "Use the weaver-scan skill on the whole System." },
        { kind: "agent", text: "Reading **code**", complete: false },
        { kind: "end", outcome: "failed", reason: "the agent exited with code 3", details: "boom" },
      ],
    });
    const list = await screen.findByRole("navigation", { name: "Jobs" });
    expect(within(list).getByText("running")).toBeInTheDocument();
    expect(within(list).getByText("Reading the Model")).toBeInTheDocument();
    expect(within(list).getByRole("button", { name: "Remove Run bbbbbbbb" })).toBeInTheDocument();
    expect(within(list).queryByRole("button", { name: "Remove Run aaaaaaaa" })).toBeNull();
    expect(await screen.findByText("Use the weaver-scan skill on the whole System.")).toBeInTheDocument();
    expect(await screen.findByText("code")).toBeInTheDocument();
    expect(screen.getByText(/Failed: the agent exited with code 3\./)).toBeInTheDocument();
  });

  it("puts `Stop job` in the job header, not in the message box, and asks before stopping", async () => {
    mount([job("aaaaaaaa", "running")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] });
    const user = userEvent.setup();
    const conversation = await screen.findByRole("region", { name: "Conversation" });
    const header = screen.getByRole("heading", { level: 2, name: "Run aaaaaaaa" }).closest("header")!;
    const stop = within(header).getByRole("button", { name: "Stop job" });
    const box = within(conversation).getByRole("combobox", { name: "Message the agent" });
    expect(header.compareDocumentPosition(box) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(conversation).queryByRole("button", { name: /^Stop/ })).toBeNull();
    expect(within(conversation).queryByRole("button", { name: "Stop the agent" })).toBeNull();
    await user.click(stop);
    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText("Stop Run aaaaaaaa?")).toBeInTheDocument();
    expect(within(dialog).getByText("The agent stops at once and every process it started ends.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(call).not.toHaveBeenCalledWith("job_stop", expect.anything());
    await user.click(screen.getByRole("button", { name: "Stop job" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Stop" }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("job_stop", { projectId: PROJECT, jobId: "aaaaaaaa" }));
  });

  it.each(["queued", "running", "waiting", "suspended"] as const)("offers `Stop job` while the job is %s", async (state) => {
    mount([job("aaaaaaaa", state)], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] });
    expect(await screen.findByRole("button", { name: "Stop job" })).toBeInTheDocument();
  });

  it.each(["done", "failed", "stopped"] as const)("has no `Stop job` once the job is %s", async (state) => {
    mount([job("aaaaaaaa", state)], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] });
    await screen.findByRole("heading", { level: 2, name: "Run aaaaaaaa" });
    expect(screen.queryByRole("button", { name: "Stop job" })).toBeNull();
  });

  describe("View request", () => {
    const design = (patch: Partial<JobRow> = {}) => job("aaaaaaaa", "running", { kind: "design", name: "Design", target: "2026-11-02T07-00-00", request: "2026-11-02T07-00-00", requestSent: true, ...patch });
    const empty = { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] };

    it("comes before `Stop job` in the header of a design job whose request is still sent to it, and opens that request from the Jobs origin on the Board", async () => {
      const showView = vi.fn();
      mount([design()], empty, {}, { showView, requests: true });
      const user = userEvent.setup();
      await screen.findByRole("region", { name: "Conversation" });
      const header = screen.getByRole("heading", { level: 2, name: "Design" }).closest("header")!;
      const view = within(header).getByRole("button", { name: "View request" });
      const stop = within(header).getByRole("button", { name: "Stop job" });
      expect(view.compareDocumentPosition(stop) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
      expect(screen.getByLabelText("Editor")).toHaveTextContent("closed");
      await user.click(view);
      expect(screen.getByLabelText("Editor")).toHaveTextContent("2026-11-02T07-00-00 from jobs/aaaaaaaa");
      expect(showView).toHaveBeenCalledWith("board");
    });

    it("shows on a setup job too, also one that Chat started", async () => {
      mount([design({ kind: "setup", name: "Setup" })], empty, {}, { requests: true });
      expect(await screen.findByRole("button", { name: "View request" })).toBeInTheDocument();
    });

    it("is gone when the request is not sent to this job any more (consumed, discarded, deleted, back in writing or sent to another job)", async () => {
      mount([design({ requestSent: false })], empty, {}, { requests: true });
      await screen.findByRole("heading", { level: 2, name: "Design" });
      expect(screen.queryByRole("button", { name: "View request" })).toBeNull();
    });

    it.each(["scan", "run", "import", "resolve", "rework", "refinement", "merge"] as const)("never shows on a %s job", async (kind) => {
      mount([job("aaaaaaaa", "running", { kind, name: "Job" })], empty, {}, { requests: true });
      await screen.findByRole("heading", { level: 2, name: "Job" });
      expect(screen.queryByRole("button", { name: "View request" })).toBeNull();
    });

    it("also shows once the job has ended, while the request is still sent to it", async () => {
      mount([design({ state: "failed" })], empty, {}, { requests: true });
      expect(await screen.findByRole("button", { name: "View request" })).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Stop job" })).toBeNull();
    });
  });

  it("heads the conversation with the name, the state pill with the working time and when the job started", async () => {
    const started = new Date(Date.now() - 136_000).toISOString();
    mount([job("aaaaaaaa", "running", { started })], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] });
    await screen.findByRole("region", { name: "Conversation" });
    const header = screen.getByRole("heading", { level: 2, name: "Run aaaaaaaa" }).closest("header")!;
    expect(header).toHaveTextContent(/Running · 2m \d\ds/);
    expect(header).toHaveTextContent(startedLine("2026-11-02T07:00:00.000Z"));
    expect(header).not.toHaveTextContent(/%|files? changed|checked|batch/);
  });

  it("shows no working time in the pill of a job that ended", async () => {
    mount([job("aaaaaaaa", "done", { started: "2026-11-02T07:01:00.000Z", ended: "2026-11-02T07:05:00.000Z" })], {
      jobId: "aaaaaaaa",
      seq: 0,
      entries: [],
      notes: [],
    });
    await screen.findByRole("region", { name: "Conversation" });
    const header = screen.getByRole("heading", { level: 2, name: "Run aaaaaaaa" }).closest("header")!;
    expect(header).toHaveTextContent("Done");
    expect(header).not.toHaveTextContent("·");
  });

  it("sends a message with Enter and keeps Shift+Enter for a new line", async () => {
    mount([job("aaaaaaaa", "running")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] });
    const user = userEvent.setup();
    const box = await screen.findByPlaceholderText("Steer the agent while it runs…");
    await user.type(box, "Use the old parser{Shift>}{Enter}{/Shift}now{Enter}");
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("job_send_message", { projectId: PROJECT, jobId: "aaaaaaaa", text: "Use the old parser\nnow" }),
    );
  });

  it("offers the options of the agent on a permission card headed `Weaver asks permission` and sends the choice", async () => {
    mount([job("aaaaaaaa", "waiting")], {
      jobId: "aaaaaaaa",
      seq: 1,
      notes: [],
      entries: [
        {
          kind: "permission",
          request: 4,
          tool: null,
          state: { status: "asking" },
          options: [
            { id: "allow", name: "Allow once", rejects: false },
            { id: "no", name: "Reject", rejects: true },
          ],
        },
      ],
    });
    const user = userEvent.setup();
    expect(await screen.findByRole("group", { name: "Weaver asks permission" })).toHaveTextContent("Weaver asks permission");
    await user.click(await screen.findByRole("button", { name: "Allow once" }));
    expect(call).toHaveBeenCalledWith("job_answer_permission", { projectId: PROJECT, jobId: "aaaaaaaa", request: 4, option: "allow" });
  });

  it("makes a finished conversation read-only and points to Chat", async () => {
    mount([job("aaaaaaaa", "done")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] });
    expect(await screen.findByText("This job has ended. Continue in Chat.")).toBeInTheDocument();
    expect(screen.queryByPlaceholderText("Steer the agent while it runs…")).toBeNull();
    expect(screen.getByRole("button", { name: "Open Chat" })).toBeInTheDocument();
  });

  it("shows the instruction in a closed `Instructions from Weaver` box and developer messages in bubbles on Paper", async () => {
    mount([job("aaaaaaaa", "running")], {
      jobId: "aaaaaaaa",
      seq: 1,
      notes: [],
      entries: [
        { kind: "instruction", text: "Use the weaver-scan skill on the whole System." },
        { kind: "developer", message: 1, text: "Mind the tests", delivery: { status: "delivered" } },
      ],
    });
    const summary = await screen.findByText("Instructions from Weaver");
    const details = summary.closest("details")!;
    expect(details).not.toHaveAttribute("open");
    expect(details).toHaveClass("bg-lane", "rounded-xl");
    expect(details).toHaveTextContent("Use the weaver-scan skill on the whole System.");
    const bubble = screen.getByText("Mind the tests");
    expect(bubble).toHaveClass("bg-paper", "rounded-2xl", "rounded-br-[4px]", "max-w-[560px]");
    expect(bubble.parentElement).toHaveClass("justify-end");
  });

  it("puts the steering box in a ring with a 16 px radius, with a primary icon button to send", async () => {
    mount([job("aaaaaaaa", "running")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] });
    const box = await screen.findByRole("combobox", { name: "Message the agent" });
    const frame = box.closest(".rounded-2xl")!;
    expect(frame).toHaveClass("bg-surface", "max-w-[860px]");
    const send = within(frame as HTMLElement).getByRole("button", { name: "Send" });
    expect(send).toHaveClass("size-[34px]");
    expect(send).toHaveAttribute("aria-disabled", "true");
    expect(within(frame as HTMLElement).queryByRole("combobox", { name: "Model" })).toBeNull();
  });

  it("gives the textarea and the mirror of the reply box the same metrics", async () => {
    mount([job("aaaaaaaa", "running")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] });
    expectOneGeometry(await screen.findByRole("combobox", { name: "Message the agent" }));
  });

  it("leaves Enter and Tab of a composition in the reply box to the input method: nothing is sent, picked or moved", async () => {
    const user = userEvent.setup();
    mount([job("aaaaaaaa", "running")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] });
    const box = await screen.findByRole("combobox", { name: "Message the agent" });
    await user.type(box, "Mind");
    pressWhileComposing(box, "Enter");
    expect(call).not.toHaveBeenCalledWith("job_send_message", expect.anything());
    await user.type(box, " @sp");
    pressWhileComposing(box, "Enter");
    pressWhileComposing(box, "Tab");
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["spec.pdf"]);
    expect(box).toHaveValue("Mind @sp");
    expect(box).toHaveFocus();
    expect(call).not.toHaveBeenCalledWith("job_send_message", expect.anything());
  });

  describe("rows", () => {
    it("show the name, the kind in Muted, the pill, the note on two lines with its tooltip, and the started and working times", async () => {
      const started = new Date(Date.now() - 65_000).toISOString();
      mount([job("aaaaaaaa", "running", { started, note: "Reading the Model and the Requirements" }), job("bbbbbbbb", "done")], {
        jobId: "aaaaaaaa",
        seq: 0,
        entries: [],
        notes: [],
      });
      const list = await screen.findByRole("navigation", { name: "Jobs" });
      const row = within(list).getByRole("button", { name: /^Run aaaaaaaa/ });
      expect(within(row).getByText("Run aaaaaaaa")).toHaveClass("font-semibold");
      expect(within(row).getByText("run")).toHaveClass("text-muted-foreground");
      const note = within(row).getByText("Reading the Model and the Requirements");
      expect(note).toHaveClass("line-clamp-2");
      expect(note).toHaveAttribute("title", "Reading the Model and the Requirements");
      expect(row).toHaveTextContent(`${startedText("2026-11-02T07:00:00.000Z")} · 1m 0`);
      expect(row.closest("li")).toHaveClass("bg-surface", "rounded-xl");
      expect(within(list).getByRole("button", { name: /^Run bbbbbbbb/ }).closest("li")).not.toHaveClass("bg-surface");
    });

    it("tell each state with a pill that always has its text and an icon", async () => {
      const states = ["queued", "running", "waiting", "suspended", "done", "failed", "stopped"] as const;
      mount(
        states.map((state, index) => job(`job${index}000`, state)),
        { jobId: "job1000", seq: 0, entries: [], notes: [] },
      );
      const list = await screen.findByRole("navigation", { name: "Jobs" });
      const tint: Record<(typeof states)[number], string> = {
        queued: "bg-sky-tint",
        running: "bg-sky-tint",
        waiting: "bg-peach-tint",
        suspended: "bg-muted",
        done: "bg-sage-tint",
        failed: "bg-rose-tint",
        stopped: "bg-rose-tint",
      };
      for (const state of states) {
        const pill = within(list).getByText(state);
        expect(pill).toHaveClass("rounded-full", tint[state]);
        expect(pill.querySelector("svg, [aria-hidden]")).not.toBeNull();
      }
    });
  });

  describe("the filter", () => {
    const jobs = () => [job("aaaaaaaa", "running"), job("bbbbbbbb", "queued"), job("cccccccc", "done"), job("dddddddd", "failed"), job("eeeeeeee", "stopped")];
    const snapshot = { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] };
    const names = (list: HTMLElement) =>
      within(list)
        .queryAllByRole("button", { name: /^Run / })
        .map((button) => button.querySelector("span.font-semibold")?.textContent);

    it("starts on `All` with the counts of the three parts, and lists every job", async () => {
      mount(jobs(), snapshot);
      const list = await screen.findByRole("navigation", { name: "Jobs" });
      expect(screen.getByRole("radio", { name: "All 5" })).toBeChecked();
      expect(screen.getByRole("radio", { name: "Active 2" })).not.toBeChecked();
      expect(screen.getByRole("radio", { name: "Ended 3" })).not.toBeChecked();
      expect(names(list)).toEqual(["Run aaaaaaaa", "Run bbbbbbbb", "Run cccccccc", "Run dddddddd", "Run eeeeeeee"]);
      const filter = screen.getByRole("radiogroup", { name: "Filter" });
      expect(filter).not.toContainElement(list);
      expect(filter.compareDocumentPosition(list) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    });

    it("shows only the queued, running, waiting and suspended jobs under `Active`, and the finished ones under `Ended`", async () => {
      mount([...jobs(), job("ffffffff", "waiting"), job("gggggggg", "suspended")], snapshot);
      const user = userEvent.setup();
      const list = await screen.findByRole("navigation", { name: "Jobs" });
      await user.click(screen.getByRole("radio", { name: "Active 4" }));
      expect(screen.getByRole("radio", { name: "Active 4" })).toBeChecked();
      expect(names(list).sort()).toEqual(["Run aaaaaaaa", "Run bbbbbbbb", "Run ffffffff", "Run gggggggg"]);
      await user.click(screen.getByRole("radio", { name: "Ended 3" }));
      expect(names(list).sort()).toEqual(["Run cccccccc", "Run dddddddd", "Run eeeeeeee"]);
      await user.click(screen.getByRole("radio", { name: "All 7" }));
      expect(names(list)).toHaveLength(7);
    });

    it("says `No jobs here` in Muted for a filter that leaves nothing, and the selected job stays selected", async () => {
      mount([job("aaaaaaaa", "running")], snapshot);
      const user = userEvent.setup();
      const list = await screen.findByRole("navigation", { name: "Jobs" });
      await user.click(screen.getByRole("radio", { name: "Ended 0" }));
      expect(within(list).getByText("No jobs here")).toHaveClass("text-muted-foreground");
      expect(within(list).queryByText("No jobs yet")).toBeNull();
      expect(within(list).queryByRole("button", { name: /^Run aaaaaaaa/ })).toBeNull();
      expect(screen.getByRole("heading", { level: 2, name: "Run aaaaaaaa" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "Stop job" })).toBeInTheDocument();
    });
  });

  describe("sub-agents", () => {
    const nodes: AgentNode[] = [
      {
        call: "t1",
        id: "0-Lead",
        name: "Lead",
        state: "working",
        started: "2026-11-02T07:05:00.000Z",
        children: [{ call: "c9", id: "1-Child", name: "A very long name that is truncated in the list", state: "succeeded", started: null, children: [] }],
      },
      { call: "t1", id: "2-Twin", name: "Twin", state: "failed", started: "2026-11-02T07:06:00.000Z", children: [] },
      { call: "t1", id: "3-Twin", name: "Twin", state: "interrupted", started: "2026-11-02T07:07:00.000Z", children: [] },
    ];
    const taskRow: Entry = {
      kind: "tool",
      id: "t1",
      title: "Task",
      icon: "sub-agent",
      command: null,
      status: "done",
      subAgents: [{ id: "0-Lead", name: "Lead", state: "working" }],
    };
    const reply = (path: Step[], patch: Partial<SubAgentConversation> = {}): SubAgentConversation => ({
      projectId: PROJECT,
      owner: { kind: "job", id: "aaaaaaaa" },
      path,
      seq: 0,
      entries: [{ kind: "assignment", from: "Run aaaaaaaa", text: "Read the code." }, { kind: "agent", text: "On it.", complete: true }],
      unavailable: null,
      ...patch,
    });
    const handlers = (conversation: (path: Step[]) => SubAgentConversation) => ({
      job_sub_agents: () => ({ projectId: PROJECT, owner: { kind: "job", id: "aaaaaaaa" }, nodes }),
      job_sub_agent_conversation: ({ path }: { path: Step[] }) => conversation(path),
    });

    it("lists them under the selected job only, nested, with the start time only between siblings of the same name", async () => {
      mount([job("aaaaaaaa", "running"), job("bbbbbbbb", "running")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] }, handlers((path) => reply(path)));
      const list = await screen.findByRole("list", { name: "Sub-agents" });
      expect(screen.getAllByRole("list", { name: "Sub-agents" })).toHaveLength(1);
      const lead = within(list).getByRole("button", { name: /^Lead/ });
      expect(lead).toHaveTextContent("working");
      expect(lead).not.toHaveTextContent("07:05");
      const twins = within(list).getAllByRole("button", { name: /^Twin/ });
      expect(twins[0]).toHaveTextContent(/\d\d:\d\d/);
      expect(twins[0]).toHaveTextContent("failed");
      expect(twins[1]).toHaveTextContent("interrupted");
      const child = within(list).getByTitle("A very long name that is truncated in the list");
      expect(child).toHaveClass("truncate");
      expect(list.closest("li")).toContainElement(screen.getByRole("button", { name: /Run aaaaaaaa/ }));
    });

    it("shows no list for a job without sub-agents, and tells the backend what is on screen", async () => {
      mount([job("aaaaaaaa", "running")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] }, {
        job_sub_agents: () => ({ projectId: PROJECT, owner: { kind: "job", id: "aaaaaaaa" }, nodes: [] }),
      });
      await screen.findByRole("heading", { level: 2, name: "Run aaaaaaaa" });
      expect(screen.queryByRole("list", { name: "Sub-agents" })).toBeNull();
      await waitFor(() =>
        expect(call).toHaveBeenCalledWith("job_sub_agents_view", { projectId: PROJECT, jobId: "aaaaaaaa", listShown: true, path: null }),
      );
    });

    it("opens a sub-agent's read-only conversation from its list row and goes back with the job's row", async () => {
      const user = userEvent.setup();
      mount([job("aaaaaaaa", "running")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] }, handlers((path) => reply(path)));
      const list = await screen.findByRole("list", { name: "Sub-agents" });
      const row = within(list).getByRole("button", { name: /^Lead/ });
      await user.click(row);
      expect(await screen.findByText("Read the code.")).toBeInTheDocument();
      expect(screen.getByText("On it.")).toBeInTheDocument();
      expect(row).toHaveAttribute("aria-current", "true");
      const header = screen.getByRole("heading", { name: "Run aaaaaaaa › Lead" });
      expect(header.parentElement).toHaveTextContent("working");
      expect(screen.getByText("Run aaaaaaaa", { selector: "div" })).toBeInTheDocument();
      const disabledBox = screen.getByPlaceholderText("Sub-agents take no messages.");
      expect(disabledBox).toBeDisabled();
      expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
      expect(screen.queryByPlaceholderText("Steer the agent while it runs…")).toBeNull();
      expect(screen.queryByRole("button", { name: "Stop job" })).toBeNull();
      expect(call).toHaveBeenCalledWith("job_sub_agents_view", {
        projectId: PROJECT,
        jobId: "aaaaaaaa",
        listShown: true,
        path: [{ call: "t1", id: "0-Lead" }],
      });

      await user.click(within(list).getByRole("button", { name: /^A very long name/ }));
      expect(await screen.findByRole("heading", { name: "Run aaaaaaaa › Lead › A very long name that is truncated in the list" })).toBeInTheDocument();

      await user.click(screen.getByRole("button", { name: /Run aaaaaaaa/ }));
      expect(await screen.findByPlaceholderText("Steer the agent while it runs…")).toBeInTheDocument();
    });

    it("opens the same conversation from the line under the task row, and says when it is not available", async () => {
      const user = userEvent.setup();
      mount(
        [job("aaaaaaaa", "running")],
        { jobId: "aaaaaaaa", seq: 0, entries: [taskRow], notes: [] },
        handlers((path) => reply(path, { entries: [], unavailable: "the transcript file was not found" })),
      );
      const conversation = await screen.findByRole("region", { name: "Conversation" });
      await user.click(await within(conversation).findByRole("button", { name: "Working · 1 step · Task" }));
      const line = await within(conversation).findByRole("button", { name: /^Lead\s*working$/ });
      await user.click(line);
      expect(await screen.findByText("Conversation not available: the transcript file was not found.")).toBeInTheDocument();
      expect(screen.getByPlaceholderText("Sub-agents take no messages.")).toBeDisabled();
      const list = screen.getByRole("list", { name: "Sub-agents" });
      expect(within(list).getByRole("button", { name: /^Lead/ })).toHaveAttribute("aria-current", "true");
    });

    it("shows the messages of a sub-agent under a sender row with a 24 px `Bot` tile and its name, never `Weaver`", async () => {
      const user = userEvent.setup();
      mount([job("aaaaaaaa", "running")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] }, handlers((path) => reply(path)));
      const list = await screen.findByRole("list", { name: "Sub-agents" });
      await user.click(within(list).getByRole("button", { name: /^Lead/ }));
      const message = await screen.findByText("On it.");
      const conversation = screen.getByRole("region", { name: "Conversation" });
      const sender = within(conversation).getByText("Lead").parentElement!;
      expect(sender.firstElementChild).toHaveClass("size-6");
      expect(sender.firstElementChild!.querySelector("svg")).toHaveClass("lucide-bot");
      expect(within(conversation).queryByText("Weaver")).toBeNull();
      expect(sender.compareDocumentPosition(message) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
      // The path with its state is in the header band, not in the conversation.
      const band = screen.getByRole("heading", { level: 1, name: "Jobs" }).closest("header")!;
      expect(within(band).getByRole("heading", { level: 2, name: "Run aaaaaaaa › Lead" }).parentElement).toHaveTextContent("working");
      expect(within(conversation).queryByRole("heading")).toBeNull();
    });
  });
});

describe("the @ tags in the reply box of a job", () => {
  it("opens the list of assets, inserts the chosen one, does not send with the list open, and colours the recognised tags", async () => {
    mount([job("aaaaaaaa", "running")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] });
    const user = userEvent.setup();
    const box = await screen.findByPlaceholderText("Steer the agent while it runs…");
    await user.type(box, "Say @nothing and @lo");
    // A tag that names no asset keeps the normal colour.
    expect(document.querySelectorAll("[data-tag]")).toHaveLength(0);
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["logo.png"]);
    await user.keyboard("{Enter}");
    expect(call).not.toHaveBeenCalledWith("job_send_message", expect.anything());
    expect(box).toHaveValue("Say @nothing and @logo.png ");
    expect(document.querySelector("[data-tag='logo.png']")?.textContent).toBe("@logo.png");
    expect(document.querySelectorAll("[data-tag]")).toHaveLength(1);
    await user.keyboard("{Enter}");
    await waitFor(() =>
      expect(call).toHaveBeenCalledWith("job_send_message", { projectId: PROJECT, jobId: "aaaaaaaa", text: "Say @nothing and @logo.png" }),
    );
  });
});

describe("the list of the Jobs view", () => {
  beforeEach(() => vi.clearAllMocks());

  const snapshot = { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] };
  const stored = (width: number, hidden: boolean) => ({ jobs_list_state: () => ({ width, hidden }) });
  /** The pane of the list: the box whose width the separator sets. */
  const pane = () => screen.getByRole("navigation", { name: "Jobs" }).parentElement!;
  const saved = (width: number, hidden: boolean) => ["jobs_list_set", { projectId: PROJECT, width, hidden }] as const;

  it("is 280 px wide and shown when the backend remembers nothing, with a separator that tells its range", async () => {
    mount([job("aaaaaaaa", "running")], snapshot);
    const separator = await screen.findByRole("separator", { name: "Resize jobs list" });
    expect(separator).toHaveAttribute("aria-orientation", "vertical");
    expect(separator).toHaveAttribute("aria-valuemin", "240");
    expect(separator).toHaveAttribute("aria-valuemax", "420");
    expect(separator).toHaveAttribute("aria-valuenow", "280");
    expect(separator).toHaveAttribute("tabindex", "0");
    expect(separator).toHaveClass("w-2", "cursor-col-resize");
    expect(pane()).toHaveStyle({ width: "280px" });
    expect(call).toHaveBeenCalledWith("jobs_list_state", { projectId: PROJECT });
  });

  it("moves 16 px with the arrows, goes to 240 and 420 with `Home` and `End`, and back to 280 with `Enter`, saving each step", async () => {
    mount([job("aaaaaaaa", "running")], snapshot);
    const user = userEvent.setup();
    const separator = await screen.findByRole("separator", { name: "Resize jobs list" });
    separator.focus();
    await user.keyboard("{ArrowRight}");
    expect(separator).toHaveAttribute("aria-valuenow", "296");
    expect(pane()).toHaveStyle({ width: "296px" });
    expect(call).toHaveBeenLastCalledWith(...saved(296, false));
    await user.keyboard("{ArrowLeft}{ArrowLeft}");
    expect(separator).toHaveAttribute("aria-valuenow", "264");
    expect(call).toHaveBeenLastCalledWith(...saved(264, false));
    await user.keyboard("{Home}");
    expect(separator).toHaveAttribute("aria-valuenow", "240");
    expect(call).toHaveBeenLastCalledWith(...saved(240, false));
    await user.keyboard("{ArrowLeft}");
    expect(separator).toHaveAttribute("aria-valuenow", "240");
    await user.keyboard("{End}");
    expect(separator).toHaveAttribute("aria-valuenow", "420");
    expect(call).toHaveBeenLastCalledWith(...saved(420, false));
    await user.keyboard("{Enter}");
    expect(separator).toHaveAttribute("aria-valuenow", "280");
    expect(call).toHaveBeenLastCalledWith(...saved(280, false));
  });

  it("goes back to 280 px with a double click", async () => {
    mount([job("aaaaaaaa", "running")], snapshot, stored(380, false));
    const user = userEvent.setup();
    const separator = await screen.findByRole("separator", { name: "Resize jobs list" });
    expect(pane()).toHaveStyle({ width: "380px" });
    await user.dblClick(separator);
    expect(pane()).toHaveStyle({ width: "280px" });
    expect(call).toHaveBeenLastCalledWith(...saved(280, false));
  });

  it("follows a drag within 240–420 px and saves only when the drag ends", async () => {
    mount([job("aaaaaaaa", "running")], snapshot);
    const separator = await screen.findByRole("separator", { name: "Resize jobs list" });
    fireEvent.pointerDown(separator, { button: 0, clientX: 280, pointerId: 1 });
    fireEvent.pointerMove(separator, { clientX: 330, pointerId: 1 });
    expect(pane()).toHaveStyle({ width: "330px" });
    fireEvent.pointerMove(separator, { clientX: 900, pointerId: 1 });
    expect(pane()).toHaveStyle({ width: "420px" });
    fireEvent.pointerMove(separator, { clientX: 100, pointerId: 1 });
    expect(pane()).toHaveStyle({ width: "240px" });
    fireEvent.pointerMove(separator, { clientX: 350, pointerId: 1 });
    expect(call).not.toHaveBeenCalledWith("jobs_list_set", expect.anything());
    fireEvent.pointerUp(separator, { clientX: 350, pointerId: 1 });
    expect(pane()).toHaveStyle({ width: "350px" });
    expect(call).toHaveBeenCalledWith(...saved(350, false));
    fireEvent.pointerMove(separator, { clientX: 500, pointerId: 1 });
    expect(pane()).toHaveStyle({ width: "350px" });
  });

  it("hides the list and its rule, leaves `Jobs` with `Show jobs list` and brings the list back as it was", async () => {
    mount([job("aaaaaaaa", "running"), job("bbbbbbbb", "done")], snapshot, stored(360, false));
    const user = userEvent.setup();
    await screen.findByRole("navigation", { name: "Jobs" });
    await user.click(screen.getByRole("radio", { name: "Ended 1" }));
    await user.click(screen.getByRole("button", { name: "Hide jobs list" }));
    expect(call).toHaveBeenLastCalledWith(...saved(360, true));
    expect(screen.queryByRole("navigation", { name: "Jobs" })).toBeNull();
    expect(screen.queryByRole("separator")).toBeNull();
    expect(screen.queryByRole("button", { name: "Hide jobs list" })).toBeNull();
    const band = screen.getByRole("heading", { level: 1, name: "Jobs" }).closest("header")!;
    expect(within(band).getByRole("button", { name: "Show jobs list" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "Jobs" }).parentElement).not.toHaveClass("border-r");
    // The right pane takes the whole width and still shows the selection.
    expect(screen.getByRole("region", { name: "Conversation" })).toHaveClass("flex-1");
    expect(screen.getByRole("heading", { level: 2, name: "Run aaaaaaaa" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Show jobs list" }));
    expect(call).toHaveBeenLastCalledWith(...saved(360, false));
    expect(pane()).toHaveStyle({ width: "360px" });
    expect(screen.getByRole("radio", { name: "Ended 1" })).toBeChecked();
    expect(screen.queryByRole("button", { name: "Show jobs list" })).toBeNull();
  });

  it("opens hidden when the backend remembers it hidden, and a job opened from elsewhere leaves it hidden", async () => {
    mount([job("aaaaaaaa", "running"), job("bbbbbbbb", "running")], snapshot, stored(300, true), { openedJob: "bbbbbbbb" });
    expect(await screen.findByRole("button", { name: "Show jobs list" })).toBeInTheDocument();
    expect(screen.queryByRole("navigation", { name: "Jobs" })).toBeNull();
    expect(await screen.findByRole("heading", { level: 2, name: "Run bbbbbbbb" })).toBeInTheDocument();
    expect(call).not.toHaveBeenCalledWith("jobs_list_set", expect.anything());
  });

  it.each([
    ["a width above the range", { width: 500, hidden: true }],
    ["a width below the range", { width: 100, hidden: true }],
    ["a width that is not an integer", { width: 300.5, hidden: true }],
    ["a width of the wrong type", { width: "300", hidden: true }],
    ["a hidden state of the wrong type", { width: 300, hidden: "yes" }],
  ])("counts as 280 px and shown for %s", async (_, value) => {
    mount([job("aaaaaaaa", "running")], snapshot, { jobs_list_state: () => value });
    await screen.findByRole("navigation", { name: "Jobs" });
    expect(pane()).toHaveStyle({ width: "280px" });
    expect(screen.getByRole("separator", { name: "Resize jobs list" })).toHaveAttribute("aria-valuenow", "280");
  });

  describe("in a narrow window", () => {
    afterEach(() => vi.restoreAllMocks());
    const windowOf = (width: number) =>
      vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({ width, height: 600, x: 0, y: 0, top: 0, left: 0, right: width, bottom: 600, toJSON: () => ({}) });

    it("narrows the list to leave the conversation 480 px, never under 240 px, without changing the width that was chosen or saving anything", async () => {
      windowOf(760);
      mount([job("aaaaaaaa", "running")], snapshot, stored(400, false));
      await screen.findByRole("navigation", { name: "Jobs" });
      expect(pane()).toHaveStyle({ width: "280px" });
      expect(screen.getByRole("separator", { name: "Resize jobs list" })).toHaveAttribute("aria-valuenow", "400");
      expect(call).not.toHaveBeenCalledWith("jobs_list_set", expect.anything());
    });

    it("stops at 240 px however narrow the window is", async () => {
      windowOf(500);
      mount([job("aaaaaaaa", "running")], snapshot, stored(400, false));
      await screen.findByRole("navigation", { name: "Jobs" });
      expect(pane()).toHaveStyle({ width: "240px" });
    });

    it("keeps the chosen width in a window wide enough", async () => {
      windowOf(1400);
      mount([job("aaaaaaaa", "running")], snapshot, stored(400, false));
      await screen.findByRole("navigation", { name: "Jobs" });
      expect(pane()).toHaveStyle({ width: "400px" });
    });
  });
});

describe("the conversation of a job", () => {
  beforeEach(() => vi.clearAllMocks());

  const AT = new Date(2026, 10, 2, 9, 41).getTime();
  const tool = (id: string, title: string, status: "working" | "done" = "done"): Entry => ({ kind: "tool", id, title, icon: "read", command: null, status, subAgents: [] });
  const entriesOf = (entries: Entry[], arrivals: number[] = []) => ({ jobId: "aaaaaaaa", seq: 0, notes: [], entries, arrivals });

  it("centres the entries in a column of at most 860 px, with the message box in the same column", async () => {
    mount([job("aaaaaaaa", "running")], entriesOf([{ kind: "agent", text: "Reading the Model", complete: true }], [AT]));
    const column = (await screen.findByText("Reading the Model")).closest(".max-w-\\[860px\\]")!;
    expect(column).toHaveClass("mx-auto", "w-full");
    const frame = screen.getByRole("combobox", { name: "Message the agent" }).closest(".rounded-2xl")!;
    expect(frame).toHaveClass("max-w-[860px]");
    expect(frame.parentElement).toHaveClass("justify-center", "bg-gradient-to-b");
  });

  it("shows the messages of the agent under a sender row with the logo, `Weaver` and the time, and the Chat text style", async () => {
    mount([job("aaaaaaaa", "running")], entriesOf([{ kind: "agent", text: "Reading the Model", complete: true }], [AT]));
    const message = await screen.findByText("Reading the Model");
    expect(message.closest(".text-\\[14\\.5px\\]")).toHaveClass("leading-[1.65]");
    const sender = screen.getByText("Weaver").parentElement!;
    expect(sender).toHaveTextContent(clockText(AT));
    expect(sender.querySelector("svg, img")).not.toBeNull();
    expect(sender.compareDocumentPosition(message) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("folds every run of `Thinking` and tool rows into a closed summary that reads `Working · <n> steps · <latest step>` while the turn works", async () => {
    const user = userEvent.setup();
    mount(
      [job("aaaaaaaa", "running")],
      entriesOf([
        { kind: "developer", message: 1, text: "Go", delivery: { status: "delivered" } },
        { kind: "thinking", text: "Let me look" },
        tool("t1", "Read the Model"),
        tool("t2", "Search the code", "working"),
      ]),
    );
    const summary = await screen.findByRole("button", { name: "Working · 3 steps · Search the code" });
    expect(summary).toHaveAttribute("aria-expanded", "false");
    expect(summary.querySelector("span.truncate")).not.toBeNull();
    expect(screen.queryByRole("button", { name: "Read the Model" })).toBeNull();
    await user.click(summary);
    expect(summary).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("button", { name: "Read the Model" })).toBeInTheDocument();
  });

  it("names `Thinking` as the latest step when the run ends with a thinking block", async () => {
    mount([job("aaaaaaaa", "running")], entriesOf([tool("t1", "Read the Model"), { kind: "thinking", text: "Hmm" }]));
    expect(await screen.findByRole("button", { name: "Working · 2 steps · Thinking" })).toBeInTheDocument();
  });

  it("reads `Worked for <duration> · <n> steps` once the run is not the last thing of a working turn", async () => {
    mount([job("aaaaaaaa", "done")], entriesOf([tool("t1", "Read the Model"), tool("t2", "Search the code")], [AT, AT + 4000]));
    expect(await screen.findByRole("button", { name: "Worked for 4s · 2 steps" })).toBeInTheDocument();
    expect(screen.queryByText(/Working/)).toBeNull();
  });

  it("keeps the notes, the permission cards and the end line out of the summary, and the instruction first and closed", async () => {
    mount(
      [job("aaaaaaaa", "failed")],
      entriesOf([
        { kind: "instruction", text: "Use the weaver-scan skill on the whole System." },
        tool("t1", "Read the Model"),
        { kind: "note", text: "Halfway", at: "2026-11-02T07:30:00.000Z" },
        { kind: "permission", request: 4, tool: null, state: { status: "asking" }, options: [{ id: "allow", name: "Allow once", rejects: false }] },
        { kind: "end", outcome: "failed", reason: "the agent refused", details: null },
      ]),
    );
    const summary = await screen.findByRole("button", { name: /^Worked/ });
    const details = screen.getByText("Instructions from Weaver").closest("details")!;
    expect(details).not.toHaveAttribute("open");
    expect(details.compareDocumentPosition(summary) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(summary.parentElement).not.toContainElement(screen.getByText("Halfway"));
    expect(summary.parentElement).not.toContainElement(screen.getByRole("group", { name: "Weaver asks permission" }));
    expect(summary.parentElement).not.toContainElement(screen.getByText(/Failed: the agent refused/));
  });

  it("puts the line of a job that ended in the column, over the fade and with no rule", async () => {
    mount([job("aaaaaaaa", "done")], entriesOf([]));
    const line = (await screen.findByText("This job has ended. Continue in Chat.")).closest(".max-w-\\[860px\\]")!;
    expect(line).toHaveClass("justify-center");
    expect(line.parentElement).toHaveClass("justify-center", "bg-gradient-to-b");
    expect(line.className).not.toMatch(/border/);
    expect(line.parentElement!.className).not.toMatch(/border/);
  });
});
