import { ArrowLeft, Plus } from "lucide-react";
import { useCallback, useEffect, useRef, useState, type KeyboardEvent } from "react";
import { toast } from "sonner";

import type { RequestSummary } from "@/bindings/RequestSummary";
import { createdLabel, displayTitle, TOAST_DELETED } from "@/components/requests/request-model";
import { RequestPane, SIDE_BY_SIDE_FROM } from "@/components/requests/request-pane";
import { useRequests } from "@/components/requests/use-requests";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

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

function RequestRow({ request, selected, onSelect }: { request: RequestSummary; selected: boolean; onSelect: () => void }) {
  const title = displayTitle(request);
  const badge = { writing: "Draft", sent: "Sent", "held-by-chat": "Chat" }[request.status];
  return (
    <li>
      <button
        type="button"
        data-request={request.id}
        aria-current={selected ? "true" : undefined}
        onClick={onSelect}
        className={cn(
          "flex w-full flex-col gap-1 rounded-md border border-transparent px-2 py-2 text-left text-sm hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
          selected && "border-border bg-accent",
        )}
      >
        <span className="truncate font-medium">{title}</span>
        <span className="flex items-center gap-2 text-xs text-muted-foreground">
          <span>{createdLabel(request.created)}</span>
          <Badge variant={request.status === "writing" ? "muted" : "info"}>{badge}</Badge>
        </span>
      </button>
    </li>
  );
}

/**
 * The request editor: the open requests (writing, sent or held by Chat) newest first on the left, the chosen one
 * on the right. Closing it never sends anything.
 */
export function RequestEditor({ projectId }: { projectId: string }) {
  const { snapshot, selected, select, closeEditor, newRequest } = useRequests();
  const [paneRef, width] = useWidth();
  const requests = snapshot?.requests ?? [];

  // The request on show is the one selected while it is listed, else the newest.
  const current = requests.find((request) => request.id === selected) ?? requests[0];

  // A request being written that vanished without the developer discarding it was deleted from
  // outside (`workspace.REQUEST_DELETED`); a sent one that vanished was consumed by its Proposal.
  const known = useRef(new Map<string, RequestSummary["status"]>());
  const discarding = useRef(new Set<string>());
  useEffect(() => {
    if (!snapshot) return;
    const now = new Map(snapshot.requests.map((request) => [request.id, request.status]));
    for (const [id, status] of known.current) {
      if (!now.has(id) && status === "writing" && !discarding.current.delete(id)) toast.warning(TOAST_DELETED);
    }
    known.current = now;
  }, [snapshot]);
  const onDiscarding = useCallback((id: string, on: boolean) => {
    if (on) discarding.current.add(id);
    else discarding.current.delete(id);
  }, []);

  // Nothing left to show: back to the Board.
  useEffect(() => {
    if (snapshot && snapshot.requests.length === 0) closeEditor();
  }, [snapshot, closeEditor]);

  const gone = useCallback(() => {
    const next = requests.find((request) => request.id !== current?.id);
    if (next) select(next.id);
    else closeEditor();
  }, [requests, current, select, closeEditor]);

  const onKeyDown = (event: KeyboardEvent<HTMLUListElement>) => {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    const index = requests.findIndex((request) => request.id === current?.id);
    const next = requests[index + (event.key === "ArrowDown" ? 1 : -1)];
    if (!next) return;
    event.preventDefault();
    select(next.id);
    event.currentTarget.querySelector<HTMLElement>(`[data-request="${next.id}"]`)?.focus();
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex items-center gap-2 border-b px-4 py-3">
        <Button type="button" variant="ghost" size="sm" onClick={closeEditor}>
          <ArrowLeft aria-hidden />
          Board
        </Button>
        <Button type="button" variant="outline" size="sm" className="ml-auto" onClick={newRequest}>
          <Plus aria-hidden />
          New request
        </Button>
      </div>
      <div className="flex min-h-0 flex-1">
        <nav aria-label="Open requests" className="w-60 shrink-0 overflow-auto border-r p-2">
          <ul className="flex flex-col gap-1" onKeyDown={onKeyDown}>
            {requests.map((request) => (
              <RequestRow
                key={request.id}
                request={request}
                selected={request.id === current?.id}
                onSelect={() => select(request.id)}
              />
            ))}
          </ul>
        </nav>
        <div ref={paneRef} className="flex min-w-0 flex-1 flex-col">
          {current ? (
            <RequestPane
              key={current.id}
              projectId={projectId}
              requestId={current.id}
              modelEmpty={snapshot?.modelEmpty ?? false}
              wide={width >= SIDE_BY_SIDE_FROM}
              onGone={gone}
              onDiscarding={(on) => onDiscarding(current.id, on)}
            />
          ) : null}
        </div>
      </div>
    </div>
  );
}
