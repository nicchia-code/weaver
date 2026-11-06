import { ArrowDown, ArrowLeft, ArrowUp, Eraser, Square } from "lucide-react";
import { Fragment, useLayoutEffect, useMemo, useRef, useState } from "react";

import type { Entry } from "@/bindings/Entry";
import type { Step } from "@/bindings/Step";
import { TaggedTextarea } from "@/components/assets/tagged-textarea";
import { ConfirmDialog } from "@/components/dialogs";
import { generalSubAgentsApi, useGeneral } from "@/components/general/use-general";
import { GuardedButton } from "@/components/guarded-button";
import { ViewHeader } from "@/components/view-header";
import { EntryView, SubAgentConversationView } from "@/components/jobs/jobs-view";
import { useProject } from "@/components/project-context";
import { useSubAgentList, useSubAgentsView } from "@/components/sub-agents/api";
import { SubAgentNavProvider, type SubAgentNav } from "@/components/sub-agents/sub-agent-ui";
import { Button } from "@/components/ui/button";
import { call } from "@/lib/backend";
import { useFollowScroll } from "@/hooks/use-follow-scroll";
import { useOmp } from "@/hooks/use-omp";

const EMPTY = "Think with omp about this project. Ask it to write a Proposal or start a job of Weaver when you want one.";
const LOADING = "Loading conversation…";

/** The index of the last message of the developer that omp has not received yet, or -1. */
function lastQueued(entries: readonly Entry[]): number {
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    const entry = entries[index];
    if (entry?.kind === "developer" && entry.delivery.status === "queued") return index;
  }
  return -1;
}

/** General: the free conversation with omp of the project, always there in its window. */
export function GeneralView() {
  const { projectId } = useProject();
  const { status } = useOmp();
  const { state, chatDrafts, entries, problem, send, stop, answer, clear, refreshDrafts } = useGeneral(projectId);
  const [text, setText] = useState("");
  const [confirming, setConfirming] = useState(false);
  const { scrollerRef, contentRef, scroller, following, follow, onScroll } = useFollowScroll(entries);
  // The chat comes back where it was left: the sub-agent's conversation replaced it, and with it
  // its scrolled element.
  const position = useRef(0);
  const [path, setPath] = useState<Step[] | null>(null);
  // Only the swap matters: `following` is what it was when the chat was left.
  useLayoutEffect(() => {
    const element = scroller.current;
    if (path === null && element && !following) element.scrollTop = position.current;
  }, [path]);
  const api = useMemo(() => generalSubAgentsApi(projectId), [projectId]);
  const shown = path ? api : null;
  const nodes = useSubAgentList(shown);
  useSubAgentsView(shown, path);
  const nav = useMemo<SubAgentNav>(() => ({ base: path ?? [], open: setPath }), [path]);

  const working = state?.working ?? false;
  const waiting = state?.starting ? lastQueued(entries) : -1;
  const empty = entries.length === 0 && !state?.kept;
  const missing = status?.state === "missing";

  const submit = () => {
    const message = text.trim();
    if (!message) return;
    setText("");
    follow();
    send(message);
  };

  const clearButton = (
    <GuardedButton
      variant="outline"
      size="sm"
      reasons={[empty && "The conversation is empty"]}
      onClick={() => {
        refreshDrafts();
        setConfirming(true);
      }}
    >
      <Eraser aria-hidden />
      Clear
    </GuardedButton>
  );

  const chat = (
    <div className="flex h-full min-h-0 flex-col">
      <ViewHeader view="general">{clearButton}</ViewHeader>
      <div
        ref={scrollerRef}
        className="relative min-h-0 flex-1 overflow-y-auto p-4"
        onScroll={(event) => {
          position.current = event.currentTarget.scrollTop;
          onScroll(event);
        }}
      >
        <div ref={contentRef} className="space-y-3">
          {state?.loading ? (
            <p className="py-8 text-center text-sm text-muted-foreground">{LOADING}</p>
          ) : (
            entries.length === 0 && <p className="py-8 text-center text-sm text-muted-foreground">{EMPTY}</p>
          )}
          {entries.map((entry, index) => (
            <Fragment key={index}>
              <EntryView entry={entry} answer={answer} />
              {index === waiting && <p className="text-right text-xs text-muted-foreground">Starting omp…</p>}
            </Fragment>
          ))}
        </div>
        {!following && (
          <Button className="sticky bottom-2 ml-auto mt-3 flex" size="sm" variant="secondary" onClick={follow}>
            <ArrowDown aria-hidden />
            Jump to latest
          </Button>
        )}
      </div>
      {missing && (
        <div role="status" className="flex items-center gap-2 border-t p-3 text-sm text-red-600 dark:text-red-400">
          omp is not installed.
          <Button variant="outline" size="sm" onClick={() => void call("omp_install").catch(() => {})}>
            Install omp
          </Button>
        </div>
      )}
      {problem && (
        <p role="alert" className="border-t px-3 pt-2 text-xs text-red-600 dark:text-red-400">
          {problem}
        </p>
      )}
      <div className="flex items-end gap-2 border-t p-3">
        <TaggedTextarea
          aria-label="Message omp"
          placeholder="Message omp…"
          rows={1}
          className="max-h-60 min-h-9 flex-1"
          value={text}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              submit();
            }
          }}
        />
        <Button size="icon" aria-label="Send" aria-disabled={text.trim() === ""} onClick={submit}>
          <ArrowUp aria-hidden />
        </Button>
        {working && (
          <Button variant="outline" size="sm" onClick={stop}>
            <Square aria-hidden />
            Stop
          </Button>
        )}
      </div>
    </div>
  );

  return (
    <SubAgentNavProvider value={nav}>
      {path ? (
        <SubAgentConversationView
          api={api}
          ownerName="Chat"
          headerOf="general"
          nodes={nodes}
          path={path}
          back={
            <Button variant="outline" size="sm" onClick={() => setPath(null)}>
              <ArrowLeft aria-hidden />
              Back to Chat
            </Button>
          }
          actions={clearButton}
        />
      ) : (
        chat
      )}
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title="Clear Chat?"
        description={
          (working
            ? "The agent's turn stops, the conversation is deleted and a new one starts."
            : "The conversation is deleted and a new one starts.") +
          (chatDrafts > 0 ? ` Its ${chatDrafts} Draft ${chatDrafts === 1 ? "Proposal is" : "Proposals are"} discarded.` : "")
        }
        confirmLabel="Clear"
        destructive
        onConfirm={() => {
          setPath(null);
          clear();
        }}
      />
    </SubAgentNavProvider>
  );
}
