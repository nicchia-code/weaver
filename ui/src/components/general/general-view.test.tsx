import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AgentNode } from "@/bindings/AgentNode";
import type { ChatProposal } from "@/bindings/ChatProposal";
import type { Entry } from "@/bindings/Entry";
import type { GeneralState } from "@/bindings/GeneralState";
import type { OmpStatus } from "@/bindings/OmpStatus";
import type { Step } from "@/bindings/Step";
import type { SubAgentConversation } from "@/bindings/SubAgentConversation";
import { expectOneGeometry, pressWhileComposing } from "@/components/assets/tag-box-checks";
import { ChatDraftProvider, useChatDraft } from "@/components/general/chat-draft";
import { GeneralView } from "@/components/general/general-view";
import { ProjectContext, type ProjectContextValue } from "@/components/project-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const ompStatus = vi.hoisted(() => ({ current: null as OmpStatus | null }));
vi.mock("@/hooks/use-omp", () => ({ useOmp: () => ({ status: ompStatus.current }) }));

const settings = vi.hoisted(() => ({ analysis: "", save: vi.fn(async () => {}) }));
vi.mock("@/hooks/use-settings", () => ({
  settingText: (value: { models: { analysis: string } }) => value.models.analysis,
  useSettings: () => ({
    snapshot: {
      revision: 1,
      settings: { models: { analysis: settings.analysis, scan: "", execution: "" } },
      issues: [],
      fileError: null,
      schema: [{ key: "models.analysis", title: "Analysis", description: "Designs Proposals.", advice: null, kind: "model", defaultValue: "", choices: [] }],
    },
    loadError: null,
    reload: () => {},
    save: settings.save,
  }),
}));

/** The instant the first entry of a test conversation arrived: 09:21:05 local time. */
const ARRIVED = new Date(2026, 10, 7, 9, 21, 5).getTime();

const PROJECT = "0123456789abcdef";

const ASSETS = vi.hoisted(() => [
  { name: "logo.png", size: 1 },
  { name: "spec.pdf", size: 1 },
]);
vi.mock("@/components/assets/use-assets", () => ({ useAssets: () => ({ assets: ASSETS, loaded: true, apply: () => {} }) }));

function state(patch: Partial<GeneralState> = {}): GeneralState {
  return { projectId: PROJECT, working: false, starting: false, loading: false, kept: false, waiting: false, ...patch };
}

const handlers = new Map<string, (payload: never) => void>();

function mount(
  entries: Entry[],
  general: GeneralState = state(),
  openJob = vi.fn(),
  commands: Record<string, (args: never) => unknown> = {},
) {
  handlers.clear();
  vi.mocked(on).mockImplementation(async (name: string, handler: unknown) => {
    handlers.set(name, handler as (payload: never) => void);
    return () => {};
  });
  vi.mocked(call).mockImplementation(async (name: string, args?: unknown) => {
    if (name === "general_snapshot") return { state: general, seq: 0, entries, arrivals: entries.map((_, index) => ARRIVED + index * 1000) };
    if (name === "general_proposals") return (commands[name]?.(args as never) ?? { projectId: PROJECT, revision: 1, proposals: [] }) as never;
    return (commands[name]?.(args as never) ?? null) as null;
  });
  render(
    <TooltipProvider>
      <ProjectContext.Provider value={{ projectId: PROJECT, showView: vi.fn(), openJob, openedJob: null, openedJobRequest: 0 } as unknown as ProjectContextValue}>
        <GeneralView />
      </ProjectContext.Provider>
    </TooltipProvider>,
  );
  return { openJob };
}

const developer = (text: string, status: "queued" | "delivered" = "delivered"): Entry => ({
  kind: "developer",
  message: 1,
  text,
  delivery: { status },
});

describe("GeneralView", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    ompStatus.current = null;
  });

  it("starts with the view header: the title Chat, the agent pill and New chat on the right", async () => {
    mount([]);
    const header = (await screen.findByRole("heading", { level: 1, name: "Chat" })).closest("header")!;
    expect(header).toHaveClass("h-[76px]", "border-b");
    expect(within(header).getByRole("button", { name: "New chat" })).toBeInTheDocument();
  });

  it("shows the invitation when the conversation is empty, with the message box enabled and New chat disabled", async () => {
    mount([]);
    expect(await screen.findByText("Think with Weaver about this project. Ask it for a Proposal or a job when you want one.")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Message Weaver" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "New chat" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
  });

  it("keeps New chat usable while a kept conversation has not come back", async () => {
    mount([], state({ kept: true }));
    await screen.findByText(/Think with Weaver/);
    await waitFor(() => expect(screen.getByRole("button", { name: "New chat" })).not.toHaveAttribute("aria-disabled"));
  });

  it("says Loading conversation… in place of the invitation while the kept conversation loads, with New chat usable", async () => {
    mount([], state({ kept: true, loading: true, starting: true }));
    expect(await screen.findByText("Loading conversation…")).toBeInTheDocument();
    expect(screen.queryByText(/Think with Weaver/)).toBeNull();
    expect(screen.queryByText("The agent is not installed.")).toBeNull();
    expect(screen.getByRole("combobox", { name: "Message Weaver" })).toBeEnabled();
    await waitFor(() => expect(screen.getByRole("button", { name: "New chat" })).not.toHaveAttribute("aria-disabled"));
  });

  it("shows the message sent while loading below the loading text, with Starting Weaver… under it", async () => {
    mount([developer("Meanwhile", "queued")], state({ kept: true, loading: true, starting: true }));
    const loading = await screen.findByText("Loading conversation…");
    const message = screen.getByText("Meanwhile");
    expect(loading.compareDocumentPosition(message) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.getByText("Starting Weaver…")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
  });

  it("adds the missing-agent line and Install the agent to the loading text at once, and shows the loading text alone otherwise", async () => {
    ompStatus.current = { state: "missing" } as OmpStatus;
    mount([], state({ kept: true, loading: true, starting: true }));
    expect(await screen.findByText("Loading conversation…")).toBeInTheDocument();
    expect(screen.getByText("The agent is not installed.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Install the agent" })).toBeInTheDocument();
  });

  it("shows the loading text alone while the agent is only checking", async () => {
    ompStatus.current = { state: "checking" } as OmpStatus;
    mount([], state({ kept: true, loading: true, starting: true }));
    expect(await screen.findByText("Loading conversation…")).toBeInTheDocument();
    expect(screen.queryByText("The agent is not installed.")).toBeNull();
    expect(screen.queryByRole("button", { name: "Install the agent" })).toBeNull();
  });

  it("keeps the plain New chat text while loading, even with a message waiting", async () => {
    mount([developer("Meanwhile", "queued")], state({ kept: true, loading: true, starting: true }));
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "New chat" }));
    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText("Start a new chat?")).toBeInTheDocument();
    expect(within(dialog).getByText("The conversation is deleted and a new one starts.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.click(within(dialog).getByRole("button", { name: "New chat" }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("general_clear", { projectId: PROJECT }));
  });

  it("sends the message with Enter and keeps Shift+Enter for a new line", async () => {
    mount([]);
    const user = userEvent.setup();
    const box = await screen.findByRole("combobox", { name: "Message Weaver" });
    await user.type(box, "Hello{Shift>}{Enter}{/Shift}omp{Enter}");
    await waitFor(() => expect(call).toHaveBeenCalledWith("general_send_message", { projectId: PROJECT, text: "Hello\nomp" }));
    expect(box).toHaveValue("");
  });

  it("shows Starting Weaver… under the message that waits while the agent starts", async () => {
    mount([developer("Hi omp", "queued")], state({ working: true, starting: true }));
    expect(await screen.findByText("Hi omp")).toBeInTheDocument();
    expect(screen.getByText("Starting Weaver…")).toBeInTheDocument();
  });

  it("stops the turn at once, without a dialog", async () => {
    mount([developer("Do it")], state({ working: true }));
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Stop" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(call).toHaveBeenCalledWith("general_stop", { projectId: PROJECT });
  });

  it("asks before clearing, with another text during a turn, and clears only when confirmed", async () => {
    mount([developer("One")]);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "New chat" }));
    let dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText("Start a new chat?")).toBeInTheDocument();
    expect(within(dialog).getByText("The conversation is deleted and a new one starts.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(call).not.toHaveBeenCalledWith("general_clear", expect.anything());

    act(() => handlers.get("general-changed")?.(state({ working: true }) as never));
    await user.click(screen.getByRole("button", { name: "New chat" }));
    dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText("The agent's turn stops, the conversation is deleted and a new one starts.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "New chat" }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("general_clear", { projectId: PROJECT }));
  });

  it("shows Clock and Queued under a message sent while the Chat waits for the answers of its round", async () => {
    mount([developer("While you ask", "queued")], state({ working: true, waiting: true }));
    const message = (await screen.findByText("While you ask")).closest("div") as HTMLElement;
    expect(within(message).getByText("Queued")).toBeInTheDocument();
    expect(message.querySelector("svg")).toHaveClass("lucide-clock");
    expect(screen.queryByText("Starting Weaver…")).toBeNull();
    expect(screen.getByRole("button", { name: "Stop" })).toBeInTheDocument();
  });

  it("shows why the agent stopped, then says the next message starts it again", async () => {
    mount([
      developer("Hi"),
      { kind: "omp-stopped", reason: "the agent exited with code 3", details: "boom" },
      { kind: "turn-failed", reason: "rate limited" },
      { kind: "notice", text: "The conversation could not be reloaded (gone). A new one started." },
    ]);
    expect(await screen.findByText("The agent stopped: the agent exited with code 3.")).toBeInTheDocument();
    expect(screen.getByText("Your next message starts the agent again.")).toBeInTheDocument();
    expect(screen.getByText("The turn failed: rate limited.")).toBeInTheDocument();
    expect(screen.getByText("The conversation could not be reloaded (gone). A new one started.")).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Show details" }));
    expect(screen.getByText("boom")).toBeInTheDocument();
  });

  it("offers to install the agent when it is missing", async () => {
    ompStatus.current = { state: "missing" } as OmpStatus;
    mount([]);
    const user = userEvent.setup();
    expect(await screen.findByText("The agent is not installed.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Install the agent" }));
    expect(call).toHaveBeenCalledWith("omp_install");
  });

  it("shows a started job with a button that opens its conversation", async () => {
    const { openJob } = mount([developer("Scan please"), { kind: "job-started", job: "aaaaaaaa", name: "Scan System" }]);
    expect(await screen.findByText("Started Scan System")).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Open conversation" }));
    expect(openJob).toHaveBeenCalledWith("aaaaaaaa");
  });

  it("shows a refused start in the warning colour", async () => {
    mount([{ kind: "job-refused", jobKind: "run", reason: "A run is in progress" }]);
    const line = await screen.findByText("Weaver did not start run: A run is in progress.");
    expect(line).toHaveClass("text-peach-ink");
  });

  it("shows a refused design or setup request as a refused start of that kind", async () => {
    mount([
      { kind: "job-refused", jobKind: "design", reason: "Another job is writing" },
      { kind: "job-refused", jobKind: "setup", reason: "A merge is in progress" },
    ]);
    expect(await screen.findByText("Weaver did not start design: Another job is writing.")).toBeInTheDocument();
    expect(screen.getByText("Weaver did not start setup: A merge is in progress.")).toBeInTheDocument();
  });

  it("answers a permission card through Chat", async () => {
    mount([
      {
        kind: "permission",
        request: 4,
        tool: null,
        state: { status: "asking" },
        options: [{ id: "allow", name: "Allow once", rejects: false }],
      },
    ]);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Allow once" }));
    expect(call).toHaveBeenCalledWith("general_answer_permission", { projectId: PROJECT, request: 4, option: "allow" });
  });

  it("applies the batches of entries that follow the snapshot and fetches again on a reset", async () => {
    mount([developer("One")]);
    await screen.findByText("One");
    act(() =>
      handlers.get("general-entries")?.({
        projectId: PROJECT,
        seq: 1,
        reset: false,
        changes: [{ index: 1, entry: { kind: "agent", text: "Answer", complete: true } }],
      } as never),
    );
    expect(await screen.findByText("Answer")).toBeInTheDocument();
    const fetches = () => vi.mocked(call).mock.calls.filter(([name]) => name === "general_snapshot").length;
    expect(fetches()).toBe(1);
    act(() => handlers.get("general-entries")?.({ projectId: PROJECT, seq: 2, reset: true, changes: [] } as never));
    await waitFor(() => expect(fetches()).toBe(2));
  });

  describe("sub-agents", () => {
    /** The line of the sub-agent sits in the work summary of its turn, closed at first: open it. */
    async function leadButton() {
      const summary = await screen.findByRole("button", { name: /Worked for|Working/ });
      if (summary.getAttribute("aria-expanded") === "false") await userEvent.setup().click(summary);
      return screen.findByRole("button", { name: /^Lead/ });
    }
    const nodes: AgentNode[] = [
      {
        call: "t1",
        id: "0-Lead",
        name: "Lead",
        state: "working",
        started: "2026-11-02T07:05:00.000Z",
        children: [{ call: "c9", id: "1-Child", name: "Child", state: "succeeded", started: null, children: [] }],
      },
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
    const nestedRow: Entry = {
      kind: "tool",
      id: "c9",
      title: "Task",
      icon: "sub-agent",
      command: null,
      status: "done",
      subAgents: [{ id: "1-Child", name: "Child", state: "succeeded" }],
    };
    const reply = (path: Step[], patch: Partial<SubAgentConversation> = {}): SubAgentConversation => ({
      projectId: PROJECT,
      owner: { kind: "general" },
      path,
      seq: 0,
      entries: [
        { kind: "assignment", from: "Chat", text: "Read the code." },
        { kind: "agent", text: "On it.", complete: true },
        ...(path.length === 1 ? [nestedRow] : []),
      ],
      unavailable: null,
      ...patch,
    });
    const sub = (conversation: (path: Step[]) => SubAgentConversation = reply) => ({
      general_sub_agents: () => ({ projectId: PROJECT, owner: { kind: "general" }, nodes }),
      general_sub_agent_conversation: ({ path }: { path: Step[] }) => conversation(path),
    });
    const chat = [developer("Go"), taskRow, { kind: "agent", text: "Chat text", complete: true } as Entry];
    const lead: Step[] = [{ call: "t1", id: "0-Lead" }];

    it("opens a sub-agent's read-only conversation in place of the chat from its line under the task row", async () => {
      mount(chat, state({ working: true }), vi.fn(), sub());
      const user = userEvent.setup();
      await user.click(await leadButton());
      expect(await screen.findByText("Read the code.")).toBeInTheDocument();
      expect(screen.getByText("On it.")).toBeInTheDocument();
      // The first entry is the assignment, labelled with who started it.
      expect(screen.getByText("Chat", { selector: "div" })).toBeInTheDocument();
      const header = await screen.findByRole("heading", { name: "Chat › Lead" });
      await waitFor(() => expect(header.parentElement).toHaveTextContent("working"));
      expect(header.tagName).toBe("H1");
      expect(header.parentElement).toHaveClass("h-[76px]", "border-b");
      expect(screen.getByRole("button", { name: "Back to Chat" })).toBeInTheDocument();
      // The chat is gone, and nothing can be sent, stopped or answered here.
      expect(screen.queryByText("Chat text")).toBeNull();
      expect(screen.queryByRole("combobox", { name: "Message Weaver" })).toBeNull();
      expect(screen.getByRole("textbox", { name: "Message the agent" })).toBeDisabled();
      expect(screen.getByPlaceholderText("Sub-agents take no messages.")).toBeDisabled();
      expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
      expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
      expect(call).toHaveBeenCalledWith("general_sub_agent_conversation", { projectId: PROJECT, path: lead });
      await waitFor(() => expect(call).toHaveBeenCalledWith("general_sub_agents_view", { projectId: PROJECT, path: lead }));
    });

    it("opens the sub-agents of a sub-agent with every parent in the header, and the Back button leaves them all", async () => {
      mount(chat, state(), vi.fn(), sub());
      const user = userEvent.setup();
      await user.click(await leadButton());
      await user.click(await screen.findByRole("button", { name: /^Child/ }));
      const header = await screen.findByRole("heading", { name: "Chat › Lead › Child" });
      await waitFor(() => expect(header.parentElement).toHaveTextContent("succeeded"));
      expect(call).toHaveBeenCalledWith("general_sub_agent_conversation", {
        projectId: PROJECT,
        path: [...lead, { call: "c9", id: "1-Child" }],
      });
      await user.click(screen.getByRole("button", { name: "Back to Chat" }));
      expect(await screen.findByText("Chat text")).toBeInTheDocument();
    });

    it("goes back to the chat as it was, with what was typed, the chat's news and the backend told nothing is shown", async () => {
      mount(chat, state(), vi.fn(), sub());
      const user = userEvent.setup();
      await user.type(await screen.findByRole("combobox", { name: "Message Weaver" }), "Half a thought");
      await user.click(await leadButton());
      await screen.findByText("Read the code.");
      // Meanwhile the chat goes on.
      act(() =>
        handlers.get("general-entries")?.({
          projectId: PROJECT,
          seq: 1,
          reset: false,
          changes: [{ index: 3, entry: { kind: "agent", text: "Later answer", complete: true } }],
        } as never),
      );
      await user.click(screen.getByRole("button", { name: "Back to Chat" }));
      expect(await screen.findByText("Chat text")).toBeInTheDocument();
      expect(screen.getByText("Later answer")).toBeInTheDocument();
      expect(screen.getByRole("combobox", { name: "Message Weaver" })).toHaveValue("Half a thought");
      expect(screen.queryByRole("heading", { name: /Chat ›/ })).toBeNull();
      await waitFor(() => expect(call).toHaveBeenLastCalledWith("general_sub_agents_view", { projectId: PROJECT, path: null }));
    });

    it("returns to the chat at the scroll position it had", async () => {
      const scrolls = new WeakMap<object, number>();
      Object.defineProperty(HTMLElement.prototype, "scrollTop", {
        configurable: true,
        get() {
          return scrolls.get(this) ?? 0;
        },
        set(value: number) {
          scrolls.set(this, value);
        },
      });
      Object.defineProperty(HTMLElement.prototype, "scrollHeight", { configurable: true, get: () => 1000 });
      Object.defineProperty(HTMLElement.prototype, "clientHeight", { configurable: true, get: () => 100 });
      try {
        mount(chat, state(), vi.fn(), sub());
        const user = userEvent.setup();
        const scroller = (await screen.findByText("Chat text")).closest(".overflow-y-auto") as HTMLElement;
        // The developer scrolled up: the chat is no longer following the latest text.
        scroller.scrollTop = 300;
        fireEvent.scroll(scroller);
        await user.click(await leadButton());
        await screen.findByText("Read the code.");
        await user.click(screen.getByRole("button", { name: "Back to Chat" }));
        const again = (await screen.findByText("Chat text")).closest(".overflow-y-auto") as HTMLElement;
        expect(again).not.toBe(scroller);
        expect(again.scrollTop).toBe(300);
      } finally {
        Reflect.deleteProperty(HTMLElement.prototype, "scrollTop");
        Reflect.deleteProperty(HTMLElement.prototype, "scrollHeight");
        Reflect.deleteProperty(HTMLElement.prototype, "clientHeight");
      }
    });

    it("shows only `Conversation not available: <reason>.` under the header when the transcript is missing", async () => {
      mount(chat, state(), vi.fn(), sub((path) => reply(path, { entries: [], unavailable: "the transcript file was not found" })));
      const user = userEvent.setup();
      await user.click(await leadButton());
      const text = await screen.findByText("Conversation not available: the transcript file was not found.");
      expect(text).toHaveClass("text-muted-foreground");
      expect(screen.queryByText("Read the code.")).toBeNull();
      expect(screen.getByRole("heading", { name: "Chat › Lead" })).toBeInTheDocument();
    });

    it("follows the sub-agent's new text through the events of its conversation", async () => {
      mount(chat, state(), vi.fn(), sub());
      const user = userEvent.setup();
      await user.click(await leadButton());
      await screen.findByText("On it.");
      act(() =>
        handlers.get("sub-agent-entries")?.({
          projectId: PROJECT,
          owner: { kind: "general" },
          path: lead,
          seq: 1,
          reset: false,
          unavailable: null,
          changes: [{ index: 2, entry: { kind: "agent", text: "Found it.", complete: true } }],
        } as never),
      );
      expect(await screen.findByText("Found it.")).toBeInTheDocument();
      // The events of a job's sub-agent are not this one's.
      act(() =>
        handlers.get("sub-agent-entries")?.({
          projectId: PROJECT,
          owner: { kind: "job", id: "aaaaaaaa" },
          path: lead,
          seq: 2,
          reset: false,
          unavailable: null,
          changes: [{ index: 3, entry: { kind: "agent", text: "Not for Chat", complete: true } }],
        } as never),
      );
      expect(screen.queryByText("Not for Chat")).toBeNull();
    });

    it("keeps New chat in the sub-agent's header and returns to the chat when it is confirmed", async () => {
      mount(chat, state(), vi.fn(), sub());
      const user = userEvent.setup();
      await user.click(await leadButton());
      await screen.findByText("Read the code.");
      await user.click(screen.getByRole("button", { name: "New chat" }));
      const dialog = await screen.findByRole("alertdialog");
      await user.click(within(dialog).getByRole("button", { name: "New chat" }));
      await waitFor(() => expect(call).toHaveBeenCalledWith("general_clear", { projectId: PROJECT }));
      expect(await screen.findByRole("combobox", { name: "Message Weaver" })).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Back to Chat" })).toBeNull();
    });
  });

  it("keeps following the streaming text when a late scroll event finds it grown, and stops only when the developer scrolls up", async () => {
    const scrolls = new WeakMap<object, number>();
    let height = 1000;
    Object.defineProperty(HTMLElement.prototype, "scrollTop", {
      configurable: true,
      get() {
        return scrolls.get(this) ?? 0;
      },
      set(value: number) {
        scrolls.set(this, value);
      },
    });
    Object.defineProperty(HTMLElement.prototype, "scrollHeight", { configurable: true, get: () => height });
    Object.defineProperty(HTMLElement.prototype, "clientHeight", { configurable: true, get: () => 100 });
    const arrive = (seq: number, text: string) =>
      act(() =>
        handlers.get("general-entries")?.({
          projectId: PROJECT,
          seq,
          reset: false,
          changes: [{ index: seq, entry: { kind: "agent", text, complete: false } }],
        } as never),
      );
    try {
      mount([{ kind: "agent", text: "First", complete: false }]);
      const scroller = (await screen.findByText("First")).closest(".overflow-y-auto") as HTMLElement;
      await waitFor(() => expect(scroller.scrollTop).toBe(1000));
      // The text grew before the scroll event of the jump to the bottom arrived.
      height = 1500;
      fireEvent.scroll(scroller);
      arrive(1, "Second");
      expect(scroller.scrollTop).toBe(1500);
      expect(screen.queryByRole("button", { name: "Jump to latest" })).toBeNull();

      // The developer scrolls up: new text no longer moves the chat.
      scroller.scrollTop = 300;
      fireEvent.scroll(scroller);
      height = 2000;
      arrive(2, "Third");
      expect(scroller.scrollTop).toBe(300);
      await userEvent.setup().click(screen.getByRole("button", { name: "Jump to latest" }));
      expect(scroller.scrollTop).toBe(2000);
      expect(screen.queryByRole("button", { name: "Jump to latest" })).toBeNull();
    } finally {
      Reflect.deleteProperty(HTMLElement.prototype, "scrollTop");
      Reflect.deleteProperty(HTMLElement.prototype, "scrollHeight");
      Reflect.deleteProperty(HTMLElement.prototype, "clientHeight");
    }
  });
});

describe("the @ tags in the Chat box", () => {
  it("opens the list of assets, inserts the chosen one, does not send with the list open, and colours the recognised tags", async () => {
    mount([]);
    const user = userEvent.setup();
    const box = await screen.findByRole("combobox", { name: "Message Weaver" });
    await user.type(box, "Look at @lo");
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["logo.png"]);
    await user.keyboard("{Enter}");
    expect(call).not.toHaveBeenCalledWith("general_send_message", expect.anything());
    expect(box).toHaveValue("Look at @logo.png ");
    expect(document.querySelector("[data-tag='logo.png']")?.textContent).toBe("@logo.png");
    await user.keyboard("{Enter}");
    await waitFor(() => expect(call).toHaveBeenCalledWith("general_send_message", { projectId: PROJECT, text: "Look at @logo.png" }));
  });

  it("gives the textarea and the mirror the same metrics, and takes no side padding from the textarea", async () => {
    mount([]);
    const box = await screen.findByRole("combobox", { name: "Message Weaver" });
    expectOneGeometry(box, { has: ["px-3"], lacks: ["px-0"] });
  });

  it("leaves Enter and Tab of a composition to the input method: nothing is sent, picked or moved", async () => {
    mount([]);
    const user = userEvent.setup();
    const box = await screen.findByRole("combobox", { name: "Message Weaver" });
    await user.type(box, "Hello");
    pressWhileComposing(box, "Enter");
    expect(call).not.toHaveBeenCalledWith("general_send_message", expect.anything());
    expect(box).toHaveValue("Hello");
    await user.type(box, " @lo");
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["logo.png"]);
    pressWhileComposing(box, "Enter");
    pressWhileComposing(box, "Tab");
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["logo.png"]);
    expect(box).toHaveValue("Hello @lo");
    expect(box).toHaveFocus();
    expect(call).not.toHaveBeenCalledWith("general_send_message", expect.anything());
  });
});

describe("a text a view asks Chat for", () => {
  const PREFIX = "Change to the Requirements: ";

  function Asker({ text = PREFIX }: { text?: string }) {
    const ask = useChatDraft();
    return <button onClick={() => ask(text)}>ask</button>;
  }

  async function open(showView = vi.fn()) {
    vi.clearAllMocks();
    ompStatus.current = null;
    vi.mocked(on).mockImplementation(async () => () => {});
    vi.mocked(call).mockImplementation(async (name: string) => (name === "general_snapshot" ? { state: state(), seq: 0, entries: [], arrivals: [] } : (null as null)));
    render(
      <TooltipProvider>
        <ProjectContext.Provider value={{ projectId: PROJECT, showView, openJob: vi.fn(), openedJob: null, openedJobRequest: 0 } as unknown as ProjectContextValue}>
          <ChatDraftProvider>
            <Asker />
            <GeneralView />
          </ChatDraftProvider>
        </ProjectContext.Provider>
      </TooltipProvider>,
    );
    await screen.findByText(/Think with Weaver/);
    return screen.getByRole("combobox", { name: "Message Weaver" }) as HTMLTextAreaElement;
  }

  it("shows Chat and puts it in the empty message box, focused, with the cursor at the end, sending nothing", async () => {
    const showView = vi.fn();
    const box = await open(showView);
    await userEvent.setup().click(screen.getByRole("button", { name: "ask" }));
    expect(showView).toHaveBeenCalledWith("general");
    await waitFor(() => expect(box).toHaveFocus());
    expect(box.value).toBe(PREFIX);
    expect(box.selectionStart).toBe(PREFIX.length);
    expect(box.selectionEnd).toBe(PREFIX.length);
    expect(vi.mocked(call).mock.calls.map(([name]) => name)).not.toContain("general_send");
  });

  it("keeps the text already in the box and goes on a new line after it", async () => {
    const user = userEvent.setup();
    const box = await open();
    await user.type(box, "Remember this");
    await user.click(screen.getByRole("button", { name: "ask" }));
    await waitFor(() => expect(box).toHaveFocus());
    expect(box.value).toBe(`Remember this\n${PREFIX}`);
    expect(box.selectionStart).toBe(box.value.length);
  });

  it("does it again at every request", async () => {
    const user = userEvent.setup();
    const box = await open();
    await user.click(screen.getByRole("button", { name: "ask" }));
    await waitFor(() => expect(box.value).toBe(PREFIX));
    await user.click(screen.getByRole("button", { name: "ask" }));
    await waitFor(() => expect(box.value).toBe(`${PREFIX}\n${PREFIX}`));
  });
});

const tool = (id: string, title: string, status: "working" | "done" = "done"): Entry => ({ kind: "tool", id, title, icon: "read", command: null, status, subAgents: [] });

describe("the conversation of Chat", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    ompStatus.current = null;
    settings.analysis = "";
  });

  it("shows the message of the agent under a sender row with the logo, Weaver and the time of its first text", async () => {
    mount([developer("Hi"), { kind: "agent", text: "Hello **there**", complete: true }]);
    const sender = (await screen.findByText("Weaver")).parentElement!;
    expect(within(sender).getByText("09:21")).toBeInTheDocument();
    expect(sender.querySelector("svg")).not.toBeNull();
  });

  it("folds every run of Thinking blocks and tool rows into one closed disclosure with its duration and steps", async () => {
    const entries: Entry[] = [
      developer("Go"),
      { kind: "thinking", text: "Let me think" },
      tool("t1", "Read the Model"),
      tool("t2", "Read the Specs"),
      { kind: "agent", text: "Done reading", complete: true },
    ];
    mount(entries);
    const summary = await screen.findByRole("button", { name: /Worked for 2s · 3 steps/ });
    expect(summary).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByText("Read the Model")).toBeNull();
    await userEvent.setup().click(summary);
    expect(summary).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByText("Read the Model")).toBeInTheDocument();
    expect(screen.getByText("Thinking")).toBeInTheDocument();
  });

  it("reads Working with the number of steps while the run is the last thing of a turn in progress", async () => {
    mount([developer("Go"), tool("t1", "Read the Model", "working")], state({ working: true }));
    expect(await screen.findByRole("button", { name: "Working · 1 step" })).toBeInTheDocument();
  });

  it("never puts a permission card, a notice or a message inside a run", async () => {
    mount([
      developer("Go"),
      tool("t1", "Read A"),
      { kind: "permission", request: 9, tool: null, state: { status: "asking" }, options: [{ id: "ok", name: "Allow once", rejects: false }] },
      tool("t2", "Read B"),
      { kind: "notice", text: "A notice" },
    ]);
    expect(await screen.findAllByRole("button", { name: /Worked for/ })).toHaveLength(2);
    expect(screen.getByRole("button", { name: "Allow once" })).toBeInTheDocument();
    expect(screen.getByText("A notice")).toBeInTheDocument();
  });

  it("shows the agent state in the pill and what to do when it is not ready", async () => {
    ompStatus.current = { state: "missing" } as OmpStatus;
    mount([]);
    expect(await screen.findByText("Weaver · not installed")).toBeInTheDocument();
  });

  it("ends the conversation with a line that brings the round back while the Chat waits for its answers", async () => {
    mount([developer("Ask me")], state({ working: true, waiting: true }));
    expect(await screen.findByText("Weaver is waiting for your answers.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Answer" })).toBeInTheDocument();
  });
});

describe("the model control of the message box", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    ompStatus.current = null;
    settings.analysis = "";
  });

  it("shows Default model when the analysis model is empty and the model of the settings otherwise", async () => {
    mount([]);
    expect(await screen.findByRole("button", { name: /Default model/ })).toBeInTheDocument();
  });

  it("shows the analysis model of the settings", async () => {
    settings.analysis = "anthropic/claude-opus";
    mount([]);
    expect(await screen.findByRole("button", { name: /anthropic\/claude-opus/ })).toBeInTheDocument();
  });

  it("opens the choice of the Analysis setting", async () => {
    mount([]);
    await userEvent.setup().click(await screen.findByRole("button", { name: /Default model/ }));
    expect(await screen.findByText("Analysis")).toBeInTheDocument();
  });

  it("is disabled during a turn, while loading and while the Chat waits for its round, with its reason", async () => {
    for (const patch of [{ working: true }, { loading: true, kept: true }, { working: true, waiting: true }]) {
      const { unmount } = render_(patch);
      const control = await screen.findByRole("button", { name: /Default model/ });
      expect(control).toHaveAttribute("aria-disabled", "true");
      expect(control).toHaveAccessibleDescription("Change the model when Weaver is not working");
      unmount();
    }
  });

  it("shows Send when idle and Stop in its place during a turn", async () => {
    mount([]);
    expect(await screen.findByRole("button", { name: "Send" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
  });
});

function render_(patch: Partial<GeneralState>) {
  mount([developer("x")], state(patch));
  return { unmount: () => cleanup() };
}

function proposal(patch: Partial<Omit<ChatProposal, "head">> & { head?: Partial<ChatProposal["head"]> } = {}): ChatProposal {
  const { head, ...rest } = patch;
  return {
    job: "aaaaaaaa",
    head: { id: "pppppppp", shortNumber: "P1", title: "Pay by card", body: "", created: "2026-11-07T09:00:00.000Z", agentFinishing: false, ...head },
    status: "open",
    view: {
      label: null,
      stale: false,
      blocked: [],
      model: null,
      files: { added: ["model/pay.md"], changed: ["specs.yaml"], removed: [] },
      carriesTasks: true,
      creates: 1,
      tasks: [{ id: "tttttttt", title: "Implement payment", body: "" }],
      removalTasks: [],
      requires: [],
      approve: { reasons: [], tooltip: null, refines: false },
      rejectReasons: [],
      canRework: true,
      scopeRefs: ["order/Paid"],
    },
    tasksOnBoard: 0,
    reworkJob: null,
    reworked: false,
    ...rest,
  };
}

describe("the Proposal card under the Started line", () => {
  const started: Entry[] = [developer("Design it"), { kind: "job-started", job: "aaaaaaaa", name: "Design Pay" }];
  const cards = (list: ChatProposal[]) => ({ general_proposals: () => ({ projectId: PROJECT, revision: 1, proposals: list }) });

  beforeEach(() => {
    vi.clearAllMocks();
    ompStatus.current = null;
  });

  it("shows the Proposal the job handed over under its line, with files, Tasks, id and the Board's controls", async () => {
    mount(started, state(), vi.fn(), cards([proposal()]));
    const card = await screen.findByRole("article", { name: "Proposal P1" });
    expect(call).toHaveBeenCalledWith("general_proposals", { projectId: PROJECT, jobs: ["aaaaaaaa"] });
    expect(within(card).getByText("Pay by card")).toBeInTheDocument();
    expect(within(card).getByText("Open")).toBeInTheDocument();
    expect(within(card).getByText("model/pay.md")).toBeInTheDocument();
    expect(within(card).getByText("added")).toBeInTheDocument();
    expect(within(card).getByText("changed")).toBeInTheDocument();
    expect(within(card).getByText("Creates 1 Task")).toBeInTheDocument();
    expect(within(card).getByText("Implement payment")).toBeInTheDocument();
    expect(within(card).getByText("pppppppp")).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Approve" })).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Reject" })).toBeInTheDocument();
    for (const absent of ["View requirements", "Model", "View diff", "Undo"]) expect(within(card).queryByRole("button", { name: absent })).toBeNull();
    const line = screen.getByText("Started Design Pay");
    expect(line.compareDocumentPosition(card) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("lists at most six paths and Tasks, then and <n> more", async () => {
    const many = proposal();
    many.view.files.added = ["a", "b", "c", "d", "e", "f", "g", "h"];
    many.view.tasks = Array.from({ length: 8 }, (_, index) => ({ id: `t${index}`, title: `Task ${index}`, body: "" }));
    many.view.creates = 8;
    mount(started, state(), vi.fn(), cards([many]));
    const card = await screen.findByRole("article", { name: "Proposal P1" });
    expect(within(card).getByText("and 2 more")).toBeInTheDocument();
    expect(within(card).queryByText("g")).toBeNull();
    expect(within(card).queryByText("Task 7")).toBeNull();
  });

  it("shows no card for a job other than the ones the conversation lists", async () => {
    mount(started, state(), vi.fn(), cards([]));
    await screen.findByText("Started Design Pay");
    expect(screen.queryByRole("article")).toBeNull();
  });

  it("shows the badges and the Peach box with every blocking reason", async () => {
    const blocked = proposal();
    blocked.view.stale = true;
    blocked.view.blocked = ["Stale: `specs.yaml` changed", "Proposal zzzzzzzz is missing"];
    mount(started, state(), vi.fn(), cards([blocked]));
    const card = await screen.findByRole("article", { name: "Proposal P1" });
    expect(within(card).getByText("stale")).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Blocked: 2 reasons" })).toBeInTheDocument();
    const box = within(card).getByRole("group", { name: "Blocked by" });
    expect(within(box).getByText("Proposal zzzzzzzz is missing")).toBeInTheDocument();
  });

  it("disables Approve with every reason of the backend, in its order", async () => {
    const waiting = proposal();
    waiting.view.approve = { reasons: ["The agent is still finishing", "A run is in progress"], tooltip: null, refines: false };
    mount(started, state(), vi.fn(), cards([waiting]));
    const approve = await screen.findByRole("button", { name: "Approve" });
    expect(approve).toHaveAttribute("aria-disabled", "true");
    expect(approve).toHaveAccessibleDescription(/The agent is still finishing\s*A run is in progress/);
  });

  it("approves through the Board's command and shows the refusal under the controls when the backend refuses", async () => {
    const commands = { ...cards([proposal()]), approve_proposal: () => Promise.reject({ code: "refused", message: "Could not approve P1: a run is in progress." }) };
    mount(started, state(), vi.fn(), commands);
    await userEvent.setup().click(await screen.findByRole("button", { name: "Approve" }));
    expect(call).toHaveBeenCalledWith("approve_proposal", { projectId: PROJECT, proposalId: "pppppppp" });
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not approve P1: a run is in progress.");
    expect(screen.getByRole("button", { name: "Approve" })).toBeInTheDocument();
  });

  it("rejects with the reason from the dialog and reworks with feedback from the menu", async () => {
    const user = userEvent.setup();
    mount(started, state(), vi.fn(), cards([proposal()]));
    await user.click(await screen.findByRole("button", { name: "Reject" }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByRole("textbox"), "Too wide");
    await user.click(within(dialog).getByRole("button", { name: "Reject" }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("reject_proposal", { projectId: PROJECT, proposalId: "pppppppp", feedback: "Too wide", rework: false }));
    await user.click(screen.getByRole("button", { name: "More ways to reject" }));
    await user.click(await screen.findByRole("menuitem", { name: "Rework with feedback…" }));
    const rework = await screen.findByRole("dialog");
    await user.type(within(rework).getByRole("textbox"), "Split it");
    await user.click(within(rework).getByRole("button", { name: "Rework" }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("reject_proposal", { projectId: PROJECT, proposalId: "pppppppp", feedback: "Split it", rework: true }));
  });

  it("offers Refine and Discard for a Draft a refinement left half-way, and Refine in place of Approve for a stale Proposal", async () => {
    const draft = proposal({ status: "draft" });
    draft.view.approve = { reasons: [], tooltip: "Continue refining, then approve", refines: true };
    mount(started, state(), vi.fn(), cards([draft]));
    const card = await screen.findByRole("article", { name: "Proposal P1" });
    expect(within(card).getByText("Draft")).toBeInTheDocument();
    expect(within(card).getByRole("button", { name: "Refine" })).toBeInTheDocument();
    await userEvent.setup().click(within(card).getByRole("button", { name: "Discard" }));
    await userEvent.setup().click(await screen.findByRole("button", { name: "Discard" }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("discard_proposal", { projectId: PROJECT, proposalId: "pppppppp" }));
  });

  it("turns the footer into one line once decided: Approved with Open Board, Rejected, Sent back for rework with Open conversation, Discarded", async () => {
    const showView = vi.fn();
    const openJob = vi.fn();
    mount(
      started,
      state(),
      openJob,
      cards([
        proposal({ status: "approved", tasksOnBoard: 2 }),
        proposal({ status: "rejected", head: { id: "qqqqqqqq", shortNumber: "P2" } }),
        proposal({ status: "rejected", reworked: true, reworkJob: "wwwwwwww", head: { id: "rrrrrrrr", shortNumber: "P3" } }),
        proposal({ status: "discarded", head: { id: "ssssssss", shortNumber: "P4" } }),
      ]),
    );
    expect(await screen.findByText("Approved. 2 Tasks are on the Board.")).toBeInTheDocument();
    expect(screen.getByText("Rejected.")).toBeInTheDocument();
    expect(screen.getByText("Sent back for rework.")).toBeInTheDocument();
    expect(screen.getByText("Discarded.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Approve" })).toBeNull();
    const user = userEvent.setup();
    const rework = screen.getAllByRole("button", { name: "Open conversation" });
    await user.click(rework[rework.length - 1]!);
    expect(openJob).toHaveBeenCalledWith("wwwwwwww");
    expect(showView).not.toHaveBeenCalled();
  });
});
