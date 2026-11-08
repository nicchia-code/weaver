// What the Requirements page shows once the focus's `Only these` and the search have taken their
// share: the Areas, their fragments and the sections that remain. The navigation counts the same
// sections, so both always agree.

import type { AreaView } from "@/bindings/AreaView";
import type { FragmentView } from "@/bindings/FragmentView";
import type { SectionView } from "@/bindings/SectionView";
import { sectionKey } from "@/components/views/focus-model";
import { containsWords } from "@/lib/highlight";

export interface VisibleFragment {
  fragment: FragmentView;
  /** The key of the fragment in the page: `<area>/<fragment>`. */
  key: string;
  /** Its path relative to the Area's folder: `data/stored.md`. */
  path: string;
  sections: SectionView[];
}

export interface VisibleArea {
  area: AreaView;
  /** The Area itself is a step of the focus. */
  focused: boolean;
  fragments: VisibleFragment[];
}

export interface Filters {
  /** The keys (`sectionKey`) of the steps of the focus. */
  focusedKeys: ReadonlySet<string>;
  /** `Only these`: nothing outside the focus. */
  only: boolean;
  /** The words of the search; none when it is empty. */
  words: readonly string[];
}

/** A section matches a search when each word is in its title or in its text. */
export function sectionMatches(section: SectionView, words: readonly string[]): boolean {
  return words.every((word) => containsWords(section.title, [word]) || containsWords(section.body, [word]));
}

/**
 * The Areas of the page. A section stays when the focus (with `Only these`) and the search both
 * keep it. A fragment with no section left is hidden under `Only these` or a search, and so is an
 * Area with no fragment left (an Area that is itself focused stays under `Only these`).
 */
export function visibleAreas(areas: readonly AreaView[], { focusedKeys, only, words }: Filters): VisibleArea[] {
  const searching = words.length > 0;
  const result: VisibleArea[] = [];
  for (const area of areas) {
    const focused = focusedKeys.has(sectionKey(area.id, null));
    const fragments = area.fragments
      .map((fragment) => ({
        fragment,
        key: `${area.id}/${fragment.name}`,
        path: `${area.id}/${fragment.name}.md`,
        sections: fragment.sections.filter(
          (section) =>
            (!only || focused || focusedKeys.has(sectionKey(area.id, section.title))) && (!searching || sectionMatches(section, words)),
        ),
      }))
      .filter(({ sections }) => sections.length > 0 || !(only || searching));
    if (fragments.length === 0 && ((only && !focused) || searching)) continue;
    result.push({ area, focused, fragments });
  }
  return result;
}

/** The overlaid Proposal adds, removes or changes the fragment, or any of its sections. */
export function isChanged(fragment: FragmentView): boolean {
  return fragment.change !== null || fragment.sections.some((section) => section.change !== null);
}
