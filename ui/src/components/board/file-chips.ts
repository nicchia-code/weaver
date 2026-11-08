// The chips a Task card shows for the files its references name: one per Requirements fragment and
// one per Specs field (`requirements/ui#task`). The Board holds no copy of the Requirements: the
// fragments come from the views' snapshot.

import type { ViewData } from "@/bindings/ViewData";
import type { ViewName } from "@/components/views/focus-model";

export interface FileChip {
  /** Tells one chip from another across pictures. */
  key: string;
  /** `stack.md`, `specs.yaml`. */
  label: string;
  /** The view that opens: Requirements for a fragment, Specs for a field. */
  view: ViewName;
  /** The first reference the chip stands for; its single-ref focus is what the chip opens. */
  ref: string;
}

const SPECS_FILE = "specs.yaml";

/**
 * The chips of `refs`, in the order the references name them, without repeats. A reference names the
 * fragment that holds the Requirements section it concerns and the Specs fields it concerns; one
 * that concerns an Area as a whole, or nothing the views show, names no file. Two fragments of
 * different Areas with one file name are told by their Area (`ui/board.md`); several fields of the
 * Specs are told by the field (`specs.yaml · languages`).
 */
export function fileChips(refs: readonly string[], data: ViewData | null | undefined): FileChip[] {
  if (!data) return [];
  const fragments = new Map<string, { area: string; name: string; ref: string }>();
  const fields = new Map<string, { field: string; ref: string }>();
  for (const ref of refs) {
    const target = data.targets[ref];
    if (!target) continue;
    for (const anchor of target.requirements) {
      if (anchor.section === null) continue;
      const area = data.requirements.find((candidate) => candidate.id === anchor.area);
      const fragment = area?.fragments.find((candidate) => candidate.sections.some((section) => section.title === anchor.section));
      if (!area || !fragment) continue;
      const key = `${area.id}/${fragment.name}`;
      if (!fragments.has(key)) fragments.set(key, { area: area.id, name: fragment.name, ref });
    }
    for (const field of target.specs) {
      if (!fields.has(field)) fields.set(field, { field, ref });
    }
  }
  const shared = (name: string) => [...fragments.values()].filter((fragment) => fragment.name === name).length > 1;
  const chips: FileChip[] = [...fragments.entries()].map(([key, fragment]) => ({
    key: `requirements:${key}`,
    label: shared(fragment.name) ? `${fragment.area}/${fragment.name}.md` : `${fragment.name}.md`,
    view: "requirements",
    ref: fragment.ref,
  }));
  for (const { field, ref } of fields.values()) {
    chips.push({ key: `specs:${field}`, label: fields.size > 1 ? `${SPECS_FILE} · ${field}` : SPECS_FILE, view: "specs", ref });
  }
  return chips;
}
