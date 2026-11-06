import { Eye, MessageSquare, SendHorizontal, Trash2 } from "lucide-react";
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ClipboardEvent,
  type DragEvent,
  type KeyboardEvent,
} from "react";
import { toast } from "sonner";

import type { RequestView } from "@/bindings/RequestView";
import { ConfirmDialog } from "@/components/dialogs";
import { GuardedButton } from "@/components/guarded-button";
import { Markdown } from "@/components/markdown";
import { useProject } from "@/components/project-context";
import {
  acceptableImage,
  hasImageExtension,
  IMAGE_REFUSED,
  insertAt,
  isEmptyRequest,
  ownImageLinks,
  sameText,
  toBase64,
} from "@/components/requests/request-model";
import { useRequests } from "@/components/requests/use-requests";
import { TaggedTextarea } from "@/components/assets/tagged-textarea";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { useViewShown } from "@/components/view-shown";
import { call, isAppError, onFilesDropped } from "@/lib/backend";
import { cn } from "@/lib/utils";

/** After the last keystroke the text is written. */
export const AUTOSAVE_MS = 500;
/** The preview follows the text this late at most. */
export const PREVIEW_MS = 150;
/** Pane width from which the preview sits beside the text. */
export const SIDE_BY_SIDE_FROM = 900;

type Save = { kind: "idle" } | { kind: "saving" } | { kind: "saved" } | { kind: "error"; reason: string };

const TOAST_KEPT = "The request changed on disk; Weaver kept your version.";
const RETURNED = "The job ended without a Proposal. Change the request and send it again.";
const HELD_BY_CHAT = "Chat is writing a Proposal from this request";

function messageOf(error: unknown, fallback: string): string {
  return isAppError(error) ? error.message : fallback;
}

function SaveStatus({ save }: { save: Save }) {
  const text =
    save.kind === "saving"
      ? "Saving…"
      : save.kind === "saved"
        ? "Saved"
        : save.kind === "error"
          ? `Could not save: ${save.reason.replace(/\.$/, "")}.`
          : "";
  return (
    <span role="status" className={cn("text-xs", save.kind === "error" ? "text-destructive" : "text-muted-foreground")}>
      {text}
    </span>
  );
}

export interface RequestPaneProps {
  projectId: string;
  requestId: string;
  /** The Model is empty: a request sent now is a setup request. */
  modelEmpty: boolean;
  /** The pane is wide enough for the preview beside the text. */
  wide: boolean;
  /** The request is gone (discarded, consumed, deleted from outside): the editor shows another. */
  onGone: () => void;
  /** The developer is discarding this request (`true`), or the discard failed (`false`): its going is expected. */
  onDiscarding: (discarding: boolean) => void;
}

/**
 * One request in the editor: the text being written (saved as it is typed) or, once sent, its
 * rendered text. Typing starts no process: only `Send` does, through the backend.
 */
export function RequestPane({ projectId, requestId, modelEmpty, wide, onGone, onDiscarding }: RequestPaneProps) {
  const { changes, setFlusher } = useRequests();
  const { openJob, showView } = useProject();
  const [view, setView] = useState<RequestView | null>(null);
  const [text, setText] = useState("");
  const [saveState, setSaveState] = useState<Save>({ kind: "idle" });
  const [preview, setPreview] = useState(false);
  const [shown, setShown] = useState("");
  const [images, setImages] = useState<Record<string, string>>({});
  const [confirming, setConfirming] = useState(false);

  const latest = useRef("");
  const dirty = useRef(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const running = useRef<Promise<void> | null>(null);
  const written = useRef<string[]>([]);
  const closed = useRef(false);
  const statusRef = useRef<RequestView["status"] | null>(null);
  const seenChanges = useRef(changes);
  const areaRef = useRef<HTMLTextAreaElement>(null);
  const cursor = useRef<number | null>(null);
  const asked = useRef(new Set<string>());

  const writable = view?.status === "writing";

  const save = useCallback(async (): Promise<void> => {
    clearTimeout(timer.current);
    if (!dirty.current || closed.current) return;
    if (running.current) {
      await running.current;
      return save();
    }
    const value = latest.current;
    setSaveState({ kind: "saving" });
    const run = call("request_save", { projectId, requestId, text: value })
      .then(() => {
        written.current = [...written.current.slice(-2), value];
        if (latest.current === value) {
          dirty.current = false;
          setSaveState({ kind: "saved" });
        }
      })
      .catch((error: unknown) => {
        setSaveState({ kind: "error", reason: messageOf(error, "the request could not be written") });
      })
      .finally(() => {
        running.current = null;
      });
    running.current = run;
    await run;
  }, [projectId, requestId]);

  const change = useCallback(
    (next: string) => {
      latest.current = next;
      dirty.current = true;
      setText(next);
      setSaveState({ kind: "idle" });
      clearTimeout(timer.current);
      timer.current = setTimeout(() => void save(), AUTOSAVE_MS);
    },
    [save],
  );

  const load = useCallback(
    async (first: boolean) => {
      try {
        const next = await call("request_read", { projectId, requestId });
        if (closed.current) return;
        const wasStatus = statusRef.current;
        statusRef.current = next.status;
        setView(next);
        if (next.status !== "writing") {
          latest.current = next.text;
          dirty.current = false;
          setText(next.text);
          return;
        }
        if (first || wasStatus !== "writing") {
          latest.current = next.text;
          dirty.current = false;
          setText(next.text);
          return;
        }
        // An outside change is a disk text that is neither the typed one nor one this pane wrote.
        if (sameText(latest.current, next.text) || written.current.some((own) => sameText(own, next.text))) return;
        if (dirty.current) {
          toast.warning(TOAST_KEPT);
          await save();
        } else {
          latest.current = next.text;
          setText(next.text);
        }
      } catch (error: unknown) {
        if (closed.current || !isAppError(error) || error.code !== "refused") return;
        // The editor tells the developer when a request being written vanished (see `RequestEditor`).
        closed.current = true;
        onGone();
      }
    },
    [projectId, requestId, save, onGone],
  );

  useEffect(() => {
    closed.current = false;
    void load(true);
    setFlusher(save);
    return () => {
      setFlusher(null);
      clearTimeout(timer.current);
      // Leaving the request or the window: what is pending is written (never sent).
      if (dirty.current && !closed.current) {
        call("request_save", { projectId, requestId, text: latest.current }).catch(() => {});
      }
      closed.current = true;
    };
    // The pane is keyed by request: it loads once.
  }, []);

  useEffect(() => {
    if (changes === seenChanges.current) return;
    seenChanges.current = changes;
    void load(false);
  }, [changes, load]);

  // The preview follows the text within 150 ms.
  useEffect(() => {
    if (!preview && writable) return;
    const wait = setTimeout(() => setShown(text), writable ? PREVIEW_MS : 0);
    return () => clearTimeout(wait);
  }, [text, preview, writable]);

  // The images of the request's own folder the preview needs, asked once each.
  const previewed = writable ? (preview ? shown : "") : text;
  useEffect(() => {
    for (const link of ownImageLinks(previewed, requestId)) {
      if (asked.current.has(link)) continue;
      asked.current.add(link);
      call("request_image", { projectId, requestId, link })
        .then((url) => !closed.current && setImages((known) => ({ ...known, [link]: url })))
        .catch(() => asked.current.delete(link));
    }
  }, [previewed, projectId, requestId]);
  const resolveImage = useCallback((path: string) => images[path] ?? null, [images]);

  // The editor opens with the focus in the text.
  const focused = useRef(false);
  useEffect(() => {
    if (writable && !focused.current && areaRef.current) {
      focused.current = true;
      areaRef.current.focus();
    }
  }, [writable]);

  useLayoutEffect(() => {
    if (cursor.current !== null && areaRef.current) {
      areaRef.current.setSelectionRange(cursor.current, cursor.current);
      cursor.current = null;
    }
  }, [text]);

  const insertLinks = useCallback(
    (links: string[], start: number, end: number) => {
      if (links.length === 0) return;
      const at = Math.min(start, latest.current.length);
      const inserted = insertAt(latest.current, at, Math.min(Math.max(end, at), latest.current.length), links.map((l) => `![](${l})`).join("\n"));
      cursor.current = inserted.cursor;
      change(inserted.text);
    },
    [change],
  );

  const addFiles = useCallback(
    async (files: File[], start: number, end: number) => {
      const links: string[] = [];
      for (const file of files) {
        if (!acceptableImage(file.type, file.size)) {
          toast.error(IMAGE_REFUSED);
          continue;
        }
        try {
          const data = toBase64(new Uint8Array(await file.arrayBuffer()));
          links.push(await call("request_image_add", { projectId, requestId, data }));
        } catch (error: unknown) {
          toast.error(messageOf(error, IMAGE_REFUSED));
        }
      }
      if (!closed.current) insertLinks(links, start, end);
    },
    [projectId, requestId, insertLinks],
  );

  const importPaths = useCallback(
    async (paths: string[]) => {
      const area = areaRef.current;
      const start = area?.selectionStart ?? latest.current.length;
      const end = area?.selectionEnd ?? start;
      const links: string[] = [];
      for (const path of paths) {
        if (!hasImageExtension(path)) {
          toast.error(IMAGE_REFUSED);
          continue;
        }
        try {
          links.push(await call("request_image_import", { projectId, requestId, path }));
        } catch (error: unknown) {
          toast.error(messageOf(error, IMAGE_REFUSED));
        }
      }
      if (!closed.current) insertLinks(links, start, end);
    },
    [projectId, requestId, insertLinks],
  );

  // Files dropped on the window (the system drag and drop of the webview), only while this view is
  // the one shown: the Assets view takes the drops made over it.
  const viewShown = useViewShown();
  useEffect(() => {
    if (!writable || !viewShown) return;
    const stop = onFilesDropped((paths) => void importPaths(paths));
    return () => {
      stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [writable, viewShown, importPaths]);

  const onPaste = (event: ClipboardEvent<HTMLTextAreaElement>) => {
    const files = [...event.clipboardData.files];
    if (files.length === 0 || event.clipboardData.getData("text/plain") !== "") return;
    event.preventDefault();
    void addFiles(files, event.currentTarget.selectionStart, event.currentTarget.selectionEnd);
  };

  const onDrop = (event: DragEvent<HTMLTextAreaElement>) => {
    const files = [...event.dataTransfer.files];
    if (files.length === 0) return;
    event.preventDefault();
    void addFiles(files, event.currentTarget.selectionStart, event.currentTarget.selectionEnd);
  };

  const send = useCallback(async () => {
    if (!writable || isEmptyRequest(latest.current)) return;
    await save();
    try {
      await call("request_send", { projectId, requestId });
      await load(false);
    } catch (error: unknown) {
      toast.error(messageOf(error, "The request could not be sent."));
    }
  }, [writable, save, projectId, requestId, load]);

  const discard = useCallback(() => {
    closed.current = true;
    clearTimeout(timer.current);
    onDiscarding(true);
    call("request_discard", { projectId, requestId })
      .then(() => onGone())
      .catch((error: unknown) => {
        closed.current = false;
        onDiscarding(false);
        toast.error(messageOf(error, "The request could not be discarded."));
      });
  }, [projectId, requestId, onGone, onDiscarding]);

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      void send();
    }
  };

  const togglePreview = () => {
    void save();
    setPreview((on) => !on);
  };

  if (!view) return <div className="p-4 text-sm text-muted-foreground">Loading…</div>;

  const heading = modelEmpty ? "Setup request" : "Design request";
  const empty = isEmptyRequest(text);
  const sideBySide = preview && wide;
  return (
    <div className="flex h-full min-h-0 flex-1 flex-col">
      <header className="flex flex-wrap items-center gap-2 border-b px-4 py-3">
        <h2 className="text-sm font-semibold">{heading}</h2>
        {writable ? <SaveStatus save={saveState} /> : null}
        <div className="ml-auto flex items-center gap-2">
          {writable ? (
            <>
              <Button
                type="button"
                variant={preview ? "secondary" : "outline"}
                size="sm"
                aria-pressed={preview}
                onClick={togglePreview}
              >
                <Eye aria-hidden />
                Preview
              </Button>
              <Button type="button" variant="outline" size="sm" onClick={() => setConfirming(true)}>
                <Trash2 aria-hidden />
                Discard
              </Button>
              <GuardedButton
                type="button"
                size="sm"
                reasons={[empty && "Write the request first."]}
                tooltip="Send the request to the agent (Ctrl/Cmd+Enter)"
                onClick={() => void send()}
              >
                <SendHorizontal aria-hidden />
                Send
              </GuardedButton>
            </>
          ) : null}
        </div>
      </header>
      {view.status === "sent" ? (
        <Alert role="status" className="m-3 mb-0 items-center">
          <MessageSquare aria-hidden />
          <p className="min-w-0 flex-1">{view.jobName ? `Sent to the agent · ${view.jobName}` : "Sent to the agent"}</p>
          {view.jobId ? (
            <Button type="button" variant="outline" size="sm" onClick={() => openJob(view.jobId ?? "")}>
              Open conversation
            </Button>
          ) : null}
        </Alert>
      ) : view.status === "held-by-chat" ? (
        <Alert role="status" className="m-3 mb-0 items-center">
          <MessageSquare aria-hidden />
          <p className="min-w-0 flex-1">{HELD_BY_CHAT}</p>
          <Button type="button" variant="outline" size="sm" onClick={() => showView("general")}>
            Open Chat
          </Button>
        </Alert>
      ) : view.returned ? (
        <Alert role="status" variant="warning" className="m-3 mb-0">
          <p className="min-w-0 flex-1">{RETURNED}</p>
        </Alert>
      ) : null}
      {writable ? (
        <div className={cn("grid min-h-0 flex-1 gap-3 p-3", sideBySide ? "grid-cols-2" : "grid-cols-1")}>
          {!preview || sideBySide ? (
            <TaggedTextarea
              ref={areaRef}
              aria-label="Request text"
              value={text}
              placeholder="Describe what to add or change. Markdown, images pasted or dropped."
              spellCheck={false}
              className="h-full min-h-0 font-mono"
              areaClassName="resize-none"
              onChange={(event) => change(event.target.value)}
              onBlur={() => void save()}
              onKeyDown={onKeyDown}
              onPaste={onPaste}
              onDrop={onDrop}
              onDragOver={(event) => event.dataTransfer.types.includes("Files") && event.preventDefault()}
            />
          ) : null}
          {preview ? (
            <div className="min-h-0 overflow-auto rounded-md border p-3" aria-label="Preview">
              <Markdown holderHeadingLevel={2} resolveImage={resolveImage}>
                {shown}
              </Markdown>
            </div>
          ) : null}
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-auto p-3">
          <Markdown holderHeadingLevel={2} resolveImage={resolveImage}>
            {shown || text}
          </Markdown>
        </div>
      )}
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title="Discard this request?"
        description="Its file and images are deleted."
        confirmLabel="Discard"
        destructive
        onConfirm={discard}
      />
    </div>
  );
}
