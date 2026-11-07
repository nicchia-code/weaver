import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { ShowRequest } from "@/bindings/ShowRequest";
import { useLeaveGuard } from "@/components/files/leave-guard";
import { ProjectWindow } from "@/components/project-window";
import { TooltipProvider } from "@/components/ui/tooltip";
import { call, on } from "@/lib/backend";

vi.mock("@/hooks/use-omp", () => ({ useOmp: () => ({ status: null }) }));
vi.mock("@/lib/backend", () => ({ call: vi.fn(), on: vi.fn() }));

const PROJECT = "0123456789abcdef";
let push: (request: ShowRequest) => void;

beforeEach(() => {
  vi.mocked(call).mockImplementation(((name: string) => Promise.resolve(name === "startup_failures" ? [] : null)) as never);
  vi.mocked(on).mockImplementation(((name: string, handler: (request: ShowRequest) => void) => {
    if (name === "show-requested") push = handler;
    return Promise.resolve(() => {});
  }) as unknown as typeof on);
});

function mount() {
  render(
    <TooltipProvider>
      <ProjectWindow projectId={PROJECT} />
    </TooltipProvider>,
  );
}

/** The entry that is selected: its name (the gear of Settings is named by its label, not by text). */
const shown = () => {
  const entry = screen.getByRole("button", { current: "page" });
  return entry.getAttribute("aria-label") ?? entry.textContent;
};

describe("a window asked to show something", () => {
  it("shows the view a notification was about", () => {
    mount();
    expect(shown()).toMatch(/^Board/);
    act(() => push({ projectId: PROJECT, view: "history", jobId: null }));
    expect(shown()).toMatch(/^History/);
    expect(screen.getByRole("heading", { level: 1, name: "History" }).closest("section")).not.toHaveAttribute("hidden");
  });

  it("opens the Settings view when the System Certificates notification is clicked", () => {
    mount();
    act(() => push({ projectId: PROJECT, view: "settings", jobId: null }));
    expect(shown()).toMatch(/^Settings/);
    expect(screen.getByRole("heading", { level: 1, name: "Settings" }).closest("section")).not.toHaveAttribute("hidden");
  });

  it("shows the System Certificates notice at the top and Open Settings shows the Settings view", async () => {
    vi.mocked(call).mockImplementation(((name: string) =>
      Promise.resolve(
        name === "system_certificates_status"
          ? {
              revision: 1,
              phase: "ready",
              automatic: true,
              count: 3,
              writtenAt: null,
              error: null,
              turnOnNotice: true,
              failureBanner: false, failureReason: null,
            }
          : name === "startup_failures"
            ? []
            : null,
      )) as never);
    mount();
    await act(async () => {});
    expect(shown()).toMatch(/^Board/);
    fireEvent.click(screen.getByRole("button", { name: "Open Settings" }));
    expect(shown()).toMatch(/^Settings/);
    expect(call).toHaveBeenCalledWith("system_certificates_dismiss_notice");
  });

  it("opens the conversation of the job in Jobs", () => {
    mount();
    act(() => push({ projectId: PROJECT, view: "jobs", jobId: "job-7" }));
    expect(shown()).toMatch(/^Jobs/);
  });

  it("ignores what was meant for another project", () => {
    mount();
    act(() => push({ projectId: "ffffffffffffffff", view: "history", jobId: null }));
    expect(shown()).toMatch(/^Board/);
  });
});

describe("the frame of the window", () => {
  it("stands on Paper, with the sidebar on it and the view in a Surface panel inset 8 px with a Line ring", () => {
    mount();
    const panel = document.querySelector<HTMLElement>("[data-work-surface]")!;
    expect(panel).toHaveClass("bg-surface", "m-2", "rounded-xl", "shadow-ring");
    expect(panel.parentElement).toHaveClass("bg-paper");
    const sidebar = screen.getByRole("navigation", { name: "Views" });
    expect(sidebar.parentElement).toBe(panel.parentElement);
    expect(sidebar.className).not.toMatch(/bg-|border-/);
    expect(panel).toContainElement(screen.getByRole("heading", { level: 1, name: "Board" }));
  });

  it("shows the banners of the window at the top of the work surface, above the header of the view", async () => {
    const failure = { projectId: null, projectName: null, step: "git", reason: "git is missing" };
    vi.mocked(call).mockImplementation(((name: string) => Promise.resolve(name === "startup_failures" ? [failure] : null)) as never);
    mount();
    const panel = document.querySelector<HTMLElement>("[data-work-surface]")!;
    const banner = await screen.findByRole("alert");
    expect(panel).toContainElement(banner);
    const heading = screen.getByRole("heading", { level: 1, name: "Board" });
    expect(banner.compareDocumentPosition(heading) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("shows its footer at the bottom of the work surface, in the layout under the view and outside the sidebar", () => {
    render(
      <TooltipProvider>
        <ProjectWindow projectId={PROJECT} footer={<div role="region" aria-label="Footer" />} />
      </TooltipProvider>,
    );
    const panel = document.querySelector<HTMLElement>("[data-work-surface]")!;
    const footer = screen.getByRole("region", { name: "Footer" });
    const main = panel.querySelector("main")!;
    // A sibling after the views, in the flow: the main area is what gets shorter.
    expect(footer.parentElement).toBe(panel);
    expect(main.nextElementSibling).toBe(footer);
    expect(main).not.toContainElement(footer);
    expect(main).toHaveClass("min-h-0", "flex-1");
    expect(screen.getByRole("navigation", { name: "Views" })).not.toContainElement(footer);
  });
});

describe("the region of a view", () => {
  it("is labelled by the visible title of the view, with no hidden heading repeating the name", () => {
    mount();
    const title = screen.getByRole("heading", { level: 1, name: "Board" });
    expect(title).not.toHaveClass("sr-only");
    expect(screen.getByRole("region", { name: "Board" })).toContainElement(title);
    expect(document.querySelectorAll("h1.sr-only")).toHaveLength(0);
  });
});

describe("the File view in a window", () => {
  it("is not kept when another entry is chosen, and the guard of the view decides whether the other entry is shown", () => {
    let decide: (proceed: () => void) => void = (proceed) => proceed();
    const Files = () => {
      useLeaveGuard((proceed) => decide(proceed));
      return <p>Files content</p>;
    };
    render(
      <TooltipProvider>
        <ProjectWindow projectId={PROJECT} initialView="files" views={{ files: <Files />, board: <p>Board content</p> }} />
      </TooltipProvider>,
    );
    expect(screen.getByText("Files content")).toBeInTheDocument();
    // The guard keeps the view: the other entry is not shown.
    decide = () => {};
    fireEvent.click(screen.getByRole("button", { name: "Board" }));
    expect(screen.getByText("Files content")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Files" })).toHaveAttribute("aria-current", "page");
    // The guard lets it go: the File view is unmounted, not hidden.
    decide = (proceed) => proceed();
    fireEvent.click(screen.getByRole("button", { name: "Board" }));
    expect(screen.queryByText("Files content")).not.toBeInTheDocument();
    expect(screen.getByText("Board content")).toBeInTheDocument();
  });
});
