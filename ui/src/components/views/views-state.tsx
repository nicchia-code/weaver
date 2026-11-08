import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import type { ViewSnapshot } from "@/bindings/ViewSnapshot";
import { BoardActionsContext, type FocusView } from "@/components/board/board-ui";
import type { Notice } from "@/components/notices";
import { useProject } from "@/components/project-context";
import {
  EMPTY_PLAN,
  planFocus,
  settleStep,
  stepCount,
  type FocusPlan,
  type FocusSubject,
  type ViewName,
} from "@/components/views/focus-model";
import { call, isAppError, on } from "@/lib/backend";

/** The focus of one view as the view shows it. */
export interface ActiveFocus {
  subject: FocusSubject;
  plan: FocusPlan;
  /** The current step, 0-based; meaningless when the plan has no step. */
  index: number;
  /** Requirements only: hide what is not focused. */
  only: boolean;
  /** Changes whenever a step was asked for, so a view scrolls again to a step it already is on. */
  nonce: number;
}

interface FocusState {
  subject: FocusSubject;
  refs: readonly string[];
  key: string | null;
  index: number;
  only: boolean;
  nonce: number;
  /** The focus was just opened: it goes to its first step once the snapshot can tell. */
  fresh: boolean;
}

export interface ViewsState {
  snapshot: ViewSnapshot | null;
  loadError: string | null;
  reload: () => void;
  /** The Proposal drawn over the Model, or `null` for `Current Model`. */
  overlay: string | null;
  notices: readonly Notice[];
  closeNotice: (id: string) => void;
  focus: Record<ViewName, ActiveFocus | null>;
  /** Goes to a step (0-based, clamped). */
  goStep: (view: ViewName, index: number) => void;
  clearFocus: (view: ViewName) => void;
  setOnly: (only: boolean) => void;
  /** Opens a view focused on `refs`; a Proposal's focus also draws that Proposal. */
  focusOn: (view: ViewName, refs: readonly string[], subject: FocusSubject) => void;
}

const ViewsContext = createContext<ViewsState | null>(null);

export function useViews(): ViewsState {
  const value = useContext(ViewsContext);
  if (!value) throw new Error("useViews needs a ViewsProvider");
  return value;
}

/** The views' state, or `null` where no provider is above (a card that only decorates itself with it). */
export function useOptionalViews(): ViewsState | null {
  return useContext(ViewsContext);
}

const SLICES = ["model", "requirements", "specs", "board"];
const NO_FOCUS: Record<ViewName, FocusState | null> = { requirements: null, specs: null };

/**
 * What the Requirements and Specs of one window share: the snapshot the backend worked out, the
 * overlay and each view's focus. The UI keeps no copy of the domain: it asks for a snapshot when
 * the window loads, again whenever a `project-changed` event names a slice the views draw, and
 * drops an answer that is no longer the latest asked for. It also gives the Board's cards their
 * `View requirements`.
 */
export function ViewsProvider({ projectId, children }: { projectId: string; children: ReactNode }) {
  const { showView } = useProject();
  const parentActions = useContext(BoardActionsContext);
  const [snapshot, setSnapshot] = useState<ViewSnapshot | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [overlay, setOverlayState] = useState<string | null>(null);
  const [notices, setNotices] = useState<readonly Notice[]>([]);
  const [focusState, setFocusState] = useState(NO_FOCUS);
  const [attempt, setAttempt] = useState(0);
  const overlayRef = useRef<string | null>(null);
  // The view whose focus drew the overlay: ending that focus ends the overlay.
  const overlayOwner = useRef<ViewName | null>(null);
  const snapshotRef = useRef<ViewSnapshot | null>(null);
  const sequence = useRef(0);
  const shown = useRef(-1);
  const seen = useRef(-1);
  const alive = useRef(true);
  const nonce = useRef(0);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const fetchViews = useCallback(() => {
    sequence.current += 1;
    const mine = sequence.current;
    const asked = overlayRef.current;
    call("view_snapshot", { projectId, overlay: asked })
      .then((incoming) => {
        if (!alive.current || mine !== sequence.current || incoming.projectId !== projectId || incoming.revision < shown.current) return;
        shown.current = incoming.revision;
        seen.current = Math.max(seen.current, incoming.revision);
        if (asked !== null && incoming.overlay !== asked) {
          const was = snapshotRef.current?.overlays.find((o) => o.id === asked);
          const name = was ? `Proposal ${was.shortNumber ?? was.id}` : "The Proposal";
          overlayRef.current = null;
          overlayOwner.current = null;
          setOverlayState(null);
          setNotices((previous) => [
            ...previous,
            { id: `overlay-${incoming.revision}`, text: `${name} is no longer open, so the views show the Current Model.` },
          ]);
        }
        snapshotRef.current = incoming;
        setSnapshot(incoming);
        setLoadError(null);
      })
      .catch((error: unknown) => {
        if (alive.current && mine === sequence.current) setLoadError(isAppError(error) ? error.message : "The views could not be read.");
      });
  }, [projectId]);

  useEffect(() => {
    // Listen first, then ask: a change between the two is not lost.
    const stop = on("project-changed", (event) => {
      if (event.projectId !== projectId) return;
      const gap = seen.current >= 0 && event.revision > seen.current + 1;
      seen.current = Math.max(seen.current, event.revision);
      if (gap || event.slices.some((s) => SLICES.includes(s))) fetchViews();
    });
    fetchViews();
    return () => {
      stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [projectId, fetchViews, attempt]);

  const setOverlay = useCallback(
    (id: string | null) => {
      if (overlayRef.current === id) return;
      overlayRef.current = id;
      setOverlayState(id);
      fetchViews();
    },
    [fetchViews],
  );

  const plans = useMemo(() => {
    const plan = (view: ViewName): FocusPlan => {
      const state = focusState[view];
      if (!state || !snapshot) return EMPTY_PLAN;
      const drawn = state.subject.kind === "proposal" && snapshot.overlay === state.subject.id;
      return planFocus(view, drawn ? snapshot.scope : state.refs, snapshot.data);
    };
    return { requirements: plan("requirements"), specs: plan("specs") };
  }, [focusState, snapshot]);
  const plansRef = useRef(plans);
  plansRef.current = plans;

  const goStep = useCallback((view: ViewName, index: number) => {
    const plan = plansRef.current[view];
    if (plan.steps.length === 0) return;
    const at = Math.min(Math.max(index, 0), plan.steps.length - 1);
    const step = plan.steps[at]!;
    nonce.current += 1;
    const mine = nonce.current;
    setFocusState((previous) => {
      const state = previous[view];
      return state ? { ...previous, [view]: { ...state, key: step.key, index: at, nonce: mine, fresh: false } } : previous;
    });
  }, []);

  // A focus just opened goes to its first step as soon as the snapshot it needs has arrived.
  useEffect(() => {
    for (const view of ["requirements", "specs"] as const) {
      const state = focusState[view];
      if (!state?.fresh || !snapshot) continue;
      const ready = state.subject.kind !== "proposal" || snapshot.overlay === state.subject.id || overlay !== state.subject.id;
      if (ready) goStep(view, 0);
    }
  }, [focusState, snapshot, overlay, goStep]);

  const focusOn = useCallback(
    (view: ViewName, refs: readonly string[], subject: FocusSubject) => {
      nonce.current += 1;
      const mine = nonce.current;
      setFocusState((previous) => ({
        ...previous,
        [view]: { subject, refs, key: null, index: 0, only: false, nonce: mine, fresh: true },
      }));
      // The overlay is the Proposal of the last focus opened: a focus on anything else, or on a
      // Proposal that cannot be overlaid, sends both views back to the Current Model.
      const drawn = subject.kind === "proposal" && snapshotRef.current?.overlays.some((o) => o.id === subject.id) ? subject.id : null;
      overlayOwner.current = drawn === null ? null : view;
      setOverlay(drawn);
      showView(view);
    },
    [setOverlay, showView],
  );

  const clearFocus = useCallback(
    (view: ViewName) => {
      // Ending the focus that drew the Proposal ends the overlay too.
      if (overlayOwner.current === view) {
        overlayOwner.current = null;
        setOverlay(null);
      }
      setFocusState((previous) => ({ ...previous, [view]: null }));
    },
    [setOverlay],
  );
  const setOnly = useCallback(
    (only: boolean) =>
      setFocusState((previous) =>
        previous.requirements ? { ...previous, requirements: { ...previous.requirements, only, nonce: previous.requirements.nonce + 1 } } : previous,
      ),
    [],
  );

  const focus = useMemo(() => {
    const active = (view: ViewName): ActiveFocus | null => {
      const state = focusState[view];
      if (!state) return null;
      const plan = plans[view];
      return { subject: state.subject, plan, index: settleStep(plan.steps, state.key, state.index), only: state.only, nonce: state.nonce };
    };
    return { requirements: active("requirements"), specs: active("specs") };
  }, [focusState, plans]);

  const closeNotice = useCallback((id: string) => setNotices((previous) => previous.filter((n) => n.id !== id)), []);
  const reload = useCallback(() => setAttempt((n) => n + 1), []);

  const value = useMemo<ViewsState>(
    () => ({
      snapshot,
      loadError,
      reload,
      overlay,
      notices,
      closeNotice,
      focus,
      goStep,
      clearFocus,
      setOnly,
      focusOn,
    }),
    [snapshot, loadError, reload, overlay, notices, closeNotice, focus, goStep, clearFocus, setOnly, focusOn],
  );

  const actions = useMemo(
    () => ({
      ...parentActions,
      focusSteps: (view: FocusView, refs: readonly string[], subject?: FocusSubject) =>
        stepCount(view, refs, snapshot?.data ?? null, subject?.kind === "proposal"),
      viewOn: (view: FocusView, refs: readonly string[], subject?: FocusSubject) =>
        focusOn(view, refs, subject ?? { kind: "ref", ref: refs[0] ?? "" }),
    }),
    [parentActions, snapshot, focusOn],
  );

  return (
    <ViewsContext.Provider value={value}>
      <BoardActionsContext.Provider value={actions}>{children}</BoardActionsContext.Provider>
    </ViewsContext.Provider>
  );
}
