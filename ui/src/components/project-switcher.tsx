import { Check, ChevronsUpDown, FolderOpen, X } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";

import type { OpenProjectEntry } from "@/bindings/OpenProjectEntry";
import type { ProjectSwitcher as ProjectSwitcherList } from "@/bindings/ProjectSwitcher";
import type { RecentEntry } from "@/bindings/RecentEntry";
import { Logo } from "@/components/logo";
import { useProject } from "@/components/project-context";
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from "@/components/ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useProjectGit } from "@/hooks/use-project-git";
import { call, isAppError, on } from "@/lib/backend";
import { cn } from "@/lib/utils";

/** The projects both lists show for what was typed: by folder name, case-insensitively, the current project first among the open ones. */
export function filterProjects(list: ProjectSwitcherList, currentId: string, query: string): { open: OpenProjectEntry[]; recent: RecentEntry[] } {
  const needle = query.trim().toLowerCase();
  const matches = (name: string) => name.toLowerCase().includes(needle);
  const open = list.open.filter((project) => matches(project.name));
  // The others keep the order of the backend: the most recently used first.
  return {
    open: [...open.filter((project) => project.id === currentId), ...open.filter((project) => project.id !== currentId)],
    recent: list.recent.filter((project) => matches(project.name)),
  };
}

/** A path cut in the middle with `…` when it is longer than `max` characters. */
export function cutMiddle(path: string, max: number): string {
  if (path.length <= max) return path;
  const keep = max - 1;
  const head = Math.ceil(keep / 2);
  return `${path.slice(0, head)}…${path.slice(path.length - (keep - head))}`;
}

/** The list the menu shows: read when the window loads and each time the menu opens, and replaced by every `project-switcher-changed`. */
export function useProjectList(open: boolean): { list: ProjectSwitcherList | null; reload: () => void } {
  const [list, setList] = useState<ProjectSwitcherList | null>(null);
  // An answer that was asked for before an event arrived is older than the event: dropped.
  const events = useRef(0);
  const alive = useRef(true);

  const reload = useCallback(() => {
    const asked = events.current;
    call("project_switcher")
      .then((answer) => {
        if (alive.current && events.current === asked) setList(answer);
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    alive.current = true;
    const unlisten = on("project-switcher-changed", (incoming) => {
      if (!alive.current) return;
      events.current += 1;
      setList(incoming);
    });
    return () => {
      alive.current = false;
      unlisten.then((stop) => stop()).catch(() => {});
    };
  }, []);

  useEffect(reload, [reload]);
  useEffect(() => {
    if (open) reload();
  }, [open, reload]);

  return { list, reload };
}

export const reasonOf = (error: unknown) => (isAppError(error) ? error.message : String(error));

function Working({ count }: { count: number }) {
  return (
    <span className="inline-flex shrink-0 items-center gap-1.5 rounded-full bg-sky-tint px-2 py-0.5 text-xs font-medium text-sky-ink">
      <span aria-hidden className="size-1.5 rounded-full bg-sky" />
      {count} working
    </span>
  );
}

function OpenRow({ project, current }: { project: OpenProjectEntry; current: boolean }) {
  return (
    <CommandItem value={`open:${project.id}`} aria-current={current ? "true" : undefined}>
      <span className="flex min-w-0 flex-1 flex-col">
        <span className="flex items-center gap-1.5">
          {current ? <Check aria-hidden className="size-4 shrink-0" /> : null}
          <span className="truncate font-medium">{project.name}</span>
        </span>
        <span title={project.path} className={cn("truncate text-xs text-muted-foreground", current && "pl-[22px]")}>
          {cutMiddle(project.path, 38)}
        </span>
      </span>
      {project.working > 0 ? <Working count={project.working} /> : null}
    </CommandItem>
  );
}

function RecentRow({ project, onRemove }: { project: RecentEntry; onRemove: (project: RecentEntry) => void }) {
  if (project.exists) {
    return (
      <CommandItem value={`recent:${project.path}`}>
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="truncate font-medium">{project.name}</span>
          <span title={project.path} className="truncate text-xs text-muted-foreground">
            {cutMiddle(project.path, 38)}
          </span>
        </span>
      </CommandItem>
    );
  }
  return (
    <div aria-disabled="true" className="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm text-muted-foreground">
      <span className="min-w-0 flex-1 truncate">{`${project.name} — Not found`}</span>
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            aria-label="Remove from recent projects"
            onClick={() => onRemove(project)}
            className="grid size-6 shrink-0 place-items-center rounded-md text-ink-2 hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          >
            <X aria-hidden className="size-3.5" />
          </button>
        </TooltipTrigger>
        <TooltipContent>Remove from recent projects</TooltipContent>
      </Tooltip>
    </div>
  );
}

/**
 * The project switcher at the top of the sidebar (`requirements/ui#window`): the logo, the project
 * folder's name and the branch git is on, opening a menu with the open projects, the recent ones
 * and `Open Folder…`. Choosing another project is `developer.OPEN_PROJECT`: its window opens or
 * comes to the front and this window stays as it was. With icons only the button is the logo.
 */
export function ProjectSwitcher({ compact }: { compact: boolean }) {
  const { projectId } = useProject();
  const git = useProjectGit();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const { list, reload } = useProjectList(open);

  // The folder's name comes with the git value, or from the list the menu holds.
  const name = git?.name ?? list?.open.find((project) => project.id === projectId)?.name ?? "";
  const branch = git?.branch ?? null;
  const accessibleName = [name ? `Project ${name}` : "Project", branch ? `branch ${branch}` : null].filter((part) => part !== null).join(", ");

  const changeOpen = (next: boolean) => {
    setOpen(next);
    if (!next) setQuery("");
  };

  const openProject = (path: string, folder: string) => {
    call("open_project", { path }).catch((error: unknown) => toast.error(`Could not open ${folder}.`, { description: reasonOf(error) }));
  };

  const choose = (value: string) => {
    changeOpen(false);
    if (value === "folder") {
      call("open_folder_from_window").catch((error: unknown) => toast.error("Could not open the folder.", { description: reasonOf(error) }));
    } else if (value.startsWith("open:")) {
      const project = list?.open.find((candidate) => candidate.id === value.slice("open:".length));
      // The current project: the menu only closes.
      if (project && project.id !== projectId) openProject(project.path, project.name);
    } else if (value.startsWith("recent:")) {
      const project = list?.recent.find((candidate) => candidate.path === value.slice("recent:".length));
      if (project) openProject(project.path, project.name);
    }
  };

  const remove = (project: RecentEntry) => {
    call("remove_recent", { path: project.path })
      .then(reload)
      .catch((error: unknown) => toast.error(`Could not remove ${project.name}.`, { description: reasonOf(error) }));
  };

  const shown = list ? filterProjects(list, projectId, query) : { open: [], recent: [] };
  const nothing = shown.open.length === 0 && shown.recent.length === 0;

  const button = (
    <button
      type="button"
      aria-label={accessibleName}
      className={cn(
        "flex w-full items-center rounded-[10px] text-left transition-colors duration-150 hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        compact ? "justify-center p-1" : "gap-2.5 px-2 py-1.5",
      )}
    >
      <Logo size={30} className="shrink-0" />
      {compact ? null : (
        <>
          <span className="flex min-w-0 flex-1 flex-col gap-px">
            <span className="truncate text-sm font-semibold tracking-title">{name}</span>
            {branch ? <span className="truncate text-xs text-muted-foreground">{branch}</span> : null}
          </span>
          <ChevronsUpDown aria-hidden className="size-3.5 shrink-0 text-muted-foreground" />
        </>
      )}
    </button>
  );

  return (
    <Popover open={open} onOpenChange={changeOpen}>
      {compact ? (
        <Tooltip>
          <TooltipTrigger asChild>
            <PopoverTrigger asChild>{button}</PopoverTrigger>
          </TooltipTrigger>
          <TooltipContent side="right">{accessibleName}</TooltipContent>
        </Tooltip>
      ) : (
        <PopoverTrigger asChild>{button}</PopoverTrigger>
      )}
      <PopoverContent align="start" className="w-80 overflow-hidden p-0">
        <Command onChoose={choose} className="rounded-none bg-transparent shadow-none">
          <CommandInput aria-label="Search projects" placeholder="Search projects" value={query} onValueChange={setQuery} />
          <CommandList>
            {nothing ? <CommandEmpty>No projects match.</CommandEmpty> : null}
            {shown.open.length > 0 ? (
              <CommandGroup heading="Open">
                {shown.open.map((project) => (
                  <OpenRow key={project.id} project={project} current={project.id === projectId} />
                ))}
              </CommandGroup>
            ) : null}
            {shown.recent.length > 0 ? (
              <CommandGroup heading="Recent">
                {shown.recent.map((project) => (
                  <RecentRow key={project.path} project={project} onRemove={remove} />
                ))}
              </CommandGroup>
            ) : null}
            <div role="separator" className="mx-1 my-1 h-px bg-line-soft" />
            <CommandItem value="folder">
              <FolderOpen aria-hidden className="size-4 shrink-0" />
              Open Folder…
            </CommandItem>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
