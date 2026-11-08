import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type MutableRefObject, type ReactNode } from "react";

import type { UsageStatus } from "@/bindings/UsageStatus";
import { call, on } from "@/lib/backend";

interface UsageContextValue {
  status: UsageStatus | null;
  /** Takes a status from the backend; the one with the highest revision wins. */
  apply: (incoming: UsageStatus) => void;
  /** The highest revision seen so far. */
  revision: MutableRefObject<number>;
}

const UsageContext = createContext<UsageContextValue | null>(null);

/**
 * The usage limits as the backend last sent them (the Machine `usage`, one for the whole app): the
 * summary in the sidebar and the Usage dialog both read it. The UI keeps no copy of its own: the
 * whole status arrives from `usage_status` (which reads nothing), from the `usage-changed` event
 * and from the end of `omp_usage`, and the one with the highest revision wins. Nothing here starts
 * a reading: the backend reads when Usage is opened, at each `Retry` and on its own timer.
 */
export function UsageProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<UsageStatus | null>(null);
  const revision = useRef(-1);
  const mounted = useRef(true);

  const apply = useCallback((incoming: UsageStatus) => {
    if (!mounted.current || incoming.revision < revision.current) return;
    revision.current = incoming.revision;
    setStatus(incoming);
  }, []);

  useEffect(() => {
    mounted.current = true;
    // Listen first, then ask: a change between the two is not lost.
    const unlisten = on("usage-changed", apply);
    call("usage_status")
      .then(apply)
      .catch(() => {});
    return () => {
      mounted.current = false;
      unlisten.then((stop) => stop()).catch(() => {});
    };
  }, [apply]);

  const value = useMemo(() => ({ status, apply, revision }), [status, apply]);
  return <UsageContext.Provider value={value}>{children}</UsageContext.Provider>;
}

function useUsageContext(): UsageContextValue {
  const value = useContext(UsageContext);
  if (!value) throw new Error("useUsage needs a UsageProvider");
  return value;
}

/** The status of usage as the backend last sent it; `null` until it did. */
export function useUsageStatus(): UsageStatus | null {
  return useUsageContext().status;
}

export interface UsageReading {
  /** The status of usage as the backend last sent it; `null` until it did. */
  status: UsageStatus | null;
  /**
   * `status` once it can answer the reading this dialog asked for last: a reading that runs, or
   * any status newer than the request. `null` before: the status held is from before the request
   * (a failure the developer is retrying) and says nothing of the new reading.
   */
  current: UsageStatus | null;
  /** `developer.OPEN_USAGE` / `developer.RETRY_USAGE`: asks the backend to read (or join the reading that runs). */
  read: () => void;
}

/**
 * The usage limits for the Usage dialog. While `ompReady`, a reading starts when this is first used
 * (the dialog is opened) and each time omp becomes ready again (a dialog that waited for omp reads
 * by itself). The readings of the timer reach it as statuses like any other.
 */
export function useUsage(ompReady: boolean): UsageReading {
  const { status, apply, revision } = useUsageContext();
  const [since, setSince] = useState(-1);

  const read = useCallback(() => {
    setSince(revision.current);
    call("omp_usage")
      .then(apply)
      .catch(() => {});
  }, [apply, revision]);

  useEffect(() => {
    if (ompReady) read();
  }, [ompReady, read]);

  const current = status && (status.revision > since || status.phase === "reading") ? status : null;
  return { status, current, read };
}
