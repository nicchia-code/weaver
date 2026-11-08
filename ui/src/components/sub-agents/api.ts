import { useEffect, useRef, useState } from "react";

import type { AgentNode } from "@/bindings/AgentNode";
import type { Entry } from "@/bindings/Entry";
import type { Owner } from "@/bindings/Owner";
import type { Step } from "@/bindings/Step";
import type { SubAgentConversation } from "@/bindings/SubAgentConversation";
import type { SubAgentsEvent } from "@/bindings/SubAgentsEvent";
import { call, on } from "@/lib/backend";

/**
 * How the sub-agents of one owner (a job, or the General of a project) are asked for. The Jobs view
 * and the Chat view each build one; everything else about sub-agents is shared.
 */
export interface SubAgentsApi {
  projectId: string;
  /** Whether the events of `owner` are the ones of this API's owner. */
  owns: (owner: Owner) => boolean;
  /** The list of the sub-agents now. */
  list: () => Promise<SubAgentsEvent>;
  /** The conversation of the sub-agent at `path` now. */
  conversation: (path: Step[]) => Promise<SubAgentConversation>;
  /** What is on screen: whether the list is, and the conversation of which sub-agent (`null`: none). */
  view: (listShown: boolean, path: Step[] | null) => Promise<unknown>;
}

/** The sub-agents of one job: build it with `useMemo`, so that hooks see the same API while the job stays. */
export function jobSubAgentsApi(projectId: string, jobId: string): SubAgentsApi {
  return {
    projectId,
    owns: (owner) => owner.kind === "job" && owner.id === jobId,
    list: () => call("job_sub_agents", { projectId, jobId }),
    conversation: (path) => call("job_sub_agent_conversation", { projectId, jobId, path }),
    view: (listShown, path) => call("job_sub_agents_view", { projectId, jobId, listShown, path }),
  };
}

/** The sub-agents of one Ask more conversation: build it with `useMemo`. */
export function askSubAgentsApi(projectId: string, askId: string): SubAgentsApi {
  return {
    projectId,
    owns: (owner) => owner.kind === "ask" && owner.id === askId,
    list: () => call("ask_sub_agents", { askId }),
    conversation: (path) => call("ask_sub_agent_conversation", { askId, path }),
    view: (listShown, path) => call("ask_sub_agents_view", { askId, listShown, path }),
  };
}

export function samePath(a: readonly Step[], b: readonly Step[]): boolean {
  return a.length === b.length && a.every((step, index) => step.call === b[index]?.call && step.id === b[index]?.id);
}

/** The node at `path` of the list, if it is there. */
export function nodeAt(nodes: readonly AgentNode[], path: readonly Step[]): AgentNode | null {
  let level = nodes;
  let found: AgentNode | null = null;
  for (const step of path) {
    found = level.find((node) => node.call === step.call && node.id === step.id) ?? null;
    if (!found) return null;
    level = found.children;
  }
  return found;
}

/** The names from the owner down to the sub-agent at `path`, as far as the list knows them. */
export function namesAlong(nodes: readonly AgentNode[], path: readonly Step[]): string[] {
  return path.map((step, index) => nodeAt(nodes, path.slice(0, index + 1))?.name ?? step.id);
}

/** The list of the sub-agents of an owner: the snapshot, then the whole lists that follow it. */
export function useSubAgentList(api: SubAgentsApi | null): readonly AgentNode[] {
  const [nodes, setNodes] = useState<readonly AgentNode[]>([]);
  useEffect(() => {
    setNodes([]);
    if (!api) return;
    let alive = true;
    const stop = on("sub-agents-changed", (event) => {
      if (alive && event.projectId === api.projectId && api.owns(event.owner)) setNodes(event.nodes);
    });
    api
      .list()
      .then((event) => {
        if (alive) setNodes(event.nodes);
      })
      .catch(() => {});
    return () => {
      alive = false;
      stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [api]);
  return nodes;
}

/**
 * Tells the backend what is on screen while the sub-agent list of the owner is: its transcripts
 * are read only for that. Leaving (another owner, or the view closing) says nothing is shown.
 */
export function useSubAgentsView(api: SubAgentsApi | null, path: readonly Step[] | null): void {
  useEffect(() => {
    if (!api) return;
    return () => {
      void api.view(false, null).catch(() => {});
    };
  }, [api]);
  const key = path ? JSON.stringify(path) : null;
  useEffect(() => {
    if (!api) return;
    void api.view(true, key ? (JSON.parse(key) as Step[]) : null).catch(() => {});
  }, [api, key]);
}

export interface SubAgentTranscript {
  entries: readonly Entry[];
  /** The instant each entry arrived, as `entries` is indexed. */
  arrivals: readonly number[];
  /** Why the transcript cannot be read, if it cannot. */
  unavailable: string | null;
}

/** The conversation of one sub-agent: the snapshot, then the batches of changes that follow it. */
export function useSubAgentConversation(api: SubAgentsApi | null, path: readonly Step[] | null): SubAgentTranscript {
  const [state, setState] = useState<SubAgentTranscript>({ entries: [], arrivals: [], unavailable: null });
  const seq = useRef(0);
  const key = path ? JSON.stringify(path) : null;

  useEffect(() => {
    setState({ entries: [], arrivals: [], unavailable: null });
    seq.current = 0;
    if (!api || !key) return;
    const wanted = JSON.parse(key) as Step[];
    let alive = true;
    const fetchSnapshot = () =>
      api
        .conversation(wanted)
        .then((snapshot) => {
          if (!alive || !samePath(snapshot.path, wanted)) return;
          seq.current = snapshot.seq;
          setState({ entries: snapshot.entries, arrivals: snapshot.arrivals, unavailable: snapshot.unavailable });
        })
        .catch(() => {});
    const stop = on("sub-agent-entries", (event) => {
      if (!alive || event.projectId !== api.projectId || !api.owns(event.owner) || !samePath(event.path, wanted)) return;
      if (event.reset || event.seq > seq.current + 1) {
        void fetchSnapshot();
        return;
      }
      if (event.seq <= seq.current && event.changes.length > 0) return;
      seq.current = Math.max(seq.current, event.seq);
      setState((previous) => {
        const next = [...previous.entries];
        const at = [...previous.arrivals];
        for (const { index, entry, at: arrived } of event.changes) {
          next[index] = entry;
          at[index] = arrived;
        }
        return { entries: next, arrivals: at, unavailable: event.unavailable };
      });
    });
    void fetchSnapshot();
    return () => {
      alive = false;
      stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [api, key]);

  return state;
}
