import { useEffect, useRef, useState } from "react";

import type { Entry } from "@/bindings/Entry";
import { call, on } from "@/lib/backend";

/** The conversation of one Ask more conversation: the snapshot, then the batches of changes that follow it. */
export function useAskConversation(askId: string): { entries: readonly Entry[]; arrivals: readonly number[] } {
  const [entries, setEntries] = useState<readonly Entry[]>([]);
  const [arrivals, setArrivals] = useState<readonly number[]>([]);
  const seq = useRef(0);

  useEffect(() => {
    setEntries([]);
    setArrivals([]);
    seq.current = 0;
    let alive = true;
    const fetchSnapshot = () =>
      call("ask_snapshot", { askId })
        .then((snapshot) => {
          if (!alive || snapshot.row.id !== askId) return;
          seq.current = snapshot.seq;
          setEntries(snapshot.entries);
          setArrivals(snapshot.arrivals);
        })
        .catch(() => {});
    // Listen first, then ask: a change between the two is not lost.
    const stop = on("ask-entries", (event) => {
      if (!alive || event.id !== askId) return;
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
  }, [askId]);

  return { entries, arrivals };
}
