import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ViewSnapshot } from "@/bindings/ViewSnapshot";
import { useChatDraftRequest } from "@/components/general/chat-draft";
import type { FocusSubject } from "@/components/views/focus-model";
import { RequirementsView } from "@/components/views/requirements-view";
import { useViews } from "@/components/views/views-state";
import { snapshot } from "@/components/views/view-fixtures";
import { fakeBackend, renderViews, type FakeBackend } from "@/components/views/view-test-utils";
import { call } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

let current: ViewSnapshot;
let backend: FakeBackend;
const showView = vi.fn();

beforeEach(() => {
  current = snapshot();
  backend = fakeBackend(() => current);
  showView.mockReset();
});

function FocusOpener({ refs, subject }: { refs: string[]; subject: FocusSubject }) {
  const { focusOn } = useViews();
  return <button onClick={() => focusOn("requirements", refs, subject)}>open focus</button>;
}

async function show(extra?: React.ReactNode) {
  renderViews(
    <>
      {extra}
      <RequirementsView />
    </>,
    showView,
  );
  await screen.findByRole("heading", { name: "Orders" });
}

describe("the Requirements page", () => {
  it("shows Areas in folder order, fragments with their path in a monospace tag, and sections with General first", async () => {
    await show();
    expect(screen.getAllByRole("heading", { level: 2 }).map((h) => h.textContent)).toEqual(["Orders", "Interface"]);
    expect(screen.getByText("How orders work.")).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 3, name: "Checkout" })).toHaveClass("text-[22px]", "font-semibold");
    expect(screen.getByText("orders/checkout.md")).toHaveClass("font-mono", "text-muted-foreground");
    const orders = screen.getByRole("region", { name: "Orders, General" }).parentElement!;
    expect(within(orders).getAllByRole("heading", { level: 4 }).map((h) => h.textContent)).toEqual(["General", "order"]);
    expect(within(orders).getAllByRole("heading", { level: 4 })[0]).toHaveClass("font-mono");
  });

  it("shows a section's title as plain text with `Open in editor` as its only control, and no reference chip", async () => {
    await show();
    const section = screen.getByRole("region", { name: "Orders, order" });
    expect(within(section).getByRole("heading", { level: 4, name: "order" })).toBeInTheDocument();
    expect(within(section).getAllByRole("button").map((b) => b.getAttribute("aria-label") ?? b.textContent)).toEqual(["Open in editor"]);
    expect(screen.queryByRole("button", { name: /View on diagram/ })).not.toBeInTheDocument();
  });

  it("has the title, the subtitle, the search, `Ask more` for the Requirements and `Ask for a change` in its header, and no `Ask more` anywhere else", async () => {
    await show();
    const header = screen.getByRole("heading", { level: 1, name: "Requirements" }).closest("header")!;
    expect(within(header).getByText("What the app must do, in plain words. Agents build from this.")).toBeInTheDocument();
    expect(within(header).getByRole("textbox", { name: "Search requirements" })).toBeInTheDocument();
    expect(within(header).getAllByRole("button").map((b) => b.getAttribute("aria-label") ?? b.textContent)).toEqual([
      "Ask more about the Requirements",
      "Ask for a change",
    ]);
    const buttons = screen.getAllByRole("button", { name: /Ask more/ });
    expect(buttons.map((b) => b.getAttribute("aria-label"))).toEqual(["Ask more about the Requirements"]);
    expect(buttons[0]).toHaveTextContent("Ask more");
    expect(buttons[0]).toBeEnabled();
    expect(within(header).queryByRole("combobox")).toBeNull();
    expect(screen.queryByRole("combobox", { name: "Overlay" })).not.toBeInTheDocument();
    for (const heading of screen.getAllByRole("heading", { level: 2 })) {
      expect(within(heading.parentElement!).queryByRole("button", { name: /Ask more/ })).toBeNull();
    }
  });

  it("tells what it is about with its tooltip", async () => {
    const user = userEvent.setup();
    await show();
    await user.hover(screen.getByRole("button", { name: "Ask more about the Requirements" }));
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Ask an agent about the Requirements");
  });

  it("sends only a null overlay when none is drawn, and Ask more never changes the view", async () => {
    const user = userEvent.setup();
    const askMore = vi.fn().mockResolvedValue("aaaaaaaa");
    renderViews(<RequirementsView />, showView, { askMore });
    await screen.findByRole("heading", { name: "Orders" });
    await user.click(screen.getByRole("button", { name: "Ask more about the Requirements" }));
    expect(askMore).toHaveBeenCalledTimes(1);
    expect(askMore).toHaveBeenCalledWith({ kind: "requirements", overlay: null });
    expect(showView).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Ask more about the Requirements" })).toBeEnabled();
  });

  it("sends the Proposal drawn over the Requirements and nothing else", async () => {
    const user = userEvent.setup();
    current = snapshot({ overlays: [{ id: "pppppppp", shortNumber: "P3", title: "Pay" }], overlay: "pppppppp" });
    const askMore = vi.fn().mockResolvedValue("aaaaaaaa");
    renderViews(<RequirementsView />, showView, { askMore });
    await screen.findByRole("heading", { name: "Orders" });
    await user.click(screen.getByRole("button", { name: "Ask more about the Requirements" }));
    expect(askMore).toHaveBeenCalledWith({ kind: "requirements", overlay: "pppppppp" });
  });

  it("opens the file at the line of a section's heading", async () => {
    const user = userEvent.setup();
    await show();
    const section = screen.getByRole("region", { name: "Orders, order" });
    await user.click(within(section).getByRole("button", { name: "Open in editor" }));
    await waitFor(() => expect(backend.opened).toEqual([{ file: ".weaver/requirements/orders/checkout.md", line: 9 }]));
  });

  it("shows the errors of an invalid Requirement at the top", async () => {
    current = snapshot({}, { errors: [{ message: "Two sections are titled `order`.", file: ".weaver/requirements/orders/checkout.md", line: 20 }] });
    await show();
    expect(await screen.findByText("The Model has 1 error")).toBeInTheDocument();
  });
});

describe("Ask for a change", () => {
  function Draft() {
    const draft = useChatDraftRequest();
    return <output aria-label="Chat draft" data-request={JSON.stringify(draft)} />;
  }

  const request = () => screen.getByRole("status", { name: "Chat draft" }).getAttribute("data-request");

  it("shows Chat and asks it for `Change to the Requirements: ` in its message box, sending nothing", async () => {
    const user = userEvent.setup();
    await show(<Draft />);
    expect(request()).toBe("null");
    await user.click(screen.getByRole("button", { name: "Ask for a change" }));
    expect(showView).toHaveBeenCalledWith("general");
    expect(request()).toBe(JSON.stringify({ text: "Change to the Requirements: ", nonce: 1 }));
    await user.click(screen.getByRole("button", { name: "Ask for a change" }));
    expect(request()).toBe(JSON.stringify({ text: "Change to the Requirements: ", nonce: 2 }));
    expect(vi.mocked(call).mock.calls.every(([name]) => name === "view_snapshot")).toBe(true);
  });
});

describe("the navigation of the Requirements", () => {
  const scrolled: HTMLElement[] = [];

  beforeEach(() => {
    scrolled.length = 0;
    Element.prototype.scrollIntoView = function (this: HTMLElement) {
      scrolled.push(this);
    };
  });

  const navigation = () => screen.getByRole("navigation", { name: "Requirements sections" });

  it("lists each Area as a heading with one entry per fragment and its number of sections", async () => {
    await show();
    const orders = within(navigation()).getByRole("group", { name: "Orders" });
    expect(within(orders).getAllByRole("button").map((b) => b.textContent)).toEqual(["Checkout2"]);
    const ui = within(navigation()).getByRole("group", { name: "Interface" });
    expect(within(ui).getAllByRole("button").map((b) => b.textContent)).toEqual(["App1"]);
  });

  it("scrolls the fragment of the chosen entry to the top and highlights the entry of the fragment at the top", async () => {
    const user = userEvent.setup();
    await show();
    const checkout = within(navigation()).getByRole("button", { name: /Checkout/ });
    const app = within(navigation()).getByRole("button", { name: /App/ });
    expect(checkout).toHaveAttribute("aria-current", "location");
    expect(app).not.toHaveAttribute("aria-current");
    await user.click(app);
    expect(scrolled.map((el) => el.dataset.fragmentKey)).toEqual(["ui/app"]);
    expect(app).toHaveAttribute("aria-current", "location");
    expect(checkout).not.toHaveAttribute("aria-current");
  });

  it("marks with `changed` the fragments the overlaid Proposal adds, removes or changes in any section", async () => {
    current = snapshot({ overlays: [{ id: "pppppppp", shortNumber: "P3", title: "Pay" }], overlay: "pppppppp" });
    current.data.requirements[0]!.fragments[0]!.sections[1]!.change = "added";
    await show();
    expect(within(within(navigation()).getByRole("button", { name: /Checkout/ })).getByText("changed")).toBeInTheDocument();
    expect(within(within(navigation()).getByRole("button", { name: /App/ })).queryByText("changed")).not.toBeInTheDocument();
  });

  it("has no `changed` mark without an overlay", async () => {
    await show();
    expect(within(navigation()).queryByText("changed")).not.toBeInTheDocument();
  });
});

/** Adds a fragment to the Orders Area, for the tests of the search. */
function addPayment() {
  current.data.requirements[0]!.fragments.push({
    name: "payment",
    title: "Payment",
    description: "",
    change: null,
    file: null,
    sections: [{ title: "payment", body: "Pays with a card.", before: null, change: null, movedFrom: null, file: null, line: null }],
  });
}

describe("the search of the Requirements", () => {
  const field = () => screen.getByRole("textbox", { name: "Search requirements" });

  it("keeps in place only the sections whose title or text has the words, whatever the case, and marks the matches", async () => {
    const user = userEvent.setup();
    addPayment();
    await show();
    await user.type(field(), "CART");
    expect(screen.getByRole("region", { name: "Orders, order" })).toBeInTheDocument();
    expect(within(screen.getByRole("region", { name: "Orders, order" })).getByText("cart", { selector: "mark" })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Orders, General" })).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Orders, payment" })).not.toBeInTheDocument();
  });

  it("looks at titles too, marks them, and needs every word", async () => {
    const user = userEvent.setup();
    addPayment();
    await show();
    await user.type(field(), "Payment");
    const section = screen.getByRole("region", { name: "Orders, payment" });
    expect(within(section).getByRole("heading", { level: 4 }).querySelector("mark")).toHaveTextContent("payment");
    expect(screen.queryByRole("region", { name: "Orders, order" })).not.toBeInTheDocument();

    await user.clear(field());
    await user.type(field(), "order cart");
    expect(screen.getByRole("region", { name: "Orders, order" })).toBeInTheDocument();
    await user.type(field(), " nothing");
    expect(screen.getByText("No sections match.")).toBeInTheDocument();
  });

  it("hides the fragments and Areas left empty and counts in the navigation only what remains", async () => {
    const user = userEvent.setup();
    addPayment();
    await show();
    const navigation = screen.getByRole("navigation", { name: "Requirements sections" });
    expect(within(navigation).getAllByRole("button").map((b) => b.textContent)).toEqual(["Checkout2", "Payment1", "App1"]);
    await user.type(field(), "cart");
    expect(within(navigation).getAllByRole("button").map((b) => b.textContent)).toEqual(["Checkout1"]);
    expect(within(navigation).queryByRole("group", { name: "Interface" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "Interface" })).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { level: 3, name: "Payment" })).not.toBeInTheDocument();
  });

  it("says `No sections match.` when nothing matches", async () => {
    const user = userEvent.setup();
    await show();
    await user.type(field(), "zzz");
    expect(screen.getByText("No sections match.")).toHaveClass("text-muted-foreground");
    expect(screen.queryByRole("heading", { level: 2 })).not.toBeInTheDocument();
  });

  it("shows everything again with Esc in the field, or when it is emptied", async () => {
    const user = userEvent.setup();
    await show();
    await user.type(field(), "zzz");
    await user.keyboard("{Escape}");
    expect(field()).toHaveValue("");
    expect(screen.getByRole("region", { name: "Orders, General" })).toBeInTheDocument();
    await user.type(field(), "cart");
    expect(screen.queryByRole("region", { name: "Orders, General" })).not.toBeInTheDocument();
    await user.clear(field());
    expect(screen.getByRole("region", { name: "Orders, General" })).toBeInTheDocument();
  });

  it("searches nothing but the sections of the Requirements: no Area prose, no Specs", async () => {
    const user = userEvent.setup();
    await show();
    await user.type(field(), "orders work");
    expect(screen.getByText("No sections match.")).toBeInTheDocument();
    await user.clear(field());
    await user.type(field(), "TypeScript");
    expect(screen.getByText("No sections match.")).toBeInTheDocument();
  });
});

describe("the overlay labels of the Requirements", () => {
  function overlaid(): ViewSnapshot {
    const drawn = snapshot({ overlays: [{ id: "pppppppp", shortNumber: "P3", title: "Pay" }], overlay: "pppppppp" });
    const [orders, ui] = drawn.data.requirements;
    const checkout = orders!.fragments[0]!;
    checkout.sections = [
      { ...checkout.sections[0]!, change: "changed", before: "Old general rules.", body: "New general rules." },
      { ...checkout.sections[1]!, change: "removed" },
      { title: "payment", body: "Pays.", before: null, change: "added", movedFrom: null, file: null, line: null },
      { title: "cart", body: "Carts.", before: null, change: null, movedFrom: "legacy", file: null, line: null },
    ];
    orders!.fragments.push({ name: "refunds", title: "Refunds", description: "", change: "new", file: null, sections: [] });
    ui!.fragments[0]!.change = "removed";
    return drawn;
  }

  it("uses text for every change: added, removed, changed, moved from, new fragment, fragment removed", async () => {
    current = overlaid();
    await show();
    expect(screen.getByRole("region", { name: "Orders, General, changed" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Orders, order, removed" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Orders, payment, added" })).toBeInTheDocument();
    expect(screen.getByText("moved from legacy")).toBeInTheDocument();
    expect(screen.getByText("new fragment")).toBeInTheDocument();
    expect(screen.getByText("fragment removed")).toBeInTheDocument();
  });

  it("strikes a removed section through in muted text", async () => {
    current = overlaid();
    await show();
    const removed = within(screen.getByRole("region", { name: "Orders, order, removed" })).getByText("An order has a cart.");
    expect(removed.closest("div.line-through")).toHaveClass("text-muted-foreground");
  });

  it("shows a changed section with its new text and `Show before` toggles the old one", async () => {
    current = overlaid();
    const user = userEvent.setup();
    await show();
    const section = screen.getByRole("region", { name: "Orders, General, changed" });
    expect(within(section).getByText("New general rules.")).toBeInTheDocument();
    await user.click(within(section).getByRole("button", { name: "Show before" }));
    expect(within(section).getByText("Old general rules.")).toBeInTheDocument();
    expect(within(section).queryByText("New general rules.")).not.toBeInTheDocument();
    await user.click(within(section).getByRole("button", { name: "Show before" }));
    expect(within(section).getByText("New general rules.")).toBeInTheDocument();
  });
});

describe("the focus of the Requirements", () => {
  const subject: FocusSubject = { kind: "task", id: "tttttttt", number: "T4", title: "Fix" };
  const scrolled: HTMLElement[] = [];

  beforeEach(() => {
    scrolled.length = 0;
    Element.prototype.scrollIntoView = function (this: HTMLElement) {
      scrolled.push(this);
    };
  });

  async function focused(refs: string[]) {
    const user = userEvent.setup();
    await show(<FocusOpener refs={refs} subject={subject} />);
    await user.click(screen.getByRole("button", { name: "open focus" }));
    await screen.findByRole("region", { name: "Focus" });
    return user;
  }

  it("tints focused sections and Areas, scrolls to the step and counts the references not found", async () => {
    await focused(["requirements/ui", "order", "order/Paid"]);
    const bar = screen.getByRole("region", { name: "Focus" });
    expect(within(bar).getByText("Task T4 – Fix")).toBeInTheDocument();
    expect(within(bar).getByText("Orders › order")).toBeInTheDocument();
    expect(within(bar).getByText("1 of 2")).toBeInTheDocument();
    expect(within(bar).getByText("1 not found")).toBeInTheDocument();
    await waitFor(() => expect(new Set(scrolled.map((el) => el.dataset.stepKey))).toEqual(new Set(["orders#order"])));
    expect(screen.getByRole("region", { name: "Orders, order" })).toHaveClass("border-sky");
    expect(screen.getByRole("region", { name: "Orders, General" })).not.toHaveClass("border-sky");
  });

  it("goes to the next step and scrolls to it", async () => {
    const user = await focused(["requirements/ui", "order"]);
    await user.click(within(screen.getByRole("region", { name: "Focus" })).getByRole("button", { name: "Next" }));
    await waitFor(() => expect(scrolled[scrolled.length - 1]?.dataset.stepKey).toBe("ui"));
    expect(within(screen.getByRole("region", { name: "Focus" })).getByText("2 of 2")).toBeInTheDocument();
  });

  it("keeps `Ask more` in the header with a focus and with `Only these` on, and sends nothing of them", async () => {
    const askMore = vi.fn().mockResolvedValue("aaaaaaaa");
    const user = userEvent.setup();
    renderViews(
      <>
        <FocusOpener refs={["order"]} subject={subject} />
        <RequirementsView />
      </>,
      showView,
      { askMore },
    );
    await screen.findByRole("heading", { name: "Orders" });
    await user.click(screen.getByRole("button", { name: "open focus" }));
    await screen.findByRole("region", { name: "Focus" });
    await user.click(screen.getByRole("checkbox", { name: "Only these" }));
    expect(screen.queryByRole("region", { name: "Orders, General" })).not.toBeInTheDocument();
    const button = screen.getByRole("button", { name: "Ask more about the Requirements" });
    expect(button).toBeEnabled();
    await user.click(button);
    expect(askMore).toHaveBeenCalledWith({ kind: "requirements", overlay: null });
    // The focus and `Only these` stay as they were.
    expect(screen.getByRole("region", { name: "Focus" })).toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Only these" })).toBeChecked();
  });

  it("hides what is not focused with `Only these`, off at the start", async () => {
    const user = await focused(["order"]);
    const toggle = screen.getByRole("checkbox", { name: "Only these" });
    expect(toggle).not.toBeChecked();
    expect(screen.getByRole("heading", { name: "Interface" })).toBeInTheDocument();
    await user.click(toggle);
    expect(screen.queryByRole("heading", { name: "Interface" })).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Orders, General" })).not.toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Orders, order" })).toBeInTheDocument();
    await user.click(toggle);
    expect(screen.getByRole("heading", { name: "Interface" })).toBeInTheDocument();
  });

  it("`Clear` and Esc remove the focus and keep the page", async () => {
    const user = await focused(["order"]);
    await user.click(screen.getByRole("button", { name: "Clear" }));
    expect(screen.queryByRole("region", { name: "Focus" })).not.toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Orders, order" })).not.toHaveClass("border-sky");
    await user.click(screen.getByRole("button", { name: "open focus" }));
    await screen.findByRole("region", { name: "Focus" });
    screen.getByRole("checkbox", { name: "Only these" }).focus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("region", { name: "Focus" })).not.toBeInTheDocument();
  });

  it("uses the change colour of a focused section under an overlay", async () => {
    current = snapshot({ overlay: "pppppppp", overlays: [{ id: "pppppppp", shortNumber: "P3", title: "Pay" }] });
    current.data.requirements[0]!.fragments[0]!.sections[1]!.change = "added";
    await focused(["order"]);
    expect(screen.getByRole("region", { name: "Orders, order, added" })).toHaveClass("border-sage");
  });

  it("stays on the same step when the files change", async () => {
    const user = await focused(["requirements/ui", "order"]);
    await user.click(within(screen.getByRole("region", { name: "Focus" })).getByRole("button", { name: "Next" }));
    current = snapshot({ revision: 2 });
    await act(async () => backend.emit(["requirements"], 2));
    await waitFor(() => expect(backend.asked).toHaveLength(2));
    expect(within(screen.getByRole("region", { name: "Focus" })).getByText("2 of 2")).toBeInTheDocument();
    expect(within(screen.getByRole("region", { name: "Focus" })).getByText("Interface")).toBeInTheDocument();
  });

  it("combines the search with `Only these`: a section shows only when both keep it, and Esc in the field keeps the focus", async () => {
    const user = await focused(["order"]);
    await user.click(screen.getByRole("checkbox", { name: "Only these" }));
    expect(screen.getByRole("region", { name: "Orders, order" })).toBeInTheDocument();
    const field = screen.getByRole("textbox", { name: "Search requirements" });
    await user.type(field, "general");
    expect(screen.queryByRole("region", { name: "Orders, order" })).not.toBeInTheDocument();
    expect(screen.getByText("No sections match.")).toBeInTheDocument();
    await user.keyboard("{Escape}");
    expect(field).toHaveValue("");
    expect(screen.getByRole("region", { name: "Focus" })).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Orders, order" })).toBeInTheDocument();
    // With the field empty, Esc reaches the focus.
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("region", { name: "Focus" })).not.toBeInTheDocument();
  });
});
