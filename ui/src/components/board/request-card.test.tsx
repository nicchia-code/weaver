import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BoardCard } from "@/bindings/BoardCard";
import type { BoardSnapshot } from "@/bindings/BoardSnapshot";
import { BoardProvider } from "@/components/board/board-state";
import { BoardView } from "@/components/board/board-view";
import { ProjectContext } from "@/components/project-context";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const openEditor = vi.fn();
vi.mock("@/components/requests/use-requests", () => ({ useRequests: () => ({ openEditor }) }));

const toasts = vi.hoisted(() => ({ error: vi.fn() }));
vi.mock("sonner", () => ({ toast: { error: toasts.error } }));

const PROJECT = "0123456789abcdef";
const ID = "2020-11-02T09-30-00";

function draft(patch: Partial<Extract<BoardCard, { kind: "request" }>> = {}): BoardCard {
  return { kind: "request", id: ID, title: "Add a cart", created: "2020-11-02T09:30:00", empty: false, returned: false, ...patch };
}

const commands: { name: string; args: unknown }[] = [];
let failure: unknown = null;

async function show(cards: BoardCard[]) {
  commands.length = 0;
  const snapshot: BoardSnapshot = { projectId: PROJECT, revision: 1, backlog: cards, active: [], ghosts: [], gathered: [], runAllReasons: [], doneSinceLastScan: 0, warnings: [] };
  vi.mocked(on).mockImplementation((() => Promise.resolve(() => {})) as unknown as typeof on);
  vi.mocked(call).mockImplementation((async (name: string, args: unknown) => {
    if (name === "board_snapshot") return snapshot;
    commands.push({ name, args });
    if (failure) throw failure;
    return null;
  }) as unknown as typeof call);
  render(
    <TooltipProvider>
      <ProjectContext.Provider value={{ projectId: PROJECT, showView: vi.fn(), openJob: vi.fn(), openedJob: null, openedJobRequest: 0 }}>
        <BoardProvider projectId={PROJECT}>
          <BoardView />
        </BoardProvider>
      </ProjectContext.Provider>
    </TooltipProvider>,
  );
  await screen.findByRole("region", { name: "Backlog" });
}

const card = () => screen.getByRole("region", { name: "Backlog" }).querySelector<HTMLElement>("[data-card-id]")!;

beforeEach(() => {
  vi.mocked(call).mockReset();
  openEditor.mockReset();
  toasts.error.mockReset();
  failure = null;
});

describe("the card of a request being written", () => {
  it("has the tag Request where other cards show their id, the title and `Not sent · <time>` in Muted", async () => {
    await show([draft()]);
    expect(within(card()).queryByRole("img", { name: /(Permanent ID|Short number)/ })).toBeNull();
    expect(within(card()).getByText("Request")).toBeInTheDocument();
    expect(within(card()).queryByText(ID)).toBeNull();
    const title = within(card()).getByRole("heading", { name: "Add a cart" });
    expect(title).toHaveClass("text-[15px]", "font-semibold");
    expect(within(card()).getByRole("button", { name: "Add a cart" })).toHaveClass("line-clamp-3");
    expect(within(card()).getByText("Not sent · 2 Nov 09:30")).toHaveClass("text-muted-foreground");
  });

  it("opens the request in the editor with a click outside its controls, or Enter on its title, and has no details dialog", async () => {
    const user = userEvent.setup();
    await show([draft()]);
    await user.click(within(card()).getByText("Not sent · 2 Nov 09:30"));
    expect(openEditor).toHaveBeenCalledWith(ID);
    expect(screen.queryByRole("dialog")).toBeNull();
    openEditor.mockClear();
    within(card()).getByRole("button", { name: "Add a cart" }).focus();
    await user.keyboard("{Enter}");
    expect(openEditor).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("sends from its footer, and the click does not open the editor", async () => {
    const user = userEvent.setup();
    await show([draft()]);
    const footer = card().querySelector<HTMLElement>("[data-card-controls]")!;
    await user.click(within(footer).getByRole("button", { name: "Send" }));
    expect(commands).toEqual([{ name: "request_send", args: { projectId: PROJECT, requestId: ID } }]);
    expect(openEditor).not.toHaveBeenCalled();
  });

  it("disables Send while the text is empty, with the tooltip Write the request first.", async () => {
    const user = userEvent.setup();
    await show([draft({ empty: true })]);
    const send = within(card()).getByRole("button", { name: "Send" });
    expect(send).toHaveAttribute("aria-disabled", "true");
    await user.click(send);
    expect(commands).toEqual([]);
    await user.hover(send);
    expect(await screen.findAllByText("Write the request first.")).not.toHaveLength(0);
  });

  it("discards only after the confirmation, with its file and images", async () => {
    const user = userEvent.setup();
    await show([draft()]);
    await user.click(within(card()).getByRole("button", { name: "Discard" }));
    const dialog = await screen.findByRole("alertdialog");
    expect(within(dialog).getByText("Discard this request?")).toBeInTheDocument();
    expect(within(dialog).getByText("Its file and images are deleted.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(commands).toEqual([]);

    await user.click(within(card()).getByRole("button", { name: "Discard" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Discard" }));
    expect(commands).toEqual([{ name: "request_discard", args: { projectId: PROJECT, requestId: ID } }]);
    expect(openEditor).not.toHaveBeenCalled();
  });

  it("shows the error toast of the editor when a send or a discard fails", async () => {
    const user = userEvent.setup();
    await show([draft()]);
    failure = "boom";
    await user.click(within(card()).getByRole("button", { name: "Send" }));
    await waitFor(() => expect(toasts.error).toHaveBeenCalledWith("The request could not be sent."));

    failure = { code: "refused", message: "Could not send the request: the agent is busy." };
    await user.click(within(card()).getByRole("button", { name: "Send" }));
    await waitFor(() => expect(toasts.error).toHaveBeenCalledWith("Could not send the request: the agent is busy."));

    failure = "boom";
    await user.click(within(card()).getByRole("button", { name: "Discard" }));
    await user.click(within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Discard" }));
    await waitFor(() => expect(toasts.error).toHaveBeenCalledWith("The request could not be discarded."));
  });

  it("lists the drafts as the backend sends them, one card each", async () => {
    await show([draft({ id: "2020-11-02T08-00-00", title: "First" }), draft({ title: "Second" })]);
    const region = screen.getByRole("region", { name: "Backlog" });
    expect([...region.querySelectorAll("h3")].map((h) => h.textContent)).toEqual(["First", "Second"]);
  });
});
