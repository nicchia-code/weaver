import type { OmpPhase } from "@/bindings/OmpPhase";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { useOmp } from "@/hooks/use-omp";
import { TONE_DOT, type Tone } from "@/lib/tones";
import { cn } from "@/lib/utils";

/**
 * How each state of the agent reads: its word, its thread and, when the developer has something to
 * do, what (`requirements/ui#general-chat`, **Agent pill**). The agent is never named omp here.
 */
const FACES: Record<OmpPhase, { word: string; tone: Tone; hint: string | null }> = {
  ready: { word: "ready", tone: "sage", hint: null },
  checking: { word: "checking", tone: "sky", hint: null },
  installing: { word: "installing", tone: "sky", hint: null },
  updating: { word: "updating", tone: "sky", hint: null },
  loggingIn: { word: "logging in", tone: "sky", hint: null },
  missing: { word: "not installed", tone: "peach", hint: "Install it in Settings." },
  notConnected: { word: "not connected", tone: "peach", hint: "Log in to a provider in Settings." },
};

/** `Weaver · <state>`: a pill with a dot, from the app-wide state of the agent. */
export function AgentPill() {
  const { status } = useOmp();
  const face = FACES[status?.state ?? "checking"];
  const pill = (
    <span
      tabIndex={face.hint ? 0 : undefined}
      className="inline-flex shrink-0 items-center gap-1.5 rounded-full border border-line py-[3px] pl-2 pr-2.5 text-xs text-ink-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <span aria-hidden className={cn("size-1.5 rounded-full", TONE_DOT[face.tone])} />
      {`Weaver · ${face.word}`}
    </span>
  );
  if (!face.hint) return pill;
  return (
    <Tooltip>
      <TooltipTrigger asChild>{pill}</TooltipTrigger>
      <TooltipContent>{face.hint}</TooltipContent>
    </Tooltip>
  );
}
