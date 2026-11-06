---
description: "Use when reading or writing the files of Proposals, Tasks, Drifts, History entries and design requests: front matter keys and body sections."
---
# Card files

## proposal

A Proposal is the folder `.weaver/proposals/<id>-<slug>/` holding `proposal.md` and the full new version of every file it changes at the path it has under `.weaver/`: `model/…`, `requirements/<area>/…`, `specs.yaml`, plus `tasks/<id>-<slug>.md` for its Tasks. The folder of a decided Proposal stays as it was when decided.

`proposal.md` front matter:

| Key | Content |
|---|---|
| `status` | `draft`, `open`, `refining`, `approved`, `rejected` or `discarded` |
| `kind` | `change`, `setup`, `import` or `resolution` |
| `requires` | list of Proposal identities; may be empty |
| `base` | map from each changed or deleted path to the SHA-256 (lowercase hex) of its LF-normalised content when first made editable, or `new` when it did not exist |
| `delete` | list of paths the Proposal removes |
| `created` | creation instant |
| `request` | optional: id of the design request it is bound to |
| `reworks` | optional: identity of the rejected Proposal it takes the place of |
| `reworkRequested` | optional: `true` on a Proposal rejected with Rework |
| `label` | optional: the short label, one line |
| `refining` | optional: `true` from the moment a refinement starts on it until it is handed over again; a `draft` carrying it was left half-way by a refinement |
| `job` | optional: identity of the Weaver job that created it |
| `drifts` | resolutions only: list of `{drift, decision, option?, note?}`, `decision` being `fix-code`, `proposal` or `dismissed`, `option` counted from 1 and only with `fix-code` |

- Keys a Proposal of an earlier version may carry and no longer has a meaning, `chat` and `chatWriting`, are ignored when read and dropped the next time Weaver writes that `proposal.md`; a `draft` carrying them is an ordinary Draft with no job, listed by `weaver proposal list` and discarded with `weaver proposal move <id> discarded`.
- The model the developer chose for a Proposal is never written in the repository, so that a cloned project cannot choose models for the developer: the app keeps it in its local data, `proposal-models/<project hash>.json`, a map from Proposal identity to model value, and drops an entry once no Task of that Proposal is in To do or In progress. It reaches omp only as one argument of a process started without a shell, and choosing it never sends the Proposal back to Draft.
- Body: one H1 with the title, then prose saying why. `## Feedback` is appended on rejection only when the feedback has non-blank text. `## Already present` holds one line per element the code already implements, ``- `<ref>` — <where in the code>``.
- A Task file of a Proposal has front matter with only `technical` (default `false`) and `refs` (element references and Requirement sections), then an H1 title and a body, which may end with `## Code notes` (see `task`). It has no `base` and never makes the Proposal stale.

## task

`.weaver/tasks/<id>-<slug>.md`, front matter:

| Key | Content |
|---|---|
| `column` | `todo`, `doing` or `done` |
| `technical` | `true` or `false` |
| `refs` | element references and Requirement sections |
| `proposal` | optional: identity of the Proposal whose design it belongs to |
| `drift` | optional: identity of the Drift whose fix Task it is |
| `job` | optional: identity of the job that filed it |
| `holder` | only in `doing`: identity of the job that holds it |
| `created` | creation instant (for Tasks born with a Proposal, the approval instant) |
| `finished` | only in `done`: the instant it reached Done |
| `unchanged` | only for a fix Task closed without changes: the note of what was checked |

- Body: one H1 title, then what must be true when it is done, in domain words. A fix Task's body also carries both sides of the Drift, the chosen option, the developer's note and Where in the code.
- **Code notes:** a Task written by a design, a scan or a compare may end with `## Code notes`, for whoever implements it; it is never part of what must be true. A fix Task never has it.
  - First line `Commit: <full hash>`, the commit the agent read the code on; absent when the project is not in a git repository.
  - Then short Markdown: where to look (files in backticks, project-relative with `/`, and code names), the conventions to follow, the pitfalls seen and what can be reused. About 1,500 characters at most: a guideline of the skills, never checked.
  - Weaver neither reads nor checks it: it travels with the body, to approval, `weaver task show`, the Task details and the History copy.
- A `holder` naming a job that is unknown to this computer's job registry, or `doing` without `holder`, counts as a job that ended: the Task goes back to To do when the project is opened.
- A Task that becomes Recorded in History or Forgotten has its file deleted, together with `task-images/<id>/`.
- `task-images/<task id>/` holds copies of the images tied to a Task, referenced from its body by relative path.

## drift

`.weaver/drifts/<id>-<slug>.md`, front matter:

| Key | Content |
|---|---|
| `status` | `open`, `code-to-be-fixed`, `fixed`, `proposal-accepted` or `dismissed` |
| `refs` | element references and Requirement sections, at least one |
| `origin` | optional: identity of the Task during which it was found |
| `previous` | optional: identity of the earlier Drift it repeats |
| `proposal` | optional: identity of the attached Proposal |
| `task` | after `code-to-be-fixed`: identity of its fix Task |
| `option` | after a code fix decision: the chosen fix option, from 1 |
| `note` | optional: the developer's note on the decision |
| `job` | optional: identity of the job that filed it |
| `merge` | optional: `true` when it comes from a merge; written only by `drift new`, never changed |
| `created` | creation instant |
| `decided` | the instant of the decision, once decided |

Body, in this order:

1. one H1 title;
2. `## What happens` and `## What should happen`, Markdown in behaviour words;
3. `## Fix options`, one `### <title>` per option followed by its text, the recommended option first; the section is absent when the Drift offers none;
4. `## Where in the code`: files and code names;
5. optional `## Prepared question`: exactly one fenced `json` block `{question, decisions, fingerprint, prepared}`, where `question` is one question of `weaver question new` whose `id` is the Drift identity and `multi` is `false`, `decisions` has one `{decision, option?}` per option in the same order, `fingerprint` is `sha256:<hex>` of the text fingerprint of the Drift's references at preparation, and `prepared` is the instant. A block that does not parse counts as no prepared question and is reported.

## history-entry

`.weaver/history/<id>-<slug>.md`, front matter:

- `created`: when it was first written; `updated`: when it was last written or updated (the entry's date).
- `summary`: one line of 1 to 280 characters, inline Markdown only.
- `tasks`: identities of the Tasks it cites, no duplicates, never shrinking.
- `cleared`: map from Task identity to the copy kept when that Task left `.weaver/tasks/`: `{title, refs, technical, proposal?, label?, finished, body}`. A copy is written once and never changed; writing it does not change `updated`.

Body: one H1 title, then the text in Markdown.

## design-request

- A request is `.weaver/requests/<id>.md`, Markdown with no front matter. `<id>` is the local time of creation as `YYYY-MM-DDTHH-MM-SS`, with `-2`, `-3`, … appended when that name is taken.
- Images pasted or dropped into it are copied into `.weaver/requests/<id>.images/` and linked by relative path; images already inside the project are linked where they are, not copied. Accepted types: PNG, JPEG, GIF, WebP.
- Title: the first H1 outside code fences; otherwise the first non-empty line that is not only images; otherwise the id.
- Empty: nothing left after removing whitespace. A request made only of images is not empty.
- `.weaver/requests/.state.json` records the requests that were sent (`{sent: {<id>: <job id>}, consumed: [<id>]}`), written by the app only. An id listed as consumed is never shown or sent again, even if a file with that name reappears.
- A request sent by Chat (`weaver request new`) is written the same way, with the id of its creation time: its text as given, line endings normalised to LF and one final newline, with no images folder. It is recorded under `sent` with its job in the same step; when the job cannot start, neither the file nor the record is written.
- Consuming or discarding a request deletes `<id>.md` and `<id>.images/` in one step.
