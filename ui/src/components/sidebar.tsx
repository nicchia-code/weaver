import { Search, Settings as SettingsIcon, type LucideIcon } from "lucide-react";

import { isMacPlatform } from "@/components/assets/asset-kinds";
import { ProjectSwitcher } from "@/components/project-switcher";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { UsageSummary } from "@/components/usage-summary";
import { useCompactSidebar } from "@/hooks/use-media-query";
import { useProjectGit } from "@/hooks/use-project-git";
import { TONE_DOT, TONE_TEXT, TONE_TINT } from "@/lib/tones";
import { cn } from "@/lib/utils";
import { VIEWS, type ShellCounts, type ViewEntry, type ViewId } from "@/views";

/** The number next to an entry, and what it counts in words (for assistive technologies). */
interface EntryCount {
  /** The number: `4`. */
  value: number;
  meaning: string;
  /** Muted for the cards of the Backlog, Peach for what waits for the developer, a Sky pill with a dot for what runs. */
  tone: "muted" | "peach" | "sky";
  /** What the number says on hover and focus, when it needs more than its accessible text. */
  tooltip?: string;
}

function countOf(view: ViewId, counts: ShellCounts): EntryCount | null {
  if (view === "board" && counts.backlog > 0) {
    return { value: counts.backlog, meaning: counts.backlog === 1 ? "card in Backlog" : "cards in Backlog", tone: "muted" };
  }
  if (view === "jobs" && counts.jobsWaiting > 0) {
    return { value: counts.jobsWaiting, meaning: counts.jobsWaiting === 1 ? "job waiting for you" : "jobs waiting for you", tone: "peach" };
  }
  if (view === "jobs" && counts.jobsRunning > 0) {
    return { value: counts.jobsRunning, meaning: counts.jobsRunning === 1 ? "job running" : "jobs running", tone: "sky" };
  }
  if (view === "general" && counts.chatWaiting) {
    return { value: 1, meaning: "round waiting for your answers", tone: "peach", tooltip: "Chat is waiting for your answers" };
  }
  return null;
}

/** The number as the sidebar draws it: right-aligned, or a small badge on the icon with icons only. */
function Badge({ count, compact }: { count: EntryCount; compact: boolean }) {
  if (compact) {
    const ink = count.tone === "peach" ? TONE_TEXT.peach : count.tone === "sky" ? TONE_TEXT.sky : "text-muted-foreground";
    return (
      <span aria-hidden className={cn("absolute right-1 top-0.5 text-[10px] font-semibold tabular-nums", ink)}>
        {count.value}
      </span>
    );
  }
  if (count.tone === "sky") {
    return (
      <span aria-hidden className={cn("ml-auto inline-flex items-center gap-[5px] rounded-full py-0.5 pl-[7px] pr-2 text-xs font-medium", TONE_TINT.sky, TONE_TEXT.sky)}>
        <span className={cn("size-1.5 rounded-full ring-[3px] ring-sky/30", TONE_DOT.sky)} />
        {count.value} running
      </span>
    );
  }
  return (
    <span aria-hidden className={cn("ml-auto text-xs tabular-nums", count.tone === "peach" ? cn("font-semibold", TONE_TEXT.peach) : "text-muted-foreground")}>
      {count.value}
    </span>
  );
}

/** One button of the sidebar: a view; the selected one carries `aria-current`. */
function Item({
  label,
  icon: Icon,
  selected,
  compact,
  count,
  onActivate,
}: {
  label: string;
  icon: LucideIcon;
  selected: boolean;
  compact: boolean;
  count: EntryCount | null;
  onActivate: () => void;
}) {
  const button = (
    <button
      type="button"
      aria-current={selected ? "page" : undefined}
      onClick={onActivate}
      className={cn(
        "relative flex h-[34px] w-full items-center gap-2.5 rounded-md px-2.5 text-sm font-normal text-ink-2 transition-colors duration-150 hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-paper",
        compact && "justify-center px-0",
        selected && "bg-surface font-medium text-foreground shadow-ring hover:bg-surface",
      )}
    >
      <Icon className="size-4 shrink-0" aria-hidden />
      <span className={cn(compact ? "sr-only" : "min-w-0 flex-1 truncate text-left")}>{label}</span>
      {count ? (
        <>
          <Badge count={count} compact={compact} />
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

/** `Search or run…` with its key hint; it opens the palette. */
function SearchButton({ compact, onOpen }: { compact: boolean; onOpen?: (origin: HTMLElement) => void }) {
  const mac = isMacPlatform();
  const hint = mac ? "⌘K" : "Ctrl K";
  const shortcut = mac ? "⌘K" : "Ctrl+K";
  const button = (
    <button
      type="button"
      aria-label="Search or run…"
      aria-keyshortcuts={mac ? "Meta+K" : "Control+K"}
      onClick={(event) => onOpen?.(event.currentTarget)}
      className={cn(
        "flex h-[34px] w-full items-center rounded-md border border-line bg-surface text-[13px] text-muted-foreground transition-colors duration-150 hover:bg-lane focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
        compact ? "justify-center" : "gap-2 px-2.5 text-left",
      )}
    >
      <Search aria-hidden className={compact ? "size-4 shrink-0" : "size-[15px] shrink-0"} />
      {compact ? null : (
        <>
          <span className="min-w-0 flex-1 truncate">Search or run…</span>
          <kbd className="rounded-[5px] border border-line bg-paper px-[5px] py-px font-mono text-[11px] text-muted-foreground">{hint}</kbd>
        </>
      )}
    </button>
  );
  if (!compact) return button;
  return (
    <Tooltip>
      <TooltipTrigger asChild>{button}</TooltipTrigger>
      <TooltipContent side="right">{`Search or run… (${shortcut})`}</TooltipContent>
    </Tooltip>
  );
}

/** The initials of a name: its first and its last word's first letters, or the first letter of a single word. */
export function initialsOf(name: string): string {
  const words = name.split(/\s+/).filter((word) => word !== "");
  const first = words[0];
  const last = words[words.length - 1];
  if (first === undefined || last === undefined) return "";
  const letter = (word: string) => Array.from(word)[0] ?? "";
  return (words.length === 1 ? letter(first) : letter(first) + letter(last)).toLocaleUpperCase();
}

/**
 * The user row at the bottom: a round avatar with the initials of git's user name for the project,
 * the name, and the `Settings` button, which carries `aria-current` while Settings is shown.
 * Without a name only the button stays; with icons only, only the gear.
 */
function UserRow({ compact, settingsSelected, onOpenSettings }: { compact: boolean; settingsSelected: boolean; onOpenSettings: () => void }) {
  const userName = useProjectGit()?.userName ?? null;
  const showName = !compact && userName !== null;
  return (
    <div className={cn("flex items-center gap-2.5 pt-0.5", compact ? "justify-center" : "pl-1.5 pr-1", !showName && !compact && "justify-end")}>
      {showName ? (
        <>
          <span aria-hidden className="grid size-7 shrink-0 place-items-center rounded-full bg-paper text-[11px] font-semibold text-ink-2 shadow-ring">
            {initialsOf(userName)}
          </span>
          <span className="min-w-0 flex-1 truncate text-[13px] text-ink-2">{userName}</span>
        </>
      ) : null}
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            aria-label="Settings"
            aria-current={settingsSelected ? "page" : undefined}
            onClick={onOpenSettings}
            className={cn(
              "grid size-8 shrink-0 place-items-center rounded-lg text-ink-2 transition-colors duration-150 hover:bg-accent hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
              settingsSelected && "bg-surface text-foreground shadow-ring hover:bg-surface",
            )}
          >
            <SettingsIcon aria-hidden className="size-4" />
          </button>
        </TooltipTrigger>
        <TooltipContent side="right">Settings</TooltipContent>
      </Tooltip>
    </div>
  );
}

export interface SidebarProps {
  selected: ViewId;
  onSelect: (id: ViewId) => void;
  counts: ShellCounts;
  /** `developer.OPEN_USAGE` from the summary of the usage limits; `origin` is its button, where the focus goes back when Usage closes. */
  onOpenUsage: (origin: HTMLElement) => void;
  /** `Search or run…` opens the palette; `origin` is the button, where the focus goes back when it closes. */
  onOpenPalette?: (origin: HTMLElement) => void;
}

/**
 * The left sidebar of a project window (`requirements/ui#window`): 248 px wide, 56 px with icons
 * only below a window width of 1024 px, the full height of the window. From the top: the project
 * switcher and `Search or run…`; Board, Chat and Jobs; the `PROJECT` group (a separator with icons
 * only) with Requirements, Specs, Files, Assets, Notes and History; the free space; the summary of
 * Usage; the user row with the `Settings` button. Settings is a view like the others. Board shows
 * the number of real cards in Backlog (muted), Chat a `1` in Peach while it waits for the answers
 * of its round, Jobs the jobs waiting for the developer in Peach or, when none waits, a Sky pill
 * with the jobs running; nothing at 0.
 */
export function Sidebar({ selected, onSelect, counts, onOpenUsage, onOpenPalette }: SidebarProps) {
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
      className={cn("flex h-full shrink-0 flex-col gap-[18px] pb-3 pt-3.5", compact ? "w-[56px] px-2" : "w-[248px] px-3")}
    >
      <div className="flex flex-col gap-3">
        <ProjectSwitcher compact={compact} />
        <SearchButton compact={compact} onOpen={onOpenPalette} />
      </div>
      <ul className="flex flex-col gap-0.5">{VIEWS.filter((view) => view.group === "main").map(viewItem)}</ul>
      <div role="group" aria-label="Project" className="flex flex-col gap-0.5">
        {compact ? (
          <div role="separator" className="mx-2 mb-1 h-px bg-line-soft" />
        ) : (
          <p aria-hidden className="px-2.5 pb-1.5 text-[11px] font-medium uppercase tracking-[0.06em] text-muted-foreground">
            PROJECT
          </p>
        )}
        <ul className="flex flex-col gap-0.5">{VIEWS.filter((view) => view.group === "project").map(viewItem)}</ul>
      </div>
      <div className="flex-1" />
      <UsageSummary compact={compact} onOpen={onOpenUsage} />
      <UserRow compact={compact} settingsSelected={selected === "settings"} onOpenSettings={() => onSelect("settings")} />
    </nav>
  );
}
