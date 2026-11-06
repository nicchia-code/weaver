import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BoardSnapshot } from "@/bindings/BoardSnapshot";
import { proposalView } from "@/components/board/proposal-fixtures";
import { driftView } from "@/components/board/drift-fixtures";
import { ProjectContent } from "@/components/project-content";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const PROJECT = "0123456789abcdef";

const head = (id: string, shortNumber: string, title: string) => ({
  id,
  shortNumber,
  title,
  body: "",
  created: "2026-11-02T07:00:00.000Z",
  agentFinishing: false,
});

const snapshot: BoardSnapshot = {
  projectId: PROJECT,
  revision: 1,
  backlog: [
    { kind: "proposal", head: head("pppppppp", "P1", "Add a cart"), standing: "open", proposalKind: "change", view: proposalView() },
    { kind: "drift", head: head("dddddddd", "D1", "Cart total"), origin: null, view: driftView() },
    // A Draft the agent of Chat is writing has its card, but it is not a real card of Backlog.
    { kind: "proposal", head: head("cccccccc", "P2", "Written in Chat"), standing: "chat-draft", proposalKind: "change", view: proposalView() },
  ],
  active: [],
  gathered: [],
  ghosts: [{ id: "g1", jobId: "job-1", kind: "proposal", title: "Coming", phase: "preparing", note: null }],
  runAllReasons: ["No Task is in To do"],
  doneSinceLastScan: 0,
  warnings: [],
};

// Whether the Chat of the project waits for the answers of its round, as its snapshot tells.
let chatWaiting = false;

beforeEach(() => {
  chatWaiting = false;
  vi.mocked(on).mockImplementation((() => Promise.resolve(() => {})) as unknown as typeof on);
  vi.mocked(call).mockImplementation(((name: string) => {
    if (name === "board_snapshot") return Promise.resolve(snapshot);
    if (name === "general_snapshot") {
      const state = { projectId: PROJECT, working: chatWaiting, starting: false, loading: false, kept: false, waiting: chatWaiting };
      return Promise.resolve({ state, chatDrafts: 0, seq: 0, entries: [] });
    }
    if (name === "startup_failures") return Promise.resolve([]);
    if (name === "update_status") return Promise.resolve({ available: null, downloading: false });
    return Promise.reject(new Error(`unexpected command ${name}`));
  }) as unknown as typeof call);
});

describe("ProjectContent", () => {
  it("shows the Board and puts the number of its real Backlog cards in the sidebar: ghosts and the Draft cards of Chat not counted", async () => {
    render(
      <TooltipProvider>
        <ProjectContent projectId={PROJECT} />
      </TooltipProvider>,
    );
    const backlog = await screen.findByRole("region", { name: "Backlog" });
    expect(within(backlog).getByRole("button", { name: "Add a cart" })).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Board, 2 cards in Backlog" })).toBeInTheDocument();
  });

  it("puts 1 on the Chat entry while the Chat waits for the answers of its round, and nothing on Jobs", async () => {
    chatWaiting = true;
    render(
      <TooltipProvider>
        <ProjectContent projectId={PROJECT} />
      </TooltipProvider>,
    );
    expect(await screen.findByRole("button", { name: "Chat, 1 round waiting for your answers" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Jobs" })).not.toHaveTextContent(/\d/);
  });

  it("opens the conversation of a ghost's job in Jobs", async () => {
    render(
      <TooltipProvider>
        <ProjectContent projectId={PROJECT} />
      </TooltipProvider>,
    );
    const backlog = await screen.findByRole("region", { name: "Backlog" });
    await userEvent.setup().click(within(backlog).getByRole("button", { name: /being prepared by the agent: Coming/ }));
    expect(screen.getByRole("heading", { level: 1, name: "Jobs" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1, name: "Jobs" }).closest("section")).not.toHaveAttribute("hidden");
  });
});
