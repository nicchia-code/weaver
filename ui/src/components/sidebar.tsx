import type { LucideIcon } from "lucide-react";

import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useCompactSidebar } from "@/hooks/use-media-query";
import { TONE_TEXT } from "@/lib/tones";
import { cn } from "@/lib/utils";
import { VIEWS, type ShellCounts, type ViewEntry, type ViewId } from "@/views";

/** The number next to an entry, and what it counts in words (for assistive technologies). */
interface EntryCount {
  value: number;
  meaning: string;
  warning: boolean;
  /** What the number says on hover and focus, when it needs more than its accessible text. */
  tooltip?: string;
}

function countOf(view: ViewId, counts: ShellCounts): EntryCount | null {
  if (view === "board" && counts.backlog > 0) {
    return { value: counts.backlog, meaning: counts.backlog === 1 ? "card in Backlog" : "cards in Backlog", warning: false };
  }
  if (view === "jobs" && counts.jobsWaiting > 0) {
    return { value: counts.jobsWaiting, meaning: counts.jobsWaiting === 1 ? "job waiting for you" : "jobs waiting for you", warning: true };
  }
  if (view === "general" && counts.chatWaiting) {
    return { value: 1, meaning: "round waiting for your answers", warning: true, tooltip: "Chat is waiting for your answers" };
  }
  return null;
}

/** One button of the sidebar: a view (selected one carries `aria-current`) or an action (never does). */
function Item({
  label,
  icon: Icon,
  selected,
  compact,
  count,
  onActivate,
  actionId,
}: {
  label: string;
  icon: LucideIcon;
  selected: boolean;
  compact: boolean;
  count: EntryCount | null;
  onActivate: () => void;
  actionId?: string;
}) {
  const button = (
    <button
      type="button"
      aria-current={selected ? "page" : undefined}
      onClick={onActivate}
      data-sidebar-action={actionId}
      className={cn(
        "relative flex w-full items-center gap-2 rounded-md px-3 py-2 text-sm font-medium transition-colors duration-150 hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background",
        compact && "justify-center px-0",
        selected && "bg-accent text-accent-foreground",
      )}
    >
      <Icon className="size-4 shrink-0" aria-hidden />
      <span className={cn(compact ? "sr-only" : "truncate")}>{label}</span>
      {count ? (
        <>
          <span
            aria-hidden
            className={cn(
              "text-xs font-semibold tabular-nums",
              count.warning ? TONE_TEXT.amber : "text-muted-foreground",
              compact ? "absolute right-1 top-0.5" : "ml-auto",
            )}
          >
            {count.value}
          </span>
          <span className="sr-only">
            , {count.value} {count.meaning}
          </span>
        </>
      ) : null}
    </button>
  );
  // Names are tooltips only while they are not visible; sidebar entries have none otherwise, but a
  // number may carry one.
  if (!compact && !count?.tooltip) return <li>{button}</li>;
  return (
    <li>
      <Tooltip>
        <TooltipTrigger asChild>{button}</TooltipTrigger>
        <TooltipContent side="right">
          {compact && <div>{label}</div>}
          {count?.tooltip && <div>{count.tooltip}</div>}
        </TooltipContent>
      </Tooltip>
    </li>
  );
}

/** An entry of the bottom group that is not a view: it opens something over the view shown and is never the selected entry. */
export interface SidebarAction {
  id: string;
  label: string;
  icon: LucideIcon;
  onActivate: () => void;
}

export interface SidebarProps {
  selected: ViewId;
  onSelect: (id: ViewId) => void;
  counts: ShellCounts;
  /** Entries of the bottom group that are not views; they sit above the bottom views (Settings stays the last entry). */
  actions?: readonly SidebarAction[];
}

/**
 * The left sidebar of a project window: 208 px wide with the name of each view, and 56 px with
 * icons only (names as tooltips) below a window width of 1024 px, the full height of the window.
 * The top group holds the views from Board to History; the bottom group, kept apart by the free
 * space between the two, holds the actions and then Settings, the last entry. Board shows the
 * number of real cards in Backlog, Jobs (in amber) the number of jobs waiting for the developer;
 * nothing at 0.
 */
export function Sidebar({ selected, onSelect, counts, actions = [] }: SidebarProps) {
  const compact = useCompactSidebar();
  const viewItem = (view: ViewEntry) => (
    <Item
      key={view.id}
      label={view.label}
      icon={view.icon}
      selected={view.id === selected}
      compact={compact}
      count={countOf(view.id, counts)}
      onActivate={() => onSelect(view.id)}
    />
  );
  return (
    <nav
      aria-label="Views"
      data-compact={compact}
      className={cn("flex h-full shrink-0 flex-col justify-between gap-1 border-r bg-muted/40 px-2 pb-3 pt-4", compact ? "w-[56px]" : "w-[208px]")}
    >
      <ul className="flex flex-col gap-1">{VIEWS.filter((view) => view.group === "top").map(viewItem)}</ul>
      <ul className="flex flex-col gap-1">
        {actions.map((action) => (
          <Item key={action.id} actionId={action.id} label={action.label} icon={action.icon} selected={false} compact={compact} count={null} onActivate={action.onActivate} />
        ))}
        {VIEWS.filter((view) => view.group === "bottom").map(viewItem)}
      </ul>
    </nav>
  );
}
