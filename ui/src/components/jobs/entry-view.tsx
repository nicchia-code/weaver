import {
  Bot,
  Check,
  ChevronDown,
  ChevronRight,
  Clock,
  FilePen,
  FileText,
  Info,
  Loader,
  Search,
  SquareTerminal,
  StickyNote,
  TriangleAlert,
  Wrench,
  X,
  type LucideIcon,
} from "lucide-react";
import { useState } from "react";

import type { Entry } from "@/bindings/Entry";
import type { Tool } from "@/bindings/Tool";
import { startedText } from "@/components/jobs/format";
import { Markdown } from "@/components/markdown";
import { useProject } from "@/components/project-context";
import { SubAgentLine } from "@/components/sub-agents/sub-agent-ui";
import { Button } from "@/components/ui/button";

const TOOL_ICONS: Record<Tool["icon"], LucideIcon> = {
  read: FileText,
  search: Search,
  command: SquareTerminal,
  edit: FilePen,
  "sub-agent": Bot,
  other: Wrench,
};

export function ToolRow({ tool }: { tool: Tool }) {
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

/** A message of the developer: a box on Paper, right-aligned, at most 560 px wide. */
function DeveloperMessage({ entry }: { entry: Extract<Entry, { kind: "developer" }> }) {
  return (
    <div className="flex justify-end">
      <div className="max-w-[560px] whitespace-pre-wrap rounded-2xl rounded-br-[4px] bg-paper px-4 py-3 text-sm leading-[1.55]">
        {entry.text}
        {entry.delivery.status === "queued" && (
          <div className="mt-1 flex items-center gap-1 text-xs text-muted-foreground">
            <Clock aria-hidden className="size-3" />
            Queued
          </div>
        )}
        {entry.delivery.status === "not-delivered" && (
          <div className="mt-1 text-xs text-rose-ink">Not delivered: {entry.delivery.reason}.</div>
        )}
      </div>
    </div>
  );
}

/**
 * One entry of a conversation: Chat, a job, an Ask more conversation and a sub-agent render alike.
 * The permission card answers through `answer`.
 */
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
      return <DeveloperMessage entry={entry} />;
    case "agent":
      return (
        <div className="text-[14.5px] leading-[1.65]">
          <Markdown>{entry.text}</Markdown>
        </div>
      );
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
