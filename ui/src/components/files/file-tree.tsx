import { ChevronDown, ChevronRight, File, FilePlus, Folder, FolderInput, FolderOpen, FolderPlus, Lock, Pencil, Trash2 } from "lucide-react";
import {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type PointerEvent,
  type ReactNode,
} from "react";
import { createPortal } from "react-dom";
import type { DragDropEvent } from "@tauri-apps/api/webview";

import type { FileEntry } from "@/bindings/FileEntry";
import { useFileTabs } from "@/components/files/file-tabs-context";
import {
  AGENT_EDITING,
  canDrop,
  inWeaver,
  nameOf,
  NEW_ROW,
  ROOT,
  selection,
  WEAVER_READ_ONLY,
  type Row,
} from "@/components/files/tree-model";
import { useDragAids, type Point } from "@/components/files/use-drag-aids";
import type { FileTree as Tree } from "@/components/files/use-file-tree";
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuSeparator, ContextMenuTrigger } from "@/components/ui/context-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { onDragDrop } from "@/lib/backend";
import { cn } from "@/lib/utils";

/** What the view can ask of the tree from outside. */
export interface FileTreeHandle {
  /** Puts the focus on the selected row (or the first). */
  focus: () => void;
}

export interface FileTreeProps {
  tree: Tree;
  /** `Move to…` was chosen for this path. */
  onRequestMove: (path: string) => void;
  /** `Delete` (menu or key) was chosen for this path. */
  onRequestDelete: (path: string) => void;
}

/** How far the pointer must move with the button held before a press on a row becomes a drag. */
const DRAG_THRESHOLD_PX = 4;

/** Where the drag label sits from the pointer. */
const LABEL_OFFSET_PX = 12;

/** A press on a row that may become a drag. */
interface Press {
  pointerId: number;
  /** Where it started. */
  x: number;
  y: number;
  entry: FileEntry;
  /** The row that holds the pointer until the press ends. */
  row: HTMLElement;
  /** The pointer moved past the threshold: it is a drag. */
  started: boolean;
}

const REASON_ID = "file-tree-read-only-reason";

/** Focus the row that is selected when the edit of a name ends. */
const SELECTED = Symbol("selected");

/** The text field of a row being made or renamed: Enter confirms, Esc or leaving the field cancels. */
function NameField({
  initial,
  folder,
  label,
  onCommit,
  onCancel,
}: {
  initial: string;
  folder: boolean;
  label: string;
  onCommit: (name: string) => Promise<boolean>;
  /** The field ended without a name: `refocus` is whether the row gets the focus (Esc), not when the developer went elsewhere. */
  onCancel: (refocus: boolean) => void;
}) {
  const field = useRef<HTMLInputElement>(null);
  const finished = useRef(false);

  useEffect(() => {
    const input = field.current;
    if (!input) return;
    input.focus();
    const [from, to] = selection(initial, folder);
    input.setSelectionRange(from, to);
  }, [initial, folder]);

  const confirm = () => {
    if (finished.current) return;
    finished.current = true;
    onCommit(field.current?.value ?? "")
      .then((done) => {
        // A refused name stays in the field, for the developer to correct.
        if (!done) finished.current = false;
      })
      .catch(() => {
        finished.current = false;
      });
  };

  return (
    <input
      ref={field}
      type="text"
      aria-label={label}
      defaultValue={initial}
      spellCheck={false}
      autoComplete="off"
      onClick={(event) => event.stopPropagation()}
      onKeyDown={(event) => {
        event.stopPropagation();
        if (event.key === "Enter") {
          event.preventDefault();
          confirm();
        } else if (event.key === "Escape") {
          event.preventDefault();
          finished.current = true;
          onCancel(true);
        }
      }}
      onBlur={() => {
        if (finished.current) return;
        finished.current = true;
        onCancel(false);
      }}
      className="h-6 min-w-0 flex-1 rounded-sm border bg-background px-1 text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    />
  );
}

/** A menu item that is shown disabled with the reason, in a tooltip and as its description. */
function BlockedItem({ children, reason }: { children: ReactNode; reason: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <ContextMenuItem aria-disabled="true" aria-describedby={REASON_ID} onSelect={(event) => event.preventDefault()}>
          {children}
        </ContextMenuItem>
      </TooltipTrigger>
      <TooltipContent side="right">{reason}</TooltipContent>
    </Tooltip>
  );
}

const isMac = () => typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

/**
 * The tree of the project's files: an ARIA `tree` with the entries the backend lists. Up and Down
 * move, Right opens a folder or goes to its first child, Left closes it or goes to its parent,
 * Enter opens a file in a tab or toggles a folder (a click does the same), `F2` renames and `Delete`
 * (`Cmd+Backspace` on macOS) deletes the selected row. A right click, the Menu key or `Shift+F10`
 * opens the menu of the row, and of the root on the empty space. A row dragged onto a folder, or
 * onto the empty space, moves there (pointer events), and files dragged in from outside are
 * copied there; both highlight the target, expand a folder held over for a second and scroll the
 * tree near its edges.
 */
export const FileTree = forwardRef<FileTreeHandle, FileTreeProps>(function FileTree({ tree, onRequestMove, onRequestDelete }, handle) {
  const tabs = useFileTabs();
  const container = useRef<HTMLDivElement>(null);
  const items = useRef(new Map<string, HTMLElement>());
  /** The row to focus once it is on screen: a path, or `SELECTED` for the row selected by then. */
  const wantFocus = useRef<string | typeof SELECTED | null>(null);
  const keepFocus = useRef(false);
  const [menuTarget, setMenuTarget] = useState<string>(ROOT);
  const [dragging, setDragging] = useState<FileEntry | null>(null);
  const [dropTarget, setDropTarget] = useState<string | null>(null);

  const { rows, selected, editing } = tree;
  const current = rows.find((row) => !row.isNew && row.entry.path === selected) ?? rows.find((row) => !row.isNew) ?? null;

  // Moves the focus once the row the arrows went to is on screen.
  useEffect(() => {
    const wanted = wantFocus.current;
    if (wanted === null) return;
    const path = wanted === SELECTED ? selected : wanted;
    const node = path === null ? undefined : items.current.get(path);
    if (!node) return;
    wantFocus.current = null;
    node.focus();
  });

  useImperativeHandle(handle, () => ({
    focus: () => {
      const path = current?.entry.path;
      if (path !== undefined) items.current.get(path)?.focus();
    },
  }));

  const moveTo = (path: string) => {
    wantFocus.current = path;
    tree.select(path);
  };

  const openEntry = (entry: FileEntry) => {
    if (entry.kind === "folder") tree.toggle(entry.path);
    else tabs.openFile(entry.path);
  };

  const requestRename = (path: string) => {
    if (!inWeaver(path)) tree.startRename(path);
  };

  const requestDelete = (path: string) => {
    if (!inWeaver(path)) onRequestDelete(path);
  };

  const openMenuOn = (row: HTMLElement | null) => {
    const target = row ?? container.current;
    if (!target) return;
    const rect = target.getBoundingClientRect();
    target.dispatchEvent(
      new MouseEvent("contextmenu", { bubbles: true, cancelable: true, button: 2, clientX: rect.left + 16, clientY: rect.top + rect.height / 2 }),
    );
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.target instanceof HTMLInputElement) return;
    if (!current) return;
    const { entry } = current;
    const real = rows.filter((row) => !row.isNew);
    const position = real.indexOf(current);
    const next = real[position + 1];
    const previous = real[position - 1];
    if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) {
      event.preventDefault();
      openMenuOn(items.current.get(entry.path) ?? null);
      return;
    }
    if (event.altKey || event.ctrlKey || (event.metaKey && event.key !== "Backspace")) return;
    switch (event.key) {
      case "ArrowDown":
        if (next) moveTo(next.entry.path);
        break;
      case "ArrowUp":
        if (previous) moveTo(previous.entry.path);
        break;
      case "Home":
        moveTo(real[0]?.entry.path ?? entry.path);
        break;
      case "End":
        moveTo(real.at(-1)?.entry.path ?? entry.path);
        break;
      case "ArrowRight":
        if (entry.kind !== "folder") break;
        if (!tree.isExpanded(entry.path)) tree.expand(entry.path);
        else if (next?.parent === entry.path) moveTo(next.entry.path);
        break;
      case "ArrowLeft":
        if (entry.kind === "folder" && tree.isExpanded(entry.path)) tree.collapse(entry.path);
        else if (current.parent !== ROOT) moveTo(current.parent);
        break;
      case "Enter":
        openEntry(entry);
        break;
      case "F2":
        requestRename(entry.path);
        break;
      case "Delete":
        requestDelete(entry.path);
        break;
      case "Backspace":
        if (!(event.metaKey && isMac())) return;
        requestDelete(entry.path);
        break;
      default:
        return;
    }
    event.preventDefault();
  };

  // --- the menu -------------------------------------------------------------------------------

  const rowOf = (event: { target: EventTarget }) => (event.target as HTMLElement).closest<HTMLElement>('[role="treeitem"]');

  const onContextMenu = (event: MouseEvent<HTMLDivElement>) => {
    const row = rowOf(event);
    const path = row?.dataset.path;
    if (path === undefined || path === NEW_ROW) {
      setMenuTarget(ROOT);
      return;
    }
    setMenuTarget(path);
    tree.select(path);
  };

  const target = menuTarget === ROOT ? null : tree.entryOf(menuTarget);
  const blocked = inWeaver(menuTarget);
  const holdsFolder = menuTarget === ROOT || target?.kind === "folder";

  const choose = (action: () => void) => () => {
    keepFocus.current = true;
    action();
  };

  const item = (label: string, icon: ReactNode, action: () => void) =>
    blocked ? (
      <BlockedItem reason={WEAVER_READ_ONLY}>
        {icon}
        {label}
      </BlockedItem>
    ) : (
      <ContextMenuItem onSelect={choose(action)}>
        {icon}
        {label}
      </ContextMenuItem>
    );

  // --- dragging -------------------------------------------------------------------------------
  //
  // A row is dragged with pointer events, never the HTML5 drag and drop API: Tauri's native drag
  // and drop handler stays on in every window (it reports the paths of dropped files) and takes
  // the HTML5 events away from the page. Dropped files are reported by that handler too.

  const aids = useDragAids(container, tree.expand, tree.isExpanded);
  /** The press on a row that may become a drag, and whether it did. */
  const press = useRef<Press | null>(null);
  const label = useRef<HTMLDivElement>(null);
  const lastPoint = useRef<Point>({ x: 0, y: 0 });
  /** Set when a drag ended: the click that follows it must not open or toggle the row. */
  const swallowClick = useRef(false);

  /**
   * The folder a drop on `element` would go to: a folder row, the empty string (the root) for the
   * empty space of the tree, and `null` for a file, the row being made, or anything outside the tree.
   */
  const folderUnder = (element: Element | null): string | null => {
    const treeNode = container.current;
    if (!element || !treeNode || !treeNode.contains(element)) return null;
    const row = element.closest<HTMLElement>('[role="treeitem"]');
    if (!row || !treeNode.contains(row)) return ROOT;
    const path = row.dataset.path;
    if (path === undefined || path === NEW_ROW) return null;
    return tree.entryOf(path)?.kind === "folder" ? path : null;
  };

  const endDrag = useCallback(() => {
    const ended = press.current;
    press.current = null;
    aids.stop();
    setDragging(null);
    setDropTarget(null);
    if (!ended) return;
    swallowClick.current = ended.started;
    if (ended.row.hasPointerCapture(ended.pointerId)) ended.row.releasePointerCapture(ended.pointerId);
  }, [aids]);

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    swallowClick.current = false;
    if (event.button !== 0 || event.ctrlKey || press.current || event.target instanceof HTMLInputElement) return;
    const row = rowOf(event);
    const path = row?.dataset.path;
    if (!row || path === undefined || path === NEW_ROW || inWeaver(path)) return;
    const entry = tree.entryOf(path);
    if (!entry || (editing?.mode === "rename" && editing.path === path)) return;
    try {
      // The row keeps the pointer until the press ends, so the click that follows a press that did not move stays the row's.
      row.setPointerCapture(event.pointerId);
    } catch {
      return;
    }
    press.current = { pointerId: event.pointerId, x: event.clientX, y: event.clientY, entry, row, started: false };
  };

  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const current = press.current;
    if (!current || event.pointerId !== current.pointerId) return;
    const point = { x: event.clientX, y: event.clientY };
    if (!current.started) {
      if (Math.hypot(point.x - current.x, point.y - current.y) < DRAG_THRESHOLD_PX) return;
      current.started = true;
      lastPoint.current = point;
      setDragging(current.entry);
    }
    lastPoint.current = point;
    if (label.current) {
      label.current.style.left = `${point.x + LABEL_OFFSET_PX}px`;
      label.current.style.top = `${point.y + LABEL_OFFSET_PX}px`;
    }
    const folder = folderUnder(document.elementFromPoint(point.x, point.y));
    setDropTarget(folder !== null && canDrop(current.entry, folder) ? folder : null);
    aids.update(point, folder === current.entry.path ? null : folder);
  };

  const onPointerUp = (event: PointerEvent<HTMLDivElement>) => {
    const current = press.current;
    if (!current || event.pointerId !== current.pointerId) return;
    const folder = current.started ? folderUnder(document.elementFromPoint(event.clientX, event.clientY)) : null;
    endDrag();
    if (folder !== null && canDrop(current.entry, folder)) tree.move(current.entry.path, folder).catch(() => {});
  };

  /** The pointer was cancelled or lost its capture: the drag ends and nothing changes. */
  const onPointerEnd = (event: PointerEvent<HTMLDivElement>) => {
    if (press.current && event.pointerId === press.current.pointerId) endDrag();
  };

  // Esc ends the drag.
  useEffect(() => {
    if (!dragging) return;
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      event.stopPropagation();
      endDrag();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [dragging, endDrag]);

  // --- drops from outside ---------------------------------------------------------------------
  //
  // Files and folders dragged in from the Finder or Explorer arrive as events of Tauri's drag and
  // drop handler, with the pointer in physical pixels.

  const dropped = (event: DragDropEvent) => {
    if (event.type === "leave") {
      setDropTarget(null);
      aids.stop();
      return;
    }
    const ratio = window.devicePixelRatio || 1;
    const point = { x: event.position.x / ratio, y: event.position.y / ratio };
    const folder = folderUnder(document.elementFromPoint(point.x, point.y));
    if (event.type === "drop") {
      setDropTarget(null);
      aids.stop();
      // Outside the tree, or on a file: nothing happens here.
      if (folder !== null && event.paths.length > 0) void tree.copyIn(folder, event.paths);
      return;
    }
    setDropTarget(folder !== null && !inWeaver(folder) ? folder : null);
    aids.update(point, folder);
  };
  const droppedNow = useRef(dropped);
  droppedNow.current = dropped;

  // The view is shown while this tree is: listening starts and ends with it.
  useEffect(() => {
    let listening = true;
    const stop = onDragDrop((event) => {
      if (listening) droppedNow.current(event);
    });
    return () => {
      listening = false;
      stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, []);

  // A copied entry is scrolled into view once its row is on screen.
  useEffect(() => {
    if (tree.reveal === null) return;
    const node = items.current.get(tree.reveal);
    if (!node) return;
    node.scrollIntoView({ block: "nearest" });
    tree.clearReveal();
  });


  return (
    <>
    <ContextMenu>
      <ContextMenuTrigger asChild>
        <div
          ref={container}
          role="tree"
          aria-label="Project files"
          aria-multiselectable="false"
          tabIndex={current ? undefined : 0}
          onKeyDown={onKeyDown}
          onContextMenu={onContextMenu}
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerEnd}
          onLostPointerCapture={onPointerEnd}
          onClickCapture={(event) => {
            // The click that follows the end of a drag is not a click on a row.
            if (!swallowClick.current) return;
            swallowClick.current = false;
            event.stopPropagation();
            event.preventDefault();
          }}
          data-drop-target={dropTarget === ROOT ? "" : undefined}
          className={cn("min-h-0 flex-1 overflow-auto py-3 text-sm", dragging && "select-none", dropTarget === ROOT && "bg-accent/50")}
        >
          {rows.map((row) => (
            <TreeRow
              key={row.entry.path}
              row={row}
              tree={tree}
              focusable={current?.entry.path === row.entry.path}
              locked={tabs.lockedPaths.has(row.entry.path)}
              dropTarget={dropTarget === row.entry.path}
              register={(node) => {
                if (node) items.current.set(row.entry.path, node);
                else items.current.delete(row.entry.path);
              }}
              onOpen={() => openEntry(row.entry)}
              onEditEnd={(refocus) => {
                tree.stopEditing();
                if (refocus) wantFocus.current = SELECTED;
              }}
            />
          ))}
          {editing === null && rows.length === 0 && tree.ready ? <p className="px-3 py-2 text-muted-foreground">This folder is empty.</p> : null}
        </div>
      </ContextMenuTrigger>
      <ContextMenuContent
        aria-label="File actions"
        onCloseAutoFocus={(event) => {
          // A name being edited takes the focus; a dialog keeps its own; otherwise the row gets it back.
          event.preventDefault();
          const field = container.current?.querySelector("input");
          if (field) field.focus();
          else if (!keepFocus.current) (current ? items.current.get(selected ?? current.entry.path) : container.current)?.focus();
          keepFocus.current = false;
        }}
      >
        <span id={REASON_ID} className="sr-only">
          {WEAVER_READ_ONLY}
        </span>
        {holdsFolder ? item("New file", <FilePlus aria-hidden />, () => tree.startCreate(menuTarget, false)) : null}
        {holdsFolder ? item("New folder", <FolderPlus aria-hidden />, () => tree.startCreate(menuTarget, true)) : null}
        {menuTarget !== ROOT ? (
          <>
            <ContextMenuSeparator />
            {item("Rename", <Pencil aria-hidden />, () => tree.startRename(menuTarget))}
            {item("Move to…", <FolderInput aria-hidden />, () => onRequestMove(menuTarget))}
            {item("Delete", <Trash2 aria-hidden />, () => onRequestDelete(menuTarget))}
          </>
        ) : null}
      </ContextMenuContent>
    </ContextMenu>
    {dragging
      ? createPortal(
          <div
            ref={label}
            data-drag-label=""
            aria-hidden
            style={{ position: "fixed", left: lastPoint.current.x + LABEL_OFFSET_PX, top: lastPoint.current.y + LABEL_OFFSET_PX, pointerEvents: "none" }}
            className="z-50 flex max-w-64 items-center gap-2 rounded-lg bg-popover px-2 py-1 text-sm text-popover-foreground shadow-pop"
          >
            {dragging.kind === "folder" ? <Folder aria-hidden className="size-4 shrink-0 text-muted-foreground" /> : <File aria-hidden className="size-4 shrink-0 text-muted-foreground" />}
            <span className="truncate">{dragging.name}</span>
          </div>,
          document.body,
        )
      : null}
    </>
  );
});

function TreeRow({
  row,
  tree,
  focusable,
  locked,
  dropTarget,
  register,
  onOpen,
  onEditEnd,
}: {
  row: Row;
  tree: Tree;
  focusable: boolean;
  locked: boolean;
  dropTarget: boolean;
  register: (node: HTMLElement | null) => void;
  onOpen: () => void;
  /** The edit of the name ended; `refocus` says the row should have the focus. */
  onEditEnd: (refocus: boolean) => void;
}) {
  const { entry } = row;
  const folder = entry.kind === "folder";
  const expanded = folder && tree.isExpanded(entry.path);
  const Icon = folder ? (expanded ? FolderOpen : Folder) : File;
  const renaming = tree.editing?.mode === "rename" && tree.editing.path === entry.path;
  const creating = row.isNew === true;

  let label: ReactNode = <span className="min-w-0 flex-1 truncate">{entry.name}</span>;
  if (creating && tree.editing?.mode === "create") {
    const { parent, folder: isFolder } = tree.editing;
    label = (
      <NameField
        initial=""
        folder={isFolder}
        label={isFolder ? "New folder name" : "New file name"}
        onCommit={async (name) => {
          const done = await tree.create(parent, name, isFolder);
          if (done) onEditEnd(true);
          return done;
        }}
        onCancel={onEditEnd}
      />
    );
  } else if (renaming) {
    label = (
      <NameField
        initial={entry.name}
        folder={folder}
        label={`New name for ${nameOf(entry.path)}`}
        onCommit={async (name) => {
          const done = name === entry.name || (await tree.rename(entry.path, name));
          if (done) onEditEnd(true);
          return done;
        }}
        onCancel={onEditEnd}
      />
    );
  }

  return (
    <div
      role="treeitem"
      ref={register}
      data-path={entry.path}
      data-drop-target={dropTarget ? "" : undefined}
      tabIndex={focusable ? 0 : -1}
      aria-level={row.level}
      aria-setsize={row.setSize}
      aria-posinset={row.position}
      aria-expanded={folder && !creating ? expanded : undefined}
      aria-selected={tree.selected === entry.path}
      onFocus={(event) => {
        if (event.target === event.currentTarget && !creating) tree.select(entry.path);
      }}
      onClick={() => {
        if (creating || renaming) return;
        tree.select(entry.path);
        onOpen();
      }}
      style={{ paddingLeft: `${(row.level - 1) * 16 + 8}px` }}
      className={cn(
        "flex cursor-pointer items-center gap-2 py-1 pr-3 hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring",
        tree.selected === entry.path && "bg-accent/60",
        dropTarget && "bg-accent ring-2 ring-inset ring-ring",
      )}
    >
      {folder ? (
        expanded ? (
          <ChevronDown aria-hidden className="size-4 shrink-0 text-muted-foreground" />
        ) : (
          <ChevronRight aria-hidden className="size-4 shrink-0 text-muted-foreground" />
        )
      ) : (
        <span aria-hidden className="size-4 shrink-0" />
      )}
      <Icon aria-hidden className="size-4 shrink-0 text-muted-foreground" />
      {label}
      {locked ? (
        <Tooltip>
          <TooltipTrigger asChild>
            <span role="img" aria-label={AGENT_EDITING} className="shrink-0 text-muted-foreground">
              <Lock aria-hidden className="size-3.5" />
            </span>
          </TooltipTrigger>
          <TooltipContent>{AGENT_EDITING}</TooltipContent>
        </Tooltip>
      ) : null}
    </div>
  );
}
