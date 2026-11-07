import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BoardSnapshot } from "@/bindings/BoardSnapshot";
import { proposalView } from "@/components/board/proposal-fixtures";
import { driftView } from "@/components/board/drift-fixtures";
import { ProjectContent } from "@/components/project-content";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/hooks/use-omp", () => ({ useOmp: () => ({ status: null }) }));
vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  onFilesDropped: vi.fn(() => Promise.resolve(() => {})),
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
  ],
  active: [],
  gathered: [],
  ghosts: [{ id: "g1", jobId: "job-1", kind: "proposal", title: "Coming", phase: "preparing", note: null }],
  runAllReasons: ["No Task is in To do"],
  doneSinceLastScan: 0,
  warnings: [],
};

beforeEach(() => {
  vi.mocked(on).mockImplementation((() => Promise.resolve(() => {})) as unknown as typeof on);
  vi.mocked(call).mockImplementation(((name: string) => {
    if (name === "board_snapshot") return Promise.resolve(snapshot);
    if (name === "general_snapshot") {
      const state = { projectId: PROJECT, working: false, starting: false, loading: false, kept: false };
      return Promise.resolve({ state, seq: 0, entries: [] });
    }
    if (name === "startup_failures") return Promise.resolve([]);
    if (name === "update_status") return Promise.resolve({ available: null, downloading: false });
    return Promise.reject(new Error(`unexpected command ${name}`));
  }) as unknown as typeof call);
});

describe("ProjectContent", () => {
  it("shows the Board and puts the number of its real Backlog cards in the sidebar: ghosts not counted", async () => {
    render(
      <TooltipProvider>
        <ProjectContent projectId={PROJECT} />
      </TooltipProvider>,
    );
    const backlog = await screen.findByRole("region", { name: "Backlog" });
    expect(within(backlog).getByRole("button", { name: "Add a cart" })).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Board, 2 cards in Backlog" })).toBeInTheDocument();
  });

  it("shows `1` on Chat in the sidebar while Chat waits for the answers of its round, and not once the round is closed", async () => {
    const standard = vi.mocked(call).getMockImplementation() as unknown as (name: string, args: unknown) => Promise<unknown>;
    const general = (waiting: boolean) => ({ projectId: PROJECT, working: true, starting: false, loading: false, kept: false, waiting });
    const handlers = new Map<string, (payload: unknown) => void>();
    vi.mocked(on).mockImplementation(((name: string, handler: (payload: unknown) => void) => {
      handlers.set(name, handler);
      return Promise.resolve(() => {});
    }) as unknown as typeof on);
    vi.mocked(call).mockImplementation(((name: string, args: unknown) =>
      name === "general_snapshot" ? Promise.resolve({ state: general(true), seq: 0, entries: [], arrivals: [] }) : standard(name, args)) as unknown as typeof call);
    render(
      <TooltipProvider>
        <ProjectContent projectId={PROJECT} />
      </TooltipProvider>,
    );
    expect(await screen.findByRole("button", { name: "Chat, 1 round waiting for your answers" })).toBeInTheDocument();
    act(() => handlers.get("general-changed")?.(general(false)));
    await waitFor(() => expect(screen.getByRole("button", { name: "Chat" })).toBeInTheDocument());
  });

  it("counts the drafts of requests in the sidebar, opens one in the editor in place of the Board and comes back", async () => {
    const id = "2026-11-02T09-30-00";
    const withDraft: BoardSnapshot = {
      ...snapshot,
      backlog: [...snapshot.backlog, { kind: "request", id, title: "Add a wishlist", created: "2026-11-02T09:30:00", empty: false, returned: false }],
    };
    const standard = vi.mocked(call).getMockImplementation() as unknown as (name: string, args: unknown) => Promise<unknown>;
    vi.mocked(call).mockImplementation(((name: string, args: unknown) => {
      if (name === "board_snapshot") return Promise.resolve(withDraft);
      if (name === "request_read") {
        return Promise.resolve({ id, text: "Add a wishlist", status: "writing", jobId: null, jobName: null, returned: false, modelEmpty: false });
      }
      if (name === "request_save" || name === "request_leave") return Promise.resolve(null);
      return standard(name, args);
    }) as unknown as typeof call);
    render(
      <TooltipProvider>
        <ProjectContent projectId={PROJECT} />
      </TooltipProvider>,
    );
    const backlog = await screen.findByRole("region", { name: "Backlog" });
    expect(await screen.findByRole("button", { name: "Board, 3 cards in Backlog" })).toBeInTheDocument();
    expect(screen.queryByText(/requests? (is|are) open/)).toBeNull();
    const user = userEvent.setup();
    await user.click(within(backlog).getByRole("button", { name: "Add a wishlist" }));
    expect(await screen.findByRole("heading", { level: 1, name: "Design request" })).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Request text" })).toBeInTheDocument();
    expect(backlog.closest("[hidden]")).not.toBeNull();
    await user.click(screen.getByRole("button", { name: "Board" }));
    expect(screen.queryByRole("heading", { level: 1, name: "Design request" })).toBeNull();
    expect(backlog.closest("[hidden]")).toBeNull();
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
