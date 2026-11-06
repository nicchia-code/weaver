import { Check, ChevronDown, Cpu, MessageSquare, Settings, Sparkles, Trash2, X } from "lucide-react";
import { useId, useMemo, useRef, useState, type ReactNode } from "react";

import type { ProposalView } from "@/bindings/ProposalView";
import { AskMoreButton } from "@/components/ask/ask-button";
import { useBoard } from "@/components/board/board-state";
import { useBoardUi } from "@/components/board/board-ui";
import { registerCardRenderer, type BoardCardOf } from "@/components/board/card-registry";
import { CardShell } from "@/components/board/card-shell";
import { KindMark } from "@/components/board/kind-mark";
import { FocusButton, LabelBadge } from "@/components/board/task-card";
import { CardId } from "@/components/card-id";
import { ConfirmDialog, FormDialog } from "@/components/dialogs";
import { GuardedButton } from "@/components/guarded-button";
import { Markdown } from "@/components/markdown";
import { modelCatalogOf } from "@/components/omp-status";
import { useProject } from "@/components/project-context";
import { Badge, badgeVariants } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Label } from "@/components/ui/label";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import type { FocusSubject } from "@/components/views/focus-model";
import { useOmp } from "@/hooks/use-omp";
import { cn } from "@/lib/utils";

type ProposalCardData = BoardCardOf<"proposal"> | BoardCardOf<"resolution">;

/** How a card is named in a sentence: its short number, else its permanent id. */
function nameOf(ref: { id: string; shortNumber: string | null }): string {
  return ref.shortNumber ?? ref.id;
}

/** The value of the model control that stands for the models of the Settings. */
const SETTINGS_MODELS = "__settings__";

/** `Open`, `Refining`, `Refinement interrupted` or `Draft written by Chat`: the standing of the Proposal in words. */
export function standingText(card: Pick<ProposalCardData, "standing">): string {
  if (card.standing === "refining") return "Refining";
  if (card.standing === "interrupted") return "Refinement interrupted";
  if (card.standing === "chat-draft") return "Draft written by Chat";
  return "Open";
}

// ---------------------------------------------------------------------------------------------
// Badges
// ---------------------------------------------------------------------------------------------

/** `blocked`: a button that opens a Popover with every reason, one per line. */
function BlockedPopover({ reasons }: { reasons: readonly string[] }) {
  const text = reasons.length === 1 ? "Blocked: 1 reason" : `Blocked: ${reasons.length} reasons`;
  return (
    <Popover>
      <Tooltip>
        <TooltipTrigger asChild>
          <PopoverTrigger asChild>
            <button
              type="button"
              aria-label={text}
              className={cn(badgeVariants({ variant: "error" }), "cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring")}
            >
              blocked
            </button>
          </PopoverTrigger>
        </TooltipTrigger>
        <TooltipContent>{text}</TooltipContent>
      </Tooltip>
      <PopoverContent aria-label="Blocked by">
        <p className="mb-2 text-xs font-semibold">Blocked by</p>
        <ul className="space-y-1">
          {reasons.map((reason) => (
            <li key={reason} className="text-xs">
              {reason}
            </li>
          ))}
        </ul>
      </PopoverContent>
    </Popover>
  );
}

/** `draft` and `Chat` (a Draft the agent of Chat writes), `stale`, `refining`, `refinement interrupted`, `blocked` and the chosen model, in that order. */
function ProposalBadges({ card }: { card: ProposalCardData }) {
  const { view, standing } = card;
  return (
    <>
      {standing === "chat-draft" ? (
        <>
          <Badge variant="outline">draft</Badge>
          <Badge variant="outline">Chat</Badge>
        </>
      ) : null}
      {view.stale ? <Badge variant="warning">stale</Badge> : null}
      {standing === "refining" ? <Badge variant="info">refining</Badge> : null}
      {standing === "interrupted" ? <Badge variant="warning">refinement interrupted</Badge> : null}
      {view.blocked.length > 0 ? <BlockedPopover reasons={view.blocked} /> : null}
      {view.model ? (
        <Badge variant="outline" title={`Model ${view.model}`}>
          <Cpu aria-hidden />
          {view.model}
        </Badge>
      ) : null}
    </>
  );
}

/** `<n> files` and a muted `+a ~c −r`; nothing when the Proposal changes no file. */
function FilesLine({ files }: { files: ProposalView["files"] }) {
  const total = files.added.length + files.changed.length + files.removed.length;
  if (total === 0) return null;
  return (
    <p className="text-xs">
      {total} {total === 1 ? "file" : "files"}{" "}
      <span className="text-muted-foreground">
        +{files.added.length} ~{files.changed.length} −{files.removed.length}
      </span>
    </p>
  );
}

/** `Creates <n> Tasks` and the label as a badge in its colour, only for a Proposal that carries Tasks. */
function TasksLine({ view }: { view: ProposalView }) {
  if (!view.carriesTasks) return null;
  return (
    <p className="flex flex-wrap items-center gap-2 text-xs">
      <span>
        Creates {view.creates} {view.creates === 1 ? "Task" : "Tasks"}
      </span>
      {view.label ? <LabelBadge label={view.label} /> : null}
    </p>
  );
}

const DECISIONS: Record<string, (option: number | null) => string> = {
  "fix-code": (option) => (option === null ? "Fix the code" : `Fix the code: option ${option}`),
  proposal: () => "Change the Model",
  dismissed: () => "Let it be",
};

function decisionText(decision: string, option: number | null): string {
  return (DECISIONS[decision] ?? (() => decision))(option);
}

/** One row per Drift a resolution lists: its id, the decision and, muted, the note. A row opens the Drift's details. */
function ResolutionRows({ rows }: { rows: BoardCardOf<"resolution">["drifts"] }) {
  const { openDetails } = useBoardUi();
  if (rows.length === 0) return null;
  return (
    <ul className="space-y-1 text-xs">
      {rows.map((row) => (
        <li key={row.drift.id} data-drift-row={row.drift.id} tabIndex={-1} className="rounded-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
          <button
            type="button"
            className="w-full rounded-sm text-left hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            onClick={() => openDetails(row.drift.id)}
          >
            <span className="font-mono">{nameOf(row.drift)}</span> {decisionText(row.decision, row.option)}
            {row.note ? <span className="text-muted-foreground"> · {row.note}</span> : null}
          </button>
        </li>
      ))}
    </ul>
  );
}

// ---------------------------------------------------------------------------------------------
// Controls
// ---------------------------------------------------------------------------------------------

type Dialogs = "reject" | "rework" | "rework-blank" | "discard" | null;

/** The model control: a Select of the models omp lists, `Use the Settings models` first; a choice applies at once. */
function ModelControl({ card }: { card: ProposalCardData }) {
  const { chooseProposalModel } = useBoard();
  const { status } = useOmp();
  const catalog = useMemo(() => modelCatalogOf(status), [status]);
  const [open, setOpen] = useState(false);
  const controlId = useId();
  const chosen = card.view.model;
  const models = catalog.status === "listed" ? catalog.models.map((model) => model.selector) : [];
  // A model that omp no longer lists stays visible, so the choice can be seen and undone.
  if (chosen && !models.includes(chosen)) models.unshift(chosen);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <Tooltip>
        <TooltipTrigger asChild>
          <PopoverTrigger asChild>
            <Button variant="outline" size="icon" className="ml-auto size-8" aria-label="Model">
              <Settings aria-hidden />
            </Button>
          </PopoverTrigger>
        </TooltipTrigger>
        <TooltipContent>Model for the jobs of this Proposal</TooltipContent>
      </Tooltip>
      <PopoverContent aria-label="Model" className="w-72">
        <Label htmlFor={controlId} className="mb-2 block text-xs font-semibold">
          Model
        </Label>
        <Select
          value={chosen ?? SETTINGS_MODELS}
          onValueChange={(value) => {
            setOpen(false);
            void chooseProposalModel(card.head.id, nameOf(card.head), value === SETTINGS_MODELS ? null : value);
          }}
        >
          <SelectTrigger id={controlId} className="font-mono">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={SETTINGS_MODELS}>Use the Settings models</SelectItem>
            {models.map((model) => (
              <SelectItem key={model} value={model} className="font-mono">
                {model}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {catalog.status === "unavailable" ? <p className="mt-2 text-xs text-muted-foreground">{catalog.reason}</p> : null}
      </PopoverContent>
    </Popover>
  );
}

/** `Reject`: a Dialog with the optional `Reason`. */
function RejectDialog({ name, open, onOpenChange, onReject }: { name: string; open: boolean; onOpenChange: (open: boolean) => void; onReject: (reason: string | null) => void }) {
  const [reason, setReason] = useState("");
  const fieldId = useId();
  return (
    <FormDialog
      open={open}
      onOpenChange={(next) => {
        if (!next) setReason("");
        onOpenChange(next);
      }}
      title={`Reject ${name}`}
      description="The Proposal is refused. The reason, when you give one, stays with it."
      footer={
        <>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button
            variant="destructive"
            onClick={() => {
              onReject(reason.trim() === "" ? null : reason);
              setReason("");
              onOpenChange(false);
            }}
          >
            Reject
          </Button>
        </>
      }
    >
      <Label htmlFor={fieldId}>Reason</Label>
      <Textarea id={fieldId} className="mt-2" value={reason} onChange={(event) => setReason(event.target.value)} />
    </FormDialog>
  );
}

/** `Rework with feedback…`: a Dialog with the `Feedback`. A blank one asks `Rework without feedback?` first. */
function ReworkDialog({
  name,
  open,
  blankAsked,
  onOpenChange,
  onBlankAskedChange,
  onRework,
  returnFocus,
}: {
  name: string;
  open: boolean;
  blankAsked: boolean;
  onOpenChange: (open: boolean) => void;
  onBlankAskedChange: (asked: boolean) => void;
  onRework: (feedback: string | null) => void;
  returnFocus: React.RefObject<HTMLElement | null>;
}) {
  const [feedback, setFeedback] = useState("");
  const fieldId = useId();
  const submit = (text: string | null) => {
    setFeedback("");
    onRework(text);
  };
  return (
    <>
      <FormDialog
        open={open}
        onOpenChange={(next) => {
          if (!next && !blankAsked) setFeedback("");
          onOpenChange(next);
        }}
        title={`Rework ${name} with feedback`}
        description="The Proposal is rejected and the agent writes it again from your feedback."
        returnFocus={returnFocus}
        footer={
          <>
            <Button variant="outline" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button
              onClick={() => {
                if (feedback.trim() === "") {
                  onOpenChange(false);
                  onBlankAskedChange(true);
                } else {
                  onOpenChange(false);
                  submit(feedback);
                }
              }}
            >
              Rework
            </Button>
          </>
        }
      >
        <Label htmlFor={fieldId}>Feedback</Label>
        <Textarea id={fieldId} className="mt-2" value={feedback} onChange={(event) => setFeedback(event.target.value)} />
      </FormDialog>
      <ConfirmDialog
        open={blankAsked}
        onOpenChange={onBlankAskedChange}
        title="Rework without feedback?"
        description="The agent writes the Proposal again without being told what to change."
        confirmLabel="Rework"
        returnFocus={returnFocus}
        onConfirm={() => submit(null)}
      />
    </>
  );
}

/**
 * The controls of a Draft the agent of Chat is writing: `Open Chat` and `Discard`, which asks first
 * and is disabled while a turn of Chat is in progress (the backend lists why).
 */
function ChatDraftControls({ card }: { card: ProposalCardData }) {
  const { discardProposal } = useBoard();
  const { showView } = useProject();
  const [open, setOpen] = useState(false);
  const { head, view } = card;
  return (
    <>
      <Button variant="outline" size="sm" onClick={() => showView("general")}>
        <MessageSquare aria-hidden />
        Open Chat
      </Button>
      <GuardedButton variant="destructive" size="sm" reasons={view.rejectReasons} onClick={() => setOpen(true)}>
        <Trash2 aria-hidden />
        Discard
      </GuardedButton>
      {/* A click inside the dialog never reaches the card behind it (the dialog lives in the React tree of the card). */}
      <div onClick={(event) => event.stopPropagation()}>
        <ConfirmDialog
          open={open}
          onOpenChange={setOpen}
          title="Discard this Draft?"
          description="Its files and Tasks are thrown away."
          confirmLabel="Discard"
          destructive
          onConfirm={() => void discardProposal(head.id, nameOf(head))}
        />
      </div>
    </>
  );
}

/**
 * The controls of a Proposal: `Approve`, or `Refine` in its place when the backend says a click
 * starts a refinement job; the `Reject` split button (its menu has `Rework with
 * feedback…`, absent once the Proposal is itself a rework), `View requirements`, and last the
 * model control, pushed to the right edge of the row (also when it wraps). A Draft a refinement left half-way has only `Refine` and
 * `Discard`. Every reason a control is disabled comes from the backend, in its fixed order.
 */
function ProposalControls({ card }: { card: ProposalCardData }) {
  const { approveProposal, rejectProposal, discardProposal } = useBoard();
  const { head, view, standing } = card;
  const name = nameOf(head);
  const subject: FocusSubject = { kind: "proposal", id: head.id, number: head.shortNumber, title: head.title };
  const [dialog, setDialog] = useState<Dialogs>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  const arrow = useRef<HTMLButtonElement>(null);
  const rejectBlocked = view.rejectReasons.length > 0;

  const refines = view.approve.refines;
  const approve = (
    <GuardedButton
      size="sm"
      variant={refines ? "refine" : "default"}
      reasons={view.approve.reasons}
      tooltip={view.approve.tooltip ?? undefined}
      onClick={() => void approveProposal(head.id, name)}
    >
      {refines ? <Sparkles aria-hidden /> : <Check aria-hidden />}
      {refines ? "Refine" : "Approve"}
    </GuardedButton>
  );

  if (standing === "chat-draft") return <ChatDraftControls card={card} />;

  // A click inside a dialog never reaches the card behind it (the dialogs live in the React tree of the card).
  if (standing === "interrupted") {
    return (
      <>
        {approve}
        <GuardedButton variant="destructive" size="sm" reasons={view.rejectReasons} onClick={() => setDialog("discard")}>
          <Trash2 aria-hidden />
          Discard
        </GuardedButton>
        <div onClick={(event) => event.stopPropagation()}>
          <ConfirmDialog
            open={dialog === "discard"}
            onOpenChange={(open) => setDialog(open ? "discard" : null)}
            title={`Discard ${name}?`}
            description="The half-refined Proposal is thrown away. The Model stays as it is."
            confirmLabel="Discard"
            destructive
            onConfirm={() => void discardProposal(head.id, name)}
          />
        </div>
      </>
    );
  }

  return (
    <>
      {approve}
      <div className="inline-flex">
        <GuardedButton
          variant="outline"
          size="sm"
          className={view.canRework ? "rounded-r-none" : undefined}
          reasons={view.rejectReasons}
          onClick={() => setDialog("reject")}
        >
          <X aria-hidden />
          Reject
        </GuardedButton>
        {view.canRework ? (
          <DropdownMenu open={menuOpen && !rejectBlocked} onOpenChange={setMenuOpen}>
            <DropdownMenuTrigger asChild>
              <GuardedButton
                ref={arrow}
                variant="outline"
                size="sm"
                label="More ways to reject"
                reasons={view.rejectReasons}
                className="rounded-l-none border-l-0 px-1.5"
              >
                <ChevronDown aria-hidden />
              </GuardedButton>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start">
              <DropdownMenuItem onSelect={() => setDialog("rework")}>Rework with feedback…</DropdownMenuItem>
            </DropdownMenuContent>
          </DropdownMenu>
        ) : null}
      </div>
      <FocusButton view="requirements" refs={view.scopeRefs} subject={subject} />
      {/* `contents`: the dialogs live in the React tree of the card but take no room in its row. */}
      <div className="contents" onClick={(event) => event.stopPropagation()}>
        <RejectDialog
          name={name}
          open={dialog === "reject"}
          onOpenChange={(open) => setDialog(open ? "reject" : null)}
          onReject={(reason) => void rejectProposal(head.id, name, reason, false)}
        />
        <ReworkDialog
          name={name}
          open={dialog === "rework"}
          blankAsked={dialog === "rework-blank"}
          onOpenChange={(open) => setDialog((current) => (open ? "rework" : current === "rework" ? null : current))}
          onBlankAskedChange={(asked) => setDialog(asked ? "rework-blank" : null)}
          onRework={(feedback) => void rejectProposal(head.id, name, feedback, true)}
          returnFocus={arrow}
        />
      </div>
      {view.carriesTasks ? <ModelControl card={card} /> : null}
    </>
  );
}

// ---------------------------------------------------------------------------------------------
// Card and details
// ---------------------------------------------------------------------------------------------

function ProposalCard({ card }: { card: BoardCardOf<"proposal"> }) {
  const { head, view } = card;
  return (
    <CardShell
      cardId={head.id}
      shortNumber={head.shortNumber}
      title={head.title}
      mark={card.proposalKind === "setup" || card.proposalKind === "import" ? <Badge variant="outline">{card.proposalKind}</Badge> : null}
      badges={<ProposalBadges card={card} />}
      body={
        view.files.added.length + view.files.changed.length + view.files.removed.length > 0 || view.carriesTasks ? (
          <div className="space-y-1">
            <FilesLine files={view.files} />
            <TasksLine view={view} />
          </div>
        ) : null
      }
      controls={<ProposalControls card={card} />}
    />
  );
}

function ResolutionCard({ card }: { card: BoardCardOf<"resolution"> }) {
  const { head, view } = card;
  const hasFiles = view.files.added.length + view.files.changed.length + view.files.removed.length > 0;
  return (
    <CardShell
      cardId={head.id}
      shortNumber={head.shortNumber}
      title={head.title}
      mark={<KindMark kind="resolution" />}
      accent="border-l-[3px] border-l-yellow-600 bg-yellow-500/10 dark:border-l-yellow-400"
      badges={<ProposalBadges card={card} />}
      body={
        card.drifts.length > 0 || hasFiles ? (
          <div className="space-y-2">
            <ResolutionRows rows={card.drifts} />
            <FilesLine files={view.files} />
          </div>
        ) : null
      }
      controls={<ProposalControls card={card} />}
    />
  );
}

const FILE_GROUPS = [
  { key: "added", title: "Added", mark: "+" },
  { key: "changed", title: "Changed", mark: "~" },
  { key: "removed", title: "Removed", mark: "−" },
] as const;

const ROW = "w-full rounded-sm px-1 py-0.5 text-left";

/** `Remove code of `order/Gone``: the text between backticks as inline code, without them. */
function titleWithCode(title: string): ReactNode[] {
  return title.split("`").map((part, index) =>
    index % 2 === 1 ? (
      <code key={index} className="rounded bg-muted px-1 font-mono text-xs">
        {part}
      </code>
    ) : (
      part
    ),
  );
}

/** `Tasks it creates`: one compact row per Task, a button that opens the Task's text in a nested Dialog. */
function CreatedTaskRows({ tasks }: { tasks: ProposalView["tasks"] }) {
  const [openId, setOpenId] = useState<string | null>(null);
  const opener = useRef<HTMLElement | null>(null);
  const shown = useRef<ProposalView["tasks"][number] | null>(null);
  if (tasks.length === 0) return null;
  const open = tasks.find((task) => task.id === openId) ?? null;
  // The last Task shown keeps the dialog's content while it closes.
  if (open) shown.current = open;
  const content = shown.current;
  return (
    <section aria-label="Tasks it creates" className="space-y-2">
      <h3 className="text-xs font-semibold">Tasks it creates</h3>
      <ul className="space-y-1">
        {tasks.map((task) => (
          <li key={task.id}>
            <button
              type="button"
              className={cn(ROW, "hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring")}
              onClick={(event) => {
                opener.current = event.currentTarget;
                setOpenId(task.id);
              }}
            >
              {task.title}
            </button>
          </li>
        ))}
      </ul>
      <FormDialog
        open={open !== null}
        onOpenChange={(next) => {
          if (!next) setOpenId(null);
        }}
        title={content?.title ?? ""}
        returnFocus={opener}
      >
        {content?.body.trim() ? <Markdown holderHeadingLevel={2}>{content.body}</Markdown> : <p className="text-muted-foreground">No description.</p>}
      </FormDialog>
    </section>
  );
}

/** `Code-removal Tasks`: the same list style, plain text. */
function RemovalTaskRows({ tasks }: { tasks: ProposalView["removalTasks"] }) {
  if (tasks.length === 0) return null;
  return (
    <section aria-label="Code-removal Tasks" className="space-y-2">
      <h3 className="text-xs font-semibold">Code-removal Tasks</h3>
      <ul className="space-y-1">
        {tasks.map((task) => (
          <li key={task.id} className={ROW}>
            {titleWithCode(task.title)}
          </li>
        ))}
      </ul>
    </section>
  );
}

/** The details dialog: kind, badges, the text, the required Proposals with their state, the files, the Tasks it creates. */
function ProposalDetails({ card }: { card: ProposalCardData }) {
  const { head, view } = card;
  const total = view.files.added.length + view.files.changed.length + view.files.removed.length;
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant="secondary">{card.kind === "resolution" ? "resolution" : card.proposalKind}</Badge>
        <ProposalBadges card={card} />
        {view.carriesTasks && view.label ? <LabelBadge label={view.label} /> : null}
      </div>
      {head.body.trim() ? <Markdown holderHeadingLevel={2}>{head.body}</Markdown> : <p className="text-muted-foreground">No description.</p>}
      {card.kind === "resolution" && card.drifts.length > 0 ? (
        <section aria-label="Drifts" className="space-y-2">
          <h3 className="text-xs font-semibold">Drifts</h3>
          <ResolutionRows rows={card.drifts} />
        </section>
      ) : null}
      {view.requires.length > 0 ? (
        <section aria-label="Requires" className="space-y-2">
          <h3 className="text-xs font-semibold">Requires</h3>
          <ul className="space-y-1">
            {view.requires.map((required) => (
              <li key={required.id} className="flex flex-wrap items-baseline gap-2">
                <CardId shortNumber={required.shortNumber} permanentId={required.id} />
                <span className="min-w-0 flex-1">{required.title}</span>
                <Badge variant={required.problem ? "error" : "success"}>{required.state}</Badge>
                {required.problem ? <span className="text-xs text-muted-foreground">{required.problem}</span> : null}
              </li>
            ))}
          </ul>
        </section>
      ) : null}
      <section aria-label="Files" className="space-y-2">
        <h3 className="text-xs font-semibold">Files</h3>
        {total === 0 ? <p className="text-muted-foreground">No file.</p> : null}
        {FILE_GROUPS.map(({ key, title, mark }) =>
          view.files[key].length > 0 ? (
            <div key={key} role="group" aria-label={title}>
              <p className="text-xs text-muted-foreground">{title}</p>
              <ul>
                {view.files[key].map((path) => (
                  <li key={path} className="font-mono text-xs">
                    <span aria-hidden>{mark} </span>
                    {path}
                  </li>
                ))}
              </ul>
            </div>
          ) : null,
        )}
      </section>
      {view.carriesTasks ? (
        <>
          <CreatedTaskRows tasks={view.tasks} />
          <RemovalTaskRows tasks={view.removalTasks} />
          {view.tasks.length + view.removalTasks.length === 0 ? <p className="text-muted-foreground">It creates no Task.</p> : null}
        </>
      ) : null}
    </div>
  );
}

/** The controls of the card, then `Ask more` (also for a resolution and for a Draft a refinement left half-way). */
function ProposalFooter({ card }: { card: ProposalCardData }) {
  const { head } = card;
  return (
    <>
      <ProposalControls card={card} />
      <AskMoreButton target={{ kind: "proposal", id: head.id }} noun="Proposal" name={head.shortNumber ?? head.id} />
    </>
  );
}

registerCardRenderer("proposal", {
  Card: ProposalCard,
  Details: ProposalDetails,
  Footer: ProposalFooter,
  description: (card) => `${standingText(card)}${card.proposalKind === "change" ? "" : ` · ${card.proposalKind}`}`,
});

registerCardRenderer("resolution", {
  Card: ResolutionCard,
  Details: ProposalDetails,
  Footer: ProposalFooter,
  description: (card) => `${standingText(card)} · resolution`,
});
