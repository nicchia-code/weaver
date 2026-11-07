import { act as reactAct, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ActionInput } from "@/bindings/ActionInput";
import type { QuestionRoundChanged } from "@/bindings/QuestionRoundChanged";
import type { QuestionRoundView } from "@/bindings/QuestionRoundView";
import type { QuestionView } from "@/bindings/QuestionView";
import { AsksProvider } from "@/components/ask/use-asks";
import { QuestionRoundLayer } from "@/components/questions/question-overlay";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const PROJECT = "0123456789abcdef";

function question(id: string, patch: Partial<QuestionView> = {}): QuestionView {
  return {
    id,
    question: `Which storage for ${id}?`,
    context: null,
    options: [
      { label: "Disk", description: "A **file** on disk.", details: "Survives a restart." },
      { label: "Memory", description: "Gone at exit.", details: null },
      { label: "Cloud", description: "Somebody else's disk.", details: null },
    ],
    recommended: 0,
    multi: false,
    status: "missing",
    draft: { selected: [], text: "" },
    confirmed: null,
    visited: false,
    ...patch,
  };
}

function round(patch: Partial<QuestionRoundView> = {}): QuestionRoundView {
  return {
    id: 7,
    projectId: PROJECT,
    origin: { kind: "agent", jobName: "Scan System" },
    // The backend preselects the recommended option on the first visit.
    questions: [question("a", { draft: { selected: [0], text: "" }, visited: true }), question("b", { multi: true })],
    position: { kind: "question", index: 0 },
    minimized: false,
    waiting: 0,
    ...patch,
  };
}

/** A backend that keeps the round as the real one does: positions, drafts, confirmed answers, skips. */
function fakeBackend(start: QuestionRoundView | null) {
  let current = start;
  const received: ActionInput[] = [];
  const listeners: ((payload: QuestionRoundChanged) => void)[] = [];
  vi.mocked(on).mockImplementation(async (name: string, handler: unknown) => {
    if (name === "question-round-changed") listeners.push(handler as (payload: QuestionRoundChanged) => void);
    return () => {};
  });
  vi.mocked(call).mockImplementation(async (name: string, args?: unknown) => {
    if (name === "question_round_get") return current;
    if (name !== "question_round_act" || !current) return null;
    const action = (args as { action: ActionInput }).action;
    received.push(action);
    const next = structuredClone(current);
    switch (action.kind) {
      case "go":
        next.position = action.position;
        if (action.position.kind === "question") {
          const slot = next.questions[action.position.index]!;
          if (!slot.visited) {
            slot.visited = true;
            if (slot.draft.selected.length === 0 && slot.status === "missing") slot.draft.selected = [slot.recommended];
          }
        }
        break;
      case "draft":
        next.questions[action.index]!.draft = { selected: action.selected, text: action.text };
        break;
      case "confirm": {
        const slot = next.questions[action.index]!;
        slot.confirmed = structuredClone(slot.draft);
        slot.status = "answered";
        break;
      }
      case "skip": {
        const slot = next.questions[action.index]!;
        slot.confirmed = null;
        slot.status = "skipped";
        break;
      }
      case "minimize":
        next.minimized = action.minimized;
        break;
      case "skip-all":
      case "send":
        current = null;
        return null;
    }
    current = next;
    return next;
  });
  return {
    received,
    kinds: () => received.map((action) => action.kind),
    emit(payload: QuestionRoundChanged) {
      current = payload.round;
      reactAct(() => listeners.forEach((listener) => listener(payload)));
    },
  };
}

function mount() {
  return render(
    <TooltipProvider>
      <QuestionRoundLayer projectId={PROJECT}>
        <main>The project</main>
      </QuestionRoundLayer>
    </TooltipProvider>,
  );
}

describe("QuestionOverlay", () => {
  beforeEach(() => vi.clearAllMocks());

  it("shows nothing while no round is presented", async () => {
    fakeBackend(null);
    mount();
    await waitFor(() => expect(call).toHaveBeenCalledWith("question_round_get", { projectId: PROJECT }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(screen.getByText("The project")).toBeInTheDocument();
  });

  it("shows the header, the position, the rounds that wait, the index and the options", async () => {
    fakeBackend(round({ waiting: 2, questions: [question("a", { draft: { selected: [0], text: "" }, status: "answered", confirmed: { selected: [0], text: "" } }), question("b", { status: "skipped" })] }));
    mount();
    const dialog = await screen.findByRole("dialog", { name: "Questions from the agent · Scan System" });
    expect(within(dialog).getByText(/Question 1 of 2/)).toBeInTheDocument();
    expect(within(dialog).getByText("+2 more rounds waiting")).toBeInTheDocument();
    const index = within(dialog).getByRole("navigation", { name: "Questions" });
    expect(within(index).getByText("Answered")).toBeInTheDocument();
    expect(within(index).getByText("Skipped")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Minimize" })).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Close" })).toBeInTheDocument();
    const options = within(dialog).getAllByRole("radio");
    expect(options).toHaveLength(3);
    expect(options[0]).toHaveAttribute("aria-checked", "true");
    expect(within(dialog).getAllByText("Recommended")).toHaveLength(1);
    expect(within(dialog).getByText("file")).toBeInTheDocument();
    expect(within(dialog).getByRole("textbox", { name: "Your answer (optional)" })).toHaveAttribute("placeholder", "Write a free answer or a note for the agent");
  });

  it("names a Drift question and says `round` when one waits", async () => {
    fakeBackend(round({ origin: { kind: "drift", drift: "D3", title: "Pick the storage" }, waiting: 1 }));
    mount();
    expect(await screen.findByRole("dialog", { name: "Question about D3 · Pick the storage" })).toBeInTheDocument();
    expect(screen.getByText("+1 more round waiting")).toBeInTheDocument();
  });

  it("Esc and a click outside do nothing", async () => {
    fakeBackend(round());
    mount();
    const user = userEvent.setup();
    await screen.findByRole("dialog");
    await user.keyboard("{Escape}");
    const overlay = document.querySelector<HTMLElement>('[data-state="open"].fixed.inset-0');
    expect(overlay).not.toBeNull();
    fireEvent.pointerDown(overlay!);
    fireEvent.click(overlay!);
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    expect(call).not.toHaveBeenCalledWith("question_round_act", expect.anything());
  });

  it("a digit selects an option, a second press clears it, and digits are inert in the textarea", async () => {
    const backend = fakeBackend(round());
    mount();
    const user = userEvent.setup();
    await screen.findByRole("dialog");
    await user.keyboard("2");
    await waitFor(() => expect(screen.getAllByRole("radio")[1]).toHaveAttribute("aria-checked", "true"));
    expect(backend.received.at(-1)).toEqual({ kind: "draft", index: 0, selected: [1], text: "" });
    await user.keyboard("2");
    await waitFor(() => expect(screen.getAllByRole("radio")[1]).toHaveAttribute("aria-checked", "false"));
    expect(backend.received.at(-1)).toEqual({ kind: "draft", index: 0, selected: [], text: "" });

    await user.click(screen.getByRole("textbox", { name: "Your answer (optional)" }));
    await user.keyboard("3");
    expect(screen.getByRole("textbox", { name: "Your answer (optional)" })).toHaveValue("3");
    expect(backend.received.filter((action) => action.kind === "draft" && action.selected.includes(2))).toHaveLength(0);
  });

  it("a click on the selected single option clears it, which disables Next with its reason", async () => {
    fakeBackend(round());
    mount();
    const user = userEvent.setup();
    await screen.findByRole("dialog");
    const next = screen.getByRole("button", { name: "Next" });
    expect(next).not.toHaveAttribute("aria-disabled");
    await user.click(screen.getAllByRole("radio")[0]!);
    await waitFor(() => expect(screen.getAllByRole("radio")[0]).toHaveAttribute("aria-checked", "false"));
    expect(next).toHaveAttribute("aria-disabled", "true");
    expect(next).toHaveAccessibleDescription("Choose an option or write an answer.");
    // Writing text makes it an answer.
    await user.type(screen.getByRole("textbox", { name: "Your answer (optional)" }), "Because.");
    expect(screen.getByRole("button", { name: "Next" })).not.toHaveAttribute("aria-disabled");
  });

  it("the arrows move the focus inside the options and Space picks", async () => {
    const backend = fakeBackend(round());
    mount();
    const user = userEvent.setup();
    await screen.findByRole("dialog");
    screen.getAllByRole("radio")[0]!.focus();
    await user.keyboard("{ArrowDown}");
    expect(screen.getAllByRole("radio")[1]).toHaveFocus();
    await user.keyboard("{ArrowDown}{ArrowDown}");
    expect(screen.getAllByRole("radio")[0]).toHaveFocus();
    await user.keyboard("{ArrowUp}");
    expect(screen.getAllByRole("radio")[2]).toHaveFocus();
    await user.keyboard(" ");
    await waitFor(() => expect(backend.received.at(-1)).toEqual({ kind: "draft", index: 0, selected: [2], text: "" }));
  });

  it("the free text reaches the backend 300 ms after the last keystroke, and at once on blur", async () => {
    const backend = fakeBackend(round());
    mount();
    const user = userEvent.setup();
    await screen.findByRole("dialog");
    const box = screen.getByRole("textbox", { name: "Your answer (optional)" });
    await user.type(box, "abc");
    expect(backend.received.filter((action) => action.kind === "draft")).toHaveLength(0);
    await waitFor(() => expect(backend.received.at(-1)).toEqual({ kind: "draft", index: 0, selected: [0], text: "abc" }), { timeout: 2000 });
    await user.type(box, "d");
    await user.tab();
    await waitFor(() => expect(backend.received.at(-1)).toEqual({ kind: "draft", index: 0, selected: [0], text: "abcd" }));
  });

  it("Next confirms and shows the next question; Review on the last one shows the summary; Ctrl+Enter does the same", async () => {
    const backend = fakeBackend(round());
    mount();
    const user = userEvent.setup();
    await screen.findByRole("dialog");
    await user.click(screen.getByRole("button", { name: "Next" }));
    await screen.findByText(/Question 2 of 2/);
    expect(backend.kinds().slice(-2)).toEqual(["confirm", "go"]);
    // The second question is a multiple choice; its recommended option is preselected on this first visit.
    expect(screen.getAllByRole("checkbox")).toHaveLength(3);
    expect(screen.getAllByRole("checkbox")[0]).toHaveAttribute("aria-checked", "true");
    expect(screen.getByRole("button", { name: "Review" })).not.toHaveAttribute("aria-disabled");
    await user.keyboard("2");
    await waitFor(() => expect(screen.getAllByRole("checkbox")[1]).toHaveAttribute("aria-checked", "true"));
    await user.keyboard("{Control>}{Enter}{/Control}");
    expect(await screen.findByRole("heading", { name: "Review your answers" })).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Send to agent" })).toBeInTheDocument();
  });

  it("the summary lists every answer, lets the developer edit, and sends only when nothing is missing", async () => {
    const backend = fakeBackend(
      round({
        position: { kind: "summary" },
        questions: [
          question("a", { status: "answered", draft: { selected: [1], text: "Note" }, confirmed: { selected: [1], text: " Note " } }),
          question("b", { multi: true }),
        ],
      }),
    );
    mount();
    const user = userEvent.setup();
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByText("Memory")).toBeInTheDocument();
    expect(within(dialog).getByText("Note")).toBeInTheDocument();
    expect(within(dialog).getByText("Missing", { selector: "p" })).toBeInTheDocument();
    const send = within(dialog).getByRole("button", { name: "Send to agent" });
    expect(send).toHaveAttribute("aria-disabled", "true");
    expect(send).toHaveAccessibleDescription("Answer or skip 1 more question to send.");
    await user.click(send);
    expect(backend.received).toHaveLength(0);

    await user.click(within(dialog).getByRole("button", { name: "Edit question 2" }));
    await screen.findByText(/Question 2 of 2/);
    await user.click(screen.getByRole("button", { name: "Skip" }));
    await screen.findByRole("button", { name: "Send to agent" });
    await waitFor(() => expect(screen.getByRole("button", { name: "Send to agent" })).not.toHaveAttribute("aria-disabled"));
    await user.click(screen.getByRole("button", { name: "Send to agent" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(backend.kinds().at(-1)).toBe("send");
  });

  it("reducing the round keeps a bar, and coming back finds the answers as they were", async () => {
    const backend = fakeBackend(round());
    mount();
    const user = userEvent.setup();
    await screen.findByRole("dialog");
    await user.keyboard("2");
    await user.type(screen.getByRole("textbox", { name: "Your answer (optional)" }), "Keep this");
    await user.click(screen.getByRole("button", { name: "Minimize" }));
    expect(await screen.findByText("The agent is waiting for your answers (question 1 of 2)")).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(backend.received.at(-2)).toEqual({ kind: "draft", index: 0, selected: [1], text: "Keep this" });

    await user.click(screen.getByRole("button", { name: "Open questions" }));
    await screen.findByRole("dialog");
    expect(screen.getByRole("textbox", { name: "Your answer (optional)" })).toHaveValue("Keep this");
    expect(screen.getAllByRole("radio")[1]).toHaveAttribute("aria-checked", "true");
  });

  it("Skip all asks first, with the focus on Go back, and skips only when confirmed; Close asks the same", async () => {
    const backend = fakeBackend(round());
    mount();
    const user = userEvent.setup();
    await screen.findByRole("dialog");
    await user.click(screen.getByRole("button", { name: "Skip all" }));
    let confirm = await screen.findByRole("alertdialog", { name: "Skip all unanswered questions?" });
    expect(within(confirm).getByText("The agent will use its recommendation for each of them.")).toBeInTheDocument();
    expect(within(confirm).getByRole("button", { name: "Go back" })).toHaveFocus();
    await user.click(within(confirm).getByRole("button", { name: "Go back" }));
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(backend.kinds()).not.toContain("skip-all");

    await user.click(screen.getByRole("button", { name: "Close" }));
    confirm = await screen.findByRole("alertdialog", { name: "Skip all unanswered questions?" });
    await user.click(within(confirm).getByRole("button", { name: "Skip all" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
    expect(backend.kinds().at(-1)).toBe("skip-all");
  });

  it("follows the backend: a round that opens shows, one of another project does not, one that ends goes away", async () => {
    const backend = fakeBackend(null);
    mount();
    await waitFor(() => expect(on).toHaveBeenCalled());
    backend.emit({ projectId: "another-project", round: round({ projectId: "another-project" }) });
    expect(screen.queryByRole("dialog")).toBeNull();
    backend.emit({ projectId: PROJECT, round: round() });
    expect(await screen.findByRole("dialog")).toBeInTheDocument();
    backend.emit({ projectId: PROJECT, round: null });
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("shows numbered dots with the status of each question under 600 px", async () => {
    const { setViewportWidth } = await import("@/test/media");
    setViewportWidth(480);
    fakeBackend(round({ questions: [question("a", { status: "answered", confirmed: { selected: [0], text: "" } }), question("b")] }));
    mount();
    const index = await screen.findByRole("navigation", { name: "Questions" });
    expect(within(index).getByRole("button", { name: "Question 1: Answered" })).toBeInTheDocument();
    expect(within(index).getByRole("button", { name: "Question 2: Missing" })).toBeInTheDocument();
  });
});

describe("Ask more on the question in view", () => {
  beforeEach(() => vi.clearAllMocks());

  /** The fake round backend, with `ask_open` answered by `open`. */
  function withAsks(start: QuestionRoundView, open: () => string | null) {
    const backend = fakeBackend(start);
    const base = vi.mocked(call).getMockImplementation()!;
    vi.mocked(call).mockImplementation((async (name: string, args?: unknown) => {
      if (name === "ask_list") return { projectId: PROJECT, asks: [] };
      if (name === "ask_open") {
        const id = open();
        if (id === null) throw { code: "refused", message: "gone" };
        return id;
      }
      return base(name as never, args as never);
    }) as never);
    return backend;
  }

  function mountWithAsks() {
    return render(
      <TooltipProvider>
        <AsksProvider projectId={PROJECT}>
          <QuestionRoundLayer projectId={PROJECT}>
            <main>The project</main>
          </QuestionRoundLayer>
        </AsksProvider>
      </TooltipProvider>,
    );
  }

  it("is an icon-only button on the question in view that sends the round and the position, then reduces the round without touching the drafts", async () => {
    const backend = withAsks(round({ questions: [question("a", { draft: { selected: [1], text: "my note" }, visited: true }), question("b")] }), () => "askaskas");
    mountWithAsks();
    const user = userEvent.setup();
    const dialog = await screen.findByRole("dialog");
    const button = within(dialog).getByRole("button", { name: "Ask more about this question" });
    expect(button).toHaveTextContent("");
    await user.click(button);
    expect(call).toHaveBeenCalledWith("ask_open", { projectId: PROJECT, target: { kind: "question", round: 7, index: 0 } });
    await waitFor(() => expect(backend.received.at(-1)).toEqual({ kind: "minimize", minimized: true }));
    // The round is the bar at the bottom; the question in view, the choices and the text are as they were.
    const bar = await screen.findByRole("region", { name: "Questions waiting" });
    expect(bar).toHaveTextContent("question 1 of 2");
    await user.click(within(bar).getByRole("button", { name: "Open questions" }));
    const reopened = await screen.findByRole("dialog");
    expect(within(reopened).getByRole("radio", { name: /Memory/ })).toHaveAttribute("aria-checked", "true");
    expect(within(reopened).getByRole("textbox", { name: "Your answer (optional)" })).toHaveValue("my note");
  });

  it("changes nothing when the question can no longer be asked about", async () => {
    const backend = withAsks(round(), () => null);
    mountWithAsks();
    const user = userEvent.setup();
    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: "Ask more about this question" }));
    await waitFor(() => expect(call).toHaveBeenCalledWith("ask_open", expect.anything()));
    expect(backend.kinds()).not.toContain("minimize");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("is not in the summary or in the reduced bar", async () => {
    withAsks(round({ position: { kind: "summary" } }), () => "askaskas");
    mountWithAsks();
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).queryByRole("button", { name: /Ask more/ })).toBeNull();
    cleanup();
    withAsks(round({ minimized: true }), () => "askaskas");
    mountWithAsks();
    const bar = await screen.findByRole("region", { name: "Questions waiting" });
    expect(within(bar).queryByRole("button", { name: /Ask more/ })).toBeNull();
  });
});
