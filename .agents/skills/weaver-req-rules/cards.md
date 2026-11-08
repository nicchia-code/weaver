# Cards and work

## General

- Permanent identity of a Proposal, Task, Drift or History entry: 8 characters from `[a-z0-9]`, drawn from the OS random generator and redrawn while it equals an identity of the same type in the project (for Tasks: `.weaver/tasks/` and every Proposal's `tasks/`). Commands take it from the file or folder name, never from the content.
- Short numbers: `P`, `T` or `D` plus the lowest free positive integer of that type, assigned in creation order to live cards that have none, freed when the card closes.
  - Live: a Proposal in Draft, Open or Refining; a Task in To do or In progress; a Drift in Open or Code to be fixed. A Done Task of a design keeps its number until every Task of the design is Done, then all free it together.
  - Input accepts either case (`t3`); letter + digits is always a short number; a wrong letter or a number no live card holds is refused.
  - Stored only in the local git-ignored cache under `.weaver/`, written atomically. A missing or unreadable cache, or one with duplicates or unknown cards, is rebuilt silently (valid lines kept, missing numbers assigned); no command fails because of it.
  - Never written into versioned files, History entries or agent instructions. JSON output carries both `id` and `number`.
- Order: oldest `created` first (UTC ISO 8601, milliseconds), ties by identity in byte order. Tasks created by one approval share its instant.
- Every change of cards takes the project's write lock, then reads, validates everything against the state after the change, then writes. A refusal writes nothing (no file, cache or lock leftover). Conditions are checked again when a decision is applied, not only when its button was enabled.

## proposal

- Scope of a change or setup, computed live against the Model, Requirements and Specs with the required Proposals applied below; never stored:
  - `changed`: each added or changed State, Transition, Event; a new Machine as one entry; a Machine, Feature or Actor whose description changed; each added or changed Requirement section, General included; each top-level Specs field whose value changed, as `specs#<field>`.
  - `leads into a change`: each Transition, in any Machine, entering a changed State or reacting to a changed Event.
  - `removed, its container`: a removed element counts as its nearest existing container (Transition → State → Machine → Feature; Event → Machine or Actor). Removed Requirement sections add nothing.
  - Sections compare by Area and reference, whatever fragment holds them; a moved unchanged section, a fragment appearing or vanishing, the System description, `area.md` and fragment titles and descriptions add nothing.
- Coverage: each entry is covered by a Task reference equal to it or containing it (`task` covers `task/To do`; a section by that section or its Area; `specs#<field>` by itself or `specs`), or by a body line under `## Already present`, ``- `<ref>` — <where in the code>``, with the same rule. `removed, its container` entries are covered by removal Tasks: one per topmost removed Model element, ``Remove code of `<ref>` ``, referencing the nearest surviving container; a removed Feature or Actor gives a Technical Task with no reference.
- Staleness: `base` records per touched file the SHA-256 (lowercase hex) of its LF-normalised content when first registered, or `new`. A file is stale when its current content, with required Proposals applied below in dependency order, hashes differently, when a `new` file now exists, or when a hashed file is gone. Editing again keeps `base`; only the refinement's rebase rewrites it. Task files and `proposal.md` never count.
- Reasons Approve is refused, all of them, in this order:
  1. `The agent is still finishing`: the job in `job` is still working;
  2. `A run is in progress`;
  3. each required Proposal in `requires` order: still being written, not approved yet, was rejected, was discarded, missing, forms a cycle;
  4. stale, naming each file;
  5. invalid with it applied, naming each error;
  6. each uncovered scope entry, each Task reference that would not exist, a missing label;
  7. for a resolution, each Drift whose code fix could no longer be applied.
- Hand-over is refused, naming every problem, for reasons 5–7 and also: an unregistered file in its folder; Task files in an import, a resolution or a Proposal attached to a Drift; a non-technical Task without Model reference or a Technical Task with one; an `## Already present` reference that would not exist; for a resolution, an empty list, a Drift not open or already in another Draft, Open or Refining resolution, `fix-code` without an existing option when the Drift has options, an option or note on another decision. Never for staleness or required Proposals. A refinement's hand-over is refused the same way.
- Auto-approval: a setup or import job records whether the Model had no Feature and no Actor when it reached Running. Only a Proposal handed over by such a job is approved at hand-over, with every check of Approve except reason 1; otherwise it stays Open with its reasons.
- Refinement: offered for every stale Open Proposal, any kind, attached to a Drift or not, whatever reasons 2–7 add. Refused only for reason 1 and, for a resolution, when no listed Drift is still open (`None of its Drifts is still open`); refused before anything is written, so no Proposal is left Refining without a job. The same holds for continuing a Draft with `refining: true`. A resolution with no file has no `base`, is never stale and is never refined.
- Refinement rebase (`rebase: true`), in this order: drop from `requires` every entry that was rejected (except one rejected with Rework whose rework has not been handed over: it counts as still being written), discarded or is missing, and every entry of this Proposal that forms a cycle; the other Proposals of the cycle are not touched. For a resolution, drop the `drifts` rows whose Drift is no longer open. Then record `base` again with the remaining required Proposals applied below. The output lists each dropped requirement and row.
- Hand-over of a refined Proposal (`refining: true`): attached to a Drift, always Open, never Approved nor waiting for a Run; otherwise Approved when nothing blocks it, waiting for the Run when only reason 2 blocks it, Open with its reasons when a required Proposal still blocks it. A refined setup keeps its kind: its approval still starts Run all.
- Label: required when carrying Tasks; 1–30 characters after trimming, one line.
- Bindings, checked by `proposal new` against the caller; a binding not allowed leaves the Proposal unbound with a note saying why:
  - `request`: a setup or design job binds only the request its instruction named, and only its first Proposal; any other caller binds none.
  - `reworks`: a rework job binds only the Proposal its instruction named; any other caller binds none. The bound Proposal takes the place of the rejected one in the order of the Board and in the `requires` of the Proposals that waited for it.
- Tag colour: FNV-1a 32-bit hash of the label's UTF-8 bytes modulo 6 picks `blue`, `green`, `amber`, `violet`, `cyan`, `pink`, in this order (pink, not red, so that no Task border looks like a Drift's).

## question-round

- A round is validated whole before queueing; any failure refuses it all, naming each bad field by path (`questions[1].options[0].label`):
  - at least 1 question; ids non-empty, unique in the round; question text non-blank;
  - at least 2 options, each with non-blank label and description; labels unique within the question;
  - `multi` boolean; `recommended` integer from 0 to options − 1;
  - `context` and `details`, when present, non-blank after trimming.
- One round is presented per project; others wait FIFO per project by submission instant, prepared Drift questions included. Projects never wait for each other.
- Confirming needs an option or non-blank text; single choice holds at most one option. Whitespace-only text is absent.
- The backend refuses a send while any question has neither a confirmed answer nor a skip, whatever the interface sent.
- The result has one entry per question id. A skipped question carries nothing: unconfirmed text or options are never delivered and no answer is made up.
