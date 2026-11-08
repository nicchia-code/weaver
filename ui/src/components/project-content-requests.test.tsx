import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BoardSnapshot } from "@/bindings/BoardSnapshot";
import { ProjectContent } from "@/components/project-content";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/hooks/use-omp", () => ({ useOmp: () => ({ status: null }) }));
vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));
// What the Board draws is not under test here, only the number the sidebar takes from its Backlog.
vi.mock("@/components/board/board-view", () => ({ BoardView: () => null }));

const PROJECT = "0123456789abcdef";

const snapshot: BoardSnapshot = {
  projectId: PROJECT,
  revision: 1,
  backlog: [
    { kind: "request", id: "rrrrrrrr", title: "Dark mode", created: "2026-11-02T07:00:00", empty: false, returned: false },
    { kind: "request", id: "ssssssss", title: "Untitled request", created: "2026-11-02T07:00:01", empty: true, returned: false },
  ],
  active: [],
  gathered: [],
  ghosts: [{ id: "g1", jobId: "job-1", kind: "proposal", title: "Coming", phase: "preparing", note: null }],
  runAllReasons: [],
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

describe("ProjectContent with drafts of requests", () => {
  it("counts the drafts among the cards of Backlog: with only drafts the number is there, the ghost is not counted", async () => {
    render(
      <TooltipProvider>
        <ProjectContent projectId={PROJECT} />
      </TooltipProvider>,
    );
    expect(await screen.findByRole("button", { name: "Board, 2 cards in Backlog" })).toBeInTheDocument();
  });
});
