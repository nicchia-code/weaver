import { useCallback, useEffect, useRef, useState } from "react";

import type { Entry } from "@/bindings/Entry";
import type { GeneralSnapshot } from "@/bindings/GeneralSnapshot";
import type { GeneralState } from "@/bindings/GeneralState";
import type { SubAgentsApi } from "@/components/sub-agents/api";
import { call, isAppError, on } from "@/lib/backend";

/**
 * The sub-agents of the General of a project: build it with `useMemo`. The Chat has no list of
 * them on screen, so what it tells the backend is only the conversation shown.
 */
export function generalSubAgentsApi(projectId: string): SubAgentsApi {
  return {
    projectId,
    owns: (owner) => owner.kind === "general",
    list: () => call("general_sub_agents", { projectId }),
    conversation: (path) => call("general_sub_agent_conversation", { projectId, path }),
    view: (_listShown, path) => call("general_sub_agents_view", { projectId, path }),
  };
}

/** The General of a project as the backend holds it; the UI keeps no copy beyond this view. */
export interface GeneralData {
  /** `null` until the backend answered. */
  state: GeneralState | null;
  /** The Drafts the agent of Chat created that Clear discards (`chat: true`). */
  chatDrafts: number;
  entries: readonly Entry[];
  /** A command the backend refused: the text to show, `null` once the developer sends again. */
  problem: string | null;
  send: (text: string) => void;
  stop: () => void;
  answer: (request: number, option: string) => void;
  clear: () => void;
  /** Reads the count of Drafts again: it moves with the Proposals, not with the conversation. */
  refreshDrafts: () => void;
}

/** The conversation of the General of a project: the snapshot, then the batches of changes after it. */
export function useGeneral(projectId: string): GeneralData {
  const [state, setState] = useState<GeneralState | null>(null);
  const [chatDrafts, setChatDrafts] = useState(0);
  const [entries, setEntries] = useState<readonly Entry[]>([]);
  const [problem, setProblem] = useState<string | null>(null);
  const seq = useRef(0);

  useEffect(() => {
    setState(null);
    setChatDrafts(0);
    setEntries([]);
    seq.current = 0;
    let alive = true;
    // A state event that arrives while the snapshot is on its way is newer than the snapshot.
    let stateEvents = 0;
    const fetchSnapshot = () => {
      const seen = stateEvents;
      return call("general_snapshot", { projectId })
        .then((snapshot: GeneralSnapshot) => {
          if (!alive || snapshot.state.projectId !== projectId) return;
          seq.current = snapshot.seq;
          setEntries(snapshot.entries);
          setChatDrafts(snapshot.chatDrafts);
          if (stateEvents === seen) setState(snapshot.state);
        })
        .catch(() => {});
    };
    // Listen first, then ask: a change between the two is not lost.
    const stopState = on("general-changed", (event) => {
      if (!alive || event.projectId !== projectId) return;
      stateEvents += 1;
      setState(event);
    });
    const stopEntries = on("general-entries", (event) => {
      if (!alive || event.projectId !== projectId) return;
      if (event.reset || event.seq > seq.current + 1) {
        void fetchSnapshot();
        return;
      }
      if (event.seq <= seq.current) return;
      seq.current = event.seq;
      setEntries((previous) => {
        const next = [...previous];
        for (const { index, entry } of event.changes) next[index] = entry;
        return next;
      });
    });
    void fetchSnapshot();
    return () => {
      alive = false;
      stopState.then((unlisten) => unlisten()).catch(() => {});
      stopEntries.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [projectId]);

  const run = useCallback((action: Promise<unknown>) => {
    action.catch((error: unknown) => setProblem(isAppError(error) ? error.message : "The command failed."));
  }, []);

  const send = useCallback(
    (text: string) => {
      setProblem(null);
      run(call("general_send_message", { projectId, text }));
    },
    [projectId, run],
  );
  const stop = useCallback(() => run(call("general_stop", { projectId })), [projectId, run]);
  const answer = useCallback(
    (request: number, option: string) => run(call("general_answer_permission", { projectId, request, option })),
    [projectId, run],
  );
  const clear = useCallback(() => run(call("general_clear", { projectId })), [projectId, run]);
  const refreshDrafts = useCallback(() => {
    call("general_snapshot", { projectId })
      .then((snapshot: GeneralSnapshot) => {
        if (snapshot.state.projectId === projectId) setChatDrafts(snapshot.chatDrafts);
      })
      .catch(() => {});
  }, [projectId]);

  return { state, chatDrafts, entries, problem, send, stop, answer, clear, refreshDrafts };
}

/** Whether the Chat of a project waits for the answers of its round: the sidebar's `1` on the Chat entry. */
export function useChatWaiting(projectId: string): boolean {
  const [waiting, setWaiting] = useState(false);
  useEffect(() => {
    setWaiting(false);
    let alive = true;
    // A state event that arrives while the snapshot is on its way is newer than the snapshot.
    let events = 0;
    const stop = on("general-changed", (event) => {
      if (!alive || event.projectId !== projectId) return;
      events += 1;
      setWaiting(event.waiting);
    });
    const seen = events;
    call("general_snapshot", { projectId })
      .then((snapshot: GeneralSnapshot) => {
        if (alive && snapshot.state.projectId === projectId && events === seen) setWaiting(snapshot.state.waiting);
      })
      .catch(() => {});
    return () => {
      alive = false;
      stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [projectId]);
  return waiting;
}
