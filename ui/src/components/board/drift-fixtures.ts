import type { AttachedProposal } from "@/bindings/AttachedProposal";
import type { DriftView } from "@/bindings/DriftView";

/** The view of a Drift with nothing to say and every control enabled, for the tests. */
export function driftView(patch: Partial<DriftView> = {}): DriftView {
  return {
    refs: [],
    happens: "",
    should: "",
    options: [],
    whereInCode: "",
    places: [],
    previous: null,
    resolving: false,
    prepared: false,
    merge: false,
    resolveReasons: [],
    dismissReasons: [],
    fixReasons: [],
    attached: null,
    ...patch,
  };
}

/** The Proposal attached to a Drift: Open, not stale, with `Accept Proposal` enabled, for the tests. */
export function attachedProposal(patch: Partial<AttachedProposal> = {}): AttachedProposal {
  return {
    id: "pppppppp",
    shortNumber: "P2",
    title: "Paid orders",
    state: "Open",
    stale: false,
    body: "",
    files: { added: [], changed: [], removed: [] },
    acceptReasons: [],
    refine: null,
    discardReasons: null,
    ...patch,
  };
}
