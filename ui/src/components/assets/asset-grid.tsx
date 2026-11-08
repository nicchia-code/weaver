import { Pencil, Trash2, TriangleAlert } from "lucide-react";
import { useEffect, useRef, type KeyboardEvent, type MouseEvent, type PointerEvent, type ReactNode, type RefObject } from "react";

import type { Asset } from "@/bindings/Asset";
import { AssetNameField } from "@/components/assets/asset-name-field";
import { iconOf, isLarge, isMacPlatform, LARGE_TEXT, tooltipOf } from "@/components/assets/asset-kinds";
import { click, moveActive, NOTHING, only, selectAll, type Move, type Selection } from "@/components/assets/selection";
import { ContextMenu, ContextMenuContent, ContextMenuItem, ContextMenuTrigger } from "@/components/ui/context-menu";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

/** How far the pointer moves with the button down before it is a drag out of Weaver, not a click. */
export const DRAG_THRESHOLD_PX = 4;

/** Why `Rename` is shown disabled when more than one asset is selected. */
export const RENAME_ONE = "Rename one asset at a time.";

const RENAME_REASON_ID = "asset-grid-rename-reason";

const ARROWS: Record<string, Move> = {
  ArrowLeft: "left",
  ArrowRight: "right",
  ArrowUp: "up",
  ArrowDown: "down",
  Home: "home",
  End: "end",
};

export interface AssetGridProps {
  /** The assets in grid order. */
  assets: readonly Asset[];
  selection: Selection;
  onSelection: (next: Selection) => void;
  /** The name of the asset whose tile shows the rename field. */
  renaming: string | null;
  /** `F2`, or `Rename` of the menu: the asset `name` is to be renamed. */
  onRenameRequest: (name: string) => void;
  onRenameCommit: (name: string, newName: string) => Promise<boolean>;
  onRenameCancel: (refocus: boolean) => void;
  /** `Delete`, or `Delete` of the menu: the selected assets are to be deleted (the view asks first). */
  onDeleteRequest: () => void;
  /** The pointer moved past the threshold on the tile `name`: a drag out of Weaver begins with these assets. */
  onDragOut: (names: string[]) => void;
  /** The asset whose tile is scrolled into view once it is on screen. */
  reveal: string | null;
  onRevealed: () => void;
  /** Receives the tile that has the place in the tab order, for a dialog that closes to give the focus back to. */
  focusRef: RefObject<HTMLElement | null>;
}

/** A menu item shown disabled with its reason, in a tooltip and as its description. */
function BlockedItem({ children, reason }: { children: ReactNode; reason: string }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <ContextMenuItem aria-disabled="true" aria-describedby={RENAME_REASON_ID} onSelect={(event) => event.preventDefault()}>
          {children}
        </ContextMenuItem>
      </TooltipTrigger>
      <TooltipContent side="right">{reason}</TooltipContent>
    </Tooltip>
  );
}

/**
 * The assets as a grid of tiles in name order (`requirements/ui#asset-view`): an ARIA `grid` of
 * `gridcell`s with `aria-multiselectable`. A click selects one tile, `Cmd`+click (`Ctrl`+click
 * elsewhere) adds or removes one, `Shift`+click selects the range from the last clicked tile, a
 * click on the empty space clears the selection, the arrow keys move the active tile (`Shift`
 * extends the selection) and `Cmd+A` / `Ctrl+A` selects every asset. A double click, Enter or
 * Space only selects. A tile has the menu `Rename` / `Delete` (right click, the Menu key or
 * `Shift+F10`), `F2` renames the selected asset and `Delete` (`Cmd+Backspace` on macOS) deletes the
 * selection. Pressing on a tile and moving 4 px drags the selection, or the tile alone when it is
 * not selected, out of Weaver (pointer events, never the HTML5 drag and drop API).
 */
export function AssetGrid({
  assets,
  selection,
  onSelection,
  renaming,
  onRenameRequest,
  onRenameCommit,
  onRenameCancel,
  onDeleteRequest,
  onDragOut,
  reveal,
  onRevealed,
  focusRef,
}: AssetGridProps) {
  const grid = useRef<HTMLDivElement>(null);
  const order = assets.map((asset) => asset.name);
  const tiles = useRef(new Map<string, HTMLElement>());
  /** The tile to focus once it is on screen. */
  const wantFocus = useRef<string | null>(null);
  /** The press on a tile that may become a drag out, and whether it did. */
  const press = useRef<{ pointerId: number; x: number; y: number; name: string; tile: HTMLElement; started: boolean } | null>(null);
  /** Set when a drag out began: the click that may follow is not a click on a tile. */
  const swallowClick = useRef(false);
  /** Set when `Rename` was chosen in the menu: the focus goes to the field, not back to the tile. */
  const choseRename = useRef(false);
  const mac = isMacPlatform();

  const { selected, active } = selection;
  // The one tile in the tab order: the active one, or the first.
  const tabbable = active !== null && order.includes(active) ? active : (order[0] ?? null);

  // Moves the focus once the tile the arrows went to is on screen.
  useEffect(() => {
    const wanted = wantFocus.current;
    if (wanted === null) return;
    const node = tiles.current.get(wanted);
    if (!node) return;
    wantFocus.current = null;
    node.focus();
  });

  useEffect(() => {
    focusRef.current = tabbable === null ? null : (tiles.current.get(tabbable) ?? null);
  });

  // A tile that was just copied in or renamed is scrolled into view once it is on screen, and takes
  // the focus when the focus was in the grid or nowhere (the tile it was on is gone).
  useEffect(() => {
    if (reveal === null) return;
    const node = tiles.current.get(reveal);
    if (!node) return;
    node.scrollIntoView({ block: "nearest" });
    const focused = document.activeElement;
    if (focused === null || focused === document.body || grid.current?.contains(focused)) node.focus({ preventScroll: true });
    onRevealed();
  });

  /** How many tiles a row holds: the tiles that share the top of the first one. */
  const columns = (): number => {
    const nodes = order.map((name) => tiles.current.get(name)).filter((node): node is HTMLElement => node !== undefined);
    const first = nodes[0];
    if (!first) return 1;
    const row = nodes.findIndex((node) => node.offsetTop !== first.offsetTop);
    return row < 0 ? nodes.length : row;
  };

  const goTo = (next: Selection) => {
    if (next.active !== null) wantFocus.current = next.active;
    onSelection(next);
  };

  const openMenuOn = (name: string | null) => {
    const node = name === null ? undefined : tiles.current.get(name);
    if (!node) return;
    const rect = node.getBoundingClientRect();
    node.dispatchEvent(
      new MouseEvent("contextmenu", { bubbles: true, cancelable: true, button: 2, clientX: rect.left + rect.width / 2, clientY: rect.top + rect.height / 2 }),
    );
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.target instanceof HTMLInputElement) return;
    const modifier = mac ? event.metaKey : event.ctrlKey;
    if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) {
      event.preventDefault();
      openMenuOn(tabbable);
      return;
    }
    if (modifier && !event.altKey && event.key.toLowerCase() === "a") {
      event.preventDefault();
      onSelection(selectAll(selection, order));
      return;
    }
    if (event.altKey || event.ctrlKey || (event.metaKey && event.key !== "Backspace")) return;
    const move = ARROWS[event.key];
    if (move) {
      event.preventDefault();
      goTo(moveActive({ ...selection, active: tabbable }, order, move, columns(), event.shiftKey));
      return;
    }
    switch (event.key) {
      case "Enter":
      case " ":
        // Only selects: nothing opens.
        if (tabbable !== null) onSelection(only(tabbable));
        break;
      case "F2":
        if (selected.length === 1 && selected[0] !== undefined) onRenameRequest(selected[0]);
        break;
      case "Delete":
        if (selected.length > 0) onDeleteRequest();
        break;
      case "Backspace":
        if (!(event.metaKey && mac) || selected.length === 0) return;
        onDeleteRequest();
        break;
      default:
        return;
    }
    event.preventDefault();
  };

  // --- the drag out ---------------------------------------------------------------------------

  const tileOf = (event: { target: EventTarget }) => (event.target as HTMLElement).closest<HTMLElement>('[role="gridcell"]');

  const endPress = () => {
    const ended = press.current;
    press.current = null;
    if (!ended) return;
    if (ended.tile.hasPointerCapture(ended.pointerId)) ended.tile.releasePointerCapture(ended.pointerId);
  };

  const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
    swallowClick.current = false;
    if (event.button !== 0 || press.current || event.target instanceof HTMLInputElement) return;
    const tile = tileOf(event);
    const name = tile?.dataset.name;
    if (!tile || name === undefined || name === renaming) return;
    try {
      // The tile keeps the pointer until the press ends, so the click after a press that did not move stays the tile's.
      tile.setPointerCapture(event.pointerId);
    } catch {
      return;
    }
    press.current = { pointerId: event.pointerId, x: event.clientX, y: event.clientY, name, tile, started: false };
  };

  const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
    const current = press.current;
    if (!current || event.pointerId !== current.pointerId || current.started) return;
    if (Math.hypot(event.clientX - current.x, event.clientY - current.y) < DRAG_THRESHOLD_PX) return;
    current.started = true;
    swallowClick.current = true;
    // The selection when the tile is one of its tiles, otherwise the tile alone; the system takes the drag over from here.
    const names = selected.includes(current.name) ? [...selected] : [current.name];
    endPress();
    onDragOut(names);
  };

  const onPointerEnd = (event: PointerEvent<HTMLDivElement>) => {
    if (press.current && event.pointerId === press.current.pointerId) endPress();
  };

  // --- the tiles ------------------------------------------------------------------------------

  const onTileClick = (event: MouseEvent<HTMLDivElement>, name: string) => {
    event.stopPropagation();
    if (swallowClick.current) {
      swallowClick.current = false;
      return;
    }
    if (name === renaming) return;
    onSelection(click(selection, order, name, { toggle: mac ? event.metaKey : event.ctrlKey, range: event.shiftKey }));
  };

  const onTileContextMenu = (name: string) => {
    // A right click on a tile outside the selection selects that tile alone first.
    if (!selected.includes(name)) onSelection(only(name));
  };

  return (
    <div
      ref={grid}
      role="grid"
      aria-label="Assets"
      aria-multiselectable="true"
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerEnd}
      onPointerCancel={onPointerEnd}
      onLostPointerCapture={onPointerEnd}
      onClick={(event) => {
        // The empty space: nothing is selected any more.
        if (!tileOf(event)) onSelection(NOTHING);
      }}
      className="min-h-full p-4"
    >
      <span id={RENAME_REASON_ID} className="sr-only">
        {RENAME_ONE}
      </span>
      <div role="row" className="flex flex-wrap content-start gap-2">
        {assets.map((asset) => {
          const { name } = asset;
          const isSelected = selected.includes(name);
          const Icon = iconOf(name);
          const editing = renaming === name;
          return (
            <ContextMenu key={name}>
              <Tooltip>
                <TooltipTrigger asChild>
                  <ContextMenuTrigger asChild>
                    <div
                      role="gridcell"
                      ref={(node) => {
                        if (node) tiles.current.set(name, node);
                        else tiles.current.delete(name);
                      }}
                      data-name={name}
                      tabIndex={name === tabbable ? 0 : -1}
                      aria-selected={isSelected}
                      onFocus={(event) => {
                        // The focus on the tile makes it the active one; the selection stays as it is.
                        if (event.target === event.currentTarget && active !== name) onSelection({ ...selection, active: name });
                      }}
                      onClick={(event) => onTileClick(event, name)}
                      onContextMenu={() => onTileContextMenu(name)}
                      className={cn(
                        "relative flex w-[104px] cursor-default select-none flex-col items-center gap-1 rounded-md p-2 text-center text-sm hover:bg-accent/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                        isSelected && "bg-accent ring-2 ring-ring",
                      )}
                    >
                      <span className="relative">
                        <Icon aria-hidden className="size-10 text-muted-foreground" />
                        {isLarge(asset.size) ? (
                          <Tooltip>
                            <TooltipTrigger asChild>
                              <span role="img" aria-label={LARGE_TEXT} className="absolute -right-2 -top-1 rounded-full bg-background text-peach-ink">
                                <TriangleAlert aria-hidden className="size-4" />
                              </span>
                            </TooltipTrigger>
                            <TooltipContent>{LARGE_TEXT}</TooltipContent>
                          </Tooltip>
                        ) : null}
                      </span>
                      {editing ? (
                        <AssetNameField
                          initial={name}
                          onCommit={(newName) => onRenameCommit(name, newName)}
                          onCancel={(refocus) => {
                            onRenameCancel(refocus);
                            if (refocus) wantFocus.current = name;
                          }}
                        />
                      ) : (
                        <span className="line-clamp-2 w-full [overflow-wrap:anywhere]">{name}</span>
                      )}
                    </div>
                  </ContextMenuTrigger>
                </TooltipTrigger>
                {editing ? null : <TooltipContent>{tooltipOf(asset)}</TooltipContent>}
              </Tooltip>
              <ContextMenuContent
                onCloseAutoFocus={(event) => {
                  // `Rename` moves the focus into the field of the tile; the menu must not take it back.
                  if (choseRename.current) {
                    choseRename.current = false;
                    event.preventDefault();
                  }
                }}
              >
                {selected.length === 1 ? (
                  <ContextMenuItem
                    onSelect={() => {
                      choseRename.current = true;
                      onRenameRequest(name);
                    }}
                  >
                    <Pencil aria-hidden />
                    Rename
                  </ContextMenuItem>
                ) : (
                  <BlockedItem reason={RENAME_ONE}>
                    <Pencil aria-hidden />
                    Rename
                  </BlockedItem>
                )}
                <ContextMenuItem onSelect={onDeleteRequest}>
                  <Trash2 aria-hidden />
                  Delete
                </ContextMenuItem>
              </ContextMenuContent>
            </ContextMenu>
          );
        })}
      </div>
    </div>
  );
}
