# Proposal

A change to the Model, the Requirements and the Specs that the agent writes and the developer decides on as a whole. It holds the full new version of every file it changes and the list of files it removes, and it is of one kind: an ordinary change, the setup of an empty project, an import that describes existing code, or a resolution that lists decisions on open Drifts. An ordinary change or a setup carries its Tasks; an import and a resolution carry none. The agent of a job writes it, or the agent of Chat on the developer's explicit order, which writes only ordinary changes and resolutions. A Proposal can require other Proposals, can be bound to the written request it comes from, can rework a rejected Proposal, and can be attached to a Drift, which then decides it.

A Proposal is stale when a file it changes is no longer the one it started from, counting the Proposals it requires as already applied. It is blocked when a Proposal it requires is not approved, was rejected or discarded, is missing or forms a cycle; when it is stale; when the Model and the Requirements with it applied would be invalid; when one of its changes is no longer covered by a Task, or a Task names an element that would not exist; or, for a resolution, when a code fix it records could no longer be applied. Every reason is listed, never only the first.

## States

### Draft (initial)
The agent of a job, or of Chat, is writing it, after opening it. Nobody can approve or reject it. The Board shows only the ghost of the job that writes it; a Draft the agent of Chat is writing has its own card instead, which shows what it holds so far and from which the developer can open Chat or discard it.
- `agent.EDIT_PROPOSAL` → Draft
- `agent.SUBMIT_PROPOSAL` → Open: when every check passes; when it is bound to a written request, the request is consumed.
- `agent.SUBMIT_PROPOSAL` → Approved: a setup or import Proposal written by a job that started on an empty Model, when every check passes and nothing blocks it; the Proposals of the same job that it requires are already approved this way, so a chain of imports is approved whole.
- `developer.DISCARD_PROPOSAL` → Discarded: only when no working job is writing it and, for one the agent of Chat created or is writing, no turn of Chat is in progress; Proposals that require it stay blocked saying it was discarded. A Draft attached to a Drift can be discarded too; the Drift stays open.
- `general-chat.CLEARED` → Discarded: only a Draft the agent of Chat created, not one it only reopened; Proposals that require it stay blocked saying it was discarded.
- `developer.APPROVE_PROPOSAL` → Refining: only a Draft left half-way by a refinement and not attached to a Drift; a new refinement job continues it. Refused while the agent that handed it over is still finishing and, for a resolution, when none of its Drifts is still open.
- `developer.REFINE_DRIFT_PROPOSAL` → Refining: only the Draft attached to that Drift and left half-way by a refinement, with the same conditions; a new refinement job continues it.
- `drift.CODE_TO_BE_FIXED` → Discarded: only the Draft attached to that Drift.
- `drift.DISMISSED` → Discarded: only the Draft attached to that Drift.

### Open
Handed over and waiting for the developer's decision. A Proposal attached to a Drift is never shown on its own and cannot be decided directly: it is decided through its Drift.
- `developer.APPROVE_PROPOSAL` → Approved: only when nothing blocks it, the job that handed it over has ended, no turn of Chat is in progress for one the agent of Chat created, and no Run is working in the project; its files replace the Model's, its Tasks are created in To do with the same identities, a Task is created to remove the code of each element it removes, and for a resolution every listed Drift still open gets its decision; it either happens whole or changes nothing.
- `developer.APPROVE_PROPOSAL` → Refining: any stale Proposal not attached to a Drift, whatever its kind and whatever else blocks it, also while a Run works; refused only while the agent that handed it over is still finishing and, for a resolution, when none of its Drifts is still open. A refinement job adapts it to the current Model and Requirements without a new interview.
- `developer.REFINE_DRIFT_PROPOSAL` → Refining: only the stale Proposal attached to that Drift, with the same conditions.
- `developer.REJECT_PROPOSAL` → Rejected: the reason, when given, stays with it; a rejected resolution leaves its Drifts open; Proposals that require it stay blocked saying it was rejected.
- `developer.REWORK_PROPOSAL` → Rejected: with the feedback; a rework job writes it again, and the first Proposal it opens takes its place in the order of the Board and in the requirements of the Proposals that waited for it. A Proposal is reworked at most once.
- `developer.CHOOSE_PROPOSAL_MODEL` → Open: only when it carries Tasks; the chosen model replaces the analysis and execution models for the jobs born from it, until none of its Tasks is in To do or In progress.
- `agent.EDIT_PROPOSAL` → Draft: it goes back to the agent, even if nothing ends up different; when the agent of Chat edits it, it becomes a Draft that Chat is writing until it is handed over again.
- `drift.PROPOSAL_ACCEPTED` → Approved: only the Proposal attached to that Drift, with the same conditions and effects as an approval.
- `drift.CODE_TO_BE_FIXED` → Rejected: only the Proposal attached to that Drift.
- `drift.DISMISSED` → Discarded: only the Proposal attached to that Drift.

### Refining
A refinement job rewrites it on the current Model and Requirements, keeping its identity, short number, place on the Board, label, chosen model and the Proposals that require it. When the refinement starts again from the current files, Weaver stops it requiring the Proposals that were rejected, discarded or are missing, and those through which it forms a cycle (only its own requirement, the others stay); a required Proposal rejected for a rework not yet handed over stays, as one still being written. For a resolution Weaver also drops the decisions on Drifts no longer open; the decisions on the Drifts still open stay taken. The agent adapts the change without what was dropped, asking the developer when something of a dropped Proposal was needed. It asks a question only when the current Model contradicts a decision already taken; when the whole change is already in the Model it asks the developer whether to remove it or keep it. Everything that would refuse a hand-over the agent fixes before handing it over.
- `agent.EDIT_PROPOSAL` → Refining
- `agent.SUBMIT_PROPOSAL` → Approved: only when it is not attached to a Drift and nothing blocks it; it waits for a working Run to end first.
- `agent.SUBMIT_PROPOSAL` → Open: always when it is attached to a Drift, which the developer then decides again; otherwise when something other than staleness and a Run still blocks it, with the reasons.
- `developer.REJECT_PROPOSAL` → Rejected: when the developer chose to remove a change already in the Model.
- `job.ENDED` → Draft: when the refinement job ended without handing it over; it stays half-way until the developer refines it again, from its Drift when it is attached to one, or discards it.

### Approved (final)
Its changes are in the Model, the Requirements and the Specs.

### Rejected (final)
The developer refused it, reworked it, or decided its Drift another way. On the developer's order, the agent of Chat may still write an ordinary change or a resolution that takes its place, of its same kind, unless another Proposal already took it or a rework job is working on it.

### Discarded (final)
Thrown away before or instead of a decision.

## Events

### SUBMITTED
Emitted when the agent hands the Proposal over, whether it becomes Open or is approved at once.

### APPROVED
Emitted when the Proposal is approved, after its changes are applied.

### DISCARDED
Emitted when the Proposal is discarded, whoever discards it.
