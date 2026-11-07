import { ArrowDown, ArrowLeft, ArrowUp, Plus, Square } from "lucide-react";
import { Fragment, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import type { Entry } from "@/bindings/Entry";
import type { Step } from "@/bindings/Step";
import { useOptionalAsks } from "@/components/ask/use-asks";
import { AgentPill } from "@/components/agent-pill";
import { TaggedTextarea } from "@/components/assets/tagged-textarea";
import { ConfirmDialog } from "@/components/dialogs";
import { ChatEntry, SenderRow, WorkSummary } from "@/components/general/chat-conversation";
import { chatItems } from "@/components/general/chat-model";
import { ChatProposalCard } from "@/components/general/chat-proposal-card";
import { ChatQuestionCard } from "@/components/general/chat-question-card";
import { ModelControl } from "@/components/general/model-control";
import { useChatCards } from "@/components/general/use-chat-cards";
import { useChatProposals } from "@/components/general/use-chat-proposals";
import { useChatDraftRequest, withDraft } from "@/components/general/chat-draft";
import { generalSubAgentsApi, useGeneral } from "@/components/general/use-general";
import { GuardedButton } from "@/components/guarded-button";
import { SubAgentConversationView } from "@/components/jobs/jobs-view";
import { useProject } from "@/components/project-context";
import { useSubAgentList, useSubAgentsView } from "@/components/sub-agents/api";
import { SubAgentNavProvider, type SubAgentNav } from "@/components/sub-agents/sub-agent-ui";
import { Button } from "@/components/ui/button";
import { ViewHeader } from "@/components/view-header";
import { useFollowScroll } from "@/hooks/use-follow-scroll";
import { useOmp } from "@/hooks/use-omp";
import { call } from "@/lib/backend";

const EMPTY = "Think with Weaver about this project. Ask it for a Proposal or a job when you want one.";
const LOADING = "Loading conversation…";
/** Why the model control is disabled: a turn and a load both mean Weaver is working. */
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
  const items = useMemo(() => chatItems(entries, arrivals), [entries, arrivals]);

  // The cards of questions: a closed card shows right before the message of the developer that
  // closed it, the active one (and any card no message names) after everything of the conversation.
  const { cards, active, act } = useChatCards(projectId);
  const asks = useOptionalAsks();
  const cardOf = (id: string) => cards.find((card) => card.id === id);
  const named = useMemo(() => new Set(entries.flatMap((entry) => (entry.kind === "developer" && entry.card ? [entry.card] : []))), [entries]);
  const trailing = cards.filter((card) => !named.has(card.id));
  const showCard = (card: (typeof cards)[number], starting = false) => (
    <ChatQuestionCard
      key={`card-${card.id}`}
      card={card}
      closed={card.state !== "active"}
      act={(action) => act(card.id, action)}
      askMore={card.state === "active" && asks ? (index) => void asks.askMore({ kind: "card", card: card.id, index }) : false}
      starting={starting}
    />
  );

  const working = state?.working ?? false;
  const waiting = state?.starting ? lastQueued(entries) : -1;
  const empty = entries.length === 0 && !state?.kept;
  const missing = status?.state === "missing";
  const busy = working || (state?.loading ?? false);

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
          {items.map((item) => {
            if (item.kind === "work") {
              const running = working && item.to === entries.length - 1;
              return (
                <Fragment key={`work-${item.from}`}>
                  {item.starts && <SenderRow at={item.sender} />}
                  <WorkSummary item={item} entries={entries} running={running} answer={answer} />
                </Fragment>
              );
            }
            const entry = entries[item.index]!;
            const closed = entry.kind === "developer" && entry.card ? cardOf(entry.card) : undefined;
            // A message made only of a card's answers has no box: `Starting Weaver…` is the card's.
            const cardOnly = closed !== undefined && entry.kind === "developer" && entry.text === "";
            return (
              <Fragment key={`entry-${item.index}`}>
                {item.starts && <SenderRow at={item.sender} />}
                {closed && showCard(closed, cardOnly && item.index === waiting)}
                <ChatEntry entry={entry} answer={answer} />
                {!cardOnly && item.index === waiting && <p className="text-right text-xs text-muted-foreground">Starting Weaver…</p>}
                {entry.kind === "job-started" &&
                  proposals
                    .filter((proposal) => proposal.job === entry.job)
                    .map((proposal) => <ChatProposalCard key={proposal.head.id} proposal={proposal} refresh={refresh} />)}
              </Fragment>
            );
          })}
          {trailing.map((card) => showCard(card))}
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
      <div className="flex flex-col items-center gap-2 bg-gradient-to-b from-transparent to-surface to-[28%] px-6 pb-6 pt-5">
        {active && <p className="w-full max-w-[760px] px-1 text-xs text-muted-foreground">Your message also sends the answers chosen so far.</p>}
        <div className="flex w-full max-w-[760px] flex-col gap-2 rounded-2xl bg-surface py-3 pl-4 pr-3 shadow-[0_0_0_1px_#E0DDD5,0_6px_20px_rgba(24,24,27,0.06)] dark:shadow-[0_0_0_1px_var(--color-line),0_6px_20px_rgba(0,0,0,0.3)]">
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
          <div className="flex items-center justify-end gap-1.5">
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
          </div>
        </div>
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
