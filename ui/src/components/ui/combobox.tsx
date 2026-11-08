import * as React from "react";

import { Input } from "@/components/ui/input";
import { Popover, PopoverAnchor, PopoverContent } from "@/components/ui/popover";
import { cn } from "@/lib/utils";

export interface ComboboxOption {
  /** What the field holds once chosen; may be empty (the "default" entry). */
  value: string;
  /** What the list shows. */
  label: string;
  /** A secondary text next to the label (the readable name of a model). */
  detail?: string;
}

export interface ComboboxProps
  extends Omit<React.ComponentProps<"input">, "value" | "onChange" | "role" | "type"> {
  value: string;
  /** Called with the text on every keystroke. */
  onValueChange: (text: string) => void;
  /** Called when the text is final: on blur, on Enter and when a suggestion is chosen. */
  onCommit: (text: string) => void;
  options: readonly ComboboxOption[];
}

function matches(option: ComboboxOption, query: string): boolean {
  if (query === "") return true;
  return [option.value, option.label, option.detail ?? ""].some((text) => text.toLowerCase().includes(query));
}

/**
 * A text field with suggestions: free typing, the list filtered while typing, arrows to move in
 * it, Enter to choose, Esc to close. The typed text is never checked against the list.
 */
const Combobox = React.forwardRef<HTMLInputElement, ComboboxProps>(
  ({ value, onValueChange, onCommit, options, onFocus, onBlur, onKeyDown, onClick, className, id, ...props }, ref) => {
    const listId = React.useId();
    const inputRef = React.useRef<HTMLInputElement | null>(null);
    const [open, setOpen] = React.useState(false);
    const [typed, setTyped] = React.useState(false);
    const [active, setActive] = React.useState(-1);

    const query = typed ? value.trim().toLowerCase() : "";
    const shown = options.filter((option) => matches(option, query));
    const expanded = open && shown.length > 0;
    const optionId = (index: number) => `${listId}-${index}`;

    const choose = (option: ComboboxOption) => {
      setOpen(false);
      setTyped(false);
      setActive(-1);
      onValueChange(option.value);
      onCommit(option.value);
    };

    const move = (step: 1 | -1) => {
      if (shown.length === 0) return;
      setOpen(true);
      setActive((current) => (step === 1 ? (current + 1) % shown.length : current <= 0 ? shown.length - 1 : current - 1));
    };

    return (
      <Popover open={expanded} onOpenChange={setOpen}>
        <PopoverAnchor asChild>
          <div className="w-full">
            <Input
              {...props}
              id={id}
              ref={(node) => {
                inputRef.current = node;
                if (typeof ref === "function") ref(node);
                else if (ref) ref.current = node;
              }}
              role="combobox"
              autoComplete="off"
              aria-expanded={expanded}
              aria-controls={expanded ? listId : undefined}
              aria-autocomplete="list"
              aria-activedescendant={expanded && active >= 0 ? optionId(active) : undefined}
              className={className}
              value={value}
              onChange={(event) => {
                setTyped(true);
                setActive(-1);
                setOpen(true);
                onValueChange(event.target.value);
              }}
              onFocus={(event) => {
                setTyped(false);
                setOpen(true);
                onFocus?.(event);
              }}
              onClick={(event) => {
                setOpen(true);
                onClick?.(event);
              }}
              onBlur={(event) => {
                setOpen(false);
                setActive(-1);
                onBlur?.(event);
                onCommit(event.target.value);
              }}
              onKeyDown={(event) => {
                onKeyDown?.(event);
                if (event.defaultPrevented) return;
                if (event.key === "ArrowDown") {
                  event.preventDefault();
                  move(1);
                } else if (event.key === "ArrowUp") {
                  event.preventDefault();
                  move(-1);
                } else if (event.key === "Escape" && expanded) {
                  event.preventDefault();
                  event.stopPropagation();
                  setOpen(false);
                  setActive(-1);
                } else if (event.key === "Enter") {
                  event.preventDefault();
                  const option = expanded ? shown[active] : undefined;
                  if (option) choose(option);
                  else {
                    setOpen(false);
                    onCommit(event.currentTarget.value);
                  }
                }
              }}
            />
          </div>
        </PopoverAnchor>
        <PopoverContent
          align="start"
          className="max-h-64 w-[var(--radix-popover-trigger-width)] overflow-auto p-1"
          onOpenAutoFocus={(event) => event.preventDefault()}
          onCloseAutoFocus={(event) => event.preventDefault()}
          onInteractOutside={(event) => {
            if (event.target === inputRef.current) event.preventDefault();
          }}
        >
          <div role="listbox" id={listId} aria-label="Suggestions">
            {shown.map((option, index) => (
              <div
                key={option.value}
                id={optionId(index)}
                role="option"
                aria-selected={option.value === value}
                // Keep the focus in the field: a blur would save the typed text before the choice.
                onMouseDown={(event) => event.preventDefault()}
                onClick={() => choose(option)}
                className={cn(
                  "flex cursor-default select-none items-baseline gap-2 rounded-md px-2 py-1.5 text-sm",
                  index === active ? "bg-accent text-accent-foreground" : "hover:bg-accent hover:text-accent-foreground",
                )}
              >
                <span className="truncate">{option.label}</span>
                {option.detail ? <span className="truncate text-xs text-muted-foreground">{option.detail}</span> : null}
              </div>
            ))}
          </div>
        </PopoverContent>
      </Popover>
    );
  },
);
Combobox.displayName = "Combobox";

export { Combobox };
