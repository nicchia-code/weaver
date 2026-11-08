import { Cloud, Code, Database, FlaskConical, GitBranch, Layers, NotebookPen, Rocket, Server, Sparkles, Workflow, type LucideIcon } from "lucide-react";
import { useEffect, useRef, useState, type ReactNode } from "react";

import type { SpecEntryView } from "@/bindings/SpecEntryView";
import type { SpecsFieldView } from "@/bindings/SpecsFieldView";
import { Markdown } from "@/components/markdown";
import { OpenInEditor } from "@/components/open-in-editor";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { ChangeBadge, focusClass } from "@/components/views/change-badge";
import { logoKey, useLogos, type LogoIndex } from "@/components/views/spec-logos";
import { ViewFrame } from "@/components/views/view-frame";
import { useViews } from "@/components/views/views-state";
import { cn } from "@/lib/utils";

/** `languages` → `Languages`; `ci` → `CI`. */
export function fieldTitle(name: string): string {
  return name === "ci" ? "CI" : name.charAt(0).toUpperCase() + name.slice(1);
}

const chosenByAgent = (entry: SpecEntryView) => entry.chosenReason !== null;

// Full component names, one per key of the Specs format (`SPECS_FIELDS` and `NOTES_FIELD` of the core).
const FIELD_ICON: Record<string, LucideIcon> = {
  languages: Code,
  frameworks: Layers,
  data: Database,
  testing: FlaskConical,
  ci: Workflow,
  repository: GitBranch,
  deployment: Rocket,
  cloud: Cloud,
  infrastructure: Server,
  notes: NotebookPen,
};

/** The 16 px icon of a card's tile: the monochrome logo of the technology, or its field's icon while there is none. */
function EntryIcon({ name, logos, Fallback }: { name: string; logos: LogoIndex | null; Fallback: LucideIcon }) {
  const path = logos?.get(logoKey(name));
  if (path === undefined) return <Fallback aria-hidden className="size-4" />;
  return (
    <svg aria-hidden viewBox="0 0 24 24" fill="currentColor" className="size-4">
      <path d={path} />
    </svg>
  );
}

function Entry({ entry, logos, Icon }: { entry: SpecEntryView; logos: LogoIndex | null; Icon: LucideIcon }) {
  const [before, setBefore] = useState(false);
  const removed = entry.change === "removed";
  return (
    <li
      aria-label={`${entry.name}${entry.change ? `, ${entry.change}` : ""}`}
      className={cn("flex min-h-[92px] flex-col gap-2 rounded-lg bg-card p-3.5 shadow-card", removed && "text-muted-foreground")}
    >
      <div className="flex items-center gap-2.5">
        <span aria-hidden className="grid size-[26px] shrink-0 place-items-center rounded-sm bg-paper text-ink-2">
          <EntryIcon name={entry.name} logos={logos} Fallback={Icon} />
        </span>
        <span className={cn("min-w-0 flex-1 text-sm font-semibold [overflow-wrap:anywhere]", removed && "line-through")}>{entry.name}</span>
        {entry.version ? <span className="shrink-0 rounded-sm bg-paper px-1.5 py-0.5 font-mono text-xs text-ink-2">{entry.version}</span> : null}
      </div>
      {entry.note ? <p className="text-[13px] leading-normal text-ink-2">{entry.note}</p> : null}
      {entry.change === "changed" && entry.before !== null ? (
        <div>
          <Button variant="ghost" size="sm" className="text-[13px]" aria-pressed={before} onClick={() => setBefore((v) => !v)}>
            Show before
          </Button>
          {before ? <p className="text-[13px] leading-normal text-muted-foreground">Before: {entry.before}</p> : null}
        </div>
      ) : null}
      {entry.change || entry.chosenReason !== null ? (
        <div className="mt-auto flex items-center gap-2">
          {entry.change ? <ChangeBadge change={entry.change} /> : null}
          {entry.chosenReason !== null ? (
            <Tooltip>
              <TooltipTrigger asChild>
                <Badge variant="lavender" tabIndex={0} className="focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                  <Sparkles aria-hidden />
                  Agent's choice
                </Badge>
              </TooltipTrigger>
              <TooltipContent>{entry.chosenReason}</TooltipContent>
            </Tooltip>
          ) : null}
        </div>
      ) : null}
    </li>
  );
}

function Field({
  field,
  entries,
  focused,
  logos,
}: {
  field: SpecsFieldView;
  entries: readonly SpecEntryView[];
  focused: boolean;
  logos: LogoIndex | null;
}) {
  const [before, setBefore] = useState(false);
  const title = fieldTitle(field.name);
  const Icon = FIELD_ICON[field.name] ?? Layers;
  return (
    // The focus border and tint hang out to the left (-ml-4 = 4 px border + 12 px padding), so the content keeps the left edge of the view header's title.
    <section
      data-step-key={field.name}
      aria-labelledby={`specs-${field.name}`}
      className={cn("-ml-4 -mr-3 space-y-3 rounded-md py-1 pl-3 pr-3", focusClass(focused, field.change) || "border-l-4 border-transparent")}
    >
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-2">
          <Icon aria-hidden className="size-4 shrink-0" />
          <h2 id={`specs-${field.name}`} className="text-[15px] font-semibold">
            {title}
          </h2>
        </div>
        {field.notes === null ? <span className="text-[13px] tabular-nums text-muted-foreground">{entries.length}</span> : null}
        {field.change ? <ChangeBadge change={field.change} /> : null}
        {field.change === "changed" && field.beforeNotes !== null ? (
          <Button variant="ghost" size="sm" aria-pressed={before} onClick={() => setBefore((v) => !v)}>
            Show before
          </Button>
        ) : null}
        <span aria-hidden className="h-px min-w-4 flex-1 bg-line" />
        {field.file ? <OpenInEditor file={field.file} line={field.line} /> : null}
      </div>
      {field.notes !== null ? (
        <div className={cn("rounded-xl bg-card p-4 ring-1 ring-line", field.change === "removed" && "text-muted-foreground line-through")}>
          <Markdown holderHeadingLevel={2}>{before && field.beforeNotes !== null ? field.beforeNotes : field.notes}</Markdown>
        </div>
      ) : (
        <ul className="grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-3">
          {entries.map((entry) => (
            <Entry key={entry.name} entry={entry} logos={logos} Icon={Icon} />
          ))}
        </ul>
      )}
    </section>
  );
}

function SpecsPage({ onlyChosen }: { onlyChosen: boolean }) {
  const { snapshot, focus } = useViews();
  const page = useRef<HTMLDivElement>(null);
  const logos = useLogos();
  const active = focus.specs;
  const focusedKeys = new Set(active?.plan.steps.map((s) => s.key));
  const current = active?.plan.steps[active.index]?.key;
  const nonce = active?.nonce;

  useEffect(() => {
    if (current === undefined || !page.current) return;
    const target = [...page.current.querySelectorAll<HTMLElement>("[data-step-key]")].find((el) => el.dataset.stepKey === current);
    target?.scrollIntoView({ block: "start" });
  }, [current, nonce]);

  const fields = snapshot!.data.specs?.fields ?? [];
  // The agent's choices only: a field with none left (the notes field included) is hidden.
  const shown = fields
    .map((field) => ({ field, entries: onlyChosen ? field.entries.filter(chosenByAgent) : field.entries }))
    .filter(({ entries }) => !onlyChosen || entries.length > 0);
  return (
    <div ref={page} className="h-full overflow-y-auto">
      <div className="max-w-[1180px] space-y-8 px-7 py-6">
        {fields.length === 0 ? <p className="text-sm text-muted-foreground">The project has no Specs.</p> : null}
        {shown.map(({ field, entries }) => (
          <Field key={field.name} field={field} entries={entries} focused={focusedKeys.has(field.name)} logos={logos} />
        ))}
      </div>
    </div>
  );
}

/** A pill toggle of the header: `All 16`, `Agent's choice 7`. */
function FilterPill({
  label,
  icon,
  pressed,
  onPress,
  count,
}: {
  label: string;
  icon?: ReactNode;
  pressed: boolean;
  onPress: () => void;
  count: number;
}) {
  return (
    <Button variant={pressed ? "default" : "outline"} className="rounded-full px-3.5" aria-label={`${label} ${count}`} aria-pressed={pressed} onClick={onPress}>
      {icon}
      {label}
      <span className="font-normal tabular-nums opacity-70">{count}</span>
    </Button>
  );
}

/**
 * The Specs: the fields in their fixed order, empty ones hidden, entries in a grid of columns of at
 * least 250 px. The pills of the header show every entry or only the ones the agent chose.
 */
export function SpecsView() {
  const { snapshot } = useViews();
  const [onlyChosen, setOnlyChosen] = useState(false);
  const entries = (snapshot?.data.specs?.fields ?? []).flatMap((field) => field.entries);
  return (
    <ViewFrame
      view="specs"
      subtitle={
        <>
          The stack the agents build with · <span className="font-mono">specs.yaml</span>
        </>
      }
      toolbar={
        <>
          <FilterPill label="All" pressed={!onlyChosen} onPress={() => setOnlyChosen(false)} count={entries.length} />
          <FilterPill
            label="Agent's choice"
            icon={<Sparkles aria-hidden />}
            pressed={onlyChosen}
            onPress={() => setOnlyChosen(true)}
            count={entries.filter(chosenByAgent).length}
          />
        </>
      }
    >
      <SpecsPage onlyChosen={onlyChosen} />
    </ViewFrame>
  );
}
