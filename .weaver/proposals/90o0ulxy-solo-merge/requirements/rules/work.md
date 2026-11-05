---
description: "Use when implementing Task blocking, slots, groups, forgetting, Drift rules, prepared Drift questions or when two cards concern the same thing."
---
# Tasks, Drifts and concerns

## task
- Blocked, computed on every read, never stored: an open Drift names it as origin or concerns the same thing (see `work`), or it is the origin of a Drift in Code to be fixed whose fix Task is not Done. A Done Task is never blocked. A refused take names the Drift (number and title) or the fix Task.
- Runnable: To do and not blocked. The next runnable Task is the oldest one, recomputed at each request.
- Slots: `weaver task list` with `runnable` and `max` N walks runnable Tasks oldest first, picking each that concerns the same thing as no Task already picked and no Task In progress, until N are picked or none is left.
- Groups: To do Tasks linked transitively by concerning the same thing or naming the same backticked file path. With `groups`, each picked Task brings up to 2 more of its group, oldest first, for the same sub-agent: at most 3 Tasks of a group per sub-agent.
- Forgetting: at each `FINISHED`, every Done Task not cited by a History entry counts the Tasks that reached Done by `FINISHED` after it (by `finished`, ties by identity), over Task files and Task copies in History entries; closed-unchanged Tasks never count. At 20 or more it is deleted without trace, unless an open Drift names it as origin.
- Citation: the Task's copy (title, body, references, Proposal, tag, `finished`) is written into every citing entry before the Task file is removed; a failed copy keeps the Task and shows a notice.
- Run all groups runnable Tasks by effective model: their design Proposal's chosen model, else the execution model (trimmed, compared exactly). One Run per group, started together. The sub-agent limit N is shared in proportion to each group's runnable Tasks, at least 1 each, the rest to the largest fractional parts (ties to the group with the oldest Task).

## drift
- Nearest container: a reference that no longer exists is replaced by its nearest existing container (Transition → State → Machine → Feature; Event → Machine or Actor); a missing Requirement section is dropped. Fixing the code is refused before any write when no Model reference is left (or, for a Drift on Requirements only, no section).
- Text fingerprint: SHA-256 (lowercase hex) over the Drift's references in order, each as a line `# <ref>` then the text `weaver model show <ref>` prints and every Requirement section titled with it, LF-normalised; a missing reference adds `(missing)`. One core function, used for dismissals and prepared questions.
- Dismissal records the fingerprint. `weaver drift list` with `refs` marks Dismissed Drifts concerning the same thing whose fingerprint is unchanged; the agent never files those differences again.
- A prepared question is valid while the Drift is Open, it parses, its fingerprint equals the current one, every `fix-code` names an existing fix option and a `proposal` decision has an attached Proposal in Draft or Open. A new preparation replaces it; an invalid one is never changed by itself.
- Prepared question shape: one single-choice question, id = the Drift's identity; one option per fix option in order plus one for the code's current behaviour (`proposal` or `dismissed`); without fix options exactly `proposal` (recommended) and `dismissed`.
- Answer: a fix option is `fix-code` with its free text as note; `proposal` or `dismissed` without text is that decision; Weaver writes the resolution (`Resolve: <Drift title>`) and hands it over with the agent's checks. A refused hand-over discards that Draft and starts a resolve job with the answer.
- Fix options: 2–5, first recommended; none only with an attached Proposal. When the origin is a fix Task, `previous` is set to that fix Task's Drift.
- Merge Drifts (`merge: true`, `requirements/data#drift`): `drift new` accepts `merge` only from the agent of a scan job started after a merge (`requirements/rules#solo-merge`), recognised by `WEAVER_JOB`; from any other caller it is refused (exit 1, nothing written). No command changes it afterwards. Dismissals and `previous` apply to merge Drifts unchanged.
- Prepared question of a merge Drift: it always has the `proposal` decision for the code's current behaviour, with an attached Proposal in Draft or Open, and `recommended` is that option, with or without fix options; a preparation without it, or recommending another option, is refused naming the reason. Its fix options keep their order, the first still listed first.

## work
- A reference's key: a Model element is itself; `requirements/<area>#<element>` is the element; `requirements/<area>#General` only itself; `requirements/<area>` the Area; `specs#<field>` the field; `specs` the Specs.
- Containment: Feature ⊃ its Machines; Machine ⊃ its States, Events and their contents; State ⊃ sub-States and Transitions leaving it; Actor ⊃ its Events; Area ⊃ its General section only; `specs` ⊃ each field.
- Two references concern the same thing when keys are equal or one contains the other; two cards when any reference of one concerns any of the other. A card without references concerns nothing.
- One core function serves blocking, slots, groups and Drift lookups by references.
