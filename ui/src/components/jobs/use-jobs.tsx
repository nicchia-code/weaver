import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import type { ConversationSnapshot } from "@/bindings/ConversationSnapshot";
import type { Entry } from "@/bindings/Entry";
import type { JobRow } from "@/bindings/JobRow";
import { call, isAppError, on } from "@/lib/backend";

/** The Jobs of a project as the backend holds them; the UI keeps no copy of the domain beyond this view. */
export interface JobsState {
  jobs: readonly JobRow[];
  ompReady: boolean;
  /** Jobs waiting for the developer: the number the sidebar shows. */
  waiting: number;
  /** Jobs in Running: the sidebar shows them when none waits. */
  running: number;
  /** A decision refused when applied: the text of the notice, `null` once closed. */
  refusal: string | null;
  clearRefusal: () => void;
  runTask: (taskId: string) => void;
  runAll: () => void;
  scanSystem: () => void;
  remove: (jobId: string) => void;
}

const JobsContext = createContext<JobsState | null>(null);

export function useJobs(): JobsState {
  const value = useContext(JobsContext);
  if (!value) throw new Error("useJobs needs a JobsProvider");
  return value;
}

/** The Jobs of the project, or `null` where no provider is above (a card that only decorates itself with them). */
export function useOptionalJobs(): JobsState | null {
  return useContext(JobsContext);
}

export function JobsProvider({ projectId, children }: { projectId: string; children: ReactNode }) {
  const [jobs, setJobs] = useState<readonly JobRow[]>([]);
  const [ompReady, setOmpReady] = useState(true);
  const [refusal, setRefusal] = useState<string | null>(null);
  const alive = useRef(true);

  useEffect(() => {
    alive.current = true;
    // Listen first, then ask: a change between the two is not lost.
    const stop = on("jobs-changed", (snapshot) => {
      if (!alive.current || snapshot.projectId !== projectId) return;
      setJobs(snapshot.jobs);
      setOmpReady(snapshot.ompReady);
    });
    call("jobs_snapshot", { projectId })
      .then((snapshot) => {
        if (!alive.current) return;
        setJobs(snapshot.jobs);
        setOmpReady(snapshot.ompReady);
      })
      .catch(() => {});
    return () => {
      alive.current = false;
      stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [projectId]);

  const decide = useCallback((run: () => Promise<unknown>) => {
    run().catch((error: unknown) => {
      if (alive.current) setRefusal(isAppError(error) ? error.message : "The agent could not be started.");
    });
  }, []);

  const value = useMemo<JobsState>(
    () => ({
      jobs,
      ompReady,
      waiting: jobs.filter((job) => job.state === "waiting").length,
      running: jobs.filter((job) => job.state === "running").length,
      refusal,
      clearRefusal: () => setRefusal(null),
      runTask: (taskId) => decide(() => call("run_task", { projectId, taskId })),
      runAll: () => decide(() => call("run_all", { projectId })),
      scanSystem: () => decide(() => call("scan_system", { projectId })),
      remove: (jobId) => decide(() => call("job_remove", { projectId, jobId })),
    }),
    [jobs, ompReady, refusal, decide, projectId],
  );
  return <JobsContext.Provider value={value}>{children}</JobsContext.Provider>;
}

/** The conversation of one job: the snapshot, then the batches of changes that follow it. */
export function useConversation(projectId: string, jobId: string | null): { entries: readonly Entry[]; arrivals: readonly number[] } {
  const [entries, setEntries] = useState<readonly Entry[]>([]);
  const [arrivals, setArrivals] = useState<readonly number[]>([]);
  const seq = useRef(0);

  useEffect(() => {
    setEntries([]);
    setArrivals([]);
    seq.current = 0;
    if (!jobId) return;
    let alive = true;
    const fetchSnapshot = () =>
      call("job_conversation", { projectId, jobId })
        .then((snapshot: ConversationSnapshot) => {
          if (!alive || snapshot.jobId !== jobId) return;
          seq.current = snapshot.seq;
          setEntries(snapshot.entries);
          setArrivals(snapshot.arrivals);
        })
        .catch(() => {});
    const stop = on("job-entries", (event) => {
      if (!alive || event.projectId !== projectId || event.jobId !== jobId) return;
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
      setArrivals((previous) => {
        const next = [...previous];
        for (const { index, at } of event.changes) next[index] = at;
        return next;
      });
    });
    void fetchSnapshot();
    return () => {
      alive = false;
      stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [projectId, jobId]);

  return { entries, arrivals };
}
