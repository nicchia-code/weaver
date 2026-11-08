import type { Ghost } from "@/bindings/Ghost";
import type { GhostPhase } from "@/bindings/GhostPhase";
import { KindMark, kindLabel } from "@/components/board/kind-mark";
import { useProject } from "@/components/project-context";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";

const PHASES: Record<GhostPhase, string> = {
  preparing: "Preparing",
  writing: "Writing",
  "waiting-for-answers": "Waiting for your answers",
  "being-refined": "Being refined",
};

/** The phase line of a ghost. */
export function phaseText(phase: GhostPhase): string {
  return PHASES[phase];
}

/** A muted bar or block of a skeleton: no text, the only part of a ghost that shimmers. */
function Placeholder({ className }: { className: string }) {
  return <span aria-hidden data-placeholder className={`shimmer block shrink-0 bg-muted ${className}`} />;
}

const BAR = "h-3 rounded-sm";
const BUTTON = "h-8 w-24 rounded-md";

/**
 * The placeholders of what the card of this kind will have and the ghost does not know yet, in the
 * order and with the shape of the real card: Proposal (id, files line, Tasks line, two controls),
 * Task (status dot next to the id, `Run`), Drift (id, `Resolve` and `Dismiss Drift`).
 */
function Placeholders({ kind }: { kind: Ghost["kind"] }) {
  switch (kind) {
    case "proposal":
      return (
        <span className="mt-1 flex flex-col gap-2">
          <Placeholder className={`${BAR} w-12`} />
          <Placeholder className={`${BAR} w-3/4`} />
          <Placeholder className={`${BAR} w-2/3`} />
          <span className="flex gap-2">
            <Placeholder className={BUTTON} />
            <Placeholder className={BUTTON} />
          </span>
        </span>
      );
    case "task":
      return (
        <span className="mt-1 flex flex-col gap-2">
          <span className="flex items-center gap-2">
            <Placeholder className="size-2 rounded-full" />
            <Placeholder className={`${BAR} w-12`} />
          </span>
          <Placeholder className={`${BUTTON} w-20`} />
        </span>
      );
    case "drift":
      return (
        <span className="mt-1 flex flex-col gap-2">
          <Placeholder className={`${BAR} w-12`} />
          <span className="flex gap-2">
            <Placeholder className={BUTTON} />
            <Placeholder className={`${BUTTON} w-28`} />
          </span>
        </span>
      );
  }
}

/**
 * A card a job announced and has not filed yet: a skeleton at the bottom of its column (dashed
 * border, 70% opacity). The known part (kind, title, phase line, the job's latest note on one
 * line) is still; only the placeholders of what the real card will have shimmer. The whole ghost
 * is one button that opens the job's conversation in Jobs.
 */
export function GhostCard({ ghost }: { ghost: Ghost }) {
  const { openJob } = useProject();
  const kind = kindLabel(ghost.kind);
  const phase = phaseText(ghost.phase);
  return (
    <button
      type="button"
      data-ghost-id={ghost.id}
      aria-label={`${kind} being prepared by the agent: ${ghost.title}, ${phase}`}
      onClick={() => openJob(ghost.jobId)}
      className="flex w-full flex-col gap-1 rounded-xl border border-dashed bg-card p-3.5 text-left text-card-foreground opacity-70 transition-colors duration-150 hover:bg-accent/40 focus-visible:opacity-100 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <KindMark kind={ghost.kind} />
      <span className="line-clamp-3 text-[15px] font-semibold leading-snug tracking-tight">{ghost.title}</span>
      <span className="w-fit text-xs text-muted-foreground">{phase}</span>
      {ghost.note ? (
        <Tooltip>
          <TooltipTrigger asChild>
            <span className="block truncate text-xs text-muted-foreground">{ghost.note}</span>
          </TooltipTrigger>
          <TooltipContent>{ghost.note}</TooltipContent>
        </Tooltip>
      ) : null}
      <Placeholders kind={ghost.kind} />
    </button>
  );
}
