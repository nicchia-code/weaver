import { Ban, CheckCheck, Sparkles, Trash2, WandSparkles, Wrench } from "lucide-react";
import { useState } from "react";

import { AskMoreButton } from "@/components/ask/ask-button";
import { useBoard } from "@/components/board/board-state";
import { registerCardRenderer, type BoardCardOf } from "@/components/board/card-registry";
import { CardShell } from "@/components/board/card-shell";
import { KindMark } from "@/components/board/kind-mark";
import { RefChip } from "@/components/board/task-card";
import { ConfirmDialog } from "@/components/dialogs";
import { cardReasons, GuardedButton } from "@/components/guarded-button";
import { Markdown } from "@/components/markdown";
import { OpenInEditor } from "@/components/open-in-editor";
import { Badge } from "@/components/ui/badge";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Textarea } from "@/components/ui/textarea";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { TONE_STRIPE } from "@/lib/tones";

type DriftCardData = BoardCardOf<"drift">;

/** Why `Fix the code` waits on a Drift from a merge, which has no fix option chosen. */
const CHOOSE_AN_OPTION = "Choose a fix option";

/** How a card is named in a sentence: its short number, else its permanent id. */
function nameOf(ref: { id: string; shortNumber: string | null }): string {
  return ref.shortNumber ?? ref.id;
}

function BadgeWithTip({ text, tip, variant }: { text: string; tip: string; variant: "outline" | "secondary" | "info" }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Badge variant={variant} tabIndex={0}>
          {text}
        </Badge>
      </TooltipTrigger>
      <TooltipContent>{tip}</TooltipContent>
    </Tooltip>
  );
}

/** `recurring`, `resolving`, `prepared` and `merge`, in that order, each with its tooltip. */
function DriftBadges({ card }: { card: DriftCardData }) {
  const { view } = card;
  return (
    <>
      {view.previous ? (
        <BadgeWithTip variant="outline" text="recurring" tip={`Repeats Drift ${nameOf(view.previous)}: ${view.previous.title}`} />
      ) : null}
      {view.resolving ? <BadgeWithTip variant="info" text="resolving" tip="An agent is resolving this Drift" /> : null}
      {view.prepared ? <BadgeWithTip variant="secondary" text="prepared" tip="Resolve asks the prepared question at once" /> : null}
      {view.merge ? <BadgeWithTip variant="outline" text="merge" tip="Found by the scan after a merge" /> : null}
    </>
  );
}

/** `Resolve` (primary) and `Dismiss Drift` (outline, confirmed); every reason comes from the backend, the agent first. */
function DriftControls({ card }: { card: DriftCardData }) {
  const { resolveDrift, dismissDrift } = useBoard();
  const [confirming, setConfirming] = useState(false);
  const { head, view } = card;
  const name = nameOf(head);
  return (
    <>
      <GuardedButton size="sm" reasons={cardReasons(head.agentFinishing, view.resolveReasons)} onClick={() => void resolveDrift(head.id, name)}>
        <WandSparkles aria-hidden />
        Resolve
      </GuardedButton>
      <GuardedButton variant="outline" size="sm" reasons={cardReasons(head.agentFinishing, view.dismissReasons)} onClick={() => setConfirming(true)}>
        <Ban aria-hidden />
        Dismiss Drift
      </GuardedButton>
      <div onClick={(event) => event.stopPropagation()}>
        <ConfirmDialog
          open={confirming}
          onOpenChange={setConfirming}
          title={`Dismiss ${name}? Nothing changes in the code or in the Model.`}
          description="The Drift is closed and the agent does not file it again while the Model and the Requirements stay as they are."
          confirmLabel="Dismiss"
          destructive
          onConfirm={() => void dismissDrift(head.id, name)}
        />
      </div>
    </>
  );
}

function DriftCard({ card }: { card: DriftCardData }) {
  const { head, origin } = card;
  return (
    <CardShell
      cardId={head.id}
      shortNumber={head.shortNumber}
      title={head.title}
      mark={<KindMark kind="drift" />}
      accent={TONE_STRIPE.peach}
      badges={<DriftBadges card={card} />}
      body={origin ? <p className="text-xs text-muted-foreground">Found while working on {nameOf(origin)}</p> : null}
      controls={<DriftControls card={card} />}
    />
  );
}

/**
 * The fix decision: the options (the recommended one preselected, except on a Drift from a merge,
 * which has none chosen: its `Accept Proposal` is the recommended one), the note and `Fix the
 * code`; with an attached Proposal `Accept Proposal`, replaced by `Refine` when it is stale and
 * Open, or by `Refine` and `Discard` when a refinement left it half-way. Every reason comes from
 * the backend.
 */
function DecisionForm({ card }: { card: DriftCardData }) {
  const { fixDriftCode, acceptDriftProposal, refineDriftProposal, discardDriftProposal } = useBoard();
  const { head, view } = card;
  const [option, setOption] = useState(view.merge ? "" : "1");
  const [note, setNote] = useState("");
  const [discarding, setDiscarding] = useState(false);
  const name = nameOf(head);
  const fixReasons = cardReasons(head.agentFinishing, [...view.fixReasons, option === "" && CHOOSE_AN_OPTION]);
  const attached = view.attached;
  return (
    <section aria-label="Decision" className="space-y-3">
      {view.options.length > 0 ? (
        <>
          <RadioGroup value={option} onValueChange={setOption} aria-label="Fix option">
            {view.options.map((item, index) => (
              <div key={item.title} className="flex items-center gap-2">
                <RadioGroupItem value={String(index + 1)} id={`${head.id}-option-${index + 1}`} />
                <Label htmlFor={`${head.id}-option-${index + 1}`}>{item.title}</Label>
              </div>
            ))}
          </RadioGroup>
          <div className="space-y-1">
            <Label htmlFor={`${head.id}-note`}>Note</Label>
            <Textarea id={`${head.id}-note`} value={note} onChange={(event) => setNote(event.target.value)} />
          </div>
          <GuardedButton size="sm" reasons={fixReasons} onClick={() => void fixDriftCode(head.id, name, Number(option), note.trim() || null)}>
            <Wrench aria-hidden />
            Fix the code
          </GuardedButton>
        </>
      ) : (
        <p className="text-muted-foreground">No code change can respect both the Model and the Requirements.</p>
      )}
      {attached?.refine ? (
        <div className="flex flex-wrap gap-2">
          <GuardedButton
            size="sm"
            reasons={cardReasons(head.agentFinishing, attached.refine.reasons)}
            tooltip={attached.refine.tooltip ?? undefined}
            onClick={() => void refineDriftProposal(head.id, name)}
          >
            <Sparkles aria-hidden />
            Refine
          </GuardedButton>
          {attached.discardReasons ? (
            <>
              <GuardedButton
                variant="destructive"
                size="sm"
                reasons={cardReasons(head.agentFinishing, attached.discardReasons)}
                onClick={() => setDiscarding(true)}
              >
                <Trash2 aria-hidden />
                Discard
              </GuardedButton>
              <ConfirmDialog
                open={discarding}
                onOpenChange={setDiscarding}
                title={`Discard ${nameOf(attached)}?`}
                description="The half-refined Proposal is thrown away. The Model stays as it is."
                confirmLabel="Discard"
                destructive
                onConfirm={() => void discardDriftProposal(head.id, name)}
              />
            </>
          ) : null}
        </div>
      ) : attached ? (
        <div className="flex flex-wrap items-center gap-2">
          <GuardedButton
            variant="outline"
            size="sm"
            reasons={cardReasons(head.agentFinishing, attached.acceptReasons)}
            onClick={() => void acceptDriftProposal(head.id, name)}
          >
            <CheckCheck aria-hidden />
            Accept Proposal
          </GuardedButton>
          {view.merge ? <Badge variant="lavender">recommended</Badge> : null}
        </div>
      ) : null}
    </section>
  );
}

function Side({ title, text }: { title: string; text: string }) {
  return (
    <div className="min-w-0 flex-1 basis-64 space-y-1">
      <h3 className="text-sm font-semibold">{title}</h3>
      <Markdown holderHeadingLevel={3}>{text || "—"}</Markdown>
    </div>
  );
}

/** The details: id and badges, references, the two sides, options, where in the code, origin, attached Proposal, decision. */
function DriftDetails({ card }: { card: DriftCardData }) {
  const { view, origin } = card;
  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-1">
        <DriftBadges card={card} />
      </div>
      <ul aria-label="References" className="flex flex-wrap gap-1">
        {view.refs.map((reference) => (
          <li key={reference}>
            <RefChip reference={reference} />
          </li>
        ))}
      </ul>
      <div className="flex flex-wrap gap-4">
        <Side title="What happens" text={view.happens} />
        <Side title="What should happen" text={view.should} />
      </div>
      {view.options.length > 0 ? (
        <section aria-label="Fix options" className="space-y-2">
          <h3 className="text-sm font-semibold">Fix options</h3>
          <ol className="list-decimal space-y-2 pl-5">
            {view.options.map((option, index) => (
              <li key={option.title}>
                <span className="font-medium">{option.title}</span>
                {index === 0 && !view.merge ? <Badge variant="lavender" className="ml-2">recommended</Badge> : null}
                <Markdown holderHeadingLevel={3}>{option.text}</Markdown>
              </li>
            ))}
          </ol>
        </section>
      ) : null}
      <section aria-label="Where in the code" className="space-y-2">
        <h3 className="text-sm font-semibold">Where in the code</h3>
        <Markdown holderHeadingLevel={3}>{view.whereInCode}</Markdown>
        <ul className="space-y-1">
          {view.places.map((place) => (
            <li key={place.file} className="flex items-center gap-2">
              <code className="font-mono text-xs">{place.line ? `${place.file}:${place.line}` : place.file}</code>
              <OpenInEditor file={place.file} line={place.line} />
            </li>
          ))}
        </ul>
      </section>
      {origin || view.previous ? (
        <p className="text-xs text-muted-foreground">
          {origin ? `Found while working on ${nameOf(origin)} ${origin.title}. ` : ""}
          {view.previous ? `Repeats Drift ${nameOf(view.previous)} ${view.previous.title}.` : ""}
        </p>
      ) : null}
      {view.attached ? (
        <section aria-label="Attached Proposal" className="space-y-2 rounded-lg p-3 shadow-ring">
          <h3 className="text-sm font-semibold">
            Attached Proposal {nameOf(view.attached)}: {view.attached.title} <Badge variant="outline">{view.attached.state}</Badge>
            {view.attached.stale ? (
              <Badge variant="warning" className="ml-1">
                stale
              </Badge>
            ) : null}
          </h3>
          <Markdown holderHeadingLevel={3}>{view.attached.body}</Markdown>
          <ul className="font-mono text-xs">
            {[
              ...view.attached.files.added.map((file) => `+ ${file}`),
              ...view.attached.files.changed.map((file) => `~ ${file}`),
              ...view.attached.files.removed.map((file) => `- ${file}`),
            ].map((line) => (
              <li key={line}>{line}</li>
            ))}
          </ul>
        </section>
      ) : null}
      <DecisionForm card={card} />
    </div>
  );
}

/** `Resolve` and `Dismiss Drift` repeat in the footer, then `Ask more`. */
function DriftFooter({ card }: { card: DriftCardData }) {
  const { head } = card;
  return (
    <>
      <DriftControls card={card} />
      <AskMoreButton target={{ kind: "drift", id: head.id }} noun="Drift" name={head.shortNumber ?? head.id} />
    </>
  );
}

registerCardRenderer("drift", { Card: DriftCard, Details: DriftDetails, Footer: DriftFooter, description: () => "Open" });
