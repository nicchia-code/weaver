import { MessageSquare, Search } from "lucide-react";
import { Fragment, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import type { AreaView } from "@/bindings/AreaView";
import type { SectionView } from "@/bindings/SectionView";
import { AskRequirementsButton } from "@/components/ask/ask-button";
import { useReveal } from "@/components/project-context";
import { useChatDraft } from "@/components/general/chat-draft";
import { Markdown } from "@/components/markdown";
import { OpenInEditor } from "@/components/open-in-editor";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ChangeBadge, focusClass } from "@/components/views/change-badge";
import { sectionKey } from "@/components/views/focus-model";
import { isChanged, visibleAreas, type VisibleArea, type VisibleFragment } from "@/components/views/requirements-model";
import { ViewFrame } from "@/components/views/view-frame";
import { useViews } from "@/components/views/views-state";
import { MATCH_CLASS, searchWords, splitMatches } from "@/lib/highlight";
import { cn } from "@/lib/utils";

const SUBTITLE = "What the app must do, in plain words. Agents build from this.";
/** What `Ask for a change` puts in the message box of Chat. */
const CHANGE_PREFIX = "Change to the Requirements: ";

/** A text with the matches of the search marked. */
function Highlighted({ text, words }: { text: string; words: readonly string[] }) {
  return (
    <>
      {splitMatches(text, words).map((piece, index) => (
        <Fragment key={index}>{piece.match ? <mark className={MATCH_CLASS}>{piece.text}</mark> : piece.text}</Fragment>
      ))}
    </>
  );
}

function SectionRow({ area, section, focused, words }: { area: AreaView; section: SectionView; focused: boolean; words: readonly string[] }) {
  const [before, setBefore] = useState(false);
  const removed = section.change === "removed";
  const key = sectionKey(area.id, section.title);
  return (
    <section
      data-step-key={key}
      aria-label={`${area.title}, ${section.title}${section.change ? `, ${section.change}` : ""}`}
      className={cn("group relative border-b border-b-accent px-4 py-3.5 last:border-b-0", focusClass(focused, section.change) || "border-l-4 border-transparent")}
    >
      <div className="grid gap-x-6 gap-y-2 @[480px]:grid-cols-[140px_minmax(0,1fr)]">
        <div className="flex min-w-0 flex-col items-start gap-1.5">
          <h4 className="font-mono text-[12.5px] font-medium text-ink-2">
            <Highlighted text={section.title} words={words} />
          </h4>
          {section.change ? <ChangeBadge change={section.change} /> : null}
          {section.movedFrom ? <Badge variant="muted">moved from {section.movedFrom}</Badge> : null}
          {section.change === "changed" && section.before !== null ? (
            <Button variant="ghost" size="sm" aria-pressed={before} onClick={() => setBefore((v) => !v)}>
              Show before
            </Button>
          ) : null}
        </div>
        {section.body.trim() !== "" ? (
          <div className={cn("min-w-0", removed && "text-muted-foreground line-through")}>
            <Markdown holderHeadingLevel={4} highlight={words}>
              {before && section.before !== null ? section.before : section.body}
            </Markdown>
          </div>
        ) : null}
      </div>
      {section.file ? (
        <OpenInEditor
          file={section.file}
          line={section.line}
          className="absolute right-2 top-2 bg-card opacity-0 shadow-ring focus-visible:opacity-100 group-focus-within:opacity-100 group-hover:opacity-100"
        />
      ) : null}
    </section>
  );
}

function FragmentBlock({
  area,
  visible,
  focusedKeys,
  words,
}: {
  area: AreaView;
  visible: VisibleFragment;
  focusedKeys: ReadonlySet<string>;
  words: readonly string[];
}) {
  const { fragment, sections } = visible;
  return (
    <div data-fragment-key={visible.key} className="scroll-mt-2 space-y-3">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5">
        <h3 className={cn("text-[22px] font-semibold leading-tight", fragment.change === "removed" && "text-muted-foreground line-through")}>{fragment.title}</h3>
        <span className="rounded-sm bg-paper px-1.5 py-0.5 font-mono text-xs text-muted-foreground">{visible.path}</span>
        {fragment.change === "new" ? <ChangeBadge change="added" label="new fragment" /> : null}
        {fragment.change === "removed" ? <ChangeBadge change="removed" label="fragment removed" /> : null}
        {fragment.file ? <OpenInEditor file={fragment.file} className="ml-auto" /> : null}
      </div>
      {sections.length > 0 ? (
        <div className="@container overflow-hidden rounded-xl bg-card ring-1 ring-line">
          {sections.map((section) => (
            <SectionRow
              key={`${fragment.name}/${section.title}`}
              area={area}
              section={section}
              focused={focusedKeys.has(sectionKey(area.id, section.title))}
              words={words}
            />
          ))}
        </div>
      ) : null}
    </div>
  );
}

function AreaBlock({ visible, focusedKeys, words }: { visible: VisibleArea; focusedKeys: ReadonlySet<string>; words: readonly string[] }) {
  const { area, focused, fragments } = visible;
  return (
    <section data-step-key={sectionKey(area.id, null)} aria-labelledby={`area-${area.id}`} className="space-y-6">
      <header className={cn("space-y-2 rounded-md py-1.5 pl-4 pr-3", focusClass(focused, null) || "border-l-4 border-transparent")}>
        <div className="flex flex-wrap items-center gap-2">
          <h2 id={`area-${area.id}`} className="text-xs font-medium uppercase tracking-wider text-muted-foreground">
            {area.title}
          </h2>
          {area.file ? <OpenInEditor file={area.file} /> : null}
        </div>
        {area.intro.trim() !== "" ? <Markdown holderHeadingLevel={2}>{area.intro}</Markdown> : null}
      </header>
      {fragments.map((fragment) => (
        <FragmentBlock key={fragment.key} area={area} visible={fragment} focusedKeys={focusedKeys} words={words} />
      ))}
    </section>
  );
}

/** The navigation: every Area with its fragments and their number of sections; choosing one scrolls its fragment to the top. */
function Navigation({ areas, top, onChoose }: { areas: readonly VisibleArea[]; top: string | null; onChoose: (key: string) => void }) {
  return (
    <nav aria-label="Requirements sections" className="w-[220px] shrink-0 overflow-y-auto px-3 py-5">
      {areas.map(({ area, fragments }) => (
        <div key={area.id} role="group" aria-labelledby={`nav-area-${area.id}`} className="mb-5 last:mb-0">
          <p id={`nav-area-${area.id}`} className="px-2.5 pb-1.5 text-[11px] font-medium uppercase tracking-wider text-muted-foreground">
            {area.title}
          </p>
          <ul className="space-y-0.5">
            {fragments.map((entry) => (
              <li key={entry.key}>
                <button
                  type="button"
                  aria-current={entry.key === top ? "location" : undefined}
                  onClick={() => onChoose(entry.key)}
                  className={cn(
                    "flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-[13px] focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    entry.key === top ? "bg-paper font-medium text-foreground" : "text-ink-2 hover:bg-accent",
                  )}
                >
                  <span className="min-w-0 flex-1 truncate">{entry.fragment.title}</span>
                  {isChanged(entry.fragment) ? <Badge variant="lavender">changed</Badge> : null}
                  <span className="text-xs tabular-nums text-muted-foreground">{entry.sections.length}</span>
                </button>
              </li>
            ))}
          </ul>
        </div>
      ))}
    </nav>
  );
}

function RequirementsPage({ search }: { search: string }) {
  const { snapshot, focus } = useViews();
  const page = useRef<HTMLDivElement>(null);
  const active = focus.requirements;
  const focusedKeys = new Set(active?.plan.steps.map((s) => s.key));
  const current = active?.plan.steps[active.index]?.key;
  const nonce = active?.nonce;
  const words = useMemo(() => searchWords(search), [search]);
  const [top, setTop] = useState<string | null>(null);

  // The palette chose a section: it goes to the top of the view.
  useReveal("section", (target) => {
    const section = [...(page.current?.querySelectorAll<HTMLElement>("[data-step-key]") ?? [])].find((el) => el.dataset.stepKey === target.key);
    section?.scrollIntoView({ block: "start" });
  });

  // Going to a step scrolls it to the top of the view.
  useEffect(() => {
    if (current === undefined || !page.current) return;
    const target = [...page.current.querySelectorAll<HTMLElement>("[data-step-key]")].find((el) => el.dataset.stepKey === current);
    target?.scrollIntoView({ block: "start" });
  }, [current, nonce]);

  const areas = visibleAreas(snapshot!.data.requirements, { focusedKeys, only: active?.only ?? false, words });
  const keys = areas.flatMap(({ fragments }) => fragments.map((fragment) => fragment.key));
  const keyList = keys.join("\n");

  // The fragment at the top of the view: the first one that reaches below a line just under the top edge.
  const findTop = useCallback(() => {
    const scroller = page.current;
    if (!scroller) return;
    const fragments = [...scroller.querySelectorAll<HTMLElement>("[data-fragment-key]")];
    const line = scroller.getBoundingClientRect().top + 24;
    const at = scroller.scrollTop === 0 ? fragments[0] : (fragments.find((el) => el.getBoundingClientRect().bottom > line) ?? fragments[fragments.length - 1]);
    setTop(at?.dataset.fragmentKey ?? null);
  }, []);
  useLayoutEffect(findTop, [findTop, keyList]);

  const choose = (key: string) => {
    const target = [...(page.current?.querySelectorAll<HTMLElement>("[data-fragment-key]") ?? [])].find((el) => el.dataset.fragmentKey === key);
    target?.scrollIntoView({ block: "start" });
    setTop(key);
  };

  return (
    <div className="flex h-full min-h-0">
      <Navigation areas={areas} top={top} onChoose={choose} />
      <div ref={page} onScroll={findTop} className="min-h-0 min-w-0 flex-1 overflow-y-auto">
        <div className="max-w-[900px] space-y-12 px-7 py-6">
          {snapshot!.data.requirements.length === 0 ? <p className="text-sm text-muted-foreground">The project has no Requirements.</p> : null}
          {snapshot!.data.requirements.length > 0 && words.length > 0 && areas.length === 0 ? (
            <p className="text-sm text-muted-foreground">No sections match.</p>
          ) : null}
          {areas.map((visible) => (
            <AreaBlock key={visible.area.id} visible={visible} focusedKeys={focusedKeys} words={words} />
          ))}
        </div>
      </div>
    </div>
  );
}

/** The search of the header: Esc empties it (and only then lets Esc reach the focus). */
function SearchField({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  return (
    <div className="relative w-[260px]">
      <Search aria-hidden className="pointer-events-none absolute left-3 top-1/2 size-4 -translate-y-1/2 text-muted-foreground" />
      <Input
        aria-label="Search requirements"
        placeholder="Search requirements"
        autoComplete="off"
        spellCheck={false}
        value={value}
        onChange={(event) => onChange(event.target.value)}
        onKeyDown={(event) => {
          if (event.key !== "Escape" || value === "") return;
          onChange("");
          event.preventDefault();
        }}
        className="pl-9"
      />
    </div>
  );
}

/**
 * The Requirements: a navigation of the fragments beside the Areas in folder order, fragments in
 * file order, sections with `General` first, each section with its title and `Open in editor`, and
 * the labels of an overlay. The search filters the sections in place. The only `Ask more` is the
 * one in the header, about the Requirements as a whole.
 */
export function RequirementsView() {
  const [search, setSearch] = useState("");
  const askChange = useChatDraft();
  return (
    <ViewFrame
      view="requirements"
      subtitle={SUBTITLE}
      toolbar={
        <>
          <SearchField value={search} onChange={setSearch} />
          <AskRequirementsButton />
          <Button onClick={() => askChange(CHANGE_PREFIX)}>
            <MessageSquare aria-hidden />
            Ask for a change
          </Button>
        </>
      }
    >
      <RequirementsPage search={search} />
    </ViewFrame>
  );
}
