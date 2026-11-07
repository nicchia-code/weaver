import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { ActionInput } from "@/bindings/ActionInput";
import type { QuestionView } from "@/bindings/QuestionView";
import { ChatQuestionCard, closingOf } from "@/components/general/chat-question-card";
import type { RoundData } from "@/components/questions/question-model";
import { TooltipProvider } from "@/components/ui/tooltip";

function question(id: string, patch: Partial<QuestionView> = {}): QuestionView {
  return {
    id,
    question: `Which storage for ${id}?`,
    context: null,
    options: [
      { label: "Disk", description: "A file on disk.", details: null },
      { label: "Memory", description: "Gone at exit.", details: null },
    ],
    recommended: 0,
    multi: false,
    status: "missing",
    draft: { selected: [0], text: "" },
    confirmed: null,
    visited: true,
    ...patch,
  };
}

function card(patch: Partial<RoundData> = {}): RoundData {
  return { id: "cccccccc", questions: [question("a"), question("b")], position: { kind: "question", index: 0 }, ...patch };
}

function mount(data: RoundData, options: { closed?: boolean; askMore?: false | ((index: number) => void); starting?: boolean } = {}) {
  const acts: ActionInput[] = [];
  function Harness() {
    const [shown, setShown] = useState(data);
    return (
      <>
        <button type="button">Message box</button>
        <ChatQuestionCard
          card={shown}
          closed={options.closed ?? false}
          askMore={options.askMore ?? false}
          starting={options.starting}
          act={(action) => {
            acts.push(action);
            if (action.kind === "go") setShown((current) => ({ ...current, position: action.position }));
          }}
        />
      </>
    );
  }
  render(
    <TooltipProvider>
      <Harness />
    </TooltipProvider>,
  );
  return acts;
}

describe("the card of questions of Chat", () => {
  it("shows the head, the question in view and its numbered dots, with no overlay control", () => {
    mount(card());
    const article = screen.getByRole("article", { name: "Questions from Weaver" });
    expect(within(article).getByRole("heading", { name: "Questions from Weaver" })).toBeInTheDocument();
    expect(within(article).getByText("Question 1 of 2")).toBeInTheDocument();
    expect(within(article).getByRole("navigation", { name: "Questions" })).toBeInTheDocument();
    expect(within(article).getByRole("button", { name: "Question 1: Missing" })).toHaveAttribute("aria-current", "step");
    for (const name of ["Minimize", "Close", "Skip all"]) expect(screen.queryByRole("button", { name })).toBeNull();
    expect(screen.queryByText(/more rounds? waiting/)).toBeNull();
    expect(within(article).getByRole("button", { name: "Skip" })).toBeInTheDocument();
    expect(within(article).getByRole("button", { name: "Next" })).toBeInTheDocument();
  });

  it("never takes the focus by itself, neither when it appears nor when it moves", async () => {
    mount(card());
    expect(document.body).toHaveFocus();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Message box" }));
    await user.click(screen.getByRole("button", { name: "Question 2: Missing" }));
    // The dot was pressed inside the card: now the question text takes the focus.
    expect(screen.getByText("Which storage for b?").closest("[tabindex='-1']")).toHaveFocus();
  });

  it("keeps the focus where it was when the card moves without a press inside it", async () => {
    const acts = mount(card());
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Message box" }));
    expect(screen.getByRole("button", { name: "Message box" })).toHaveFocus();
    expect(acts).toEqual([]);
  });

  it("takes the digits and Ctrl+Enter only with the focus inside the card", async () => {
    const acts = mount(card({ questions: [question("a", { draft: { selected: [], text: "" } }), question("b")] }));
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Message box" }));
    await user.keyboard("2");
    expect(acts).toEqual([]);
    await user.click(screen.getByRole("button", { name: "Skip" }));
    acts.length = 0;
    await user.keyboard("2");
    expect(acts).toEqual([{ kind: "draft", index: 1, selected: [1], text: "" }]);
  });

  it("goes from the last question to the Summary, which sends with `Send to agent` once nothing is missing", async () => {
    const acts = mount(
      card({
        questions: [
          question("a", { status: "answered", confirmed: { selected: [0], text: "" } }),
          question("b", { status: "skipped" }),
        ],
        position: { kind: "summary" },
      }),
    );
    const article = screen.getByRole("article", { name: "Questions from Weaver" });
    expect(within(article).getByText("Summary", { selector: "p" })).toBeInTheDocument();
    expect(within(article).getByText("Skipped")).toBeInTheDocument();
    expect(within(article).queryByRole("button", { name: "Skip all" })).toBeNull();
    const user = userEvent.setup();
    await user.click(within(article).getByRole("button", { name: "Send to agent" }));
    expect(acts).toContainEqual({ kind: "send" });
  });

  it("refuses to send while a question is missing, saying how many", async () => {
    const acts = mount(card({ position: { kind: "summary" } }));
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: "Send to agent" }));
    expect(acts).not.toContainEqual({ kind: "send" });
  });

  it("has `Ask more` only when it can be asked, and calls it with the question in view", async () => {
    const askMore = vi.fn();
    mount(card(), { askMore });
    await userEvent.setup().click(screen.getByRole("button", { name: "Ask more about this question" }));
    expect(askMore).toHaveBeenCalledWith(0);
  });

  it("hides `Ask more` while it cannot be asked", () => {
    mount(card());
    expect(screen.queryByRole("button", { name: "Ask more about this question" })).toBeNull();
  });

  it("shows `Starting Weaver…` under the card while its answers wait", () => {
    mount(card({ position: { kind: "summary" } }), { starting: true });
    expect(screen.getByText("Starting Weaver…")).toBeInTheDocument();
  });

  describe("once closed", () => {
    const answered = question("a", { status: "answered", confirmed: { selected: [1], text: "In RAM" } });
    const skipped = question("b", { status: "skipped" });
    const open = question("c");

    it("is the read-only summary of what reached the agent, without controls", () => {
      mount(card({ questions: [answered, skipped, open] }), { closed: true });
      const article = screen.getByRole("article", { name: "Questions from Weaver" });
      expect(within(article).getByText("Memory")).toBeInTheDocument();
      expect(within(article).getByText("In RAM")).toBeInTheDocument();
      expect(within(article).getByText("Skipped")).toBeInTheDocument();
      expect(within(article).getByText("Not answered", { selector: "p" })).toBeInTheDocument();
      expect(within(article).queryAllByRole("button")).toEqual([]);
      expect(within(article).queryByRole("navigation")).toBeNull();
    });

    it.each([
      [[answered, skipped], "Answered"],
      [[answered, open], "Partly answered"],
      [[open, question("d")], "Not answered"],
    ])("says in its pill how it closed: %#", (questions, text) => {
      mount(card({ questions }), { closed: true });
      expect(within(screen.getByRole("article")).getByText(text, { selector: "div,span" })).toBeInTheDocument();
      expect(closingOf({ questions }).text).toBe(text);
    });
  });
});
