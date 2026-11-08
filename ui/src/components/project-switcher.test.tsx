import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { toast } from "sonner";

import type { ProjectGit } from "@/bindings/ProjectGit";
import type { ProjectSwitcher as Switcher } from "@/bindings/ProjectSwitcher";
import { ProjectContext } from "@/components/project-context";
import { ProjectSwitcher, cutMiddle, filterProjects } from "@/components/project-switcher";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ProjectGitProvider } from "@/hooks/use-project-git";
import { call, on } from "@/lib/backend";

vi.mock("@/lib/backend", () => ({
  call: vi.fn(),
  on: vi.fn(),
  isAppError: (error: unknown) => typeof error === "object" && error !== null && "message" in error && "code" in error,
}));
vi.mock("sonner", () => ({ toast: { error: vi.fn() } }));

const CURRENT = "aaaaaaaaaaaaaaaa";
const GIT: ProjectGit = { projectId: CURRENT, name: "alpha", revision: 1, branch: "main", userName: null };

const open = (id: string, name: string, working = 0) => ({ id, path: `/work/${name}`, name, working });
const recent = (name: string, exists = true) => ({ path: `/old/${name}`, name, opened: "2026-11-02T07:00:00.000Z", exists });

const LIST: Switcher = {
  open: [open("bbbbbbbbbbbbbbbb", "beta", 2), open(CURRENT, "alpha"), open("cccccccccccccccc", "Gamma", 1)],
  recent: [recent("delta"), recent("Echo-gone", false)],
};

let list: Switcher;
let handlers: Record<string, (payload: never) => void>;
const answers: Record<string, () => Promise<unknown>> = {};

beforeEach(() => {
  list = LIST;
  handlers = {};
  for (const key of Object.keys(answers)) delete answers[key];
  vi.mocked(toast.error).mockClear();
  vi.mocked(on).mockImplementation(((name: string, handler: (payload: never) => void) => {
    handlers[name] = handler;
    return Promise.resolve(() => {});
  }) as never);
  vi.mocked(call).mockImplementation(((name: string) => {
    if (answers[name]) return answers[name]();
    if (name === "project_git") return Promise.resolve(GIT);
    if (name === "project_switcher") return Promise.resolve(list);
    return Promise.resolve(null);
  }) as never);
});

function renderSwitcher(compact = false) {
  const context = { projectId: CURRENT, showView: () => {}, openJob: () => {}, openedJob: null, openedJobRequest: 0 };
  return render(
    <TooltipProvider>
      <ProjectContext.Provider value={context}>
        <ProjectGitProvider projectId={CURRENT}>
          <ProjectSwitcher compact={compact} />
        </ProjectGitProvider>
      </ProjectContext.Provider>
    </TooltipProvider>,
  );
}

const trigger = () => screen.findByRole("button", { name: "Project alpha, branch main" });

async function openMenu(firstRow: RegExp = /beta/) {
  renderSwitcher();
  const user = userEvent.setup();
  await user.click(await trigger());
  const field = await screen.findByRole("combobox", { name: "Search projects" });
  // The list is read when the menu opens: wait for the rows.
  await screen.findByRole("option", { name: firstRow });
  return { user, field };
}

const rows = (group: string) => within(screen.getByRole("group", { name: group })).getAllByRole("option");
const text = (row: HTMLElement) => row.textContent;

describe("the menu", () => {
  it("opens a Popover 320 px wide under the button with a focused field `Search projects`", async () => {
    const { field } = await openMenu();
    expect(field).toHaveFocus();
    expect(field).toHaveAttribute("placeholder", "Search projects");
    expect(field.closest("[data-radix-popper-content-wrapper]")?.firstElementChild).toHaveClass("w-80");
  });

  it("lists the open projects, the current one first with a Check, then the others as the backend ordered them", async () => {
    await openMenu();
    const open = rows("Open");
    expect(open.map((row) => within(row).getByText(/^(alpha|beta|Gamma)$/).textContent)).toEqual(["alpha", "beta", "Gamma"]);
    const current = open[0] as HTMLElement;
    expect(current).toHaveAttribute("aria-current", "true");
    expect(current.querySelector(".lucide-check")).toBeInTheDocument();
    expect(open[1]?.querySelector(".lucide-check")).toBeNull();
    expect(open[1]).not.toHaveAttribute("aria-current");
  });

  it("shows each project's folder name, its path in Muted, and a Sky dot with `<n> working` only for working jobs", async () => {
    await openMenu();
    const [alpha, beta, gamma] = rows("Open") as [HTMLElement, HTMLElement, HTMLElement];
    expect(within(alpha).getByText("/work/alpha")).toHaveClass("text-xs", "text-muted-foreground", "truncate");
    expect(within(alpha).queryByText(/working/)).not.toBeInTheDocument();
    const working = within(beta).getByText("2 working");
    expect(working).toHaveClass("rounded-full", "bg-sky-tint", "text-sky-ink");
    expect(working.firstElementChild).toHaveClass("size-1.5", "rounded-full", "bg-sky");
    expect(within(gamma).getByText("1 working")).toBeInTheDocument();
  });

  it("cuts a long path in the middle", async () => {
    list = { open: [open(CURRENT, "alpha"), { id: "d", name: "deep", working: 0, path: "/Users/someone/Developer/clients/acme/projects/deep" }], recent: [] };
    await openMenu(/deep/);
    const deep = screen.getByRole("option", { name: /deep/ });
    const shown = within(deep).getByTitle("/Users/someone/Developer/clients/acme/projects/deep");
    expect(shown.textContent).toContain("…");
    expect(shown.textContent?.length).toBeLessThanOrEqual(38);
    expect(shown.textContent?.startsWith("/Users/")).toBe(true);
    expect(shown.textContent?.endsWith("/deep")).toBe(true);
  });

  it("lists the recent projects without a window, a missing folder as a disabled row with an X that removes it", async () => {
    const { user } = await openMenu();
    const recents = screen.getByRole("group", { name: "Recent" });
    expect(within(recents).getAllByRole("option").map(text)).toEqual(["delta/old/delta"]);
    const gone = within(recents).getByText("Echo-gone — Not found");
    expect(gone.parentElement).toHaveAttribute("aria-disabled", "true");
    expect(within(recents).queryByRole("option", { name: /Echo-gone/ })).not.toBeInTheDocument();
    const remove = within(recents).getByRole("button", { name: "Remove from recent projects" });
    await user.click(remove);
    expect(call).toHaveBeenCalledWith("remove_recent", { path: "/old/Echo-gone" });
    // Removing is not choosing: no project opens and the menu stays.
    expect(call).not.toHaveBeenCalledWith("open_project", expect.anything());
    expect(screen.getByRole("combobox", { name: "Search projects" })).toBeInTheDocument();
  });

  it("hides empty lists and says `No projects match.` when nothing does", async () => {
    list = { open: [open(CURRENT, "alpha")], recent: [] };
    const { user, field } = await openMenu(/alpha/);
    expect(screen.queryByRole("group", { name: "Recent" })).not.toBeInTheDocument();
    await user.type(field, "zzz");
    expect(screen.getByText("No projects match.")).toHaveClass("text-muted-foreground");
    expect(screen.queryByRole("group", { name: "Open" })).not.toBeInTheDocument();
    // Open Folder… is always there.
    expect(screen.getByRole("option", { name: "Open Folder…" })).toBeInTheDocument();
  });

  it("filters both lists by folder name as typed, case-insensitively", async () => {
    const { user, field } = await openMenu();
    await user.type(field, "ga");
    expect(rows("Open").map((row) => within(row).getByText("Gamma").textContent)).toEqual(["Gamma"]);
    expect(screen.queryByRole("group", { name: "Recent" })).not.toBeInTheDocument();
    await user.clear(field);
    await user.type(field, "ET");
    expect(rows("Open").map((row) => row.textContent)).toEqual(["beta/work/beta2 working"]);
    expect(screen.queryByRole("group", { name: "Recent" })).not.toBeInTheDocument();
    await user.clear(field);
    await user.type(field, "DEL");
    expect(within(screen.getByRole("group", { name: "Recent" })).getAllByRole("option").map(text)).toEqual(["delta/old/delta"]);
    expect(screen.queryByRole("group", { name: "Open" })).not.toBeInTheDocument();
  });

  it("ends with a separator and Open Folder… with the FolderOpen icon", async () => {
    await openMenu();
    const folder = screen.getByRole("option", { name: "Open Folder…" });
    expect(folder.querySelector(".lucide-folder-open")).toBeInTheDocument();
    expect(folder.previousElementSibling).toHaveAttribute("role", "separator");
    expect(folder.nextElementSibling).toBeNull();
  });
});

describe("choosing", () => {
  it("closes the menu and opens nothing when the current project is chosen", async () => {
    const { user } = await openMenu();
    await user.click(screen.getByRole("option", { name: /alpha/ }));
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
    expect(call).not.toHaveBeenCalledWith("open_project", expect.anything());
  });

  it("opens another open project as developer.OPEN_PROJECT: its path, the menu closes", async () => {
    const { user } = await openMenu();
    await user.click(screen.getByRole("option", { name: /beta/ }));
    expect(call).toHaveBeenCalledWith("open_project", { path: "/work/beta" });
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  });

  it("opens a recent project the same way", async () => {
    const { user } = await openMenu();
    await user.click(screen.getByRole("option", { name: /delta/ }));
    expect(call).toHaveBeenCalledWith("open_project", { path: "/old/delta" });
  });

  it("shows the error toast of the File menu when an opening fails, with the reason as its description", async () => {
    answers.open_project = () => Promise.reject({ code: "internal", message: "the folder does not exist" });
    const { user } = await openMenu();
    await user.click(screen.getByRole("option", { name: /beta/ }));
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith("Could not open beta.", { description: "the folder does not exist" }));
  });

  it("asks for the system folder picker, modal to this window, with Open Folder…", async () => {
    const { user } = await openMenu();
    await user.click(screen.getByRole("option", { name: "Open Folder…" }));
    expect(call).toHaveBeenCalledWith("open_folder_from_window");
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  });

  it("says so when the picker could not be asked for", async () => {
    answers.open_folder_from_window = () => Promise.reject({ code: "internal", message: "no dialog" });
    const { user } = await openMenu();
    await user.click(screen.getByRole("option", { name: "Open Folder…" }));
    await waitFor(() => expect(toast.error).toHaveBeenCalledWith("Could not open the folder.", { description: "no dialog" }));
  });
});

describe("the keyboard", () => {
  it("moves through the rows with the arrows, the focus staying in the field, and Enter chooses", async () => {
    const { user, field } = await openMenu();
    expect(field).toHaveFocus();
    // The first row is highlighted; the arrows walk the rows.
    expect(screen.getByRole("option", { name: /alpha/ })).toHaveAttribute("aria-selected", "true");
    await user.keyboard("{ArrowDown}");
    expect(screen.getByRole("option", { name: /beta/ })).toHaveAttribute("aria-selected", "true");
    expect(field).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(call).toHaveBeenCalledWith("open_project", { path: "/work/beta" });
  });

  it("closes with Esc and gives the focus back to the switcher", async () => {
    const { user } = await openMenu();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
    expect(await trigger()).toHaveFocus();
  });

  it("is opened by no keyboard shortcut", async () => {
    renderSwitcher();
    await trigger();
    const user = userEvent.setup();
    await user.keyboard("{Control>}k{/Control}");
    await user.keyboard("{Meta>}p{/Meta}");
    await user.keyboard("{Alt>}p{/Alt}");
    expect(screen.queryByRole("combobox")).not.toBeInTheDocument();
  });

  it("starts empty again each time it opens", async () => {
    const { user, field } = await openMenu();
    await user.type(field, "ga");
    await user.keyboard("{Escape}");
    await user.click(await trigger());
    expect(await screen.findByRole("combobox", { name: "Search projects" })).toHaveValue("");
  });
});

describe("the list", () => {
  it("is read each time the menu opens, and follows the backend while it is open", async () => {
    const { user } = await openMenu();
    expect(vi.mocked(call).mock.calls.filter(([name]) => name === "project_switcher").length).toBeGreaterThanOrEqual(2);
    act(() => handlers["project-switcher-changed"]?.({ open: [open(CURRENT, "alpha"), open("e", "epsilon", 3)], recent: [] } as never));
    const epsilon = await screen.findByRole("option", { name: /epsilon/ });
    expect(within(epsilon).getByText("3 working")).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /beta/ })).not.toBeInTheDocument();
    await user.keyboard("{Escape}");
  });

  it("drops the answer of a read that an event overtook", async () => {
    let answer: (value: Switcher) => void = () => {};
    answers.project_switcher = () => new Promise<Switcher>((resolve) => (answer = resolve));
    renderSwitcher();
    await trigger();
    await waitFor(() => expect(call).toHaveBeenCalledWith("project_switcher"));
    act(() => handlers["project-switcher-changed"]?.({ open: [open(CURRENT, "alpha")], recent: [] } as never));
    await act(async () => answer(LIST));
    const user = userEvent.setup();
    answers.project_switcher = () => new Promise(() => {});
    await user.click(await trigger());
    await screen.findByRole("combobox", { name: "Search projects" });
    expect(screen.queryByRole("option", { name: /beta/ })).not.toBeInTheDocument();
  });
});

describe("the button with icons only", () => {
  it("shows the logo alone and opens the same menu", async () => {
    renderSwitcher(true);
    const button = await trigger();
    expect(button.textContent).toBe("");
    const user = userEvent.setup();
    await user.click(button);
    expect(await screen.findByRole("combobox", { name: "Search projects" })).toBeInTheDocument();
    expect(await screen.findByRole("option", { name: /beta/ })).toBeInTheDocument();
  });
});

describe("filtering and cutting", () => {
  it("puts the current project first among the open ones and keeps the order of the others", () => {
    const shown = filterProjects(LIST, CURRENT, "");
    expect(shown.open.map((project) => project.name)).toEqual(["alpha", "beta", "Gamma"]);
    expect(shown.recent.map((project) => project.name)).toEqual(["delta", "Echo-gone"]);
  });

  it("matches the folder name only, ignoring case and the spaces around what was typed", () => {
    expect(filterProjects(LIST, CURRENT, "  GAM ").open.map((project) => project.name)).toEqual(["Gamma"]);
    // The path is not searched.
    expect(filterProjects(LIST, CURRENT, "work").open).toEqual([]);
    expect(filterProjects(LIST, CURRENT, "echo").recent.map((project) => project.name)).toEqual(["Echo-gone"]);
  });

  it("cuts a path longer than the limit in the middle, and leaves a short one alone", () => {
    expect(cutMiddle("/work/alpha", 38)).toBe("/work/alpha");
    const cut = cutMiddle("/Users/someone/Developer/clients/acme/projects/deep", 20);
    expect(cut).toHaveLength(20);
    expect(cut).toBe("/Users/som…ects/deep");
  });
});
