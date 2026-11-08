import { useCallback, useEffect, useRef, useState } from "react";

import type { ChatProposal } from "@/bindings/ChatProposal";
import { call, on } from "@/lib/backend";

export interface ChatProposalsData {
  /** The Proposals the listed jobs handed over, in creation order. */
  proposals: readonly ChatProposal[];
  /** Asks again now (after a decision). */
  refresh: () => void;
}

const NONE: readonly ChatProposal[] = [];

/**
 * The Proposals that the jobs of the Chat (the ids in `jobs`) handed over, as the backend works them
 * out: it asks when the jobs listed change and whenever a `project-changed` event names the `board`
 * slice (or shows a gap in the revisions), and drops an answer older than the newest it shows. The
 * UI keeps no copy of the Proposals beyond this.
 */
export function useChatProposals(projectId: string, jobs: readonly string[]): ChatProposalsData {
  const [proposals, setProposals] = useState<readonly ChatProposal[]>(NONE);
  const shown = useRef(-1);
  const seen = useRef(-1);
  const alive = useRef(true);
  const key = jobs.join(",");
  const listed = useRef(jobs);
  listed.current = jobs;

  const fetchProposals = useCallback(() => {
    if (listed.current.length === 0) {
      setProposals(NONE);
      return;
    }
    call("general_proposals", { projectId, jobs: [...listed.current] })
      .then((incoming) => {
        if (!alive.current || incoming.projectId !== projectId || incoming.revision < shown.current) return;
        shown.current = incoming.revision;
        seen.current = Math.max(seen.current, incoming.revision);
        setProposals(incoming.proposals);
      })
      .catch(() => {});
  }, [projectId]);

  useEffect(() => {
    alive.current = true;
    shown.current = -1;
    seen.current = -1;
    setProposals(NONE);
    // Listen first, then ask: a change between the two is not lost.
    const stop = on("project-changed", (event) => {
      if (event.projectId !== projectId) return;
      const gap = seen.current >= 0 && event.revision > seen.current + 1;
      seen.current = Math.max(seen.current, event.revision);
      if (gap || event.slices.includes("board")) fetchProposals();
    });
    return () => {
      alive.current = false;
      stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [projectId, fetchProposals]);

  // The jobs the conversation lists changed: a new `Started` line, or the conversation was rebuilt.
  useEffect(() => {
    // Not older than the answer already shown: the list of jobs is part of the question.
    shown.current = -1;
    fetchProposals();
  }, [key, fetchProposals]);

  return { proposals, refresh: fetchProposals };
}
