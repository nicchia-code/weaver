import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { toast } from "sonner";
import { describe, expect, it, vi } from "vitest";

import type { ProjectGit } from "@/bindings/ProjectGit";
import { ProjectContext } from "@/components/project-context";
import { ProjectWindow } from "@/components/project-window";
import { Sidebar, initialsOf } from "@/components/sidebar";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ViewHeader } from "@/components/view-header";
import { ProjectGitProvider } from "@/hooks/use-project-git";
import { UsageProvider } from "@/hooks/use-usage";
import { call, on } from "@/lib/backend";
import { setViewportWidth } from "@/test/media";
import { VIEWS, type ShellCounts, type ViewId } from "@/views";

vi.mock("@/lib/backend", () => ({
  call: vi.fn((name: string) => Promise.resolve(name === "startup_failures" ? [] : null)),
  on: vi.fn(() => Promise.resolve(() => {})),
  isAppError: (error: unknown) => typeof error === "object" && error !== null && "message" in error && "code" in error,
}));

vi.mock("sonner", () => ({ toast: { error: vi.fn() }, Toaster: () => null }));

// The Usage dialog reads the status of omp: not ready here, so it says why and offers Open Settings.
vi.mock("@/hooks/use-omp", () => ({ useOmp: () => ({ status: { state: "missing", providers: [], models: [], version: null, reason: null } }) }));

const PROJECT = "0123456789abcdef";
const SUMMARY = "Usage: Usage needs the agent";
const NONE: ShellCounts = { backlog: 0, jobsWaiting: 0, jobsRunning: 0, chatCard: false };
const ORDER = ["Board", "Chat", "Jobs", "Requirements", "Specs", "Files", "Assets", "Notes", "History", "Settings"];
const NAMES = ["Board", "Chat", "Jobs", "Requirements", "Specs", "Files", "Assets", "Notes", "History"];

const context = { projectId: PROJECT, showView: () => {}, openJob: () => {}, openedJob: null, openedJobRequest: 0 };

function sidebarOf(counts: ShellCounts = NONE, selected: ViewId = "board", props: { onOpenPalette?: (origin: HTMLElement) => void; onSelect?: (id: ViewId) => void } = {}) {
  return (
    <TooltipProvider>
      <ProjectContext.Provider value={context}>
        <UsageProvider>
          <ProjectGitProvider projectId={PROJECT}>
            <Sidebar selected={selected} onSelect={props.onSelect ?? (() => {})} counts={counts} onOpenUsage={() => {}} onOpenPalette={props.onOpenPalette} />
          </ProjectGitProvider>
        </UsageProvider>
      </ProjectContext.Provider>
    </TooltipProvider>
  );
}

function renderSidebar(counts?: ShellCounts, selected?: ViewId, props?: Parameters<typeof sidebarOf>[2]) {
  return render(sidebarOf(counts, selected, props));
}

const git = (patch: Partial<ProjectGit> = {}): ProjectGit => ({ projectId: PROJECT, name: "semplifica", revision: 1, branch: "main", userName: "Luca Soda", ...patch });

/** The backend knows the git state of the project. */
function knowGit(value: ProjectGit) {
  vi.mocked(call).mockImplementation(((name: string) => Promise.resolve(name === "project_git" ? value : name === "startup_failures" ? [] : null)) as never);
}

/** What each button of the sidebar is called: its accessible label, else its text. */
const names = (container: HTMLElement) => within(container).getAllByRole("button").map((button) => button.getAttribute("aria-label") ?? button.textContent);
const entry = (name: string) => screen.getByRole("button", { name });

const navigation = () => screen.getByRole("navigation", { name: "Views" });

describe("sidebar", () => {
  it("lists the project switcher and Search or run…, Board, Chat, Jobs, the PROJECT group, the summary of Usage and Settings in this order", () => {
    expect(VIEWS.map((v) => v.label)).toEqual(ORDER);
    renderSidebar();
    expect(names(navigation())).toEqual(["Project", "Search or run…", ...NAMES, SUMMARY, "Settings"]);
  });

  it("is 248 px wide, padded 14 px 12 px 12 px, from the top to the bottom of the window", () => {
    renderSidebar();
    expect(navigation()).toHaveClass("w-[248px]", "h-full", "flex-col", "px-3", "pb-3", "pt-3.5");
    expect(navigation()).toHaveAttribute("data-compact", "false");
    expect(navigation().className).not.toMatch(/\bbg-|\bborder/);
  });

  it("puts Board, Chat and Jobs, then the group headed PROJECT with the others, the free space, the summary of Usage and the user row", () => {
    renderSidebar();
    const lists = within(navigation()).getAllByRole("list");
    expect(lists).toHaveLength(2);
    expect(within(lists[0] as HTMLElement).getAllByRole("button").map((b) => b.textContent)).toEqual(["Board", "Chat", "Jobs"]);
    const project = within(navigation()).getByRole("group", { name: "Project" });
    expect(within(project).getByText("PROJECT")).toHaveClass("text-[11px]", "font-medium", "uppercase", "text-muted-foreground");
    expect(within(project).getAllByRole("button").map((b) => b.textContent)).toEqual(["Requirements", "Specs", "Files", "Assets", "Notes", "History"]);
    // Files, Assets and Notes keep their icons.
    expect(within(project).getByRole("button", { name: "Assets" }).querySelector("svg")).toHaveClass("lucide-paperclip");
    expect(within(project).getByRole("button", { name: "Notes" }).querySelector("svg")).toHaveClass("lucide-notebook-pen");
    // The free space pushes the summary and the user row to the bottom.
    const space = navigation().querySelector(":scope > .flex-1") as HTMLElement;
    const summary = screen.getByRole("button", { name: SUMMARY });
    const settings = entry("Settings");
    expect(project.compareDocumentPosition(space) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(space.compareDocumentPosition(summary) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(summary.compareDocumentPosition(settings) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    // Usage is no longer an entry of the lists, and Settings is not in them either.
    expect(within(navigation()).queryByRole("list", { name: /usage/i })).not.toBeInTheDocument();
    for (const list of lists) expect(within(list as HTMLElement).queryByRole("button", { name: "Settings" })).not.toBeInTheDocument();
  });

  it("stands on Paper with no border; entries are 34 px, radius 9 px, Ink 2; the chosen one is on Surface with a Line ring, in Ink and medium weight", () => {
    renderSidebar(undefined, "specs");
    const board = entry("Board");
    expect(board).toHaveClass("h-[34px]", "rounded-md", "text-ink-2");
    expect(board).not.toHaveAttribute("aria-current");
    expect(board).not.toHaveClass("bg-surface", "shadow-ring");
    const specs = entry("Specs");
    expect(specs).toHaveAttribute("aria-current", "page");
    expect(specs).toHaveClass("h-[34px]", "bg-surface", "shadow-ring", "text-foreground", "font-medium");
    expect(entry("Settings")).not.toHaveAttribute("aria-current");
  });

  it("makes Settings a view whose gear carries aria-current and the Surface look while it is shown", async () => {
    const onSelect = vi.fn();
    const { rerender } = renderSidebar(NONE, "board", { onSelect });
    await userEvent.setup().click(entry("Settings"));
    expect(onSelect).toHaveBeenCalledWith("settings");
    rerender(sidebarOf(NONE, "settings", { onSelect }));
    const gear = entry("Settings");
    expect(gear).toHaveAttribute("aria-current", "page");
    expect(gear).toHaveClass("bg-surface", "shadow-ring");
    expect(gear.querySelector("svg")).toHaveClass("lucide-settings");
    // Only the gear is the chosen entry.
    expect(within(navigation()).getAllByRole("button").filter((b) => b.getAttribute("aria-current") === "page")).toEqual([gear]);
  });

  it("puts the summary of Usage right above the user row: a button that opens Usage and is never the selected entry", async () => {
    const onOpenUsage = vi.fn();
    render(
      <TooltipProvider>
        <ProjectContext.Provider value={context}>
          <UsageProvider>
            <ProjectGitProvider projectId={PROJECT}>
              <Sidebar selected="settings" onSelect={() => {}} counts={NONE} onOpenUsage={onOpenUsage} />
            </ProjectGitProvider>
          </UsageProvider>
        </ProjectContext.Provider>
      </TooltipProvider>,
    );
    const usage = screen.getByRole("button", { name: SUMMARY });
    expect(usage).not.toHaveAttribute("aria-current");
    await userEvent.setup().click(usage);
    expect(onOpenUsage).toHaveBeenCalledWith(usage);
  });

  it("shows the number of real cards in Backlog on Board, muted, and the waiting jobs in Peach on Jobs, and nothing at 0", () => {
    const { rerender } = renderSidebar();
    expect(within(navigation()).queryByText(/^\d+$/)).not.toBeInTheDocument();

    rerender(sidebarOf({ ...NONE, backlog: 4, jobsWaiting: 2 }));
    const board = screen.getByRole("button", { name: /^Board/ });
    expect(board).toHaveTextContent("4");
    expect(board).toHaveAccessibleName("Board, 4 cards in Backlog");
    const jobs = screen.getByRole("button", { name: /^Jobs/ });
    expect(jobs).toHaveAccessibleName("Jobs, 2 jobs waiting for you");
    expect(within(jobs).getByText("2")).toHaveClass("text-peach-ink", "ml-auto");
    expect(within(board).getByText("4")).toHaveClass("text-muted-foreground", "ml-auto");
    expect(within(board).getByText("4")).not.toHaveClass("text-peach-ink");
    for (const other of ["Chat", "Requirements", "Specs", "Files", "History"]) {
      expect(entry(other)).not.toHaveTextContent(/\d/);
    }
  });

  it("shows `1` in Peach on Chat, with the tooltip `Chat has questions for you`, while a card of questions is active, and nothing otherwise", async () => {
    const { rerender } = renderSidebar();
    expect(entry("Chat")).not.toHaveTextContent(/\d/);

    rerender(sidebarOf({ ...NONE, chatCard: true }));
    const chat = screen.getByRole("button", { name: /^Chat/ });
    expect(chat).toHaveAccessibleName("Chat, 1 card of questions waiting for your answers");
    expect(within(chat).getByText("1")).toHaveClass("text-peach-ink", "ml-auto");
    for (const other of ["Board", "Jobs"]) expect(entry(other)).not.toHaveTextContent(/\d/);
    await userEvent.setup().hover(chat);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Chat has questions for you");
  });

  it("shows a Sky pill with a dot and `<n> running` on Jobs when none waits and some are running", () => {
    const { rerender } = renderSidebar({ ...NONE, jobsRunning: 3 });
    const jobs = screen.getByRole("button", { name: /^Jobs/ });
    expect(jobs).toHaveAccessibleName("Jobs, 3 jobs running");
    const pill = within(jobs).getByText("3 running");
    expect(pill).toHaveClass("rounded-full", "bg-sky-tint", "text-sky-ink");
    expect(pill.firstElementChild).toHaveClass("size-1.5", "rounded-full", "bg-sky");

    rerender(sidebarOf({ ...NONE, jobsRunning: 1 }));
    expect(screen.getByRole("button", { name: /^Jobs/ })).toHaveAccessibleName("Jobs, 1 job running");
    expect(screen.getByText("1 running")).toBeInTheDocument();

    // The jobs waiting for the developer come first: the pill shows only when none waits.
    rerender(sidebarOf({ ...NONE, jobsWaiting: 2, jobsRunning: 3 }));
    expect(screen.getByRole("button", { name: /^Jobs/ })).toHaveAccessibleName("Jobs, 2 jobs waiting for you");
    expect(screen.queryByText(/running$/)).not.toBeInTheDocument();
  });

  it("shrinks to 56 px with icons only below 1024 px, the names kept as accessible names and tooltips", async () => {
    renderSidebar({ ...NONE, backlog: 4 });
    expect(navigation()).toHaveClass("w-[248px]");

    act(() => setViewportWidth(1023));
    expect(navigation()).toHaveClass("w-[56px]");
    expect(navigation()).toHaveAttribute("data-compact", "true");
    // The same entries, now named by hidden text; the number is a small badge on the icon.
    for (const name of NAMES) {
      const button = screen.getByRole("button", { name: new RegExp(`^${name}`) });
      expect(within(button).getByText(name)).toHaveClass("sr-only");
    }
    const board = screen.getByRole("button", { name: /^Board/ });
    expect(board).toHaveAccessibleName("Board, 4 cards in Backlog");
    expect(within(board).getByText("4")).toHaveClass("absolute", "right-1", "top-0.5");

    // The `PROJECT` heading is a separator.
    expect(screen.queryByText("PROJECT")).not.toBeInTheDocument();
    expect(within(navigation()).getByRole("separator")).toBeInTheDocument();

    const user = userEvent.setup();
    await user.hover(entry("Specs"));
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Specs");
    act(() => setViewportWidth(1024));
    expect(navigation()).toHaveClass("w-[248px]");
  });

  it("keeps the gear with its name as tooltip, and hides the avatar and the name of the user, with icons only", async () => {
    knowGit(git());
    renderSidebar();
    expect(await screen.findByText("Luca Soda")).toBeInTheDocument();
    act(() => setViewportWidth(1023));
    expect(screen.queryByText("Luca Soda")).not.toBeInTheDocument();
    expect(screen.queryByText("LS")).not.toBeInTheDocument();
    await userEvent.setup().hover(entry("Settings"));
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Settings");
    act(() => setViewportWidth(1024));
  });
});

describe("Search or run…", () => {
  const setPlatform = (platform: string) => Object.defineProperty(navigator, "platform", { value: platform, configurable: true });
  const resetPlatform = () => delete (navigator as unknown as { platform?: string }).platform;

  it("is a 34 px Surface button with a Line border, the Search icon, the text in Muted and the key hint Ctrl K outside macOS", () => {
    renderSidebar();
    const search = entry("Search or run…");
    expect(search).toHaveClass("h-[34px]", "bg-surface", "border", "border-line", "text-muted-foreground");
    expect(search.querySelector("svg")).toHaveClass("lucide-search");
    expect(within(search).getByText("Search or run…")).toBeInTheDocument();
    const hint = within(search).getByText("Ctrl K");
    expect(hint.tagName).toBe("KBD");
    expect(hint).toHaveClass("font-mono", "bg-paper");
  });

  it("shows ⌘K on macOS", () => {
    setPlatform("MacIntel");
    try {
      renderSidebar();
      expect(within(entry("Search or run…")).getByText("⌘K")).toBeInTheDocument();
    } finally {
      resetPlatform();
    }
  });

  it("opens the palette with its button as the place to give the focus back to", async () => {
    const onOpenPalette = vi.fn();
    renderSidebar(NONE, "board", { onOpenPalette });
    await userEvent.setup().click(entry("Search or run…"));
    expect(onOpenPalette).toHaveBeenCalledWith(entry("Search or run…"));
  });

  it("is an icon button with the tooltip `Search or run… (Ctrl+K)`, or `(⌘K)` on macOS, with icons only", async () => {
    renderSidebar();
    act(() => setViewportWidth(1023));
    const search = entry("Search or run…");
    expect(search.querySelector("svg")).toHaveClass("lucide-search");
    expect(search.textContent).toBe("");
    await userEvent.setup().hover(search);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Search or run… (Ctrl+K)");
    act(() => setViewportWidth(1024));
  });

  it("says ⌘K in the tooltip on macOS", async () => {
    setPlatform("MacIntel");
    try {
      renderSidebar();
      act(() => setViewportWidth(1023));
      await userEvent.setup().hover(entry("Search or run…"));
      expect(await screen.findByRole("tooltip")).toHaveTextContent("Search or run… (⌘K)");
    } finally {
      resetPlatform();
      act(() => setViewportWidth(1024));
    }
  });
});

describe("the user row", () => {
  it("shows a round 28 px avatar with the initials, the name and the Settings button at its right end", async () => {
    knowGit(git());
    renderSidebar();
    const name = await screen.findByText("Luca Soda");
    expect(name).toHaveClass("text-[13px]", "text-ink-2", "truncate");
    const avatar = screen.getByText("LS");
    expect(avatar).toHaveClass("size-7", "rounded-full", "bg-paper", "text-ink-2", "text-[11px]", "font-semibold");
    expect(avatar).toHaveAttribute("aria-hidden", "true");
    const row = name.parentElement as HTMLElement;
    expect(row.lastElementChild).toBe(entry("Settings"));
    expect(entry("Settings")).toHaveAccessibleName("Settings");
  });

  it("shows only the Settings button, at its right end, when git has no user name", async () => {
    knowGit(git({ userName: null }));
    renderSidebar();
    await waitFor(() => expect(call).toHaveBeenCalledWith("project_git"));
    const gear = entry("Settings");
    const row = gear.parentElement as HTMLElement;
    expect(row).toHaveClass("justify-end");
    expect(row.children).toHaveLength(1);
    expect(screen.queryByText("LS")).not.toBeInTheDocument();
  });

  it("writes the initials of a name: first and last word, or the one letter of a single word", () => {
    expect(initialsOf("Luca Soda")).toBe("LS");
    expect(initialsOf("  ada   lovelace ")).toBe("AL");
    expect(initialsOf("Mary Ann Smith")).toBe("MS");
    expect(initialsOf("Cher")).toBe("C");
    expect(initialsOf("Émile Zola")).toBe("ÉZ");
    expect(initialsOf("   ")).toBe("");
  });
});

describe("the project switcher button", () => {
  it("shows the logo, the folder's name and the branch, and is named `Project <name>, branch <branch>`", async () => {
    knowGit(git({ name: "semplifica", branch: "feature/sidebar" }));
    renderSidebar();
    const button = await screen.findByRole("button", { name: "Project semplifica, branch feature/sidebar" });
    expect(button.querySelector("svg[aria-hidden='true']")).toBeInTheDocument();
    expect(within(button).getByText("semplifica")).toHaveClass("text-sm", "font-semibold", "truncate");
    expect(within(button).getByText("feature/sidebar")).toHaveClass("text-xs", "text-muted-foreground");
    expect(button.querySelector(".lucide-chevrons-up-down")).toBeInTheDocument();
  });

  it("shows no branch line when the folder is not in a repository or git cannot run", async () => {
    knowGit(git({ branch: null }));
    renderSidebar();
    const button = await screen.findByRole("button", { name: "Project semplifica" });
    expect(within(button).queryByText("main")).not.toBeInTheDocument();
    expect(button.querySelectorAll("span.text-xs")).toHaveLength(0);
  });

  it("follows the branch as soon as the backend reads a new one, and ignores another project's value", async () => {
    let listener: ((value: ProjectGit) => void) | undefined;
    vi.mocked(on).mockImplementation(((name: string, handler: (value: ProjectGit) => void) => {
      if (name === "project-git-changed") listener = handler;
      return Promise.resolve(() => {});
    }) as never);
    knowGit(git({ revision: 1, branch: "main" }));
    renderSidebar();
    await screen.findByRole("button", { name: "Project semplifica, branch main" });
    act(() => listener?.(git({ revision: 2, branch: "release" })));
    expect(screen.getByRole("button", { name: "Project semplifica, branch release" })).toBeInTheDocument();
    // An older value, and the value of another project, change nothing.
    act(() => listener?.(git({ revision: 1, branch: "old" })));
    act(() => listener?.(git({ projectId: "other", revision: 9, branch: "elsewhere" })));
    expect(screen.getByRole("button", { name: "Project semplifica, branch release" })).toBeInTheDocument();
    vi.mocked(on).mockImplementation((() => Promise.resolve(() => {})) as never);
  });

  it("shows only the logo with icons only, with the same name", async () => {
    knowGit(git());
    renderSidebar();
    act(() => setViewportWidth(1023));
    const button = await screen.findByRole("button", { name: "Project semplifica, branch main" });
    expect(button.textContent).toBe("");
    expect(button.querySelector("svg[aria-hidden='true']")).toBeInTheDocument();
    act(() => setViewportWidth(1024));
  });
});

function Counter() {
  const [count, setCount] = useState(0);
  return <button onClick={() => setCount(count + 1)}>Clicked {count}</button>;
}

describe("project window", () => {
  it("keeps the state of a view when another entry is selected and back", async () => {
    const user = userEvent.setup();
    render(
      <TooltipProvider>
        <ProjectWindow
          projectId="0123456789abcdef"
          views={{
            board: <Counter />,
            jobs: (
              <>
                <ViewHeader view="jobs" />
                <p>jobs content</p>
              </>
            ),
          }}
        />
      </TooltipProvider>,
    );
    await user.click(screen.getByRole("button", { name: "Clicked 0" }));
    await user.click(screen.getByRole("button", { name: "Clicked 1" }));

    await user.click(screen.getByRole("button", { name: "Jobs" }));
    expect(screen.getByText("jobs content")).toBeVisible();
    expect(screen.queryByRole("button", { name: /Clicked/ })).not.toBeInTheDocument(); // hidden from the tree
    expect(screen.getByRole("heading", { name: "Jobs" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Board" }));
    expect(screen.getByRole("button", { name: "Clicked 2" })).toBeVisible();
    expect(screen.queryByText("jobs content")).not.toBeVisible();
  });

  it("opens Settings from the bottom of the sidebar and keeps the other views' state", async () => {
    const user = userEvent.setup();
    render(
      <TooltipProvider>
        <ProjectWindow projectId="0123456789abcdef" views={{ board: <Counter />, settings: <p>settings content</p> }} />
      </TooltipProvider>,
    );
    await user.click(screen.getByRole("button", { name: "Clicked 0" }));

    await user.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByText("settings content")).toBeVisible();
    expect(screen.getByRole("button", { name: "Settings" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("button", { name: "Board" })).not.toHaveAttribute("aria-current");

    await user.click(screen.getByRole("button", { name: "Board" }));
    expect(screen.getByRole("button", { name: "Clicked 1" })).toBeVisible();
  });

  it("starts on Board", () => {
    render(
      <TooltipProvider>
        <ProjectWindow projectId="0123456789abcdef" />
      </TooltipProvider>,
    );
    expect(screen.getByRole("button", { name: "Board" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("heading", { name: "Board" })).toBeInTheDocument();
  });
});

describe("the summary of Usage in a project window", () => {
  // Omp is not ready in these tests, and no limits were read: the summary says Usage needs the agent.
  const renderWindow = () =>
    render(
      <TooltipProvider>
        <ProjectWindow projectId="0123456789abcdef" views={{ board: <p>board content</p>, settings: <p>settings content</p> }} />
      </TooltipProvider>,
    );

  it("sits right above the user row, at the bottom of the sidebar, and is one button named after its text", () => {
    renderWindow();
    expect(names(navigation()).slice(-2)).toEqual([SUMMARY, "Settings"]);
    expect(screen.queryByRole("button", { name: "Usage" })).not.toBeInTheDocument();
  });

  it("is never the selected entry and never has aria-current, even when it is clicked", async () => {
    renderWindow();
    const usage = screen.getByRole("button", { name: SUMMARY });
    expect(usage).not.toHaveAttribute("aria-current");
    await userEvent.setup().click(usage);
    // The dialog is open: the rest of the window is hidden from the accessibility tree.
    expect(usage).not.toHaveAttribute("aria-current");
    expect(screen.getAllByRole("button", { current: "page", hidden: true })).toHaveLength(1);
  });

  it("is a Gauge button whose tooltip and accessible name say its text when the sidebar shows icons only", async () => {
    renderWindow();
    act(() => setViewportWidth(1023));
    const user = userEvent.setup();
    const usage = screen.getByRole("button", { name: SUMMARY });
    expect(usage.querySelector("svg")).toHaveClass("lucide-gauge");
    expect(usage.textContent).toBe("");
    await user.hover(usage);
    expect(await screen.findByRole("tooltip")).toHaveTextContent(SUMMARY);
    act(() => setViewportWidth(1024));
  });

  it("gives Settings its name as tooltip when the sidebar shows icons only", async () => {
    renderWindow();
    act(() => setViewportWidth(1023));
    const user = userEvent.setup();
    await user.hover(screen.getByRole("button", { name: "Settings" }));
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Settings");
    act(() => setViewportWidth(1024));
  });

  it("opens the dialog over the view shown without changing the view, and the focus comes back to the summary", async () => {
    const user = userEvent.setup();
    renderWindow();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: SUMMARY }));
    expect(await screen.findByRole("dialog", { name: "Usage" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Board", hidden: true })).toHaveAttribute("aria-current", "page");
    expect(screen.getByText("board content")).toBeInTheDocument();

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: SUMMARY })).toHaveFocus();
    expect(screen.getByRole("button", { name: "Board" })).toHaveAttribute("aria-current", "page");
  });

  it("shows Settings and closes the dialog with Open Settings while omp is not ready", async () => {
    const user = userEvent.setup();
    renderWindow();
    await user.click(screen.getByRole("button", { name: SUMMARY }));
    await user.click(await screen.findByRole("button", { name: "Open Settings" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Settings" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByText("settings content")).toBeVisible();
  });
});

describe("the error toast of a window that cannot go to the background", () => {
  it("tells the reason, and leaves the window as it was", () => {
    type Failure = { reason: string };
    let failed: ((failure: Failure) => void) | undefined;
    vi.mocked(on).mockImplementation(((name: string, listener: (failure: Failure) => void) => {
      if (name === "window-hide-failed") failed = listener;
      return Promise.resolve(() => {});
    }) as never);
    vi.mocked(toast.error).mockClear();
    render(
      <TooltipProvider>
        <ProjectWindow projectId="0123456789abcdef" views={{ board: <Counter />, settings: <p>settings content</p> }} />
      </TooltipProvider>,
    );
    expect(failed).toBeDefined();
    expect(toast.error).not.toHaveBeenCalled();

    act(() => failed?.({ reason: "no window system" }));
    expect(toast.error).toHaveBeenCalledWith("Could not hide the window.", { description: "no window system" });
    expect(screen.getByRole("button", { name: "Board" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("button", { name: "Clicked 0" })).toBeVisible();

    vi.mocked(on).mockImplementation((() => Promise.resolve(() => {})) as never);
  });
});

describe("the error toast of an opening chosen from the File menu of a project window", () => {
  type Failure = { name: string | null; reason: string };
  function renderListening() {
    let failed: ((failure: Failure) => void) | undefined;
    vi.mocked(on).mockImplementation(((name: string, listener: (failure: Failure) => void) => {
      if (name === "menu-open-failed") failed = listener;
      return Promise.resolve(() => {});
    }) as never);
    vi.mocked(toast.error).mockClear();
    render(
      <TooltipProvider>
        <ProjectWindow projectId="0123456789abcdef" views={{ board: <Counter />, settings: <p>settings content</p> }} />
      </TooltipProvider>,
    );
    expect(failed).toBeDefined();
    return failed!;
  }

  it("names the project that could not open, with the reason as its description", () => {
    const failed = renderListening();
    act(() => failed({ name: "alpha", reason: "the folder does not exist" }));
    expect(toast.error).toHaveBeenCalledWith("Could not open alpha.", { description: "the folder does not exist" });
    expect(screen.getByRole("button", { name: "Board" })).toHaveAttribute("aria-current", "page");
    vi.mocked(on).mockImplementation((() => Promise.resolve(() => {})) as never);
  });

  it("says the folder could not be opened when the picker itself failed", () => {
    const failed = renderListening();
    act(() => failed({ name: null, reason: "no display" }));
    expect(toast.error).toHaveBeenCalledWith("Could not open the folder.", { description: "no display" });
    vi.mocked(on).mockImplementation((() => Promise.resolve(() => {})) as never);
  });
});
