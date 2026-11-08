import { CircleX } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState, useSyncExternalStore } from "react";

import { NO_STATUS, statusText, type SaveStatus } from "@/components/notes/notes-model";
import type { DropPoint } from "@/components/notes/notes-session";
import { useNotes } from "@/components/notes/use-notes";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { ViewHeader } from "@/components/view-header";
import { useViewShown } from "@/components/view-shown";
import { onDragDrop } from "@/lib/backend";
import { TONE_TEXT } from "@/lib/tones";
import { cn } from "@/lib/utils";

/** The save status at the right of the header: nothing before the first change of the window's session. */
function SaveStatusText({ status }: { status: SaveStatus }) {
  const failed = status.kind === "error";
  return (
    <span role="status" className={cn("flex items-center gap-1.5 text-xs", failed ? TONE_TEXT.rose : "text-muted-foreground")}>
      {failed ? <CircleX className="size-4 shrink-0" aria-hidden /> : null}
      {statusText(status)}
    </span>
  );
}

/**
 * The Notes view (`Notes` in the sidebar, the Machine `notes-view`): the developer's one Markdown
 * sheet, shown formatted as he writes it. Showing the view opens it (the sheet is read, the editor
 * loads the first time); choosing another view leaves it, and Weaver stops following the notes
 * until he comes back. The view stays mounted meanwhile, so the editor keeps its history, cursor
 * and scroll.
 */
export function NotesView() {
  const session = useNotes();
  const shown = useViewShown();
  const { status, editor, loadError } = useSyncExternalStore(session.subscribe, session.getSnapshot);
  const scroller = useRef<HTMLDivElement>(null);
  const scrolled = useRef(0);
  const sheet = useRef<HTMLDivElement>(null);
  const [retry, setRetry] = useState(0);
  const [dropLine, setDropLine] = useState<DropPoint["line"] | null>(null);
  const port = editor?.port ?? null;

  // developer.OPEN_NOTES_VIEW / developer.LEAVE_NOTES_VIEW
  useEffect(() => {
    if (!shown) return;
    void session.open();
    return () => void session.leave();
  }, [shown, session, retry]);

  // The scroll comes back as it was left.
  useLayoutEffect(() => {
    if (shown && port && scroller.current) scroller.current.scrollTop = scrolled.current;
  }, [shown, port]);

  // Files dragged in from the Finder or Explorer arrive as events of the webview; they are listened
  // to only while the view is shown.
  useEffect(() => {
    if (!shown || !port) return;
    let listening = true;
    const stop = onDragDrop((event) => {
      if (!listening) return;
      if (event.type === "leave") {
        setDropLine(null);
        return;
      }
      const ratio = window.devicePixelRatio || 1;
      const x = event.position.x / ratio;
      const y = event.position.y / ratio;
      const element = document.elementFromPoint(x, y);
      const inside = element !== null && sheet.current !== null && sheet.current.contains(element);
      const point = inside ? port.dropPoint(x, y) : null;
      if (event.type === "drop") {
        setDropLine(null);
        if (point && event.paths.length > 0) void session.addDropped(event.paths, point.pos);
        return;
      }
      setDropLine(point?.line ?? null);
    });
    return () => {
      listening = false;
      setDropLine(null);
      stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [shown, port, session]);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <ViewHeader view="notes">
        <SaveStatusText status={status ?? NO_STATUS} />
      </ViewHeader>
      <div
        ref={scroller}
        onScroll={(event) => {
          if (shown) scrolled.current = event.currentTarget.scrollTop;
        }}
        className="min-h-0 flex-1 overflow-auto"
      >
        <div ref={sheet} className="min-h-full">
          {editor ? <editor.module.NotesSheet port={editor.port} /> : null}
          {!editor && loadError !== null ? (
            <div className="mx-auto flex max-w-[760px] flex-col items-start gap-3 px-6 pt-12">
              <Alert variant="error">Could not read the notes: {loadError.replace(/\.+$/, "")}.</Alert>
              <Button size="sm" variant="outline" onClick={() => setRetry((n) => n + 1)}>
                Retry
              </Button>
            </div>
          ) : null}
        </div>
      </div>
      {dropLine ? (
        <div
          aria-hidden
          className="pointer-events-none fixed h-0.5 bg-primary"
          style={{ left: dropLine.left, top: dropLine.top - 1, width: dropLine.width }}
        />
      ) : null}
    </div>
  );
}
