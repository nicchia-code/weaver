import { ArrowDown, ArrowLeft, ArrowUp, Plus, Square } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import type { Entry } from "@/bindings/Entry";
import type { Step } from "@/bindings/Step";
import { AgentPill } from "@/components/agent-pill";
import { TaggedTextarea } from "@/components/assets/tagged-textarea";
import { ConfirmDialog } from "@/components/dialogs";
import { ChatProposalCard } from "@/components/general/chat-proposal-card";
import { ConversationEntries, ComposerFrame } from "@/components/jobs/conversation-ui";
import { ModelControl } from "@/components/general/model-control";
import { useChatProposals } from "@/components/general/use-chat-proposals";
import { useChatDraftRequest, withDraft } from "@/components/general/chat-draft";
import { generalSubAgentsApi, useGeneral } from "@/components/general/use-general";
import { GuardedButton } from "@/components/guarded-button";
import { SubAgentConversationView } from "@/components/jobs/jobs-view";
import { useProject } from "@/components/project-context";
import { useOptionalQuestionRound } from "@/components/questions/use-question-round";
import { useSubAgentList, useSubAgentsView } from "@/components/sub-agents/api";
import { SubAgentNavProvider, type SubAgentNav } from "@/components/sub-agents/sub-agent-ui";
import { Button } from "@/components/ui/button";
import { ViewHeader } from "@/components/view-header";
import { useFollowScroll } from "@/hooks/use-follow-scroll";
import { useOmp } from "@/hooks/use-omp";
import { call } from "@/lib/backend";

const EMPTY = "Think with Weaver about this project. Ask it for a Proposal or a job when you want one.";
const LOADING = "Loading conversation…";
/** Why the model control is disabled: a turn, a load and a round all mean Weaver is working. */
const MODEL_REASON = "Change the model when Weaver is not working";

/** The index of the last message of the developer that omp has not received yet, or -1. */
function lastQueued(entries: readonly Entry[]): number {
  for (let index = entries.length - 1; index >= 0; index -= 1) {
    const entry = entries[index];
    if (entry?.kind === "developer" && entry.delivery.status === "queued") return index;
  }
  return -1;
}

/** Chat: the one persistent conversation of the project with the agent, always there in its window. */
export function GeneralView() {
  const { projectId } = useProject();
  const { status } = useOmp();
  const round = useOptionalQuestionRound();
  const { state, entries, arrivals, problem, send, stop, answer, clear } = useGeneral(projectId);
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

  const jobs = useMemo(() => entries.flatMap((entry) => (entry.kind === "job-started" ? [entry.job] : [])), [entries]);
  const { proposals, refresh } = useChatProposals(projectId, jobs);

  const working = state?.working ?? false;
  const waiting = state?.starting ? lastQueued(entries) : -1;
  const empty = entries.length === 0 && !state?.kept;
  const missing = status?.state === "missing";
  const busy = working || (state?.loading ?? false) || (state?.waiting ?? false);

  // A text a view asks Chat for (`useChatDraft`): it goes in the box after what is there, which takes
  // the focus with the cursor at its end. Nothing is sent.
  const draft = useChatDraftRequest();
  const area = useRef<HTMLTextAreaElement>(null);
  const handled = useRef(0);
  const toEnd = useRef(false);
  useEffect(() => {
    if (!draft || draft.nonce === handled.current) return;
    handled.current = draft.nonce;
    toEnd.current = true;
    setPath(null);
    setText((current) => withDraft(current, draft.text));
  }, [draft]);
  useEffect(() => {
    if (!toEnd.current) return;
    toEnd.current = false;
    const place = () => {
      const box = area.current;
      if (!box) return;
      box.focus();
      box.setSelectionRange(box.value.length, box.value.length);
    };
    place();
    const timer = setTimeout(place, 0);
    return () => clearTimeout(timer);
  }, [text]);

  const submit = () => {
    const message = text.trim();
    if (!message) return;
    setText("");
    follow();
    send(message);
  };

  const newChat = (
    <GuardedButton variant="outline" size="sm" reasons={[empty && "The conversation is empty"]} onClick={() => setConfirming(true)}>
      <Plus aria-hidden />
      New chat
    </GuardedButton>
  );

  const chat = (
    <div className="flex h-full min-h-0 flex-col">
      <ViewHeader view="general" after={<AgentPill />}>
        {newChat}
      </ViewHeader>
      <div
        ref={scrollerRef}
        className="relative min-h-0 flex-1 overflow-y-auto"
        onScroll={(event) => {
          position.current = event.currentTarget.scrollTop;
          onScroll(event);
        }}
      >
        <div ref={contentRef} className="mx-auto flex w-full max-w-[760px] flex-col gap-4 px-6 pb-6 pt-8">
          {state?.loading ? (
            <p className="py-8 text-center text-sm text-muted-foreground">{LOADING}</p>
          ) : (
            entries.length === 0 && <p className="py-8 text-center text-sm text-muted-foreground">{EMPTY}</p>
          )}
          <ConversationEntries
            entries={entries}
            arrivals={arrivals}
            working={working}
            answer={answer}
            after={(entry, index) => (
              <>
                {index === waiting && <p className="text-right text-xs text-muted-foreground">Starting Weaver…</p>}
                {entry.kind === "job-started" &&
                  proposals
                    .filter((proposal) => proposal.job === entry.job)
                    .map((proposal) => <ChatProposalCard key={proposal.head.id} proposal={proposal} refresh={refresh} />)}
              </>
            )}
          />
          {state?.waiting && (
            <div className="flex flex-wrap items-center gap-2.5 rounded-xl px-3.5 py-3 shadow-[inset_0_0_0_1px_#E2DDF0] dark:shadow-[inset_0_0_0_1px_var(--color-line)]">
              <span aria-hidden className="grid size-5 shrink-0 place-items-center rounded-full bg-lavender-tint text-xs font-semibold text-lavender-ink">
                ?
              </span>
              <span className="flex-1 text-[13px]">Weaver is waiting for your answers.</span>
              <Button variant="secondary" size="sm" onClick={() => round?.act({ kind: "minimize", minimized: false })}>
                Answer
              </Button>
            </div>
          )}
        </div>
        {!following && (
          <Button className="sticky bottom-2 ml-auto mr-4 mt-3 flex" size="sm" variant="secondary" onClick={follow}>
            <ArrowDown aria-hidden />
            Jump to latest
          </Button>
        )}
      </div>
      {missing && (
        <div role="status" className="mx-auto flex w-full max-w-[760px] items-center gap-2 px-6 pb-2 text-sm text-rose-ink">
          The agent is not installed.
          <Button variant="outline" size="sm" onClick={() => void call("omp_install").catch(() => {})}>
            Install the agent
          </Button>
        </div>
      )}
      {problem && (
        <p role="alert" className="mx-auto w-full max-w-[760px] px-6 pb-2 text-xs text-rose-ink">
          {problem}
        </p>
      )}
      <ComposerFrame
        column="chat"
        controls={
          <>
            <ModelControl reasons={[busy && MODEL_REASON]} />
            {working ? (
              <Button variant="outline" size="sm" className="h-[34px]" onClick={stop}>
                <Square aria-hidden />
                Stop
              </Button>
            ) : (
              <Button size="icon" aria-label="Send" aria-disabled={text.trim() === ""} className="size-[34px] rounded-[10px]" onClick={submit}>
                <ArrowUp aria-hidden />
              </Button>
            )}
          </>
        }
      >
        <TaggedTextarea
          ref={area}
          aria-label="Message Weaver"
          placeholder="Message Weaver… use @ to add assets"
          rows={2}
          className="max-h-60 min-h-[3.75rem] bg-transparent"
          areaClassName="resize-none border-transparent focus-visible:ring-0 focus-visible:ring-offset-0"
          value={text}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              submit();
            }
          }}
        />
      </ComposerFrame>
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
          actions={newChat}
        />
      ) : (
        chat
      )}
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title="Start a new chat?"
        description={
          working
            ? "The agent's turn stops, the conversation is deleted and a new one starts."
            : "The conversation is deleted and a new one starts."
        }
        confirmLabel="New chat"
        destructive
        onConfirm={() => {
          setPath(null);
          clear();
        }}
      />
    </SubAgentNavProvider>
  );
}
