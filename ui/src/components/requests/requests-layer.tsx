import { useMemo, type ReactNode } from "react";

import { BoardActionsContext, useBoardActions } from "@/components/board/board-ui";
import { RequestEditor } from "@/components/requests/request-editor";
import { useRequests } from "@/components/requests/use-requests";

/**
 * Adds `New Request` (a new design request) to the actions the Board already has, keeping the ones
 * other parts of the window provide.
 */
export function RequestsBoardActions({ children }: { children: ReactNode }) {
  const parent = useBoardActions();
  const { newRequest } = useRequests();
  const actions = useMemo(() => ({ ...parent, newRequest }), [parent, newRequest]);
  return <BoardActionsContext.Provider value={actions}>{children}</BoardActionsContext.Provider>;
}

/**
 * Where the request editor opens: in place of what `children` show (the Board stays mounted, so it
 * keeps its state). The Board shows no strip that counts the open requests: they are cards of its
 * Backlog.
 */
export function RequestsLayer({ projectId, children }: { projectId: string; children: ReactNode }) {
  const { editing } = useRequests();
  return (
    <div className="flex h-full min-h-0 flex-col">
      {editing ? (
        <div className="min-h-0 flex-1">
          <RequestEditor projectId={projectId} />
        </div>
      ) : null}
      <div className="min-h-0 flex-1" hidden={editing}>
        {children}
      </div>
    </div>
  );
}
