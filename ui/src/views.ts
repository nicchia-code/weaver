import { Bot, FolderTree, Gauge, History, ListChecks, MessageSquare, NotebookPen, Package, Paperclip, Settings, SquareKanban, type LucideIcon } from "lucide-react";

export type ViewId = "board" | "requirements" | "specs" | "history" | "jobs" | "files" | "assets" | "notes" | "general" | "settings";

export interface ViewEntry {
  id: ViewId;
  /** The name in the sidebar and the heading of the view. */
  label: string;
  icon: LucideIcon;
  /** Where the sidebar puts the entry: the top group, or the bottom group apart from it. */
  group: "top" | "bottom";
}

/** The views of a project window, in the order of the sidebar (top group first, then bottom group). */
export const VIEWS: readonly ViewEntry[] = [
  { id: "board", label: "Board", icon: SquareKanban, group: "top" },
  { id: "general", label: "Chat", icon: MessageSquare, group: "top" },
  { id: "jobs", label: "Jobs", icon: Bot, group: "top" },
  { id: "files", label: "Files", icon: FolderTree, group: "top" },
  { id: "assets", label: "Assets", icon: Paperclip, group: "top" },
  { id: "notes", label: "Notes", icon: NotebookPen, group: "top" },
  { id: "requirements", label: "Requirements", icon: ListChecks, group: "top" },
  { id: "specs", label: "Specs", icon: Package, group: "top" },
  { id: "history", label: "History", icon: History, group: "top" },
  { id: "settings", label: "Settings", icon: Settings, group: "bottom" },
];

/**
 * The entry `Usage` of the bottom group, above Settings. It is not a view: it opens a dialog over
 * the view shown, so it is never in `ViewId` or `VIEWS`, never the selected entry and never has
 * `aria-current`.
 */
export const USAGE_ENTRY = { id: "usage", label: "Usage", icon: Gauge } as const;

/** The numbers the sidebar shows next to Board and Jobs; the backend holds them. */
export interface ShellCounts {
  /** Real cards in Backlog (ghosts never count). */
  backlog: number;
  /** Jobs waiting for the developer. */
  jobsWaiting: number;
  /** Chat waits for the answers of its round: the entry shows `1`. Jobs never count Chat. */
  chatWaiting: boolean;
}

export const NO_COUNTS: ShellCounts = { backlog: 0, jobsWaiting: 0, chatWaiting: false };
