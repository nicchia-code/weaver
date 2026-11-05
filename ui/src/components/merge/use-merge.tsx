import { createContext, useCallback, useContext, useMemo, useRef, useState, type ReactNode } from "react";

import type { MergeAnswer } from "@/bindings/MergeAnswer";
import type { MergeBranches } from "@/bindings/MergeBranches";
import type { MergeStatus } from "@/bindings/MergeStatus";
import { useBoard } from "@/components/board/board-state";
import { useJobs } from "@/components/jobs/use-jobs";
import { useProjectStatus } from "@/components/project/use-project-status";
import { call, isAppError } from "@/lib/backend";

/** `requirements/ui#solo-merge`: why the banner's actions wait while the agent resolves the conflicts. */
export const AGENT_RESOLVING = "The agent is resolving the conflicts";

/** What the Board knows of Merge (`requirements/ui#solo-merge`); every fact comes from the backend. */
export interface MergeState {
  /** The state of Merge; `null` while Solo mode is off (no `Merge` button, no banner). */
  merge: MergeStatus | null;
  /** The job `Merge conflicts` has not ended. */
  agentResolving: boolean;
  /** Asks the backend again (`merge_refresh`): availability and merge in progress. */
  refresh: () => void;
  /** The branches to choose from, and Weaver's files git still tracks. */
  branches: () => Promise<MergeBranches>;
  /** Merges `branch` into the current branch; every outcome that is not a merge shows as a notice. */
  start: (branch: string) => Promise<void>;
  /** `Abort merge`, once confirmed. */
  abort: () => Promise<void>;
  /** `Resolve with the agent`. */
  resolveWithAgent: () => Promise<void>;
}

const MergeContext = createContext<MergeState | null>(null);

/** Merge as the Board shows it, or `null` outside a `MergeProvider` (the Board then has no Merge). */
export function useMerge(): MergeState | null {
  return useContext(MergeContext);
}

/** An answer of a command, until a newer status of the project (a `project-state`) replaces it. */
interface Answered {
  merge: MergeStatus | null;
  /** The revision of the project status when the answer arrived. */
  base: number;
}

/**
 * The state of Merge for the Board. The status comes with `project-state` (`ProjectStatus.merge`,
 * absent while Solo mode is off); the answer of a command shows at once and gives way to the next
 * status the backend sends. The interface computes nothing: availability, reasons and the number of
 * files with conflicts are the backend's.
 */
export function MergeProvider({ projectId, children }: { projectId: string; children: ReactNode }) {
  const { status } = useProjectStatus();
  const { notify } = useBoard();
  const { jobs } = useJobs();
  const [answered, setAnswered] = useState<Answered | null>(null);
  const notices = useRef(0);

  const revision = status?.revision ?? -1;
  const merge = answered && answered.base === revision ? answered.merge : (status?.merge ?? null);
  const agentResolving = jobs.some((job) => job.kind === "merge" && job.state !== "done" && job.state !== "failed" && job.state !== "stopped");

  const answer = useCallback(
    (next: MergeStatus | null) => setAnswered({ merge: next, base: revision }),
    [revision],
  );

  const say = useCallback(
    (text: string, paths?: readonly string[]) => {
      notices.current += 1;
      notify(`merge-${notices.current}`, text, paths);
    },
    [notify],
  );

  /** A command the backend refused or could not run: its own sentence, or `fallback`. */
  const fail = useCallback(
    (error: unknown, fallback: string) => say(isAppError(error) && error.code === "refused" ? error.message : `${fallback}${isAppError(error) ? `: ${error.message}` : ""}.`),
    [say],
  );

  const refresh = useCallback(() => {
    call("merge_refresh", { projectId })
      .then(answer)
      .catch(() => {});
  }, [projectId, answer]);

  const branches = useCallback(() => call("merge_branches", { projectId }), [projectId]);

  const start = useCallback(
    async (branch: string) => {
      try {
        const result: MergeAnswer = await call("merge_start", { projectId, branch });
        answer(result.merge);
        if (result.notice) say(result.notice.text, result.notice.paths);
      } catch (error) {
        fail(error, `Could not merge ${branch}`);
      }
    },
    [projectId, answer, say, fail],
  );

  const abort = useCallback(async () => {
    try {
      const result: MergeAnswer = await call("merge_abort", { projectId });
      answer(result.merge);
      if (result.notice) say(result.notice.text, result.notice.paths);
    } catch (error) {
      fail(error, "Could not abort the merge");
    }
  }, [projectId, answer, say, fail]);

  const resolveWithAgent = useCallback(async () => {
    try {
      answer(await call("resolve_merge_conflicts", { projectId }));
    } catch (error) {
      fail(error, "Could not resolve the conflicts");
    }
  }, [projectId, answer, fail]);

  const value = useMemo<MergeState>(
    () => ({ merge, agentResolving, refresh, branches, start, abort, resolveWithAgent }),
    [merge, agentResolving, refresh, branches, start, abort, resolveWithAgent],
  );
  return <MergeContext.Provider value={value}>{children}</MergeContext.Provider>;
}
