import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import type { BoardSnapshot } from "@/bindings/BoardSnapshot";
import type { Notice } from "@/components/notices";
import { call, isAppError, on } from "@/lib/backend";

export interface BoardState {
  /** The Board as the backend worked it out; `null` until the first answer. */
  snapshot: BoardSnapshot | null;
  /** Why the first snapshot could not be read. */
  loadError: string | null;
  /** Notices in arrival order. */
  notices: readonly Notice[];
  closeNotice: (id: string) => void;
  /** Shows a notice (`paths` listed under it in monospace); one with an id already shown changes nothing. */
  notify: (id: string, text: string, paths?: readonly string[]) => void;
  /** Asks again after a failed load. */
  reload: () => void;
  /**
   * **Delete** on a Task in To do. A decision the backend refuses when applying it becomes a
   * notice and changes nothing; `label` is how the notice names the Task (`T4`).
   */
  deleteTask: (taskId: string, label: string) => Promise<void>;
  /** `Approve` on a Proposal; a refinement or a plain approval, the backend decides. */
  approveProposal: (proposalId: string, label: string) => Promise<void>;
  /** `Reject` (the optional reason) and, with `rework`, `Rework with feedback…`. */
  rejectProposal: (proposalId: string, label: string, feedback: string | null, rework: boolean) => Promise<void>;
  /** `Discard` on a Draft a refinement left half-way. */
  discardProposal: (proposalId: string, label: string) => Promise<void>;
  /** The model control: a model of omp, or `null` for the Settings models. */
  chooseProposalModel: (proposalId: string, label: string, model: string | null) => Promise<void>;
  /** `Resolve` on a Drift. */
  resolveDrift: (driftId: string, label: string) => Promise<void>;
  /** `Fix the code`: the fix option (null without options) and the optional note. */
  fixDriftCode: (driftId: string, label: string, option: number | null, note: string | null) => Promise<void>;
  /** `Accept Proposal` on a Drift. */
  acceptDriftProposal: (driftId: string, label: string) => Promise<void>;
  /** `Refine` on a Drift whose attached Proposal is stale or was left half-way by a refinement; the Drift is not decided. */
  refineDriftProposal: (driftId: string, label: string) => Promise<void>;
  /** `Discard` on a Drift whose attached Draft a refinement left half-way, once confirmed; the Drift is not decided. */
  discardDriftProposal: (driftId: string, label: string) => Promise<void>;
  /** `Dismiss Drift`, once confirmed. */
  dismissDrift: (driftId: string, label: string) => Promise<void>;
}

const BoardContext = createContext<BoardState | null>(null);

export function useBoard(): BoardState {
  const value = useContext(BoardContext);
  if (!value) throw new Error("useBoard needs a BoardProvider");
  return value;
}

/**
 * The Board of one project, as the backend holds it. The UI keeps no copy of the domain: it
 * asks for a snapshot when it loads, asks again whenever a `project-changed` event names the
 * `board` slice (or shows a gap in the revisions), and drops an answer older than the newest
 * one it already shows. Notices come from the `notice` event and from decisions the backend
 * refused; they stay until closed or the window closes.
 */
export function BoardProvider({ projectId, children }: { projectId: string; children: ReactNode }) {
  const [snapshot, setSnapshot] = useState<BoardSnapshot | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [notices, setNotices] = useState<readonly Notice[]>([]);
  const [attempt, setAttempt] = useState(0);
  const shown = useRef(-1);
  const seen = useRef(-1);
  const alive = useRef(true);
  const refusals = useRef(0);

  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  const fetchBoard = useCallback(() => {
    call("board_snapshot", { projectId })
      .then((incoming) => {
        if (!alive.current || incoming.projectId !== projectId || incoming.revision < shown.current) return;
        shown.current = incoming.revision;
        seen.current = Math.max(seen.current, incoming.revision);
        setSnapshot(incoming);
        setLoadError(null);
      })
      .catch((error: unknown) => {
        if (alive.current) setLoadError(isAppError(error) ? error.message : "The Board could not be read.");
      });
  }, [projectId]);

  const addNotice = useCallback((id: string, text: string, paths?: readonly string[]) => {
    setNotices((previous) => (previous.some((n) => n.id === id) ? previous : [...previous, { id, text, ...(paths ? { paths } : {}) }]));
  }, []);

  useEffect(() => {
    // Listen first, then ask: a change between the two is not lost.
    const stops = [
      on("project-changed", (event) => {
        if (event.projectId !== projectId) return;
        const gap = seen.current >= 0 && event.revision > seen.current + 1;
        seen.current = Math.max(seen.current, event.revision);
        if (gap || event.slices.includes("board")) fetchBoard();
      }),
      on("notice", (notice) => {
        if (notice.projectId === projectId) addNotice(notice.id, notice.text);
      }),
    ];
    fetchBoard();
    return () => {
      for (const stop of stops) stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [projectId, fetchBoard, addNotice, attempt]);

  const closeNotice = useCallback((id: string) => setNotices((previous) => previous.filter((n) => n.id !== id)), []);
  const reload = useCallback(() => setAttempt((n) => n + 1), []);

  /**
   * Runs a decision of the developer. One the backend refuses when applying it becomes a notice
   * (the refusal carries its own text) and changes nothing; the Board is then asked again, as what
   * the button showed was out of date.
   */
  const decide = useCallback(
    async (verb: string, label: string, run: () => Promise<unknown>) => {
      try {
        await run();
      } catch (error) {
        refusals.current += 1;
        const text =
          isAppError(error) && error.code === "refused"
            ? error.message
            : `Could not ${verb} ${label}${isAppError(error) ? `: ${error.message}` : ""}.`;
        addNotice(`refused-${refusals.current}`, text);
        fetchBoard();
      }
    },
    [addNotice, fetchBoard],
  );

  const deleteTask = useCallback(
    (taskId: string, label: string) => decide("delete", label, () => call("delete_task", { projectId, taskId })),
    [projectId, decide],
  );
  const approveProposal = useCallback(
    (proposalId: string, label: string) => decide("approve", label, () => call("approve_proposal", { projectId, proposalId })),
    [projectId, decide],
  );
  const rejectProposal = useCallback(
    (proposalId: string, label: string, feedback: string | null, rework: boolean) =>
      decide(rework ? "rework" : "reject", label, () => call("reject_proposal", { projectId, proposalId, feedback, rework })),
    [projectId, decide],
  );
  const discardProposal = useCallback(
    (proposalId: string, label: string) => decide("discard", label, () => call("discard_proposal", { projectId, proposalId })),
    [projectId, decide],
  );
  const chooseProposalModel = useCallback(
    (proposalId: string, label: string, model: string | null) =>
      decide("choose the model of", label, () => call("choose_proposal_model", { projectId, proposalId, model })),
    [projectId, decide],
  );
  const resolveDrift = useCallback(
    (driftId: string, label: string) => decide("resolve", label, () => call("resolve_drift", { projectId, driftId })),
    [projectId, decide],
  );
  const fixDriftCode = useCallback(
    (driftId: string, label: string, option: number | null, note: string | null) =>
      decide("fix the code of", label, () => call("fix_drift_code", { projectId, driftId, option, note })),
    [projectId, decide],
  );
  const acceptDriftProposal = useCallback(
    (driftId: string, label: string) => decide("accept the Proposal of", label, () => call("accept_drift_proposal", { projectId, driftId })),
    [projectId, decide],
  );
  const refineDriftProposal = useCallback(
    (driftId: string, label: string) => decide("refine the Proposal of", label, () => call("refine_drift_proposal", { projectId, driftId })),
    [projectId, decide],
  );
  const discardDriftProposal = useCallback(
    (driftId: string, label: string) => decide("discard the Proposal of", label, () => call("discard_drift_proposal", { projectId, driftId })),
    [projectId, decide],
  );
  const dismissDrift = useCallback(
    (driftId: string, label: string) => decide("dismiss", label, () => call("dismiss_drift", { projectId, driftId })),
    [projectId, decide],
  );

  const value = useMemo(
    () => ({
      snapshot,
      loadError,
      notices,
      closeNotice,
      notify: addNotice,
      reload,
      deleteTask,
      approveProposal,
      rejectProposal,
      discardProposal,
      chooseProposalModel,
      resolveDrift,
      fixDriftCode,
      acceptDriftProposal,
      refineDriftProposal,
      discardDriftProposal,
      dismissDrift,
    }),
    [snapshot, loadError, notices, closeNotice, addNotice, reload, deleteTask, approveProposal, rejectProposal, discardProposal, chooseProposalModel, resolveDrift, fixDriftCode, acceptDriftProposal, refineDriftProposal, discardDriftProposal, dismissDrift],
  );
  return <BoardContext.Provider value={value}>{children}</BoardContext.Provider>;
}
