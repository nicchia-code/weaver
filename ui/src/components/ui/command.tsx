import { Search } from "lucide-react";
import * as React from "react";

import { cn } from "@/lib/utils";

interface CommandState {
  listId: string;
  /** The `value` of the highlighted item, or `null` while the list is empty. */
  active: string | null;
  setActive: (value: string | null) => void;
  /** Chooses the item with this `value`. */
  choose: (value: string) => void;
}

const CommandContext = React.createContext<CommandState | null>(null);

function useCommand(): CommandState {
  const value = React.useContext(CommandContext);
  if (!value) throw new Error("A Command part needs a Command");
  return value;
}

/** The items a `Command` shows right now, in the order of the page. */
function optionsOf(root: HTMLElement | null): HTMLElement[] {
  return root ? Array.from(root.querySelectorAll<HTMLElement>('[role="option"]')) : [];
}

/** The id of the item `value`, safe in an `aria-activedescendant`. */
const itemId = (listId: string, value: string) => `${listId}-${encodeURIComponent(value)}`;

/**
 * A searchable list: a `CommandInput` that keeps the focus, and a `CommandList` of `CommandItem`s
 * the input's arrow keys walk through (`aria-activedescendant`, so the focus never leaves the
 * field), Enter choosing the highlighted one. The filtering belongs to the caller, who passes only
 * the items that match, so the list never holds a stale item; the first item is highlighted again
 * whenever the items change. Same parts, names and keys as shadcn's `Command`.
 */
function Command({
  className,
  children,
  onChoose,
  ...props
}: Omit<React.HTMLAttributes<HTMLDivElement>, "onChange"> & { onChoose: (value: string) => void }) {
  const listId = React.useId();
  const root = React.useRef<HTMLDivElement>(null);
  const [active, setActive] = React.useState<string | null>(null);

  // Highlight the first item whenever the set of items changes (or the highlighted one is gone).
  React.useLayoutEffect(() => {
    const options = optionsOf(root.current).map((option) => option.dataset.value ?? "");
    setActive((current) => (current !== null && options.includes(current) ? current : (options[0] ?? null)));
  });

  const state = React.useMemo<CommandState>(() => ({ listId, active, setActive, choose: onChoose }), [listId, active, onChoose]);

  const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    props.onKeyDown?.(event);
    if (event.defaultPrevented) return;
    const options = optionsOf(root.current).map((option) => option.dataset.value ?? "");
    if (options.length === 0) return;
    const index = active === null ? -1 : options.indexOf(active);
    let next: number | null = null;
    if (event.key === "ArrowDown") next = (index + 1) % options.length;
    else if (event.key === "ArrowUp") next = index <= 0 ? options.length - 1 : index - 1;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = options.length - 1;
    else if (event.key === "Enter" && active !== null) {
      event.preventDefault();
      onChoose(active);
      return;
    }
    if (next !== null) {
      event.preventDefault();
      const value = options[next] ?? null;
      setActive(value);
      optionsOf(root.current)[next]?.scrollIntoView?.({ block: "nearest" });
    }
  };

  return (
    <CommandContext.Provider value={state}>
      <div ref={root} className={cn("flex w-full flex-col overflow-hidden rounded-md border bg-popover text-popover-foreground", className)} {...props} onKeyDown={onKeyDown}>
        {children}
      </div>
    </CommandContext.Provider>
  );
}

/** The search field: a combobox over the list, the DOM focus staying in it. */
const CommandInput = React.forwardRef<HTMLInputElement, Omit<React.ComponentProps<"input">, "role" | "type" | "value" | "onChange"> & { value: string; onValueChange: (text: string) => void }>(
  ({ className, value, onValueChange, ...props }, ref) => {
    const { listId, active } = useCommand();
    return (
      <div className="flex items-center gap-2 border-b px-3">
        <Search className="size-4 shrink-0 text-muted-foreground" aria-hidden />
        <input
          ref={ref}
          {...props}
          type="text"
          role="combobox"
          autoComplete="off"
          aria-expanded
          aria-controls={listId}
          aria-autocomplete="list"
          aria-activedescendant={active !== null ? itemId(listId, active) : undefined}
          value={value}
          onChange={(event) => onValueChange(event.target.value)}
          className={cn("flex h-10 w-full bg-transparent py-3 text-sm outline-none placeholder:text-muted-foreground disabled:cursor-not-allowed disabled:opacity-50", className)}
        />
      </div>
    );
  },
);
CommandInput.displayName = "CommandInput";

function CommandList({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  const { listId } = useCommand();
  return <div id={listId} role="listbox" className={cn("max-h-72 overflow-y-auto overflow-x-hidden p-1", className)} {...props} />;
}

/** What the list says when it holds no item (`No other branch`). */
function CommandEmpty({ className, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <p className={cn("py-6 text-center text-sm text-muted-foreground", className)} {...props} />;
}

/** Items under a heading. A group with no item is not rendered by the caller. */
function CommandGroup({ heading, className, children, ...props }: Omit<React.HTMLAttributes<HTMLDivElement>, "role"> & { heading: string }) {
  const headingId = React.useId();
  return (
    <div role="group" aria-labelledby={headingId} className={cn("overflow-hidden p-1", className)} {...props}>
      <div id={headingId} className="px-2 py-1.5 text-xs font-medium text-muted-foreground">
        {heading}
      </div>
      {children}
    </div>
  );
}

/** One choice; `value` identifies it to `onChoose`. */
function CommandItem({ value, className, children, ...props }: Omit<React.HTMLAttributes<HTMLDivElement>, "role" | "id" | "onClick"> & { value: string }) {
  const { listId, active, setActive, choose } = useCommand();
  const selected = active === value;
  return (
    <div
      {...props}
      id={itemId(listId, value)}
      role="option"
      data-value={value}
      aria-selected={selected}
      onMouseMove={() => (selected ? undefined : setActive(value))}
      onClick={() => choose(value)}
      className={cn("relative flex cursor-pointer select-none items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-none", selected && "bg-accent text-accent-foreground", className)}
    >
      {children}
    </div>
  );
}

export { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList };
