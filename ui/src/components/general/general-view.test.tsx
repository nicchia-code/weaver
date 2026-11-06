import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { AgentNode } from "@/bindings/AgentNode";
import type { Entry } from "@/bindings/Entry";
import type { GeneralState } from "@/bindings/GeneralState";
import type { OmpStatus } from "@/bindings/OmpStatus";
import type { Step } from "@/bindings/Step";
import type { SubAgentConversation } from "@/bindings/SubAgentConversation";
import { GeneralView } from "@/components/general/general-view";
import { ProjectContext } from "@/components/project-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const ompStatus = vi.hoisted(() => ({ current: null as OmpStatus | null }));
vi.mock("@/hooks/use-omp", () => ({ useOmp: () => ({ status: ompStatus.current }) }));

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
    if (name === "general_snapshot") return { state: general, seq: 0, entries };
    return (commands[name]?.(args as never) ?? null) as null;
  });
  render(
    <TooltipProvider>
      <ProjectContext.Provider value={{ projectId: PROJECT, showView: vi.fn(), openJob, openedJob: null }}>
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

  it("starts with the view header: the title Chat on the left and Clear on the right", async () => {
    mount([]);
    const header = (await screen.findByRole("heading", { level: 1, name: "Chat" })).closest("header")!;
    expect(header).toHaveClass("h-14", "border-b");
    expect(within(header).getByRole("button", { name: "Clear" })).toBeInTheDocument();
  });

  it("shows the invitation when the conversation is empty, with the message box enabled and Clear disabled", async () => {
    mount([]);
    expect(await screen.findByText("Think with omp about this project. Ask it for a Proposal or a job of Weaver when you want one.")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Message omp" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Clear" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
  });

  it("keeps Clear usable while a kept conversation has not come back", async () => {
    mount([], state({ kept: true }));
    await screen.findByText(/Think with omp/);
    await waitFor(() => expect(screen.getByRole("button", { name: "Clear" })).not.toHaveAttribute("aria-disabled"));
  });

  it("says Loading conversation… in place of the invitation while the kept conversation loads, with Clear usable", async () => {
    mount([], state({ kept: true, loading: true, starting: true }));
    expect(await screen.findByText("Loading conversation…")).toBeInTheDocument();
    expect(screen.queryByText(/Think with omp/)).toBeNull();
    expect(screen.queryByText("omp is not installed.")).toBeNull();
    expect(screen.getByRole("combobox", { name: "Message omp" })).toBeEnabled();
    await waitFor(() => expect(screen.getByRole("button", { name: "Clear" })).not.toHaveAttribute("aria-disabled"));
  });

  it("shows the message sent while loading below the loading text, with Starting omp… under it", async () => {
    mount([developer("Meanwhile", "queued")], state({ kept: true, loading: true, starting: true }));
    const loading = await screen.findByText("Loading conversation…");
    const message = screen.getByText("Meanwhile");
    expect(loading.compareDocumentPosition(message) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.getByText("Starting omp…")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Stop" })).toBeNull();
  });

  it("adds the missing-omp line and Install omp to the loading text at once, and shows the loading text alone otherwise", async () => {
    ompStatus.current = { state: "missing" } as OmpStatus;
    mount([], state({ kept: true, loading: true, starting: true }));
    expect(await screen.findByText("Loading conversation…")).toBeInTheDocument();
    expect(screen.getByText("omp is not installed.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Install omp" })).toBeInTheDocument();
  });

  it("shows the loading text alone while omp is only checking", async () => {
    ompStatus.current = { state: "checking" } as OmpStatus;
    mount([], state({ kept: true, loading: true, starting: true }));
    expect(await screen.findByText("Loading conversation…")).toBeInTheDocument();
    expect(screen.queryByText("omp is not installed.")).toBeNull();
    expect(screen.queryByRole("button", { name: "Install omp" })).toBeNull();
  });

  it("keeps the plain Clear text while loading, even with a message waiting", async () => {
    mount([developer("Meanwhile", "queued")], state({ kept: true, loading: true, starting: true }));
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: "Clear" }));
    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText("Clear Chat?")).toBeInTheDocument();
    expect(within(dialog).getByText("The conversation is deleted and a new one starts.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.click(within(dialog).getByRole("button", { name: "Clear" }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("general_clear", { projectId: PROJECT }));
  });

  it("sends the message with Enter and keeps Shift+Enter for a new line", async () => {
    mount([]);
    const user = userEvent.setup();
    const box = await screen.findByRole("combobox", { name: "Message omp" });
    await user.type(box, "Hello{Shift>}{Enter}{/Shift}omp{Enter}");
    await waitFor(() => expect(call).toHaveBeenCalledWith("general_send_message", { projectId: PROJECT, text: "Hello\nomp" }));
    expect(box).toHaveValue("");
  });

  it("shows Starting omp… under the message that waits while omp starts", async () => {
    mount([developer("Hi omp", "queued")], state({ working: true, starting: true }));
    expect(await screen.findByText("Hi omp")).toBeInTheDocument();
    expect(screen.getByText("Starting omp…")).toBeInTheDocument();
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
    await user.click(await screen.findByRole("button", { name: "Clear" }));
    let dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText("Clear Chat?")).toBeInTheDocument();
    expect(within(dialog).getByText("The conversation is deleted and a new one starts.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(call).not.toHaveBeenCalledWith("general_clear", expect.anything());

    act(() => handlers.get("general-changed")?.(state({ working: true }) as never));
    await user.click(screen.getByRole("button", { name: "Clear" }));
    dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText("The agent's turn stops, the conversation is deleted and a new one starts.")).toBeInTheDocument();
    await user.click(within(dialog).getByRole("button", { name: "Clear" }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("general_clear", { projectId: PROJECT }));
  });

  it("shows Clock and Queued under a message sent while the Chat waits for the answers of its round", async () => {
    mount([developer("While you ask", "queued")], state({ working: true, waiting: true }));
    const message = (await screen.findByText("While you ask")).closest("div") as HTMLElement;
    expect(within(message).getByText("Queued")).toBeInTheDocument();
    expect(message.querySelector("svg")).toHaveClass("lucide-clock");
    expect(screen.queryByText("Starting omp…")).toBeNull();
    expect(screen.getByRole("button", { name: "Stop" })).toBeInTheDocument();
  });

  it("shows why omp stopped, then says the next message starts it again", async () => {
    mount([
      developer("Hi"),
      { kind: "omp-stopped", reason: "omp exited with code 3", details: "boom" },
      { kind: "turn-failed", reason: "rate limited" },
      { kind: "notice", text: "The conversation could not be reloaded (gone). A new one started." },
    ]);
    expect(await screen.findByText("omp stopped: omp exited with code 3.")).toBeInTheDocument();
    expect(screen.getByText("Your next message starts omp again.")).toBeInTheDocument();
    expect(screen.getByText("The turn failed: rate limited.")).toBeInTheDocument();
    expect(screen.getByText("The conversation could not be reloaded (gone). A new one started.")).toBeInTheDocument();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Show details" }));
    expect(screen.getByText("boom")).toBeInTheDocument();
  });

  it("offers to install omp when it is missing", async () => {
    ompStatus.current = { state: "missing" } as OmpStatus;
    mount([]);
    const user = userEvent.setup();
    expect(await screen.findByText("omp is not installed.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Install omp" }));
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
    expect(line).toHaveClass("text-amber-600");
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
      await user.click(await screen.findByRole("button", { name: /^Lead/ }));
      expect(await screen.findByText("Read the code.")).toBeInTheDocument();
      expect(screen.getByText("On it.")).toBeInTheDocument();
      // The first entry is the assignment, labelled with who started it.
      expect(screen.getByText("Chat", { selector: "div" })).toBeInTheDocument();
      const header = await screen.findByRole("heading", { name: "Chat › Lead" });
      await waitFor(() => expect(header.parentElement).toHaveTextContent("working"));
      expect(header.tagName).toBe("H1");
      expect(header.parentElement).toHaveClass("h-14", "border-b");
      expect(screen.getByRole("button", { name: "Back to Chat" })).toBeInTheDocument();
      // The chat is gone, and nothing can be sent, stopped or answered here.
      expect(screen.queryByText("Chat text")).toBeNull();
      expect(screen.queryByRole("combobox", { name: "Message omp" })).toBeNull();
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
      await user.click(await screen.findByRole("button", { name: /^Lead/ }));
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
      await user.type(await screen.findByRole("combobox", { name: "Message omp" }), "Half a thought");
      await user.click(screen.getByRole("button", { name: /^Lead/ }));
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
      expect(screen.getByRole("combobox", { name: "Message omp" })).toHaveValue("Half a thought");
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
        await user.click(screen.getByRole("button", { name: /^Lead/ }));
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
      await user.click(await screen.findByRole("button", { name: /^Lead/ }));
      const text = await screen.findByText("Conversation not available: the transcript file was not found.");
      expect(text).toHaveClass("text-muted-foreground");
      expect(screen.queryByText("Read the code.")).toBeNull();
      expect(screen.getByRole("heading", { name: "Chat › Lead" })).toBeInTheDocument();
    });

    it("follows the sub-agent's new text through the events of its conversation", async () => {
      mount(chat, state(), vi.fn(), sub());
      const user = userEvent.setup();
      await user.click(await screen.findByRole("button", { name: /^Lead/ }));
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

    it("keeps Clear in the sub-agent's header and returns to the chat when it is confirmed", async () => {
      mount(chat, state(), vi.fn(), sub());
      const user = userEvent.setup();
      await user.click(await screen.findByRole("button", { name: /^Lead/ }));
      await screen.findByText("Read the code.");
      await user.click(screen.getByRole("button", { name: "Clear" }));
      const dialog = await screen.findByRole("alertdialog");
      await user.click(within(dialog).getByRole("button", { name: "Clear" }));
      await waitFor(() => expect(call).toHaveBeenCalledWith("general_clear", { projectId: PROJECT }));
      expect(await screen.findByRole("combobox", { name: "Message omp" })).toBeInTheDocument();
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
    const box = await screen.findByRole("combobox", { name: "Message omp" });
    await user.type(box, "Look at @lo");
    expect(screen.getAllByRole("option").map((option) => option.textContent)).toEqual(["logo.png"]);
    await user.keyboard("{Enter}");
    expect(call).not.toHaveBeenCalledWith("general_send_message", expect.anything());
    expect(box).toHaveValue("Look at @logo.png ");
    expect(document.querySelector("[data-tag='logo.png']")?.textContent).toBe("@logo.png");
    await user.keyboard("{Enter}");
    await waitFor(() => expect(call).toHaveBeenCalledWith("general_send_message", { projectId: PROJECT, text: "Look at @logo.png" }));
  });
});
