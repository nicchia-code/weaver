import {
  Check,
  CircleDashed,
  Clock,
  FileText,
  MessageCircleQuestion,
  Pause,
  PanelLeftClose,
  PanelLeftOpen,
  Square,
  X,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import type { AgentNode } from "@/bindings/AgentNode";
import type { AskRow } from "@/bindings/AskRow";
import type { JobRow } from "@/bindings/JobRow";
import type { JobState } from "@/bindings/JobState";
import type { Step } from "@/bindings/Step";
import { ConfirmDialog } from "@/components/dialogs";
import { ConversationEntries, ComposerLine, DisabledComposer, FollowScroller, MessageComposer } from "@/components/jobs/conversation-ui";
import { runningText, startedLine, startedText } from "@/components/jobs/format";
import { ListSeparator } from "@/components/jobs/list-separator";
import { useConversation, useJobs } from "@/components/jobs/use-jobs";
import { shownWidth, useElementWidth, useJobsList } from "@/components/jobs/use-jobs-list";
import { countRows, filterRows, JOB_FILTERS, type JobFilter } from "@/components/jobs/job-filter";
import { mergeRows, type ListRow } from "@/components/ask/ask-model";
import { AskConversation } from "@/components/ask/ask-conversation";
import { useOptionalAsks } from "@/components/ask/use-asks";
import { useOptionalRequests } from "@/components/requests/use-requests";
import { useProject } from "@/components/project-context";
import {
  jobSubAgentsApi,
  askSubAgentsApi,
  namesAlong,
  nodeAt,
  useSubAgentConversation,
  useSubAgentList,
  useSubAgentsView,
  type SubAgentsApi,
} from "@/components/sub-agents/api";
import { AgentState, SubAgentList, SubAgentNavProvider, type SubAgentNav } from "@/components/sub-agents/sub-agent-ui";
import { Button } from "@/components/ui/button";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { VIEW_HEADER_CLASS, ViewTitle, viewTitleId } from "@/components/view-header";
import { call, isAppError } from "@/lib/backend";
import { TONE_DOT, TONE_PILL } from "@/lib/tones";
import { cn } from "@/lib/utils";
import type { ViewId } from "@/views";

const NEUTRAL_PILL = "bg-muted text-ink-2 ring-1 ring-inset ring-line";

// Sky for queued and running, Peach for waiting for the developer, Sage for done, Rose for failed and
// stopped; suspended stays neutral (Ink 2 on Lane). Always a text and an icon.
const BADGES: Record<JobState, { label: string; icon: LucideIcon | "dot"; className: string }> = {
  queued: { label: "queued", icon: Clock, className: TONE_PILL.sky },
  running: { label: "running", icon: "dot", className: TONE_PILL.sky },
  waiting: { label: "waiting", icon: MessageCircleQuestion, className: TONE_PILL.peach },
  suspended: { label: "suspended", icon: Pause, className: NEUTRAL_PILL },
  done: { label: "done", icon: Check, className: TONE_PILL.sage },
  failed: { label: "failed", icon: X, className: TONE_PILL.rose },
  stopped: { label: "stopped", icon: Square, className: TONE_PILL.rose },
};

/** A fully rounded pill. */
function Pill({ className, children }: { className: string; children: ReactNode }) {
  return (
    <span
      className={cn(
        "inline-flex shrink-0 items-center gap-1 rounded-full px-2 py-0.5 text-[11.5px] font-medium whitespace-nowrap [&_svg]:size-2.5 [&_svg]:shrink-0",
        className,
      )}
    >
      {children}
    </span>
  );
}

/** `label` replaces the word of the state, as the job header's `Running · 2m 16s`. */
export function StateBadge({ state, label }: { state: JobState; label?: string }) {
  const { label: word, icon: Icon, className } = BADGES[state];
  return (
    <Pill className={className}>
      {Icon === "dot" ? (
        <span aria-hidden className={cn("size-1.5 rounded-full animate-pulse motion-reduce:animate-none", TONE_DOT.sky)} />
      ) : (
        <Icon aria-hidden />
      )}
      {label ?? word}
    </Pill>
  );
}

/** The state of an Ask more conversation: `running` and `waiting` as a job's, `idle` before the first message and between turns. */
function AskBadge({ status }: { status: AskRow["status"] }) {
  if (status !== "idle") return <StateBadge state={status} />;
  return (
    <Pill className={NEUTRAL_PILL}>
      <CircleDashed aria-hidden />
      idle
    </Pill>
  );
}

/** Seconds since the session started, ticking while the job works and frozen at its end. */
function useRunning(job: JobRow): string | null {
  const [now, setNow] = useState(() => Date.now());
  const working = job.state === "running" || job.state === "waiting" || job.state === "suspended";
  useEffect(() => {
    if (!working) return;
    const timer = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [working]);
  if (!job.started) return null;
  const end = job.ended ? new Date(job.ended).getTime() : now;
  return runningText(end - new Date(job.started).getTime());
}

/** A row of the list: a 12 px block, on Surface with a ring and a light shadow when selected. */
function rowClass(selected: boolean) {
  return cn("rounded-xl", selected ? "bg-surface shadow-[0_0_0_1px_var(--line-control),0_2px_6px_var(--shadow-color)]" : "hover:bg-surface/60");
}

/** What the button of a row shows: name, kind, pill, latest note on two lines, and the times. */
function RowButton({
  name,
  kind,
  pill,
  note,
  meta,
  current,
  onSelect,
}: {
  name: string;
  kind: string;
  pill: ReactNode;
  note: string | null;
  meta: string;
  current: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      aria-current={current}
      className="flex min-w-0 flex-1 flex-col gap-1.5 rounded-xl p-3 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      onClick={onSelect}
    >
      <span className="flex items-center gap-2">
        <span className="min-w-0 truncate text-[13.5px] font-semibold">{name}</span>
        <span className="shrink-0 text-xs text-muted-foreground">{kind}</span>
        <span className="ml-auto">{pill}</span>
      </span>
      {note && (
        <span className="line-clamp-2 text-[12.5px] leading-[1.45] text-ink-2" title={note}>
          {note}
        </span>
      )}
      <span className="text-xs text-muted-foreground tabular-nums">{meta}</span>
    </button>
  );
}

function Row({
  job,
  selected,
  current,
  onSelect,
  children,
}: {
  job: JobRow;
  selected: boolean;
  /** The job's own conversation is shown (not one of its sub-agents'). */
  current: boolean;
  onSelect: () => void;
  /** The list of the job's sub-agents, when it is the selected one. */
  children?: ReactNode;
}) {
  const { remove } = useJobs();
  const running = useRunning(job);
  const finished = job.state === "done" || job.state === "failed" || job.state === "stopped";
  return (
    <li className={rowClass(selected)}>
      <div className="flex items-start">
        <RowButton
          name={job.name}
          kind={job.kind}
          pill={<StateBadge state={job.state} />}
          note={job.note}
          meta={`${startedText(job.created)}${running ? ` · ${running}` : ""}`}
          current={current}
          onSelect={onSelect}
        />
        {finished && (
          <Button variant="ghost" size="icon" className="mr-1 mt-1.5 size-7" aria-label={`Remove ${job.name}`} title="Remove from the list" onClick={() => remove(job.id)}>
            <X aria-hidden />
          </Button>
        )}
      </div>
      {children}
    </li>
  );
}

const STOPPABLE: readonly JobState[] = ["queued", "running", "waiting", "suspended"];

/** The header of a job, in the header band: its name with its state pill, when it started, `View request` and, while the job is not over, `Stop job`. */
function JobHeader({ projectId, job }: { projectId: string; job: JobRow }) {
  const running = useRunning(job);
  const [confirming, setConfirming] = useState(false);
  const working = job.state === "running" || job.state === "waiting";
  const word = `${job.state.charAt(0).toUpperCase()}${job.state.slice(1)}`;
  // `View request`: only while the request this setup or design job received is still sent to it;
  // the editor lives under the Board and its back control comes back to this job.
  const requests = useOptionalRequests();
  const { showView } = useProject();
  const viewable = job.request !== null && job.requestSent && requests !== null;
  const viewRequest = () => {
    if (!requests || job.request === null) return;
    requests.openEditor(job.request, { kind: "jobs", jobId: job.id });
    showView("board");
  };
  return (
    <>
      <div className="flex min-w-0 flex-1 flex-col gap-[3px]">
        <div className="flex items-center gap-2.5">
          <h2 className="min-w-0 truncate text-[17px] font-semibold">{job.name}</h2>
          <StateBadge state={job.state} label={working && running ? `${word} · ${running}` : word} />
        </div>
        <span className="truncate text-[13px] text-muted-foreground">{startedLine(job.created)}</span>
      </div>
      {viewable && (
        <Button variant="secondary" className="h-[34px] shrink-0 rounded-[9px] px-3" onClick={viewRequest}>
          <FileText aria-hidden className="size-[13px]" />
          View request
        </Button>
      )}
      {STOPPABLE.includes(job.state) && (
        <Button variant="destructive" className="h-[34px] shrink-0 rounded-[9px] px-3" onClick={() => setConfirming(true)}>
          <Square aria-hidden className="size-[13px] fill-current" />
          Stop job
        </Button>
      )}
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title={`Stop ${job.name}?`}
        description="The agent stops at once and every process it started ends."
        confirmLabel="Stop"
        destructive
        onConfirm={() => void call("job_stop", { projectId, jobId: job.id })}
      />
    </>
  );
}

/** The header of an Ask more conversation: its name and its state, with no started line and no buttons. */
function AskHeader({ ask }: { ask: AskRow }) {
  return (
    <div className="flex min-w-0 items-center gap-2.5">
      <h2 className="min-w-0 truncate text-[17px] font-semibold">{ask.name}</h2>
      <AskBadge status={ask.status} />
    </div>
  );
}

/** The path of a sub-agent from the job (or the conversation) that owns it, with its state. */
function SubAgentPath({ ownerName, nodes, path }: { ownerName: string; nodes: readonly AgentNode[]; path: readonly Step[] }) {
  const node = nodeAt(nodes, path);
  return (
    <div className="flex min-w-0 items-center gap-3">
      <h2 className="min-w-0 truncate text-[17px] font-semibold">{[ownerName, ...namesAlong(nodes, path)].join(" › ")}</h2>
      {node && <AgentState state={node.state} />}
    </div>
  );
}

function Conversation({ projectId, job, ompReady }: { projectId: string; job: JobRow; ompReady: boolean }) {
  const { entries, arrivals } = useConversation(projectId, job.id);
  return (
    <div className="flex h-full min-h-0 flex-col">
      <FollowScroller version={entries}>
        {job.state === "queued" && (
          <p className="py-8 text-center text-sm text-muted-foreground">{ompReady ? "Starting Weaver…" : "Waiting for the agent to be ready."}</p>
        )}
        <ConversationEntries
          entries={entries}
          arrivals={arrivals}
          working={job.state === "running" || job.state === "waiting"}
          latest
          answer={(request, option) => void call("job_answer_permission", { projectId, jobId: job.id, request, option })}
        />
        {job.state === "suspended" && (
          <p className="text-sm text-muted-foreground">Weaver stopped; the job continues when its session is reloaded.</p>
        )}
      </FollowScroller>
      <MessageBox projectId={projectId} job={job} />
    </div>
  );
}

/** The message box of a job, or, once it ended, the line that sends the developer to Chat. */
function MessageBox({ projectId, job }: { projectId: string; job: JobRow }) {
  const [text, setText] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  const finished = job.state === "done" || job.state === "failed" || job.state === "stopped";
  const { showView } = useProject();
  if (finished) {
    return (
      <ComposerLine>
        This job has ended. Continue in Chat.
        <Button variant="outline" size="sm" onClick={() => showView("general")}>
          Open Chat
        </Button>
      </ComposerLine>
    );
  }
  const canSend = job.state === "running" || job.state === "waiting";
  const send = () => {
    const message = text.trim();
    if (!message || !canSend) return;
    setText("");
    call("job_send_message", { projectId, jobId: job.id, text: message }).catch((error: unknown) =>
      setProblem(isAppError(error) ? error.message : "The message was not sent."),
    );
  };
  return (
    <>
      {problem && (
        <p role="alert" className="mx-auto w-full max-w-[860px] px-7 pb-2 text-xs text-rose-ink">
          {problem}
        </p>
      )}
      <MessageComposer
        text={text}
        onTextChange={setText}
        onSend={send}
        placeholder="Steer the agent while it runs…"
        disabled={!canSend}
        sendDisabled={!canSend || text.trim() === ""}
      />
    </>
  );
}

const NOT_ASKED = () => {};
const NO_ARRIVALS: readonly number[] = [];

/**
 * The read-only conversation of the sub-agent at `path` (of a job, of an Ask more conversation or of
 * General): what it received and did, and a disabled message box at the bottom. No `Stop`, no
 * permission card, no notes, no end line. The Jobs view shows its path in its header band;
 * `headerOf` (Chat) gives it a header of its own, with `back` before and `actions` after the path.
 */
export function SubAgentConversationView({
  api,
  ownerName,
  nodes,
  path,
  back,
  actions,
  headerOf,
}: {
  api: SubAgentsApi;
  ownerName: string;
  nodes: readonly AgentNode[];
  path: readonly Step[];
  /** What leads the header, such as the `Back to Chat` button of the Chat view. */
  back?: ReactNode;
  /** What ends the header, such as the `Clear` button of the Chat view. */
  actions?: ReactNode;
  /**
   * The view whose header this one replaces (Chat): the header then has the view header's height
   * and its title is the heading the view's region is labelled by.
   */
  headerOf?: ViewId;
}) {
  const { entries, unavailable } = useSubAgentConversation(api, path);
  const node = nodeAt(nodes, path);
  const names = namesAlong(nodes, path);
  return (
    <div className="flex h-full min-h-0 flex-col">
      {headerOf && (
        <header className={cn(VIEW_HEADER_CLASS, "text-sm")}>
          {back}
          <h1 id={viewTitleId(headerOf)} className="min-w-0 truncate font-semibold">
            {[ownerName, ...names].join(" › ")}
          </h1>
          {node && <AgentState state={node.state} />}
          {actions && <div className="ml-auto">{actions}</div>}
        </header>
      )}
      <FollowScroller version={entries}>
        {unavailable !== null ? (
          <p className="text-sm text-muted-foreground">Conversation not available: {unavailable.replace(/\.$/, "")}.</p>
        ) : (
          <ConversationEntries
            entries={entries}
            arrivals={NO_ARRIVALS}
            working={node?.state === "working"}
            agent={names[names.length - 1]}
            latest
            answer={NOT_ASKED}
          />
        )}
      </FollowScroller>
      <DisabledComposer />
    </div>
  );
}

const NO_ASKS: readonly AskRow[] = [];

/** A row of the list that is an Ask more conversation: name, kind `ask`, badge, opening time and `X`. */
function AskListRow({
  ask,
  selected,
  current,
  onSelect,
  children,
}: {
  ask: AskRow;
  selected: boolean;
  /** The conversation itself is shown (not one of its sub-agents'). */
  current: boolean;
  onSelect: () => void;
  children?: ReactNode;
}) {
  const asks = useOptionalAsks();
  const [confirming, setConfirming] = useState(false);
  const working = ask.status !== "idle";
  return (
    <li className={rowClass(selected)}>
      <div className="flex items-start">
        <RowButton name={ask.name} kind="ask" pill={<AskBadge status={ask.status} />} note={null} meta={startedText(ask.opened)} current={current} onSelect={onSelect} />
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon" className="mr-1 mt-1.5 size-7" aria-label={`Close ${ask.name}`} onClick={() => (working ? setConfirming(true) : asks?.close(ask.id))}>
              <X aria-hidden />
            </Button>
          </TooltipTrigger>
          <TooltipContent>Close</TooltipContent>
        </Tooltip>
      </div>
      {children}
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title={`Close ${ask.name}?`}
        description="The agent's turn stops and the conversation ends."
        confirmLabel="Close"
        destructive
        onConfirm={() => asks?.close(ask.id)}
      />
    </li>
  );
}

/** Which row of the list is chosen: a job or a conversation. */
interface Chosen {
  type: "job" | "ask";
  id: string;
}

/** An icon-only ghost button of the header band, with its tooltip. */
function BandButton({ label, onClick, children }: { label: string; onClick: () => void; children: ReactNode }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button variant="ghost" size="icon" aria-label={label} onClick={onClick}>
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  );
}

/**
 * The Jobs view: one header band over the list and the conversation; the project's jobs and Ask
 * more conversations on the left, the selected one's conversation, or its selected sub-agent's, on
 * the right. The list can be resized and hidden, and the project remembers both.
 */
export function JobsView() {
  const { projectId, openedJob, openedJobRequest } = useProject();
  const { jobs, ompReady } = useJobs();
  const asksState = useOptionalAsks();
  const asks = asksState?.asks ?? NO_ASKS;
  const opened = asksState?.opened ?? null;
  const [chosen, setChosen] = useState<Chosen | null>(null);
  const [agent, setAgent] = useState<{ owner: string; path: Step[] } | null>(null);
  const [focus, setFocus] = useState({ id: "", nonce: 0 });
  const [filter, setFilter] = useState<JobFilter>("all");
  const list = useJobsList(projectId);
  const root = useRef<HTMLDivElement>(null);
  const available = useElementWidth(root);
  const listWidth = shownWidth(list.width, available);
  useEffect(() => {
    if (openedJob) {
      setChosen({ type: "job", id: openedJob });
      setAgent(null);
    }
  }, [openedJob, openedJobRequest]);
  // A conversation just opened: it is the one shown, and its message box takes the focus.
  useEffect(() => {
    if (opened) {
      setChosen({ type: "ask", id: opened.id });
      setAgent(null);
      setFocus({ id: opened.id, nonce: opened.nonce });
    }
  }, [opened]);
  const rows = useMemo(() => mergeRows(jobs, asks), [jobs, asks]);
  const counts = useMemo(() => countRows(rows), [rows]);
  const shown = useMemo(() => filterRows(rows, filter), [rows, filter]);
  const identity = (row: ListRow) => (row.type === "job" ? { type: row.type, id: row.job.id } : { type: row.type, id: row.ask.id });
  const selected = rows.find((row) => chosen && identity(row).type === chosen.type && identity(row).id === chosen.id) ?? rows[0] ?? null;
  const selectedKey = selected ? `${identity(selected).type}:${identity(selected).id}` : null;
  const agentPath = agent && agent.owner === selectedKey ? agent.path : null;
  const api = useMemo(() => {
    if (!selected) return null;
    return selected.type === "job" ? jobSubAgentsApi(projectId, selected.job.id) : askSubAgentsApi(projectId, selected.ask.id);
    // The API follows the identity of the selected row, not its changing fields.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectId, selectedKey]);
  const nodes = useSubAgentList(api);
  useSubAgentsView(api, agentPath);
  const nav = useMemo<SubAgentNav>(
    () => ({
      base: agentPath ?? [],
      open: (path) => {
        if (selectedKey) setAgent({ owner: selectedKey, path });
      },
    }),
    [agentPath, selectedKey],
  );
  const select = (row: ListRow) => {
    setChosen(identity(row));
    setAgent(null);
  };
  const ownerName = selected ? (selected.type === "job" ? selected.job.name : selected.ask.name) : "";

  const listPane = (
    <div className="relative flex shrink-0 flex-col border-r border-line-soft bg-lane" style={{ width: listWidth }}>
      <div className="shrink-0 px-[18px] pb-3.5 pt-4">
        <ToggleGroup
          type="single"
          aria-label="Filter"
          value={filter}
          onValueChange={(value) => {
            if (value) setFilter(value as JobFilter);
          }}
          className="grid w-full grid-cols-3 gap-0.5 rounded-[10px] border-0 bg-line-soft p-[3px] shadow-none"
        >
          {JOB_FILTERS.map(({ id, label }) => (
            <ToggleGroupItem
              key={id}
              value={id}
              className="h-[30px] rounded-lg px-0 text-[12.5px] font-normal text-ink-2 hover:bg-transparent data-[state=on]:bg-surface data-[state=on]:font-medium data-[state=on]:text-foreground data-[state=on]:shadow-sm data-[state=on]:hover:bg-surface"
            >
              {`${label} ${counts[id]}`}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </div>
      <nav aria-label="Jobs" className="min-h-0 flex-1 overflow-y-auto">
        {rows.length === 0 ? (
          <p className="p-4 text-sm text-muted-foreground">No jobs yet</p>
        ) : shown.length === 0 ? (
          <p className="p-4 text-sm text-muted-foreground">No jobs here</p>
        ) : (
          <ul className="space-y-1.5 px-2.5 pt-3">
            {shown.map((row) => {
              const key = `${identity(row).type}:${identity(row).id}`;
              const here = key === selectedKey;
              const subAgents = here && nodes.length > 0 && (
                <SubAgentList nodes={nodes} selected={agentPath} onSelect={(path) => setAgent({ owner: key, path })} />
              );
              return row.type === "job" ? (
                <Row key={key} job={row.job} selected={here} current={here && !agentPath} onSelect={() => select(row)}>
                  {subAgents}
                </Row>
              ) : (
                <AskListRow key={key} ask={row.ask} selected={here} current={here && !agentPath} onSelect={() => select(row)}>
                  {subAgents}
                </AskListRow>
              );
            })}
          </ul>
        )}
      </nav>
      <ListSeparator width={list.width} shown={listWidth} onResize={list.setWidth} />
    </div>
  );

  return (
    <div ref={root} className="flex h-full min-h-0 flex-col">
      <header className={cn(VIEW_HEADER_CLASS, "gap-0 px-0")}>
        <div
          className={cn("flex h-full shrink-0 items-center gap-3", list.hidden ? "px-7" : "border-r border-line-soft pl-7 pr-3")}
          style={list.hidden ? undefined : { width: listWidth }}
        >
          <ViewTitle view="jobs" />
          {list.hidden ? (
            <BandButton label="Show jobs list" onClick={() => list.setHidden(false)}>
              <PanelLeftOpen aria-hidden />
            </BandButton>
          ) : (
            <span className="ml-auto">
              <BandButton label="Hide jobs list" onClick={() => list.setHidden(true)}>
                <PanelLeftClose aria-hidden />
              </BandButton>
            </span>
          )}
        </div>
        <div className="flex h-full min-w-0 flex-1 items-center gap-3 px-7">
          {selected &&
            (agentPath ? (
              <SubAgentPath ownerName={ownerName} nodes={nodes} path={agentPath} />
            ) : selected.type === "job" ? (
              <JobHeader projectId={projectId} job={selected.job} />
            ) : (
              <AskHeader ask={selected.ask} />
            ))}
        </div>
      </header>
      {list.loaded && (
        <div className="flex min-h-0 flex-1">
          {!list.hidden && listPane}
          <section aria-label="Conversation" className="min-w-0 flex-1">
            <SubAgentNavProvider value={nav}>
              {selected &&
                (agentPath && api ? (
                  <SubAgentConversationView api={api} ownerName={ownerName} nodes={nodes} path={agentPath} />
                ) : selected.type === "job" ? (
                  <Conversation key={selected.job.id} projectId={projectId} job={selected.job} ompReady={ompReady} />
                ) : (
                  <AskConversation key={selected.ask.id} ask={selected.ask} focus={focus.id === selected.ask.id ? focus.nonce : 0} />
                ))}
            </SubAgentNavProvider>
          </section>
        </div>
      )}
    </div>
  );
}
