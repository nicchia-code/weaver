import { Check, FileCheck, TriangleAlert } from "lucide-react";
import { useCallback, useState, type ReactNode } from "react";

import type { ChatProposal } from "@/bindings/ChatProposal";
import type { ChatProposalStatus } from "@/bindings/ChatProposalStatus";
import { BlockedPopover, ProposalControlRow, type ProposalDecisions } from "@/components/board/proposal-card";
import { useProject } from "@/components/project-context";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { call, isAppError } from "@/lib/backend";

/** The most paths or Tasks a box lists before `and <n> more`. */
const LISTED = 6;

const PILLS: Record<ChatProposalStatus, { text: string; variant: "muted" | "lavender" | "info" | "success" | "error" }> = {
  draft: { text: "Draft", variant: "muted" },
  open: { text: "Open", variant: "lavender" },
  refining: { text: "Refining", variant: "info" },
  approved: { text: "Approved", variant: "success" },
  rejected: { text: "Rejected", variant: "error" },
  discarded: { text: "Discarded", variant: "muted" },
};

/** How the controls name where it stands: a Draft in a card is one a refinement left half-way. */
const STANDINGS = { draft: "interrupted", open: "open", refining: "refining" } as const;

const FILE_ACTIONS = [
  { key: "added", word: "added" },
  { key: "changed", word: "changed" },
  { key: "removed", word: "removed" },
] as const;

/** A box of the body: its title in Muted, then its lines, at most `LISTED` and `and <n> more`. */
function ListBox({ title, lines, label }: { title: string; lines: readonly ReactNode[]; label: string }) {
  const rest = lines.length - LISTED;
  return (
    <section aria-label={label} className="flex min-w-0 flex-col gap-2 rounded-[10px] px-3.5 py-3 shadow-[inset_0_0_0_1px_var(--color-line-soft)]">
      <h4 className="text-xs font-normal text-muted-foreground">{title}</h4>
      <ul className="flex flex-col gap-1.5">
        {lines.slice(0, LISTED).map((line, index) => (
          <li key={index} className="flex min-w-0 items-baseline gap-2 text-xs">
            {line}
          </li>
        ))}
      </ul>
      {rest > 0 ? <p className="text-xs text-muted-foreground">and {rest} more</p> : null}
    </section>
  );
}

function FilesBox({ files }: { files: ChatProposal["view"]["files"] }) {
  const lines = FILE_ACTIONS.flatMap(({ key, word }) =>
    files[key].map((path) => (
      <>
        <span className="min-w-0 flex-1 truncate font-mono">{path}</span>
        <span className="shrink-0 text-muted-foreground">{word}</span>
      </>
    )),
  );
  if (lines.length === 0) return null;
  return <ListBox label="Files" title="Files" lines={lines} />;
}

function TasksBox({ view }: { view: ChatProposal["view"] }) {
  if (!view.carriesTasks) return null;
  const titles = [...view.tasks, ...view.removalTasks].map((task) => task.title);
  const heading = `Creates ${view.creates} ${view.creates === 1 ? "Task" : "Tasks"}`;
  return <ListBox label="Tasks it creates" title={heading} lines={titles.map((title) => <span className="min-w-0 truncate text-[13px] font-medium">{title}</span>)} />;
}

/** The reasons that block it, one per line, in a Peach box. */
function BlockedBox({ reasons }: { reasons: readonly string[] }) {
  return (
    <div role="group" aria-label="Blocked by" className="flex gap-2.5 rounded-[10px] bg-peach-tint px-3.5 py-3 text-peach-ink">
      <TriangleAlert aria-hidden className="mt-0.5 size-4 shrink-0" />
      <ul className="flex flex-col gap-1 text-[13px] leading-[1.55]">
        {reasons.map((reason) => (
          <li key={reason}>{reason}</li>
        ))}
      </ul>
    </div>
  );
}

/** The footer of a decided Proposal: one line. */
function DecidedFooter({ proposal }: { proposal: ChatProposal }) {
  const { showView, openJob } = useProject();
  const { status } = proposal;
  if (status === "approved") {
    const count = proposal.tasksOnBoard;
    const text = count === 0 ? "Approved." : `Approved. ${count} ${count === 1 ? "Task is" : "Tasks are"} on the Board.`;
    return (
      <div className="flex flex-wrap items-center gap-2.5 border-t border-[#DCEBD8] bg-sage-tint px-[18px] py-3.5 text-sage-ink dark:border-sage/30">
        <Check aria-hidden className="size-4 shrink-0" />
        <span className="flex-1 text-[13px] font-medium">{text}</span>
        <Button variant="outline" size="sm" onClick={() => showView("board")}>
          Open Board
        </Button>
      </div>
    );
  }
  const text = status === "discarded" ? "Discarded." : proposal.reworked ? "Sent back for rework." : "Rejected.";
  return (
    <div className="flex flex-wrap items-center gap-2.5 border-t border-line-soft bg-lane px-[18px] py-3.5 text-ink-2">
      <span className="flex-1 text-[13px] font-medium">{text}</span>
      {proposal.reworked && proposal.reworkJob ? (
        <Button variant="outline" size="sm" onClick={() => openJob(proposal.reworkJob!)}>
          Open conversation
        </Button>
      ) : null}
    </div>
  );
}

/**
 * The card of a Proposal a job of the Chat handed over (`requirements/ui#general-chat`, **Proposal
 * card**): what the card of the Board shows, decided here with the Board's controls, reasons and
 * dialogs, and then one line. A decision the backend refuses shows its reason under the controls and
 * changes nothing. `refresh` asks the backend again once a decision went through.
 */
export function ChatProposalCard({ proposal, refresh }: { proposal: ChatProposal; refresh: () => void }) {
  const { projectId } = useProject();
  const { head, view, status } = proposal;
  const name = head.shortNumber ?? head.id;
  const [refusal, setRefusal] = useState<string | null>(null);
  const live = status === "draft" || status === "open" || status === "refining";
  const pill = PILLS[status];
  const hasLists = view.files.added.length + view.files.changed.length + view.files.removed.length > 0 || view.carriesTasks;
  const showBlocked = live && view.blocked.length > 0;

  /** Runs a decision; the backend checks it again when applying it, and a refusal is shown, not applied. */
  const decide = useCallback(
    async (verb: string, run: () => Promise<unknown>) => {
      setRefusal(null);
      try {
        await run();
      } catch (error) {
        setRefusal(
          isAppError(error) && error.code === "refused"
            ? error.message
            : `Could not ${verb} ${name}${isAppError(error) ? `: ${error.message}` : ""}.`,
        );
      } finally {
        refresh();
      }
    },
    [name, refresh],
  );
  const decisions: ProposalDecisions = {
    approve: (id) => decide("approve", () => call("approve_proposal", { projectId, proposalId: id })),
    reject: (id, _name, feedback, rework) =>
      decide(rework ? "rework" : "reject", () => call("reject_proposal", { projectId, proposalId: id, feedback, rework })),
    discard: (id) => decide("discard", () => call("discard_proposal", { projectId, proposalId: id })),
  };

  return (
    <article
      aria-label={`Proposal ${name}`}
      className="flex flex-col overflow-hidden rounded-[14px] bg-surface shadow-[0_0_0_1px_#E2DDF0,0_4px_14px_rgba(91,74,153,0.07)] dark:shadow-[0_0_0_1px_var(--color-line),0_4px_14px_rgba(0,0,0,0.25)]"
    >
      <div className="flex items-start gap-3 border-b border-[#ECE7F6] bg-lavender-tint/40 px-[18px] py-4 dark:border-line">
        <span aria-hidden className="grid size-8 shrink-0 place-items-center rounded-[9px] bg-lavender-tint text-lavender-ink">
          <FileCheck className="size-4" />
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs font-medium text-lavender-ink">Proposal {name}</span>
            <Badge variant={pill.variant}>{pill.text}</Badge>
            {live && view.stale ? <Badge variant="warning">stale</Badge> : null}
            {live && view.blocked.length > 0 ? <BlockedPopover reasons={view.blocked} /> : null}
          </div>
          <h3 className="text-base font-semibold tracking-title">{head.title}</h3>
        </div>
      </div>

      {hasLists || showBlocked ? (
        <div className="flex flex-col gap-4 p-[18px]">
          {hasLists ? (
            <div className="grid grid-cols-[repeat(auto-fit,minmax(240px,1fr))] gap-2.5">
              <FilesBox files={view.files} />
              <TasksBox view={view} />
            </div>
          ) : null}
          {showBlocked ? <BlockedBox reasons={view.blocked} /> : null}
        </div>
      ) : null}

      {live ? (
        <div className="flex flex-col gap-2 border-t border-line-soft bg-lane px-[18px] py-3.5">
          <div className="flex flex-wrap items-center gap-2">
            <ProposalControlRow card={{ head, view, standing: STANDINGS[status as keyof typeof STANDINGS] }} decisions={decisions} chat />
            <span className="ml-auto font-mono text-[11.5px] text-muted-foreground">{head.id}</span>
          </div>
          {refusal ? (
            <p role="alert" className="text-xs text-rose-ink">
              {refusal}
            </p>
          ) : null}
        </div>
      ) : (
        <DecidedFooter proposal={proposal} />
      )}
    </article>
  );
}
