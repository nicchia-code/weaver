import { useMemo, useRef, useState } from "react";
import { toast } from "sonner";

import { useBoard } from "@/components/board/board-state";
import { useBoardActions } from "@/components/board/board-ui";
import { useHistory } from "@/components/history/use-history";
import { useMerge } from "@/components/merge/use-merge";
import { arrange, type PaletteChoice, type PaletteGroupName, type PaletteItem } from "@/components/palette-model";
import { useProject, useWindowControls } from "@/components/project-context";
import { filterProjects, reasonOf, useProjectList } from "@/components/project-switcher";
import { useRequests } from "@/components/requests/use-requests";
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList } from "@/components/ui/command";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { useViews } from "@/components/views/views-state";
import { call } from "@/lib/backend";
import { formatCalendarDate } from "@/lib/format";
import { cn } from "@/lib/utils";
import { VIEWS } from "@/views";

/** The rows of every group, in the order the palette shows them; matching and limits come later (`arrange`). */
function useRows(query: string): ReadonlyMap<PaletteGroupName, readonly PaletteItem[]> {
  const { projectId } = useProject();
  const { snapshot: board } = useBoard();
  const { snapshot: views } = useViews();
  const { snapshot: history } = useHistory(projectId);
  const merge = useMerge();
  const { list } = useProjectList(true);
  const typed = query.trim() !== "";

  return useMemo(() => {
    const rows = new Map<PaletteGroupName, PaletteItem[]>();
    const item = (value: string, label: string, choice: PaletteChoice, extra: Partial<PaletteItem> = {}): PaletteItem => ({
      value,
      label,
      terms: [label],
      reasons: [],
      choice,
      ...extra,
    });

    rows.set(
      "Views",
      VIEWS.map((view) => item(`view:${view.id}`, view.label, { kind: "view", view: view.id })),
    );
    rows.set("Actions", [
      item("action:scan", "Scan System", { kind: "action", action: "scan" }, { reasons: merge?.merge?.inProgress ? ["A merge is in progress"] : [] }),
      item("action:run-all", "Run all", { kind: "action", action: "run-all" }, { reasons: board?.runAllReasons ?? [] }),
      item("action:new-request", "New Request", { kind: "action", action: "new-request" }),
      item("action:open-folder", "Open Folder…", { kind: "action", action: "open-folder" }),
      item("action:usage", "Usage", { kind: "action", action: "usage" }),
    ]);
    // The projects of the switcher: the open ones (the current first) and the recent ones.
    if (list) {
      const shown = filterProjects(list, projectId, "");
      rows.set("Projects", [
        ...shown.open.map((project) =>
          item(`project:${project.id}`, project.name, { kind: "project", id: project.id, path: project.path, name: project.name, current: project.id === projectId }, { detail: project.path }),
        ),
        ...shown.recent.map((project) =>
          project.exists
            ? item(`recent:${project.path}`, project.name, { kind: "project", id: project.path, path: project.path, name: project.name, current: false }, { detail: project.path })
            : item(`recent:${project.path}`, `${project.name} — Not found`, { kind: "project", id: project.path, path: project.path, name: project.name, current: false }, { terms: [project.name], reasons: ["The folder no longer exists"] }),
        ),
      ]);
    }
    // The rest is searched only when something was typed.
    if (typed) {
      rows.set(
        "Cards",
        [...(board?.backlog ?? []), ...(board?.active ?? [])].map((card) => {
          // A draft of a request is a card of Backlog without a head: its title is already the one
          // of its card, and `Request` takes the place of the short number; choosing it opens the
          // request editor, not the details dialog.
          if (card.kind === "request") return item(`request:${card.id}`, card.title, { kind: "request", id: card.id }, { detail: "Request" });
          return item(`card:${card.head.id}`, card.head.title, { kind: "card", id: card.head.id }, { detail: card.head.shortNumber ?? undefined, terms: [card.head.shortNumber ?? "", card.head.title].filter((term) => term !== "") });
        }),
      );
      rows.set(
        "Requirements",
        (views?.data.requirements ?? []).flatMap((area) =>
          area.fragments.flatMap((fragment) =>
            fragment.sections.map((section) =>
              item(`section:${area.id}/${fragment.name}#${section.title}`, section.title, { kind: "section", key: `${area.id}#${section.title}` }, { detail: `${area.title} · ${fragment.title}` }),
            ),
          ),
        ),
      );
      rows.set(
        "History",
        (history?.entries ?? []).map((entry) =>
          item(`history:${entry.id}`, entry.title, { kind: "history", id: entry.id }, { detail: formatCalendarDate(new Date(entry.updated)) }),
        ),
      );
    }
    return rows;
  }, [projectId, board, views, history, merge?.merge?.inProgress, list, typed]);
}

function Palette() {
  const { showView } = useProject();
  const { openEditor } = useRequests();
  const { openUsage, reveal, palette } = useWindowControls();
  const actions = useBoardActions();
  const [query, setQuery] = useState("");
  const chosen = useRef(false);
  const rows = useRows(query);
  const groups = arrange(rows, query);

  const choose = (value: string) => {
    const row = groups.flatMap((group) => group.items).find((candidate) => candidate.value === value);
    // A row with reasons cannot be chosen: it says why, and the palette stays.
    if (!row || row.reasons.length > 0) return;
    chosen.current = true;
    palette.close();
    const choice = row.choice;
    switch (choice.kind) {
      case "view":
        showView(choice.view);
        break;
      case "card":
      case "section":
      case "history":
        reveal(choice);
        break;
      case "request":
        showView("board");
        openEditor(choice.id);
        break;
      case "project":
        if (!choice.current) call("open_project", { path: choice.path }).catch((error: unknown) => toast.error(`Could not open ${choice.name}.`, { description: reasonOf(error) }));
        break;
      case "action":
        if (choice.action === "scan") reveal({ kind: "scan" });
        else if (choice.action === "run-all") actions.runAll?.();
        else if (choice.action === "new-request") {
          showView("board");
          actions.newRequest?.();
        } else if (choice.action === "usage") openUsage(palette.origin);
        else call("open_folder_from_window").catch((error: unknown) => toast.error("Could not open the folder.", { description: reasonOf(error) }));
        break;
    }
  };

  return (
    <Dialog open onOpenChange={(open) => (open ? undefined : palette.close())}>
      <DialogContent
        hideClose
        aria-describedby={undefined}
        className="mt-[12vh] max-w-[560px] gap-0 self-start overflow-hidden p-0"
        onCloseAutoFocus={(event) => {
          // The focus goes back to where it was, unless a choice took it somewhere else.
          event.preventDefault();
          if (!chosen.current && palette.origin?.isConnected) palette.origin.focus();
        }}
      >
        <DialogTitle className="sr-only">Search or run</DialogTitle>
        <Command onChoose={choose} className="rounded-none bg-transparent shadow-none">
          <CommandInput aria-label="Search or run…" placeholder="Search or run…" value={query} onValueChange={setQuery} />
          <CommandList className="max-h-[60vh]">
            {groups.length === 0 ? <CommandEmpty>No results.</CommandEmpty> : null}
            {groups.map((group) => (
              <CommandGroup key={group.name} heading={group.name}>
                {group.items.map((row) => (
                  <CommandItem key={row.value} value={row.value} aria-disabled={row.reasons.length > 0 ? true : undefined} className={cn(row.reasons.length > 0 && "cursor-not-allowed opacity-60")}>
                    <span className="min-w-0 flex-1 truncate">{row.label}</span>
                    {row.reasons.length > 0 ? <span className="shrink-0 text-xs text-muted-foreground">{row.reasons.join(" · ")}</span> : null}
                    {row.detail && row.reasons.length === 0 ? <span className="max-w-[45%] shrink-0 truncate text-xs text-muted-foreground">{row.detail}</span> : null}
                  </CommandItem>
                ))}
              </CommandGroup>
            ))}
          </CommandList>
        </Command>
      </DialogContent>
    </Dialog>
  );
}

/**
 * The palette of a project window (`requirements/ui#window`): a Dialog in the top third, 560 px
 * wide, without title bar, with a search field and the results in groups (Views, Actions,
 * Projects, then Cards, Requirements and History once something is typed). Choosing a row goes to
 * it or runs it as its own control does, and closes the palette. It never searches files, assets
 * or notes. It lives inside what the views share, so it reads the Board, the Requirements, the
 * History and the Board's actions; its content exists only while it is open.
 */
export function CommandPalette() {
  const { palette } = useWindowControls();
  return palette.open ? <Palette /> : null;
}
