import { createContext, useContext, useEffect, useRef } from "react";

import type { ViewId } from "@/views";

/** What the palette asks a view of the window to show or do, once the window shows that view. */
export type RevealTarget =
  /** The Board opens the details dialog of the card. */
  | { kind: "card"; id: string }
  /** The Board asks to confirm `Scan System`. */
  | { kind: "scan" }
  /** The Requirements scroll to the section (`<area>#<title>`). */
  | { kind: "section"; key: string }
  /** The History opens the entry, on `All time` when the chosen period hides it. */
  | { kind: "history"; id: string };

/** A request to a view; `nonce` tells a new request from one that was already handled. */
export interface Reveal {
  target: RevealTarget;
  nonce: number;
}

/** The palette of the window (`requirements/ui#window`). */
export interface PaletteControl {
  open: boolean;
  /** Where the focus goes back when it closes (what had it when it opened). */
  origin: HTMLElement | null;
  show: (origin: HTMLElement | null) => void;
  close: () => void;
}

/** What a view may ask of its project window. */
export interface ProjectContextValue {
  /** The project hash (the window's label is `p-<hash>`). */
  projectId: string;
  /** Shows another view of the window (the sidebar entry becomes the selected one). */
  showView: (view: ViewId) => void;
  /** Opens the conversation of a job in Jobs: shows that view and names the job to open. */
  openJob: (jobId: string) => void;
  /** The job a view asked to open last, for the Jobs view to select; `null` until one was. */
  openedJob: string | null;
  /** Counts the requests to open a job, so that asking for the same job again is a new request. */
  openedJobRequest: number;
}

/** What the palette and the sidebar ask of the window beyond a view: Usage, the palette itself and the requests to views. */
export interface WindowControls {
  /** Opens Usage over the view shown; the focus goes back to `origin` when it closes. */
  openUsage: (origin: HTMLElement | null) => void;
  palette: PaletteControl;
  /** Shows the view that handles `target` and asks it to; the view answers once with [`useReveal`]. */
  reveal: (target: RevealTarget) => void;
  /** The request no view handled yet. */
  revealed: Reveal | null;
  /** A view handled the request `nonce`. */
  revealHandled: (nonce: number) => void;
}

export const WindowControlsContext = createContext<WindowControls | null>(null);

export function useWindowControls(): WindowControls {
  const value = useContext(WindowControlsContext);
  if (!value) throw new Error("useWindowControls needs a ProjectWindow");
  return value;
}

export const ProjectContext = createContext<ProjectContextValue | null>(null);

export function useProject(): ProjectContextValue {
  const value = useContext(ProjectContext);
  if (!value) throw new Error("useProject needs a ProjectWindow");
  return value;
}

/**
 * A view's answer to the palette: `handle` runs once for each request of its `kind`, as soon as
 * `ready` (the view has what it needs to answer). Outside a project window nothing is asked.
 */
export function useReveal<K extends RevealTarget["kind"]>(kind: K, handle: (target: Extract<RevealTarget, { kind: K }>) => void, ready = true) {
  const context = useContext(WindowControlsContext);
  const revealed = context?.revealed ?? null;
  const handled = context?.revealHandled;
  const latest = useRef(handle);
  latest.current = handle;
  useEffect(() => {
    if (!ready || !revealed || revealed.target.kind !== kind) return;
    latest.current(revealed.target as Extract<RevealTarget, { kind: K }>);
    handled?.(revealed.nonce);
  }, [ready, revealed, kind, handled]);
}
