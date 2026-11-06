---
description: "Use when adding or changing a `weaver` command, its input, output, help or its link with the app."
---
# weaver commands

## General

- Shape: `weaver <object> <verb> [<id>] [<state>] [--input <json>|-] [--root <dir>] [--json]`; `-h`/`--help` prints help. No other option exists.

```
model         show, list, check
project       new, edit, show
proposal      new, show, list, edit, move
task          new, show, list, move, delete
drift         new, show, list, edit, move
history       new, edit, show, list
announcement  new
job           new, show, list, edit
question      new
```

- One command registry in the CLI crate produces parsing, dispatch, help, the JSON Schemas and the command reference in the installed skills; a test fails when that reference differs from the registry. No aliases for removed names; an existing command never answers "unknown command".
- `--root <dir>` uses `<dir>/.weaver`, default the current folder, never a parent. Without `.weaver/`, every command but `project new` and help fails with exit 1 saying to initialise the project, also when `<dir>/.trama` exists; a `.trama` folder is never read or written.
- The command is named `weaver` in help, usage, examples and messages, whatever the name of the file it runs from (`weaver-cli` in the build and in the app bundle).
- Help: `weaver --help` lists objects and verbs; `<object> <verb> --help` prints usage, fields with types and limits, an example and the output fields; `--help --json` prints `{command, input, output}` JSON Schemas (2020-12). Help reads no stdin or file and contacts no app.
- Input: one camelCase JSON object, inline or from stdin with `--input -` (UTF-8, leading BOM accepted), validated whole first: unknown field, wrong type, value outside its list or range, missing required field → exit 2 listing every problem with its JSON path (`questions[0].options[1].label`). A command without fields accepts only `{}` or no input.
- Cards are named by identity, unique identity prefix or short number (`P3`, `t3`: letter plus digits is always a short number; its letter must match the type). Short numbers become identities under the project lock before anything is read for writing; one no live card has is refused (exit 1).
- Output: Markdown on stdout; with `--json` one JSON object where every card has `id` and `number` (`null` without a live number). Errors on stderr, and with `--json` also `{error: {code, message}}` on stdout.
- Exit codes: `0` done; `1` refused or failed; `2` wrong usage or invalid input. A refused command writes nothing.
- Reads run in the CLI process. A writing command goes to the running app (found through `WEAVER_ENDPOINT` or the app's runtime record, `requirements/tech#General`), which runs it in the project's queue with the app's checks; when no app answers, the CLI runs it itself under the project lock. Commands that need the app then fail with exit 1 saying Weaver is not running.
- `model show <ref>`, `model list {text?}` and `model check` are read-only; `check` exits 1 on errors, not on warnings (a fragment over 12,288 bytes).

## project

- `project new {force?, solo?}`: `solo` is `off` (default), `folders` or `files`. Refused, creating nothing, when `.weaver/` exists, when an agent file to install differs (all listed) unless `force: true`, or when `solo` is not `off` and Solo mode cannot be turned on (exit 1 with the reason). Prints `solo` too.
- `project edit {solo?}`: with `solo`, changes only Solo mode and prints `solo`; without, Update skills: prints `changed`, `areas` and `generated`, the structured result the app shows too.
- `project show {}`: the Board: `toDecide` (open Proposals and Drifts), `inWriting` (Drafts with their job), `todo`, `doing` (holder and blockers); and `solo` (`requirements/tech#solo-mode`).

## proposal

- `new {title, body?, kind?, label?, requires?, request?, reworks?, announcement?}`: `kind` `change` (default), `setup`, `import`, `resolution`; `label` 1–30 characters, one line. Refused: `setup` on a Model with a Feature or Actor; `request` with `reworks`. `request` binds only if no Proposal is bound to it yet, `reworks` only a Proposal rejected with Rework that nothing replaced; otherwise it is created unbound with a note saying why.
- `show <id>`: text, status, required Proposals, files, `## Tasks`, `## Created on approval`, `## Scope` (each entry's reason and coverage), `## Blocked` (every reason), validation.
- `list {all?}`: Drafts, Open and Refining; all with `all`.
- `edit <id> {label?, edit?, delete?, drifts?, removeDrifts?, images?, removeImages?, rebase?}`: every call, even without input, sends an Open Proposal back to Draft and says so.
  - `edit`: paths (`model/…`, `requirements/<area>/<fragment>.md`, `specs.yaml`, `tasks/<slug>.md`) made editable, copying the current content and recording `base` the first time; prints the path to write for each. A new Task gets its identity here; Task paths are refused in `import` and `resolution`.
  - `delete` is refused for a path the Proposal holds; `drifts`/`removeDrifts` only in a resolution; `images` `[{task, path}]` copies into `.weaver/task-images/<task>/`, `removeImages` `[{task, name}]`.
  - `rebase: true`, any kind, attached to a Drift or not, drops broken `requires` and, in a resolution, the rows of Drifts no longer open, then records `base` again and sets `refining` (`requirements/rules#proposal`); it prints what it dropped.
- `move <id> open` hands over and prints the warnings and the `show` text. `approved|rejected|discarded` are decisions; `rejected` takes `{feedback?, rework?}`. `rework: true`, and approving a stale Proposal, need the running app.

## task

- `new {title, refs?, technical?, body?, announcement?}` files a Task in To do.
- `list {column?, tasks?, runnable?, max?, proposals?, notProposals?, heldBack?}`: `runnable` gives the next runnable Task, or up to `max` (≥ 1) for free places (`requirements/rules#task`), plus `waiting` (To do Tasks with blockers) and, with `heldBack`, those left out. `proposals` excludes `notProposals`.
- `move <id> doing|todo|done {unchanged?}` is job-bound: `doing` takes (refused naming the blocker), `todo` releases, `done` finishes; `unchanged` (non-blank note) with `done` closes a Drift's fix Task. Only the holding job moves a Task out of `doing`.
- `delete <id>` deletes a Task in To do.

## drift

- `new {title, refs, happens, should, where, options?, task?, proposal?, previous?, merge?, announcement?}`: `options` 2–5 `{title, text}`, recommended first, absent only with `proposal` (a Draft or Open Proposal no other Drift holds); `task` is the origin; `previous` is set automatically when the origin is a fix Task; `merge` only `true`, from a scan after a merge (`requirements/rules#drift`). `show` and `list` print `merge`.
- `list {all?, refs?, job?, currentJob?, unprepared?}`: open only unless `all`; `refs` as Task blocking; `currentJob` uses `WEAVER_JOB` (nothing outside a job); `unprepared`: open, no valid prepared question, in no resolution being written or open.
- `edit <id> {prepared?, proposal?}`: `prepared: {question, decisions}` is a `question new` question (`id` the Drift identity, `multi: false`) with one decision per option: every fix option once as `fix-code` + `option`, plus one `proposal` or `dismissed` (both when there are no fix options). Weaver adds fingerprint and instant. `proposal` attaches a Proposal to a Drift that has none.
- `move <id> code-to-be-fixed {option?, note?}` (option from 1, required when options exist), `proposal-accepted`, `dismissed`: decisions.

## history-entry

- `history new {title, summary, body?, tasks}`: one-line title, one-line summary of 1–280 characters, at least one Task, each Done and not closed unchanged; every problem listed, nothing written.
- `history edit <id> {title?, summary?, body?, tasks?}` replaces fields and adds Tasks, never removes one.
- Both print `id`, `title`, `file`, `recorded` (Tasks now recorded). `history list {}` is newest first; `history show <id>` includes the cited Tasks.

## question-round

`weaver question new --input -` prints only JSON on stdout, always. Input is validated whole before the app is contacted; then it blocks with no time limit until a result exists.

```
input:       {questions: [{id, question, context?, options: [{label, description, details?}] (≥ 2),
              recommended: integer ≥ 0, multi: boolean}] (≥ 1)}             other fields: strings
answered:    {status: "answered", answers: {<id>: {status: "answered", selected: string[], text?}
              | {status: "skipped"}}}                                       exit 0
unavailable: {status: "unavailable", reason}                                exit 0
failed:      {error: {code: "invalid-input" | "interrupted" | "cancelled", message}}
```

- Texts are Markdown except labels; ids unique and non-empty; `context`/`details` non-blank when present; `recommended` within the options.
- One answer per id; `selected` holds original labels; blank text is absent; a skip carries nothing else.
- `invalid-input` (exit 2) names question and field. `interrupted` (exit 1): the job ended, the window closed, or the connection was lost after submission (reported at once). `cancelled` (exit 1): SIGINT/SIGTERM (Ctrl+C on Windows) or stdout closed; the round is withdrawn.
- `unavailable`: not in a working Weaver job, or no app answers before submission. Reason: "Weaver cannot show questions for this command. Do not ask in any other way: stop and tell the developer."
- `--help` says: texts are self-contained, since the developer sees only the round, with no first person and no "Other" option (a free answer is always offered); run it with no time limit.

## job

- `announcement new {kind, title}`: `kind` `proposal`, `task` or `drift`; `title` one line, ≤ 60 characters. Prints the `id` to pass as `announcement` when creating the card; an unknown or foreign one still creates the card, with a `note`.
- `job edit {note}`: line breaks become spaces, ends trimmed; empty → exit 2; over 120 characters cut to 119 plus `…`, said in the output. The last 6 notes are kept with their instants; a repeat of the last only renews its instant.
- `job show` without id reads only the environment and the job's registry record (`id`, `kind`, `target`, `merged` for a scan after a merge, `run` from `WEAVER_RUN`, `task` from `WEAVER_TASK`); outside a job it says so, exit 0. `job show <id>` and `job list {all?}` ask the app; `job show <id>` adds `measure`, the job's line of `requirements/data#job` once written (text: durations, then one line per part). No command starts, aborts or shows a merge.
- `job new {kind, request?, task?, drift?}`: `design` with `request` (written, unsent, not empty), `scan` (whole System), `run` (one `task`, or Run all), `resolve` with `drift`. Accepted only from General's agent (`WEAVER_GENERAL`, recognised by the app as this project's working General) or from an Ask more agent (`WEAVER_ASK`, recognised by the app as a working Ask more conversation of this project), with the developer's refusals; its `--help` says: only when the developer explicitly asked for that job.

## agent

- `WEAVER_JOB` is the job identity Weaver sets on each job's omp; the app accepts it only for a job of that project that is Running or Waiting for the developer.
- `announcement new`, `job edit` and `task move` are refused (exit 1, nothing written) for any other caller, saying only the agent of a Weaver job can do it; `question new` answers `unavailable`; `job new` needs General's or an Ask more conversation's identity.
- `proposal new`, `task new` and `drift new` work without `WEAVER_JOB` and record it as `job` when set.
- Decision commands (`proposal move approved|rejected|discarded`, `drift move`, `task delete`) accept any caller and count as the developer's Events with the same conditions. Skills and `--help` allow the agent to run them only when the developer explicitly asked for that decision in the conversation. Choosing a Proposal's model and answering a prepared question exist only in the app.
