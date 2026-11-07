import { useCallback, useEffect, useLayoutEffect, useRef, useState, type RefObject } from "react";

import type { JobsList } from "@/bindings/JobsList";
import { call } from "@/lib/backend";

/** The width of the Jobs list at first, and the widths the developer can give it, in px. */
export const LIST_WIDTH = 280;
export const LIST_MIN = 240;
export const LIST_MAX = 420;
/** What an arrow key moves the separator by, in px. */
export const LIST_STEP = 16;
/** The least the conversation keeps, in px: a narrower window narrows the list instead. */
export const CONVERSATION_MIN = 480;

export const clampWidth = (width: number) => Math.round(Math.min(LIST_MAX, Math.max(LIST_MIN, width)));

/**
 * The width the list shows: the chosen one, or less when the window leaves the conversation less
 * than 480 px (never below 240 px). An `available` width of 0 is a window not measured yet.
 */
export function shownWidth(chosen: number, available: number): number {
  if (available <= 0) return chosen;
  return Math.max(LIST_MIN, Math.min(chosen, available - CONVERSATION_MIN));
}

/** What the backend sent, or the defaults when it is not a width in range and a boolean. */
function valid(list: JobsList | null): JobsList {
  if (list && Number.isInteger(list.width) && list.width >= LIST_MIN && list.width <= LIST_MAX && typeof list.hidden === "boolean") return list;
  return { width: LIST_WIDTH, hidden: false };
}

export interface JobsListControl {
  /** The values of the backend have arrived. */
  loaded: boolean;
  /** The width the developer chose. */
  width: number;
  hidden: boolean;
  /** Sets the chosen width; `commit` sends it to the backend (a drag does not, until it ends). */
  setWidth: (width: number, commit: boolean) => void;
  setHidden: (hidden: boolean) => void;
}

/**
 * The width and the hidden state of the Jobs list of the project: the backend remembers them
 * (`requirements/data#window`), this only mirrors them and sends the changes.
 */
export function useJobsList(projectId: string): JobsListControl {
  const [state, setState] = useState({ loaded: false, ...valid(null) });
  const latest = useRef(state);
  latest.current = state;

  useEffect(() => {
    let alive = true;
    setState({ loaded: false, ...valid(null) });
    call("jobs_list_state", { projectId })
      .then((list) => alive && setState({ loaded: true, ...valid(list) }))
      .catch(() => alive && setState({ loaded: true, ...valid(null) }));
    return () => {
      alive = false;
    };
  }, [projectId]);

  const send = useCallback(
    (width: number, hidden: boolean) => {
      void call("jobs_list_set", { projectId, width, hidden }).catch(() => {});
    },
    [projectId],
  );
  const setWidth = useCallback(
    (width: number, commit: boolean) => {
      const next = clampWidth(width);
      setState((previous) => ({ ...previous, width: next }));
      if (commit) send(next, latest.current.hidden);
    },
    [send],
  );
  const setHidden = useCallback(
    (hidden: boolean) => {
      setState((previous) => ({ ...previous, hidden }));
      send(latest.current.width, hidden);
    },
    [send],
  );
  return { loaded: state.loaded, width: state.width, hidden: state.hidden, setWidth, setHidden };
}

/** The width of an element in px, kept up to date; 0 until it is measured. */
export function useElementWidth(ref: RefObject<HTMLElement | null>): number {
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    setWidth(element.getBoundingClientRect().width);
    const observer = new ResizeObserver((entries) => {
      const box = entries[0]?.contentRect;
      if (box) setWidth(box.width);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref]);
  return width;
}
