import { ChevronLeft, ChevronRight, Crosshair, X } from "lucide-react";
import { useId } from "react";

import { GuardedButton } from "@/components/guarded-button";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { subjectText, type ViewName } from "@/components/views/focus-model";
import { useViews } from "@/components/views/views-state";

/**
 * The focus bar under a view's header: what the focus is about, the step name, `<n> of <N>` with
 * `Previous` / `Next` (only when there are several steps), `<M> not found`, on Requirements the
 * `Only these` toggle, and `Clear`. Step changes are announced to assistive technologies.
 */
export function FocusBar({ view }: { view: ViewName }) {
  const { focus, goStep, clearFocus, setOnly } = useViews();
  const active = focus[view];
  const onlyId = useId();
  if (!active) return null;
  const { steps, notFound } = active.plan;
  const step = steps[active.index];
  const announcement = step ? `${step.name}${steps.length > 1 ? `, ${active.index + 1} of ${steps.length}` : ""}` : "";
  return (
    <div role="region" aria-label="Focus" className="flex flex-wrap items-center gap-x-4 gap-y-1 border-b bg-sky-tint px-4 py-2 text-sm">
      <Crosshair className="size-4 shrink-0 text-sky-ink" aria-hidden />
      <span className="min-w-0 truncate font-medium">{subjectText(active.subject)}</span>
      {step ? <span className="min-w-0 truncate text-muted-foreground">{step.name}</span> : null}
      {steps.length > 1 ? (
        <span className="flex items-center gap-1">
          <GuardedButton
            variant="ghost"
            size="icon"
            className="size-7"
            label="Previous"
            reasons={[active.index === 0 && "This is the first step"]}
            onClick={() => goStep(view, active.index - 1)}
          >
            <ChevronLeft aria-hidden />
          </GuardedButton>
          <span className="tabular-nums">
            {active.index + 1} of {steps.length}
          </span>
          <GuardedButton
            variant="ghost"
            size="icon"
            className="size-7"
            label="Next"
            reasons={[active.index === steps.length - 1 && "This is the last step"]}
            onClick={() => goStep(view, active.index + 1)}
          >
            <ChevronRight aria-hidden />
          </GuardedButton>
        </span>
      ) : null}
      {notFound.length > 0 ? (
        <Tooltip>
          <TooltipTrigger asChild>
            <span tabIndex={0} className="rounded-sm text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
              {notFound.length} not found
            </span>
          </TooltipTrigger>
          <TooltipContent>{notFound.join("\n")}</TooltipContent>
        </Tooltip>
      ) : null}
      <span className="ml-auto flex items-center gap-3">
        {view === "requirements" ? (
          <span className="flex items-center gap-2">
            <Checkbox id={onlyId} checked={active.only} onCheckedChange={(checked) => setOnly(checked === true)} />
            <Label htmlFor={onlyId}>Only these</Label>
          </span>
        ) : null}
        <Button variant="outline" size="sm" onClick={() => clearFocus(view)}>
          <X aria-hidden />
          Clear
        </Button>
      </span>
      <span role="status" aria-live="polite" className="sr-only">
        {announcement}
      </span>
    </div>
  );
}
