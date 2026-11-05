---
name: weaver-req-rules
description: "Use when implementing calculations, limits, policies or other business rules behind the Model's conditions and consequences."
---

Every rule here is computed in the Rust core, the same for the app and the `weaver` command line, never in the interface.

## General

- Permanent identity of a Proposal, Task, Drift or History entry: 8 characters from `[a-z0-9]`, drawn from the OS random generator and redrawn while it equals an identity of the same type in the project (for Tasks: `.weaver/tasks/` and every Proposal's `tasks/`). Commands take it from the file or folder name, never from the content.
- Short numbers: `P`, `T` or `D` plus the lowest free positive integer of that type, assigned in creation order to live cards that have none, freed when the card closes.
  - Live: a Proposal in Draft, Open or Refining; a Task in To do or In progress; a Drift in Open or Code to be fixed. A Done Task of a design keeps its number until every Task of the design is Done, then all free it together.
  - Input accepts either case (`t3`); letter + digits is always a short number; a wrong letter or a number no live card holds is refused.
  - Stored only in the local git-ignored cache under `.weaver/`, written atomically. A missing or unreadable cache, or one with duplicates or unknown cards, is rebuilt silently (valid lines kept, missing numbers assigned); no command fails because of it.
  - Never written into versioned files, History entries or agent instructions. JSON output carries both `id` and `number`.
- Order: oldest `created` first (UTC ISO 8601, milliseconds), ties by identity in byte order. Tasks created by one approval share its instant.
- Every change of cards takes the project's write lock, then reads, validates everything against the state after the change, then writes. A refusal writes nothing (no file, cache or lock leftover). Conditions are checked again when a decision is applied, not only when its button was enabled.

## Cards and work

Use when implementing card identities, short numbers, ordering, Proposal scope, staleness, approval, refinement or question round checks.

Sections: `General`, `proposal`, `question-round`

File: `cards.md`

## Files

Use when implementing the limits of the File view, Assets and Notes: names, refused actions, binary files, images, size, tabs, line endings, the lock, the notes sheet.

Sections: `file-view`, `file-tab`, `file-lock`, `asset-view`, `notes-view`

File: `files.md`

## Agent instructions

Use when changing the instructions Weaver gives omp for jobs and Ask more conversations.

Sections: `job`, `ask-conversation`, `assets`

File: `instructions.md`

## Agent skills

Use when changing the skills and files Weaver installs for omp or the Requirement skills.

Sections: `project`

File: `skills.md`

## Solo mode

Use when implementing Solo mode's block in git's local exclusions, its modes and paths, the list of Weaver's files git still tracks, or the git steps of Merge.

Sections: `solo-mode`, `solo-merge`

File: `solo-mode.md`

## System Certificates

Use when implementing which certificates go in the System Certificates bundle, when it is refreshed, or how certificate and HTTP errors are recognised.

Sections: `system-certificates`

File: `system-certificates.md`

## Tasks, Drifts and concerns

Use when implementing Task blocking, slots, groups, forgetting, Drift rules, prepared Drift questions or when two cards concern the same thing.

Sections: `task`, `drift`, `work`

File: `work.md`
