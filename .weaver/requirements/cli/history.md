---
description: "Use when adding or changing the `weaver history` commands, their input with the files and message of a commit, or their output."
---
# weaver history commands

## history-entry

- `history new {title, summary, body?, tasks, commit}`: one-line title, one-line summary of 1–280 characters, at least one Task, each Done and not closed unchanged; every problem listed, nothing written.
- `history edit <id> {title?, summary?, body?, tasks?, commit?}` replaces fields and adds Tasks, never removes one. `commit` is required when `tasks` adds at least one Task the entry did not cite, and ignored otherwise.
- `commit` is `{files, message, checksFailed?}`, always given whether Atomic commits is on or off:
  - `files`: paths of the code of this entry's feature, relative to the project root with `/`, no duplicates; may be empty.
  - `message`: the commit message in English, a non-blank first line of at most 72 characters, optionally followed by a blank line and a body.
  - `checksFailed`: `true` when the final checks of the Run failed; default `false`.
- While Atomic commits is on, the paths are checked with the other fields (`requirements/rules#atomic-commits`) and each invalid one is listed as a problem, nothing written. While it is off, `commit` is checked only for its shape and otherwise ignored.
- Both print `id`, `title`, `file`, `recorded` (Tasks now recorded) and `commit`: `{status, hash?, reason?}`, `status` being `off` (Atomic commits is off), `none` (nothing to commit, or an edit adding no Task), `committed` (with the full `hash`) or `not-committed` (with the `reason` of `requirements/rules#atomic-commits`). The text output ends with `Committed <short hash>.` or `Not committed: <reason>.` and says nothing for `off` and `none`.
- `history list {}` is newest first; `history show <id>` includes the cited Tasks.
