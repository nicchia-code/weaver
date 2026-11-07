import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { BoardCard } from "@/bindings/BoardCard";
import { RequestsProvider, useRequests } from "@/components/requests/use-requests";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (value: unknown) => typeof value === "object" && value !== null && "code" in value && "message" in value,
}));

const toasts = vi.hoisted(() => ({ error: vi.fn() }));
vi.mock("sonner", () => ({ toast: { error: toasts.error } }));

const PROJECT = "0123456789abcdef";

const draft = (id: string): BoardCard => ({ kind: "request", id, title: id, created: "2026-11-02T09:30:00", empty: false, returned: false });
const drift = (id: string): BoardCard => ({
  kind: "drift",
  head: { id, shortNumber: "D1", title: "A drift", body: "", created: "2026-11-02T07:00:00.000Z", agentFinishing: false },
  origin: null,
  view: {} as never,
});

/** The Backlog of the Board as the backend sends it: Proposals and Drifts, then the drafts, oldest first. */
let backlog: BoardCard[];

beforeEach(() => {
  backlog = [];
  vi.mocked(on).mockResolvedValue(() => {});
  vi.mocked(call).mockReset();
  toasts.error.mockReset();
  vi.mocked(call).mockImplementation((async (name: string) => {
    switch (name) {
      case "board_snapshot":
        return { projectId: PROJECT, revision: 1, backlog, active: [], ghosts: [], gathered: [], runAllReasons: [], doneSinceLastScan: 0, warnings: [] };
      case "request_new":
        return "2026-11-02T10-00-00";
      default:
        throw new Error(`unexpected command ${name}`);
    }
  }) as unknown as typeof call);
});

function mount() {
  const wrapper = ({ children }: { children: ReactNode }) => <RequestsProvider projectId={PROJECT}>{children}</RequestsProvider>;
  return renderHook(() => useRequests(), { wrapper });
}

const created = () => vi.mocked(call).mock.calls.filter(([name]) => name === "request_new");

describe("Start from Scratch", () => {
  it("opens the oldest request being written and creates none", async () => {
    backlog = [drift("dddddddd"), draft("a"), draft("c")];
    const { result } = mount();
    act(() => result.current.startFromScratch());
    await waitFor(() => expect(result.current.editing).toBe(true));
    expect(result.current.selected).toBe("a");
    expect(result.current.origin).toEqual({ kind: "board" });
    expect(created()).toHaveLength(0);
  });

  it("creates a new request and opens it when none is being written", async () => {
    backlog = [drift("dddddddd")];
    const { result } = mount();
    act(() => result.current.startFromScratch());
    await waitFor(() => expect(result.current.editing).toBe(true));
    expect(created()).toHaveLength(1);
    expect(result.current.selected).toBe("2026-11-02T10-00-00");
  });

  it("looks at the Board as it is now", async () => {
    const { result } = mount();
    act(() => result.current.startFromScratch());
    await waitFor(() => expect(result.current.editing).toBe(true));
    expect(created()).toHaveLength(1);
    act(() => result.current.closeEditor());
    backlog = [draft("a")];
    act(() => result.current.startFromScratch());
    await waitFor(() => expect(result.current.selected).toBe("a"));
    expect(created()).toHaveLength(1);
  });
});

describe("New Request", () => {
  it("always creates a request, even when others are being written", async () => {
    backlog = [draft("a")];
    const { result } = mount();
    act(() => result.current.newRequest());
    await waitFor(() => expect(result.current.editing).toBe(true));
    expect(created()).toHaveLength(1);
    expect(result.current.selected).toBe("2026-11-02T10-00-00");
  });
});

describe("opening the editor", () => {
  it("shows a draft from the Board, and a sent request from its job with the way back to it", () => {
    const { result } = mount();
    act(() => result.current.openEditor("a"));
    expect(result.current).toMatchObject({ editing: true, selected: "a", origin: { kind: "board" } });
    act(() => result.current.openEditor("b", { kind: "jobs", jobId: "jjjjjjjj" }));
    expect(result.current).toMatchObject({ editing: true, selected: "b", origin: { kind: "jobs", jobId: "jjjjjjjj" } });
    act(() => result.current.closeEditor());
    expect(result.current.editing).toBe(false);
  });
});
