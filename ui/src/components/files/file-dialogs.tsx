import { ChevronDown, ChevronRight, Folder, FolderOpen } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { toast } from "sonner";

import type { FileEntry } from "@/bindings/FileEntry";
import { ConfirmDialog, FormDialog } from "@/components/dialogs";
import { canDrop, nameOf, ROOT } from "@/components/files/tree-model";
import { GuardedButton } from "@/components/guarded-button";
import { useProject } from "@/components/project-context";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { call, isAppError } from "@/lib/backend";
import { cn } from "@/lib/utils";

const plural = (count: number, one: string, many: string) => `${count} ${count === 1 ? one : many}`;

/**
 * `developer.DELETE_ENTRY` asks first: `Move "<name>" to the trash?`, and says how many open files
 * have unsaved changes that will be lost when tabs on the entry have some.
 */
export function DeleteDialog({
  path,
  folder,
  unsaved,
  onConfirm,
  onClose,
}: {
  /** The entry to delete; `null` while no dialog is open. */
  path: string | null;
  folder: boolean;
  /** How many open files at or inside it have unsaved changes. */
  unsaved: number;
  onConfirm: (path: string) => void;
  onClose: () => void;
}) {
  const lastPath = useRef(path);
  if (path !== null) lastPath.current = path;
  const shown = path ?? lastPath.current;
  const name = shown === null ? "" : nameOf(shown);
  const loss = unsaved === 1 ? "1 open file has unsaved changes that will be lost." : `${unsaved} open files have unsaved changes that will be lost.`;
  const description = unsaved > 0 ? loss : folder ? "It goes to the system's trash with everything inside it." : "It goes to the system's trash.";
  return (
    <ConfirmDialog
      open={path !== null}
      onOpenChange={(open) => !open && onClose()}
      title={`Move "${name}" to the trash?`}
      description={description}
      confirmLabel="Move to Trash"
      destructive
      onConfirm={() => path !== null && onConfirm(path)}
    />
  );
}

/**
 * `developer.LEAVE_FILE_VIEW` with unsaved changes: `Save changes to <n> files?`, the paths, and
 * `Save all`, `Discard all` and `Cancel` (last, and where the focus starts).
 */
export function LeaveDialog({
  paths,
  onSaveAll,
  onDiscardAll,
  onCancel,
}: {
  /** The files with unsaved changes; `null` while no dialog is open. */
  paths: readonly string[] | null;
  onSaveAll: () => void;
  onDiscardAll: () => void;
  onCancel: () => void;
}) {
  const last = useRef<readonly string[]>([]);
  if (paths !== null) last.current = paths;
  const listed = paths ?? last.current;
  return (
    <AlertDialog open={paths !== null} onOpenChange={(open) => !open && onCancel()}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Save changes to {plural(listed.length, "file", "files")}?</AlertDialogTitle>
          <AlertDialogDescription>These files have unsaved changes:</AlertDialogDescription>
        </AlertDialogHeader>
        <ul className="max-h-48 overflow-auto rounded-lg px-3 py-2 font-mono shadow-ring text-xs">
          {listed.map((path) => (
            <li key={path} className="truncate">
              {path}
            </li>
          ))}
        </ul>
        <AlertDialogFooter>
          <AlertDialogAction onClick={onSaveAll}>Save all</AlertDialogAction>
          <AlertDialogAction variant="destructive" onClick={onDiscardAll}>
            Discard all
          </AlertDialogAction>
          <AlertDialogCancel autoFocus>Cancel</AlertDialogCancel>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

function reasonOf(error: unknown): string {
  return isAppError(error) ? error.message : "The folder could not be read.";
}

/** One visible row of the folders tree of `Move to…`. */
interface FolderRow {
  path: string;
  name: string;
  level: number;
  parent: string;
  allowed: boolean;
}

/**
 * `Move to…`: a dialog with the project's folders as a tree, read one folder at a time as it
 * expands, and `Move` / `Cancel`. A folder the entry cannot go to (where it is, itself and what is
 * inside it, `.weaver/`) is shown disabled.
 */
export function MoveDialog({
  source,
  onMove,
  onClose,
}: {
  /** The entry to move; `null` while no dialog is open. */
  source: FileEntry | null;
  onMove: (path: string, target: string) => Promise<boolean>;
  onClose: () => void;
}) {
  return (
    <FormDialog
      open={source !== null}
      onOpenChange={(open) => !open && onClose()}
      title={source ? `Move "${nameOf(source.path)}" to…` : "Move to…"}
      description="Choose the folder to move it into."
    >
      {source ? <FolderChooser source={source} onMove={onMove} onClose={onClose} /> : null}
    </FormDialog>
  );
}

function FolderChooser({
  source,
  onMove,
  onClose,
}: {
  source: FileEntry;
  onMove: (path: string, target: string) => Promise<boolean>;
  onClose: () => void;
}) {
  const { projectId } = useProject();
  const [folders, setFolders] = useState<Record<string, readonly FileEntry[]>>({});
  const [open, setOpen] = useState<ReadonlySet<string>>(new Set([ROOT]));
  const [chosen, setChosen] = useState<string | null>(null);
  const [focused, setFocused] = useState<string>(ROOT);
  const [moving, setMoving] = useState(false);
  const requested = useRef(new Set<string>());
  const items = useRef(new Map<string, HTMLElement>());
  const wantFocus = useRef(false);

  const load = useCallback(
    (path: string) => {
      if (requested.current.has(path)) return;
      requested.current.add(path);
      call("files_folders", { projectId, path })
        .then((entries) => setFolders((known) => ({ ...known, [path]: entries })))
        .catch((error: unknown) => {
          requested.current.delete(path);
          toast.error(reasonOf(error));
        });
    },
    [projectId],
  );

  useEffect(() => load(ROOT), [load]);

  const rows: FolderRow[] = [{ path: ROOT, name: "Project folder", level: 1, parent: ROOT, allowed: canDrop(source, ROOT) }];
  const walk = (parent: string, level: number) => {
    for (const entry of folders[parent] ?? []) {
      rows.push({ path: entry.path, name: entry.name, level, parent, allowed: canDrop(source, entry.path) });
      if (open.has(entry.path)) walk(entry.path, level + 1);
    }
  };
  if (open.has(ROOT)) walk(ROOT, 2);

  useEffect(() => {
    if (!wantFocus.current) return;
    wantFocus.current = false;
    items.current.get(focused)?.focus();
  });

  const moveFocus = (path: string) => {
    wantFocus.current = true;
    setFocused(path);
  };

  const expand = (path: string) => {
    load(path);
    setOpen((known) => new Set(known).add(path));
  };
  const collapse = (path: string) =>
    setOpen((known) => {
      const next = new Set(known);
      next.delete(path);
      return next;
    });

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const index = rows.findIndex((row) => row.path === focused);
    const row = rows[index];
    if (!row || event.altKey || event.ctrlKey || event.metaKey) return;
    switch (event.key) {
      case "ArrowDown":
        if (index + 1 < rows.length) moveFocus(rows[index + 1]!.path);
        break;
      case "ArrowUp":
        if (index > 0) moveFocus(rows[index - 1]!.path);
        break;
      case "ArrowRight":
        if (!open.has(row.path)) expand(row.path);
        else if (rows[index + 1]?.parent === row.path) moveFocus(rows[index + 1]!.path);
        break;
      case "ArrowLeft":
        if (open.has(row.path) && row.path !== ROOT) collapse(row.path);
        else if (row.path !== ROOT) moveFocus(row.parent);
        break;
      case "Enter":
      case " ":
        if (row.allowed) setChosen(row.path);
        break;
      default:
        return;
    }
    event.preventDefault();
  };

  const move = () => {
    if (chosen === null) return;
    setMoving(true);
    onMove(source.path, chosen)
      .then((done) => done && onClose())
      .catch(() => {})
      .finally(() => setMoving(false));
  };

  return (
    <div className="flex flex-col gap-3">
      <div
        role="tree"
        aria-label="Folders of the project"
        onKeyDown={onKeyDown}
        className="max-h-72 overflow-auto rounded-lg py-1 text-sm shadow-ring"
      >
        {rows.map((row) => {
          const isOpen = open.has(row.path);
          const Icon = isOpen ? FolderOpen : Folder;
          const siblings = rows.filter((r) => r.parent === row.parent && r.path !== ROOT);
          return (
            <div
              key={row.path || "root"}
              role="treeitem"
              ref={(node) => {
                if (node) items.current.set(row.path, node);
                else items.current.delete(row.path);
              }}
              tabIndex={focused === row.path ? 0 : -1}
              aria-level={row.level}
              aria-setsize={row.path === ROOT ? 1 : siblings.length}
              aria-posinset={row.path === ROOT ? 1 : siblings.findIndex((r) => r.path === row.path) + 1}
              aria-expanded={isOpen}
              aria-selected={chosen === row.path}
              aria-disabled={row.allowed ? undefined : true}
              onFocus={(event) => event.target === event.currentTarget && setFocused(row.path)}
              onClick={() => {
                setFocused(row.path);
                if (row.allowed) setChosen(row.path);
              }}
              style={{ paddingLeft: `${(row.level - 1) * 16 + 8}px` }}
              className={cn(
                "flex cursor-pointer items-center gap-2 py-1 pr-3 hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring",
                chosen === row.path && "bg-accent",
                !row.allowed && "cursor-default opacity-50 hover:bg-transparent",
              )}
            >
              <button
                type="button"
                tabIndex={-1}
                aria-hidden
                onClick={(event) => {
                  event.stopPropagation();
                  if (isOpen) collapse(row.path);
                  else expand(row.path);
                }}
                className="shrink-0 text-muted-foreground"
              >
                {isOpen ? <ChevronDown className="size-4" aria-hidden /> : <ChevronRight className="size-4" aria-hidden />}
              </button>
              <Icon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
              <span className="min-w-0 flex-1 truncate">{row.name}</span>
            </div>
          );
        })}
      </div>
      <div className="flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
        <Button type="button" variant="outline" onClick={onClose}>
          Cancel
        </Button>
        <GuardedButton type="button" reasons={[chosen === null && "Choose a folder.", moving && "The entry is being moved"]} onClick={move}>
          Move
        </GuardedButton>
      </div>
    </div>
  );
}
