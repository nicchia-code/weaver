import {
  ArrowDown,
  ArrowUp,
  Bot,
  Check,
  ChevronDown,
  ChevronRight,
  CircleDashed,
  Clock,
  FilePen,
  FileText,
  Info,
  Loader,
  MessageCircleQuestion,
  Pause,
  Search,
  Square,
  SquareTerminal,
  StickyNote,
  TriangleAlert,
  Wrench,
  X,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";

import type { AgentNode } from "@/bindings/AgentNode";
import type { AskRow } from "@/bindings/AskRow";
import type { Entry } from "@/bindings/Entry";
import type { JobRow } from "@/bindings/JobRow";
import type { JobState } from "@/bindings/JobState";
import type { Step } from "@/bindings/Step";
import type { Tool } from "@/bindings/Tool";
import { ConfirmDialog } from "@/components/dialogs";
import { runningText, startedLine, startedText } from "@/components/jobs/format";
import { useConversation, useJobs } from "@/components/jobs/use-jobs";
import { countRows, filterRows, JOB_FILTERS, type JobFilter } from "@/components/jobs/job-filter";
import { mergeRows, type ListRow } from "@/components/ask/ask-model";
import { AskConversation } from "@/components/ask/ask-conversation";
import { useOptionalAsks } from "@/components/ask/use-asks";
import { useOptionalRequests } from "@/components/requests/use-requests";
import { Markdown } from "@/components/markdown";
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
import { AgentState, SubAgentLine, SubAgentList, SubAgentNavProvider, type SubAgentNav } from "@/components/sub-agents/sub-agent-ui";
import { Button } from "@/components/ui/button";
import { TaggedTextarea } from "@/components/assets/tagged-textarea";
import { Textarea } from "@/components/ui/textarea";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { VIEW_HEADER_CLASS, ViewHeader, viewTitleId } from "@/components/view-header";
import { useFollowScroll } from "@/hooks/use-follow-scroll";
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

const TOOL_ICONS: Record<Tool["icon"], LucideIcon> = {
  read: FileText,
  search: Search,
  command: SquareTerminal,
  edit: FilePen,
  "sub-agent": Bot,
  other: Wrench,
};

function ToolRow({ tool }: { tool: Tool }) {
  const [open, setOpen] = useState(false);
  const Icon = TOOL_ICONS[tool.icon];
  const text = tool.command ?? tool.title;
  const Status = tool.status === "done" ? Check : tool.status === "failed" ? X : Loader;
  return (
    <div className="text-sm">
      <button
        type="button"
        className="flex w-full items-center gap-2 rounded-lg px-1.5 py-1 text-left hover:bg-lane"
        aria-expanded={open}
        aria-label={tool.title}
        onClick={() => setOpen((value) => !value)}
      >
        {open ? <ChevronDown aria-hidden className="size-4" /> : <ChevronRight aria-hidden className="size-4" />}
        <Icon aria-hidden className="size-4" />
        <span className="truncate">{tool.title}</span>
        <Status aria-hidden className={`ml-auto size-4 ${tool.status === "failed" ? "text-rose-ink" : ""} ${tool.status === "working" ? "animate-spin motion-reduce:animate-none" : ""}`} />
      </button>
      {open && <pre className="mt-1 max-h-48 overflow-auto rounded-lg bg-lane p-2.5 font-mono text-xs whitespace-pre-wrap shadow-[inset_0_0_0_1px_var(--line-soft)]">{text}</pre>}
      {tool.subAgents.map((agent) => (
        <SubAgentLine key={agent.id} call={tool.id} agent={agent} />
      ))}
    </div>
  );
}

/** The conversation of a job and of General render alike; the permission card answers through `answer`. */
export function EntryView({ entry, answer }: { entry: Entry; answer: (request: number, option: string) => void }) {
  switch (entry.kind) {
    case "instruction":
      return (
        <details className="rounded-xl bg-lane px-3.5 py-3 shadow-[inset_0_0_0_1px_var(--line-soft)]">
          <summary className="cursor-pointer text-[12.5px] font-medium text-ink-2">Instructions from Weaver</summary>
          <p className="mt-2.5 text-[13px] leading-relaxed text-ink-2">{entry.text}</p>
        </details>
      );
    case "assignment":
      return (
        <div className="rounded-2xl bg-paper px-3.5 py-2.5 text-sm">
          <div className="text-xs font-semibold text-muted-foreground">{entry.from}</div>
          <div className="whitespace-pre-wrap">{entry.text}</div>
        </div>
      );
    case "developer":
      return (
        <div className="ml-auto max-w-[80%] rounded-2xl rounded-br-[4px] bg-paper px-3.5 py-2.5 text-sm whitespace-pre-wrap">
          {entry.text}
          {entry.delivery.status === "queued" && (
            <div className="flex items-center gap-1 text-xs text-muted-foreground">
              <Clock aria-hidden className="size-3" />
              Queued
            </div>
          )}
          {entry.delivery.status === "not-delivered" && (
            <div className="text-xs text-rose-ink">Not delivered: {entry.delivery.reason}.</div>
          )}
        </div>
      );
    case "agent":
      return <Markdown>{entry.text}</Markdown>;
    case "thinking":
      return (
        <details className="text-sm text-muted-foreground">
          <summary>Thinking</summary>
          <div className="whitespace-pre-wrap">{entry.text}</div>
        </details>
      );
    case "tool":
      return <ToolRow tool={entry} />;
    case "note":
      return (
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <StickyNote aria-hidden className="size-3" />
          <span>{startedText(entry.at).slice(-5)}</span>
          <span>{entry.text}</span>
        </div>
      );
    case "permission":
      return (
        <div role="group" aria-label="Weaver asks permission" className="rounded-lg border border-peach p-2 text-sm">
          <div className="font-semibold">Weaver asks permission</div>
          {entry.tool && <ToolRow tool={entry.tool} />}
          {entry.state.status === "asking" ? (
            <div className="mt-2 flex flex-wrap gap-2">
              {entry.options.map((option) => (
                <Button
                  key={option.id}
                  size="sm"
                  variant={option.rejects ? "outline" : "default"}
                  onClick={() => answer(entry.request, option.id)}
                >
                  {option.name}
                </Button>
              ))}
            </div>
          ) : (
            <div className="mt-1 text-xs text-muted-foreground">
              {entry.state.status === "chosen" ? entry.state.name : entry.state.status === "void" ? "No longer needed." : "Cancelled."}
            </div>
          )}
        </div>
      );
    case "reloaded":
      return <div className="text-xs text-muted-foreground">Session reloaded; the agent continues its work.</div>;
    case "end":
      return entry.outcome === "failed" ? (
        <FailedLine reason={entry.reason} details={entry.details} />
      ) : (
        <div className="text-xs text-muted-foreground">{entry.outcome === "finished" ? "Finished." : "Stopped."}</div>
      );
    case "job-started":
      return <JobStartedLine job={entry.job} name={entry.name} />;
    case "job-refused":
      return (
        <div className="flex items-center gap-2 text-sm text-peach-ink">
          <TriangleAlert aria-hidden className="size-4 shrink-0" />
          Weaver did not start {entry.jobKind}: {entry.reason.replace(/\.$/, "")}.
        </div>
      );
    case "notice":
      return (
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <Info aria-hidden className="size-3 shrink-0" />
          {entry.text}
        </div>
      );
    case "turn-failed":
      return <ErrorLine text={`The turn failed: ${entry.reason.replace(/\.$/, "")}.`} details={null} />;
    case "omp-stopped":
      return (
        <div className="space-y-1">
          <ErrorLine text={`The agent stopped: ${entry.reason.replace(/\.$/, "")}.`} details={entry.details} />
          <div className="text-xs text-muted-foreground">Your next message starts the agent again.</div>
        </div>
      );
  }
}

/** `Started <job name>` with the button that selects that job in the Jobs view. */
function JobStartedLine({ job, name }: { job: string; name: string }) {
  const { openJob } = useProject();
  return (
    <div className="flex items-center gap-2 text-sm">
      <Bot aria-hidden className="size-4 shrink-0" />
      <span>Started {name}</span>
      <Button variant="outline" size="sm" onClick={() => openJob(job)}>
        Open conversation
      </Button>
    </div>
  );
}

function FailedLine({ reason, details }: { reason: string; details: string | null }) {
  return <ErrorLine text={`Failed: ${reason.replace(/\.$/, "")}.`} details={details} />;
}

/** A failure in the error colour, with an icon, and its details on request. */
function ErrorLine({ text, details }: { text: string; details: string | null }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="text-sm text-rose-ink">
      <div className="flex items-center gap-2">
        <TriangleAlert aria-hidden className="size-4" />
        {text}
        {details && (
          <Button variant="ghost" size="sm" onClick={() => setOpen((value) => !value)}>
            Show details
          </Button>
        )}
      </div>
      {open && details && <pre className="mt-1 overflow-auto rounded-lg bg-muted p-2 font-mono text-xs whitespace-pre-wrap">{details}</pre>}
    </div>
  );
}

/** The box of a message to the agent: a Surface box with a ring, a light shadow and a 14 px radius. */
function MessageFrame({ children }: { children: ReactNode }) {
  return (
    <div className="px-7 pb-6">
      <div className="flex max-w-[860px] items-end gap-2 rounded-[14px] bg-surface p-2 pl-1 shadow-[0_0_0_1px_var(--line-control),0_4px_14px_var(--shadow-color)] focus-within:ring-2 focus-within:ring-ring">
        {children}
      </div>
    </div>
  );
}

/** The textarea grows with its text to about 10 lines, then scrolls. */
const MAX_BOX_HEIGHT = 218;

function MessageBox({ projectId, job }: { projectId: string; job: JobRow }) {
  const [text, setText] = useState("");
  const [problem, setProblem] = useState<string | null>(null);
  const area = useRef<HTMLTextAreaElement>(null);
  const finished = job.state === "done" || job.state === "failed" || job.state === "stopped";
  const { showView } = useProject();
  useLayoutEffect(() => {
    const box = area.current;
    if (!box) return;
    box.style.height = "auto";
    if (box.scrollHeight > 0) box.style.height = `${Math.min(box.scrollHeight + 2, MAX_BOX_HEIGHT)}px`;
  }, [text, finished]);
  if (finished) {
    return (
      <div className="flex items-center gap-2 border-t border-line-soft px-7 py-4 text-sm text-muted-foreground">
        This job has ended. Continue in Chat.
        <Button variant="outline" size="sm" onClick={() => showView("general")}>
          Open Chat
        </Button>
      </div>
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
        <p role="alert" className="px-7 pb-2 text-xs text-rose-ink">
          {problem}
        </p>
      )}
      <MessageFrame>
        <TaggedTextarea
          ref={area}
          aria-label="Message the agent"
          placeholder="Steer the agent while it runs…"
          rows={1}
          className="min-h-9 min-w-0 flex-1 bg-transparent"
          areaClassName="resize-none border-transparent bg-transparent shadow-none focus-visible:ring-0 focus-visible:ring-offset-0"
          value={text}
          disabled={!canSend}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault();
              send();
            }
          }}
        />
        <Button size="icon" aria-label="Send" className="size-[34px] rounded-[10px]" aria-disabled={!canSend || text.trim() === ""} onClick={send}>
          <ArrowUp aria-hidden />
        </Button>
      </MessageFrame>
    </>
  );
}

/** The box of a sub-agent conversation: the layout of the job's message box, with nothing to send. */
function DisabledMessageBox() {
  return (
    <MessageFrame>
      <Textarea
        aria-label="Message the agent"
        placeholder="Sub-agents take no messages."
        rows={1}
        className="min-h-9 min-w-0 flex-1 resize-none border-transparent bg-transparent shadow-none"
        value=""
        readOnly
        disabled
      />
      <Button size="icon" aria-label="Send" className="size-[34px] rounded-[10px]" disabled>
        <ArrowUp aria-hidden />
      </Button>
    </MessageFrame>
  );
}

/** Follows new text unless the developer scrolled up; then `Jump to latest` shows. `version` changes with the content. */
export function FollowScroller({
  version,
  className = "p-4",
  contentClassName = "space-y-3",
  children,
}: {
  version: unknown;
  /** The padding of the scrolling box. */
  className?: string;
  /** The width and spacing of the entries. */
  contentClassName?: string;
  children: ReactNode;
}) {
  const { scrollerRef, contentRef, following, follow, onScroll } = useFollowScroll(version);
  return (
    <div ref={scrollerRef} className={cn("relative min-h-0 flex-1 overflow-y-auto", className)} onScroll={onScroll}>
      <div ref={contentRef} className={contentClassName}>
        {children}
      </div>
      {!following && (
        <Button className="sticky bottom-2 ml-auto mt-3 flex" size="sm" variant="secondary" onClick={follow}>
          <ArrowDown aria-hidden />
          Jump to latest
        </Button>
      )}
    </div>
  );
}

/** The padding and the entries of the conversation of a job or of a sub-agent: 860 px at most, 22 px 28 px of padding. */
const CONVERSATION_PADDING = "px-7 py-[22px]";
const CONVERSATION_CONTENT = "max-w-[860px] space-y-[18px]";

const STOPPABLE: readonly JobState[] = ["queued", "running", "waiting", "suspended"];

/** The job's name with its state pill, when it started and `Stop job`, while the job is not over. */
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
    <header className="flex flex-wrap items-center gap-3 border-b border-line-soft px-7 py-[18px]">
      <div className="flex min-w-0 flex-1 basis-64 flex-col gap-[3px]">
        <div className="flex items-center gap-2.5">
          <h2 className="min-w-0 truncate text-[17px] font-semibold">{job.name}</h2>
          <StateBadge state={job.state} label={working && running ? `${word} · ${running}` : word} />
        </div>
        <span className="text-[13px] text-muted-foreground">{startedLine(job.created)}</span>
      </div>
      {viewable && (
        <Button variant="secondary" className="h-[34px] rounded-[9px] px-3" onClick={viewRequest}>
          <FileText aria-hidden className="size-[13px]" />
          View request
        </Button>
      )}
      {STOPPABLE.includes(job.state) && (
        <Button variant="destructive" className="h-[34px] rounded-[9px] px-3" onClick={() => setConfirming(true)}>
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
    </header>
  );
}

function Conversation({ projectId, job, ompReady }: { projectId: string; job: JobRow; ompReady: boolean }) {
  const entries = useConversation(projectId, job.id);
  return (
    <div className="flex h-full min-h-0 flex-col">
      <JobHeader projectId={projectId} job={job} />
      <FollowScroller version={entries} className={CONVERSATION_PADDING} contentClassName={CONVERSATION_CONTENT}>
        {job.state === "queued" && (
          <p className="py-8 text-center text-sm text-muted-foreground">{ompReady ? "Starting Weaver…" : "Waiting for the agent to be ready."}</p>
        )}
        {entries.map((entry, index) => (
          <EntryView
            key={index}
            entry={entry}
            answer={(request, option) => void call("job_answer_permission", { projectId, jobId: job.id, request, option })}
          />
        ))}
        {job.state === "suspended" && (
          <p className="text-sm text-muted-foreground">Weaver stopped; the job continues when its session is reloaded.</p>
        )}
      </FollowScroller>
      <MessageBox projectId={projectId} job={job} />
    </div>
  );
}

const NOT_ASKED = () => {};

/**
 * The read-only conversation of the sub-agent at `path` (of a job, or of General): the path from
 * the owner with the state, then what it received and did, and a disabled message box at the
 * bottom. No `Stop`, no permission card, no notes, no end line.
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
  const Title = headerOf ? "h1" : "h2";
  return (
    <div className="flex h-full min-h-0 flex-col">
      <header className={cn(headerOf ? VIEW_HEADER_CLASS : "flex items-center gap-3 border-b border-line-soft px-7 py-[18px]", "text-sm")}>
        {back}
        <Title id={headerOf && viewTitleId(headerOf)} className="min-w-0 truncate font-semibold">
          {[ownerName, ...namesAlong(nodes, path)].join(" › ")}
        </Title>
        {node && <AgentState state={node.state} />}
        {actions && <div className="ml-auto">{actions}</div>}
      </header>
      <FollowScroller version={entries} className={CONVERSATION_PADDING} contentClassName={CONVERSATION_CONTENT}>
        {unavailable !== null ? (
          <p className="text-sm text-muted-foreground">Conversation not available: {unavailable.replace(/\.$/, "")}.</p>
        ) : (
          entries.map((entry, index) => <EntryView key={index} entry={entry} answer={NOT_ASKED} />)
        )}
      </FollowScroller>
      <DisabledMessageBox />
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
        <RowButton
          name={ask.name}
          kind="ask"
          pill={
            ask.status === "idle" ? (
              <Pill className={NEUTRAL_PILL}>
                <CircleDashed aria-hidden />
                idle
              </Pill>
            ) : (
              <StateBadge state={ask.status} />
            )
          }
          note={null}
          meta={startedText(ask.opened)}
          current={current}
          onSelect={onSelect}
        />
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

/**
 * The Jobs view: the project's jobs and Ask more conversations on the left, the selected one's
 * conversation, or its selected sub-agent's, on the right.
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
  const panels = (
    <div className="flex min-h-0 flex-1">
      <div className="flex w-[300px] shrink-0 flex-col border-r border-line-soft bg-lane">
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
                const list = here && nodes.length > 0 && (
                  <SubAgentList nodes={nodes} selected={agentPath} onSelect={(path) => setAgent({ owner: key, path })} />
                );
                return row.type === "job" ? (
                  <Row key={key} job={row.job} selected={here} current={here && !agentPath} onSelect={() => select(row)}>
                    {list}
                  </Row>
                ) : (
                  <AskListRow key={key} ask={row.ask} selected={here} current={here && !agentPath} onSelect={() => select(row)}>
                    {list}
                  </AskListRow>
                );
              })}
            </ul>
          )}
        </nav>
      </div>
      <section aria-label="Conversation" className="min-w-0 flex-1">
        <SubAgentNavProvider value={nav}>
          {selected &&
            (agentPath && api ? (
              <SubAgentConversationView
                api={api}
                ownerName={selected.type === "job" ? selected.job.name : selected.ask.name}
                nodes={nodes}
                path={agentPath}
              />
            ) : selected.type === "job" ? (
              <Conversation key={selected.job.id} projectId={projectId} job={selected.job} ompReady={ompReady} />
            ) : (
              <AskConversation key={selected.ask.id} ask={selected.ask} focus={focus.id === selected.ask.id ? focus.nonce : 0} />
            ))}
        </SubAgentNavProvider>
      </section>
    </div>
  );
  return (
    <div className="flex h-full min-h-0 flex-col">
      <ViewHeader view="jobs" />
      {panels}
    </div>
  );
}
