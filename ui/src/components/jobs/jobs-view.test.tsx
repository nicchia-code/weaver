import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AgentNode } from "@/bindings/AgentNode";
import type { ConversationSnapshot } from "@/bindings/ConversationSnapshot";
import type { Entry } from "@/bindings/Entry";
import type { JobRow } from "@/bindings/JobRow";
import type { Step } from "@/bindings/Step";
import type { SubAgentConversation } from "@/bindings/SubAgentConversation";
import { expectOneGeometry, pressWhileComposing } from "@/components/assets/tag-box-checks";
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
}

function EditorProbe() {
  const { editing, selected, origin } = useRequests();
  return <output aria-label="Editor">{editing ? `${selected} from ${origin.kind === "jobs" ? `jobs/${origin.jobId}` : "board"}` : "closed"}</output>;
}

function mount(
  jobs: JobRow[],
  conversation: ConversationSnapshot,
  handlers: Record<string, (args: never) => unknown> = {},
  around: Around = {},
) {
  vi.mocked(on).mockResolvedValue(() => {});
  vi.mocked(call).mockImplementation(async (name: string, args?: unknown) => {
    if (name === "jobs_snapshot") return { projectId: PROJECT, jobs, ompReady: true };
    if (name === "job_conversation") return conversation;
    return (handlers[name]?.(args as never) ?? null) as null;
  });
  const view = (
    <JobsProvider projectId={PROJECT}>
      <JobsView />
    </JobsProvider>
  );
  render(
    <TooltipProvider>
      <ProjectContext.Provider value={{ projectId: PROJECT, showView: around.showView ?? vi.fn(), openJob: vi.fn(), openedJob: null, openedJobRequest: 0 }}>
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

  it("starts with the view header across both panels, with the list and the conversation under it", async () => {
    mount([job("aaaaaaaa", "running")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] });
    const list = await screen.findByRole("navigation", { name: "Jobs" });
    const header = screen.getByRole("heading", { level: 1, name: "Jobs" }).closest("header")!;
    expect(header).toHaveClass("h-[76px]", "border-b");
    expect(header.querySelector("button")).toBeNull();
    expect(header.compareDocumentPosition(list) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(header.parentElement).toContainElement(screen.getByRole("region", { name: "Conversation" }));
    expect(header.contains(list)).toBe(false);
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
    const header = within(conversation).getByRole("heading", { level: 2, name: "Run aaaaaaaa" }).closest("header")!;
    const stop = within(header).getByRole("button", { name: "Stop job" });
    const box = within(conversation).getByRole("combobox", { name: "Message the agent" });
    expect(header.compareDocumentPosition(box) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(conversation).getAllByRole("button", { name: /^Stop/ })).toEqual([stop]);
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
      const conversation = await screen.findByRole("region", { name: "Conversation" });
      const header = within(conversation).getByRole("heading", { level: 2, name: "Design" }).closest("header")!;
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
    const conversation = await screen.findByRole("region", { name: "Conversation" });
    const header = within(conversation).getByRole("heading", { level: 2, name: "Run aaaaaaaa" }).closest("header")!;
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
    const conversation = await screen.findByRole("region", { name: "Conversation" });
    const header = within(conversation).getByRole("heading", { level: 2, name: "Run aaaaaaaa" }).closest("header")!;
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
    expect(bubble).toHaveClass("ml-auto", "bg-paper", "rounded-2xl", "rounded-br-[4px]");
  });

  it("puts the steering box in a ring with a 14 px radius, with a primary icon button to send", async () => {
    mount([job("aaaaaaaa", "running")], { jobId: "aaaaaaaa", seq: 0, entries: [], notes: [] });
    const box = await screen.findByRole("combobox", { name: "Message the agent" });
    const frame = box.closest(".rounded-\\[14px\\]")!;
    expect(frame).toHaveClass("bg-surface");
    const send = within(frame as HTMLElement).getByRole("button", { name: "Send" });
    expect(send).toHaveClass("size-[34px]");
    expect(send).toHaveAttribute("aria-disabled", "true");
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
      const conversation = screen.getByRole("region", { name: "Conversation" });
      expect(within(conversation).getByRole("heading", { level: 2, name: "Run aaaaaaaa" })).toBeInTheDocument();
      expect(within(conversation).getByRole("button", { name: "Stop job" })).toBeInTheDocument();
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
      const line = await within(await screen.findByRole("region", { name: "Conversation" })).findByRole("button", { name: /^Lead\s*working$/ });
      await user.click(line);
      expect(await screen.findByText("Conversation not available: the transcript file was not found.")).toBeInTheDocument();
      expect(screen.getByPlaceholderText("Sub-agents take no messages.")).toBeDisabled();
      const list = screen.getByRole("list", { name: "Sub-agents" });
      expect(within(list).getByRole("button", { name: /^Lead/ })).toHaveAttribute("aria-current", "true");
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
