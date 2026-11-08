import { useEffect, useRef, useState } from "react";

import { RequestPane, SIDE_BY_SIDE_FROM } from "@/components/requests/request-pane";
import { useRequests } from "@/components/requests/use-requests";

/** The width of an element, followed live; 0 until it is measured. */
function useWidth() {
  const ref = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  useEffect(() => {
    const element = ref.current;
    if (!element) return;
    setWidth(element.getBoundingClientRect().width);
    const observer = new ResizeObserver((entries) => setWidth(entries[0]?.contentRect.width ?? 0));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  return [ref, width] as const;
}

/**
 * The request editor (`requirements/ui#design-request`): one request across the whole Board view,
 * in place of the Board. It has no list of requests and no way to start another: it opens on the
 * request the Backlog, the palette, Start from Scratch or `View request` of a job chose. Closing
 * it never sends anything.
 */
export function RequestEditor({ projectId }: { projectId: string }) {
  const { selected, editing } = useRequests();
  const [paneRef, width] = useWidth();
  return (
    <div ref={paneRef} className="flex h-full min-h-0 min-w-0 flex-col">
      {editing && selected ? <RequestPane key={selected} projectId={projectId} requestId={selected} wide={width >= SIDE_BY_SIDE_FROM} /> : null}
    </div>
  );
}
