import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { toast } from "sonner";

import { localTimeOf } from "@/components/requests/request-model";
import { call, isAppError, on } from "@/lib/backend";

/** Where the request editor was opened from: what its back control says and where it goes. */
export type EditorOrigin = { kind: "board" } | { kind: "jobs"; jobId: string };

const FROM_BOARD: EditorOrigin = { kind: "board" };

/** What the editor of design requests shows and asks of the backend, for one project window. */
export interface RequestsState {
  /** Grows with every `requests-changed`: an editor with a request open reads it again. */
  changes: number;
  /** The editor is shown (in place of the Board). */
  editing: boolean;
  /** The request shown in the editor. */
  selected: string | null;
  /** `← Board`, or `← Jobs` for a request opened from `View request` of its job. */
  origin: EditorOrigin;
  /** `developer.NEW_REQUEST`: creates an empty request and opens it for writing (`New Request` in the Board header): always a new one. */
  newRequest: () => void;
  /**
   * `Start from Scratch`: opens for writing the oldest request being written, when there is one; only
   * when there is none, creates a new empty request and opens it as `newRequest` does.
   */
  startFromScratch: () => void;
  /**
   * Shows the editor on a request: a draft from its card or its row of the palette, a sent one from
   * `View request` of its job (`from` names the job: the back control returns to its conversation).
   */
  openEditor: (requestId: string, from?: EditorOrigin) => void;
  /** The editor closes: the Board shows. */
  closeEditor: () => void;
  /** The pane that holds the text being written says what to do before the window closes. */
  setFlusher: (flush: (() => Promise<void>) | null) => void;
}

const RequestsContext = createContext<RequestsState | null>(null);

export function useRequests(): RequestsState {
  const value = useContext(RequestsContext);
  if (!value) throw new Error("useRequests needs a RequestsProvider");
  return value;
}

/** Like [`useRequests`], for a view that may render without the provider (it then has no editor to open). */
export function useOptionalRequests(): RequestsState | null {
  return useContext(RequestsContext);
}

export function RequestsProvider({ projectId, children }: { projectId: string; children: ReactNode }) {
  const [changes, setChanges] = useState(0);
  const [editing, setEditing] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [origin, setOrigin] = useState<EditorOrigin>(FROM_BOARD);
  const alive = useRef(true);
  const flusher = useRef<(() => Promise<void>) | null>(null);

  useEffect(() => {
    alive.current = true;
    const changed = on("requests-changed", (event) => {
      if (!alive.current || event.projectId !== projectId) return;
      setChanges((n) => n + 1);
    });
    const flush = on("requests-flush", (event) => {
      if (!alive.current || event.projectId !== projectId) return;
      // The window is closing: what is pending is saved, never sent, and an empty request open in
      // the editor is discarded; then the backend goes on.
      Promise.resolve(flusher.current?.())
        .catch(() => {})
        .finally(() => {
          call("requests_flushed", { projectId }).catch(() => {});
        });
    });
    return () => {
      alive.current = false;
      for (const stop of [changed, flush]) stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [projectId]);

  const openEditor = useCallback((requestId: string, from: EditorOrigin = FROM_BOARD) => {
    setSelected(requestId);
    setOrigin(from);
    setEditing(true);
  }, []);

  const newRequest = useCallback(() => {
    call("request_new", { projectId, localTime: localTimeOf(new Date()) })
      .then((id) => {
        if (alive.current) openEditor(id);
      })
      .catch((error: unknown) => {
        toast.error(isAppError(error) ? error.message : "The request could not be created.");
      });
  }, [projectId, openEditor]);

  const startFromScratch = useCallback(() => {
    // The Board lists the requests being written oldest first, after its Proposals and Drifts.
    call("board_snapshot", { projectId })
      .then((board) => {
        if (!alive.current) return;
        const oldest = board.backlog.find((card) => card.kind === "request");
        if (oldest?.kind === "request") openEditor(oldest.id);
        else newRequest();
      })
      .catch((error: unknown) => {
        toast.error(isAppError(error) ? error.message : "The request could not be opened.");
      });
  }, [projectId, openEditor, newRequest]);

  const closeEditor = useCallback(() => setEditing(false), []);
  const setFlusher = useCallback((flush: (() => Promise<void>) | null) => {
    flusher.current = flush;
  }, []);

  const value = useMemo<RequestsState>(
    () => ({ changes, editing, selected, origin, newRequest, startFromScratch, openEditor, closeEditor, setFlusher }),
    [changes, editing, selected, origin, newRequest, startFromScratch, openEditor, closeEditor, setFlusher],
  );
  return <RequestsContext.Provider value={value}>{children}</RequestsContext.Provider>;
}
