import { ChevronDown, ChevronRight, File, Folder, FolderOpen, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";

import type { TreeEntry } from "@/bindings/TreeEntry";
import { GuardedButton } from "@/components/guarded-button";
import {
  excludedLabel,
  hasIncluded,
  ROOT,
  stateOf,
  toggle,
  visibleRows,
  type Listing,
} from "@/components/project/import-tree-model";
import { useProjectStatus } from "@/components/project/use-project-status";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { call, isAppError } from "@/lib/backend";

/** The reason `Start import` is disabled while nothing is included. */
export const NOTHING_INCLUDED = "Include at least one file or folder.";

function reasonOf(error: unknown, fallback: string): string {
  return isAppError(error) ? error.message : fallback;
}

/**
 * `Import from Code`: the folder tree of the project with a checkbox on each entry, `<N> excluded`
 * and `Start import`. What is unchecked is left out of this import only; it lives here and is
 * forgotten when the panel closes. The tree is an ARIA tree: arrows move and open or close, `Space`
 * toggles.
 */
export function ImportPanel({ onClose, onStarted }: { onClose: () => void; onStarted: () => void }) {
  const { projectId } = useProjectStatus();
  const [listing, setListing] = useState<Listing>({});
  const [open, setOpen] = useState<ReadonlySet<string>>(new Set());
  const [excluded, setExcluded] = useState<ReadonlySet<string>>(new Set());
  const [focused, setFocused] = useState<string | null>(null);
  const [loadFailure, setLoadFailure] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);
  const [startFailure, setStartFailure] = useState<string | null>(null);
  const items = useRef(new Map<string, HTMLElement>());
  const wantFocus = useRef(false);
  const requested = useRef(new Set<string>());
  const alive = useRef(true);

  const load = useCallback(
    (path: string) => {
      if (requested.current.has(path)) return;
      requested.current.add(path);
      call("project_folder", { projectId, path })
        .then((entries) => alive.current && setListing((known) => ({ ...known, [path]: entries })))
        .catch((error: unknown) => {
          requested.current.delete(path);
          if (alive.current) setLoadFailure(reasonOf(error, "the folder could not be read"));
        });
    },
    [projectId],
  );

  useEffect(() => {
    alive.current = true;
    load(ROOT);
    return () => {
      alive.current = false;
    };
  }, [load]);

  const rows = visibleRows(listing, open);
  const current = rows.find((row) => row.entry.path === focused) ?? rows[0] ?? null;

  useEffect(() => {
    if (!wantFocus.current || !current) return;
    wantFocus.current = false;
    items.current.get(current.entry.path)?.focus();
  });

  const moveTo = (path: string) => {
    wantFocus.current = true;
    setFocused(path);
  };

  const openFolder = (entry: TreeEntry) => {
    load(entry.path);
    setOpen((known) => new Set(known).add(entry.path));
  };
  const closeFolder = (entry: TreeEntry) =>
    setOpen((known) => {
      const next = new Set(known);
      next.delete(entry.path);
      return next;
    });
  const toggleOpen = (entry: TreeEntry) => (open.has(entry.path) ? closeFolder(entry) : openFolder(entry));
  const toggleChecked = (entry: TreeEntry) => setExcluded((known) => toggle(known, listing, entry));

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (!current || event.altKey || event.ctrlKey || event.metaKey) return;
    const index = rows.indexOf(current);
    const next = rows[index + 1];
    const previous = rows[index - 1];
    const { entry } = current;
    switch (event.key) {
      case "ArrowDown":
        if (next) moveTo(next.entry.path);
        break;
      case "ArrowUp":
        if (previous) moveTo(previous.entry.path);
        break;
      case "Home":
        moveTo(rows.at(0)?.entry.path ?? entry.path);
        break;
      case "End":
        moveTo(rows.at(-1)?.entry.path ?? entry.path);
        break;
      case "ArrowRight":
        if (!entry.folder) break;
        if (!open.has(entry.path)) openFolder(entry);
        else if (next?.parent === entry.path) moveTo(next.entry.path);
        break;
      case "ArrowLeft":
        if (entry.folder && open.has(entry.path)) closeFolder(entry);
        else if (current.parent !== ROOT) moveTo(current.parent);
        break;
      case " ":
        toggleChecked(entry);
        break;
      default:
        return;
    }
    event.preventDefault();
  };

  const start = () => {
    setStarting(true);
    setStartFailure(null);
    call("import_code", { projectId, excluded: [...excluded].sort() })
      .then(() => onStarted())
      .catch((error: unknown) => {
        if (alive.current) setStartFailure(reasonOf(error, "Could not start the import."));
      })
      .finally(() => alive.current && setStarting(false));
  };

  const included = hasIncluded(excluded, listing);

  return (
    <section aria-labelledby="import-title" className="flex flex-col gap-3 rounded-lg p-4 shadow-ring">
      <div className="flex flex-col gap-1">
        <h2 id="import-title" className="text-base font-semibold">
          Import from Code
        </h2>
        <p className="text-sm text-muted-foreground">
          The agent describes the code as Proposals. Uncheck what it should leave out; this applies to this import only.
        </p>
      </div>
      <p className="text-sm" aria-live="polite">
        {excludedLabel(excluded)}
      </p>
      {loadFailure ? (
        <Alert variant="error">
          <TriangleAlert aria-hidden />
          <p className="min-w-0 flex-1">{loadFailure}</p>
        </Alert>
      ) : null}
      <div
        role="tree"
        aria-label="Files and folders to import"
        onKeyDown={onKeyDown}
        className="max-h-72 overflow-auto rounded-lg py-1 text-sm shadow-ring"
      >
        {rows.length === 0 && listing[ROOT] ? <p className="px-3 py-2 text-muted-foreground">Nothing to import in this folder.</p> : null}
        {rows.map((row) => {
          const { entry } = row;
          const state = stateOf(excluded, listing, entry);
          const isOpen = entry.folder && open.has(entry.path);
          const Icon = entry.folder ? (isOpen ? FolderOpen : Folder) : File;
          const siblings = listing[row.parent] ?? [];
          return (
            <div
              key={entry.path}
              role="treeitem"
              ref={(node) => {
                if (node) items.current.set(entry.path, node);
                else items.current.delete(entry.path);
              }}
              tabIndex={current?.entry.path === entry.path ? 0 : -1}
              aria-level={row.level}
              aria-setsize={siblings.length}
              aria-posinset={siblings.findIndex((e) => e.path === entry.path) + 1}
              aria-expanded={entry.folder ? isOpen : undefined}
              aria-checked={state === "included" ? "true" : state === "excluded" ? "false" : "mixed"}
              onFocus={(event) => event.target === event.currentTarget && setFocused(entry.path)}
              onClick={() => {
                setFocused(entry.path);
                if (entry.folder) toggleOpen(entry);
              }}
              style={{ paddingLeft: `${(row.level - 1) * 16 + 8}px` }}
              className="flex cursor-pointer items-center gap-2 py-1 pr-3 hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring"
            >
              {entry.folder ? (
                isOpen ? (
                  <ChevronDown aria-hidden className="size-4 shrink-0 text-muted-foreground" />
                ) : (
                  <ChevronRight aria-hidden className="size-4 shrink-0 text-muted-foreground" />
                )
              ) : (
                <span aria-hidden className="size-4 shrink-0" />
              )}
              <Checkbox
                tabIndex={-1}
                aria-hidden
                checked={state === "included" ? true : state === "excluded" ? false : "indeterminate"}
                onClick={(event) => event.stopPropagation()}
                onCheckedChange={() => toggleChecked(entry)}
              />
              <Icon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
              <Tooltip>
                <TooltipTrigger asChild>
                  <span className="min-w-0 flex-1 truncate">{entry.name}</span>
                </TooltipTrigger>
                <TooltipContent>{entry.path}</TooltipContent>
              </Tooltip>
            </div>
          );
        })}
      </div>
      {startFailure ? (
        <Alert variant="error">
          <TriangleAlert aria-hidden />
          <p className="min-w-0 flex-1">{startFailure}</p>
        </Alert>
      ) : null}
      <div className="flex gap-2">
        <GuardedButton type="button" reasons={[!included && NOTHING_INCLUDED, starting && "The import is starting"]} onClick={start}>
          Start import
        </GuardedButton>
        <Button type="button" variant="outline" onClick={onClose}>
          Cancel
        </Button>
      </div>
    </section>
  );
}
