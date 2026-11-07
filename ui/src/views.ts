import { Bot, FolderTree, History, ListChecks, MessageSquare, NotebookPen, Package, Paperclip, Settings, SquareKanban, type LucideIcon } from "lucide-react";

export type ViewId = "board" | "requirements" | "specs" | "history" | "jobs" | "files" | "assets" | "notes" | "general" | "settings";

export interface ViewEntry {
  id: ViewId;
  /** The name in the sidebar and the heading of the view. */
  label: string;
  icon: LucideIcon;
  /**
   * Where the sidebar puts the entry: `main` (Board, Chat, Jobs), the `project` group headed
   * `PROJECT`, or `settings`, which is a view like the others but has its button in the user row.
   */
  group: "main" | "project" | "settings";
}

/** The views of a project window, in the order of the sidebar (`Settings` last, in the user row). */
export const VIEWS: readonly ViewEntry[] = [
  { id: "board", label: "Board", icon: SquareKanban, group: "main" },
  { id: "general", label: "Chat", icon: MessageSquare, group: "main" },
  { id: "jobs", label: "Jobs", icon: Bot, group: "main" },
  { id: "requirements", label: "Requirements", icon: ListChecks, group: "project" },
  { id: "specs", label: "Specs", icon: Package, group: "project" },
  { id: "files", label: "Files", icon: FolderTree, group: "project" },
  { id: "assets", label: "Assets", icon: Paperclip, group: "project" },
  { id: "notes", label: "Notes", icon: NotebookPen, group: "project" },
  { id: "history", label: "History", icon: History, group: "project" },
  { id: "settings", label: "Settings", icon: Settings, group: "settings" },
];

/** The numbers the sidebar shows next to Board, Chat and Jobs; the backend holds them. */
export interface ShellCounts {
  /** Real cards in Backlog (ghosts never count). */
  backlog: number;
  /** Jobs waiting for the developer. */
  jobsWaiting: number;
  /** Jobs in Running: shown as a pill on Jobs when none waits. */
  jobsRunning: number;
  /** A card of questions of Chat is active: `1` in Peach on Chat. */
  chatCard: boolean;
}

export const NO_COUNTS: ShellCounts = { backlog: 0, jobsWaiting: 0, jobsRunning: 0, chatCard: false };
