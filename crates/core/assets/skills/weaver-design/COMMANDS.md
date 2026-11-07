# Commands of `weaver`

Generated from the command registry of the `weaver` command line: do not edit it by hand.

Shape: `weaver <object> <verb> [<id>] [<state>] [--input <json>|-] [--root <dir>] [--json]`. `-h` and `--help` print help (with `--json`, the JSON Schemas of the command); no other option exists. Input is one camelCase JSON object, inline or from standard input with `--input -`; a field marked optional may be left out, and a command without fields takes only `{}` or nothing. Cards are named by identity, unique identity prefix or short number. Output is Markdown, or one JSON object with `--json`. Exit codes: `0` done, `1` refused or failed, `2` wrong usage or invalid input.

| Object | Verbs |
|---|---|
| `model` | `show`, `list`, `check` |
| `project` | `new`, `edit`, `show` |
| `proposal` | `new`, `show`, `list`, `edit`, `move` |
| `task` | `new`, `show`, `list`, `move`, `delete` |
| `drift` | `new`, `show`, `list`, `edit`, `move` |
| `history` | `new`, `edit`, `show`, `list` |
| `announcement` | `new` |
| `job` | `new`, `show`, `list`, `edit` |
| `question` | `new` |
| `request` | `new` |

## model

### `weaver model show [<ref>] [--input <json>|-] [--root <dir>] [--json]`

Prints the System, or a Model element or Requirement section with the Requirements that apply to it.

Without `<ref>` it prints the System with its Actors, Features and Requirement Areas. A Feature lists its Machines; a Machine prints its State tree, its Events with their receivers and its Transitions; a State prints its routes. Each element view is followed by the Requirement sections that apply to it (those titled with it or with what contains it).

`requirements/<area>` prints an Area with its fragments; `requirements/<area>#General` and `requirements/<area>#<element>` print one section.

- Argument `<ref>`: a Model element (`order`, `order/Paid`, `user.CANCEL`, `order/Paid/user.CANCEL`), `system`, a Requirement (`requirements/<area>`, `requirements/<area>#General`, `requirements/<area>#<element>`) or the Specs (`specs`, `specs#<field>`). Without it, the System is shown.

Output with `--json`: `kind`, `reference`, `requirements`. `--help` gives their types.

Example: `weaver model show order/Paid`

### `weaver model list [--input <json>|-] [--root <dir>] [--json]`

Lists the Model and the Requirements, or searches them for a text.

Without `text` it lists the Actors, the Features with their Machines and the Requirement Areas. With `text` it prints every element, State, Event, Requirement section and Specs entry whose text contains it, compared without regard to case; the Requirements are searched too.

Input:

- `text`, string (not blank), optional: The text to search for.

Output with `--json`: `text`, `hits`, `actors`, `features`, `areas`. `--help` gives their types.

Example: `weaver model list --input '{"text":"payment"}'`

### `weaver model check [--input <json>|-] [--root <dir>] [--json]`

Validates the Model, the Requirements and the Specs.

Exits 1 when there is an error; a warning (a fragment over 12,288 bytes) does not change the exit code.

Output with `--json`: `ok`, `errors`, `warnings`, `diagnostics`. `--help` gives their types.

Example: `weaver model check`

## project

### `weaver project new [--input <json>|-] [--root <dir>] [--json]`

Initialises `.weaver/` in the folder and installs the agent files.

Creates `.weaver/` with the System and the starting Requirement Areas, and writes the agent files (skills, sub-agent definitions, advisor instructions) in the project. It is refused, creating nothing, when `.weaver/` exists, or when an agent file to install exists with other content: every such file is listed, and `force` overwrites them.

`solo` turns Solo mode on while initialising (`folders`: the whole folders of Weaver, the agent's skills and omp; `files`: only the files Weaver installs in them): Weaver's files then stay out of git in this working copy. It needs git and a repository: otherwise the command is refused with the reason and nothing is created. The default is `off`, and git is not called then.

Input:

- `force`, boolean, optional: Overwrite the agent files that differ from the ones to install.
- `solo`, `off` | `folders` | `files`, optional: Solo mode of this working copy: `off` (default), `folders` or `files`.

Output with `--json`: `root`, `files`, `areas`, `agentFiles`, `solo`. `--help` gives their types.

Example: `weaver project new --input '{"solo":"folders"}'`

### `weaver project edit [--input <json>|-] [--root <dir>] [--json]`

Updates the installed skills, or changes Solo mode.

Without input, Update skills: writes again every installed file that differs from the one this version of Weaver carries, creates the starting Requirement Areas when the Requirements are missing, and generates the Requirement skills again. It always reports what it changed, or the error.

With `solo`, only Solo mode changes: `off`, `folders` or `files`. Weaver writes or removes its block in git's local exclusions at once and prints `solo`. A change that fails (no git, not in a repository, exclusions not writable) changes nothing and says why. Files git already tracks are not hidden: they are listed in `solo.tracked`. While a merge of Weaver is in progress (the Board shows it), Solo mode cannot change in any way: the command is refused with `Finish or abort the merge in progress first.` and nothing changes.

Input:

- `solo`, `off` | `folders` | `files`, optional: Change only Solo mode of this working copy: `off`, `folders` or `files`. Without it, Update skills runs.

Output with `--json`: `changed`, `areas`, `generated`, `solo`. `--help` gives their types.

Example: `weaver project edit --input '{"solo":"files"}'`

### `weaver project show [--input <json>|-] [--root <dir>] [--json]`

Prints the Board and Solo mode.

Output with `--json`: `toDecide`, `inWriting`, `todo`, `doing`, `solo`. `--help` gives their types.

Example: `weaver project show`

## proposal

### `weaver proposal new [--input <json>|-] [--root <dir>] [--json]`

Files a Proposal as a Draft.

`kind` is `change` (default), `setup`, `import` or `resolution`. `label` is one line of 1 to 30 characters. A `setup` is refused on a Model that has a Feature or an Actor, and `request` is refused together with `reworks`.

`request` and `reworks` bind only as Weaver allows for the caller: a setup or design job binds only the request its instruction names, and only with its first Proposal; a rework job only the Proposal its instruction names; any other caller binds nothing. A binding that is not allowed leaves the Proposal unbound, with a note saying why. `announcement` is the identity printed by `weaver announcement new`.

Input:

- `title`, string (not blank, one line), required: The title.
- `body`, string, optional: Why the change is needed, in Markdown.
- `kind`, `change` | `setup` | `import` | `resolution`, optional: `change` (default), `setup`, `import` or `resolution`.
- `label`, string (not blank, one line, at most 30 characters), optional: A short label shown on the card.
- `requires`, list of string (not blank, one line), optional: Proposals this one builds on, by identity or short number.
- `request`, string (not blank, one line), optional: The design request it answers.
- `reworks`, string (not blank, one line), optional: The rejected Proposal it replaces.
- `announcement`, string (not blank, one line), optional: The announcement of the card.

Output with `--json`: `id`, `number`, `title`, `kind`, `status`, `note`. `--help` gives their types.

Example: `weaver proposal new --input '{"title":"Cancel an order","announcement":"<id>"}'`

### `weaver proposal show <id> [--input <json>|-] [--root <dir>] [--json]`

Prints a Proposal: text, status, files, Tasks, scope, blocking and validation.

Prints the text, the status, the required Proposals, the files, `## Tasks`, `## Created on approval`, `## Scope` (each entry's reason and coverage), `## Blocked` (every reason) and the validation of the project after the Proposal.

- Argument `<id>`: the card, by identity, unique identity prefix or short number (`P3`, `t3`: a letter and digits is always a short number, and its letter must match the type).

Output with `--json`: `id`, `number`, `title`, `kind`, `status`, `requires`, `files`, `tasks`, `blocked`, `validation`. `--help` gives their types.

Example: `weaver proposal show p3`

### `weaver proposal list [--input <json>|-] [--root <dir>] [--json]`

Lists the Proposals.

Lists the Drafts, the Open and the Refining Proposals; with `all`, every Proposal.

Input:

- `all`, boolean, optional: List every Proposal, decided ones included.

Output with `--json`: `proposals`. `--help` gives their types.

Example: `weaver proposal list`

### `weaver proposal edit <id> [--input <json>|-] [--root <dir>] [--json]`

Edits a Proposal: makes files editable, records Drift decisions and images.

Every call, even without input, sends an Open Proposal back to Draft and says so.

`edit` makes paths editable (`model/...`, `requirements/<area>/<fragment>.md`, `specs.yaml`, `tasks/<slug>.md`), copying the current content and recording `base` the first time, and prints the path to write for each. A new Task gets its identity here; Task paths are refused in `import` and `resolution`. `delete` is refused for a path the Proposal holds. `drifts` and `removeDrifts` apply only to a resolution; `option` and `note` go only with `fix-code`. `images` copies files into `.weaver/task-images/<task>/`. `rebase: true`, for any kind, attached to a Drift or not, drops broken `requires` and, in a resolution, the rows of Drifts no longer open, then records `base` again and sets `refining`; it prints what it dropped.

- Argument `<id>`: the card, by identity, unique identity prefix or short number (`P3`, `t3`: a letter and digits is always a short number, and its letter must match the type).

Input:

- `label`, string (not blank, one line, at most 30 characters), optional: A short label shown on the card.
- `edit`, list of string (not blank, one line), optional: Paths to make editable.
- `delete`, list of string (not blank, one line), optional: Paths to delete when the Proposal is approved.
- `drifts`, list of object, optional: Decisions on Drifts; only in a resolution.
  - `drift`, string (not blank, one line), required: The Drift.
  - `decision`, `proposal` | `dismissed` | `fix-code`, required: The decision.
  - `option`, integer >= 1, optional: The fix option, from 1; only with `fix-code`.
  - `note`, string (not blank), optional: A note; only with `fix-code`.
- `removeDrifts`, list of string (not blank, one line), optional: Drifts to take out of the resolution.
- `images`, list of object, optional: Images to copy for Tasks.
  - `task`, string (not blank, one line), required: The Task.
  - `path`, string (not blank, one line), required: The image file.
- `removeImages`, list of object, optional: Images to remove.
  - `task`, string (not blank, one line), required: The Task.
  - `name`, string (not blank, one line), required: The image name.
- `rebase`, boolean, optional: Record `base` again and set `refining`; drops broken `requires` and, in a resolution, the rows of Drifts no longer open.

Output with `--json`: `id`, `number`, `title`, `status`, `sentBackToDraft`, `droppedRequires`, `droppedDrifts`, `files`. `--help` gives their types.

Example: `weaver proposal edit p3 --input '{"edit":["model/checkout/order.md"]}'`

### `weaver proposal move <id> open|approved|rejected|discarded [--input <json>|-] [--root <dir>] [--json]`

Hands a Proposal over, or decides it.

`open` hands the Proposal over and prints the warnings and the `show` text. `approved`, `rejected` and `discarded` are the developer's decisions: any caller can run them under the same conditions, and the agent runs them only when the developer explicitly asked for that decision in the conversation. `rejected` takes `feedback` and `rework`; `rework: true`, and approving a stale Proposal (of any kind), need Weaver to be running.

- Argument `<id>`: the card, by identity, unique identity prefix or short number (`P3`, `t3`: a letter and digits is always a short number, and its letter must match the type).
- Argument `open|approved|rejected|discarded`: the state to move to: `open`, `approved`, `rejected`, `discarded`.

Input:

- `feedback`, string (not blank), optional: Why it is rejected; only with `rejected`.
- `rework`, boolean, optional: Ask for a rework; only with `rejected`.

Output with `--json`: `id`, `number`, `title`, `kind`, `status`, `warnings`. `--help` gives their types.

Example: `weaver proposal move p3 open`

## task

### `weaver task new [--input <json>|-] [--root <dir>] [--json]`

Files a Task in To do.

`technical: true` marks a Task with no Model reference. `announcement` is the identity printed by `weaver announcement new`. The job that files it is recorded when `WEAVER_JOB` is set.

Input:

- `title`, string (not blank, one line), required: The title.
- `refs`, list of string (not blank, one line), optional: Model elements, Requirement sections and Specs fields it implements.
- `technical`, boolean, optional: Whether it has no Model reference.
- `body`, string, optional: What to do, in Markdown.
- `announcement`, string (not blank, one line), optional: The announcement of the card.

Output with `--json`: `id`, `number`, `title`, `column`. `--help` gives their types.

Example: `weaver task new --input '{"title":"Cancel an order","refs":["order/Paid"]}'`

### `weaver task show <id> [--input <json>|-] [--root <dir>] [--json]`

Prints a Task with the Model and the Requirements it refers to.

- Argument `<id>`: the card, by identity, unique identity prefix or short number (`P3`, `t3`: a letter and digits is always a short number, and its letter must match the type).

Output with `--json`: `id`, `number`, `title`, `column`, `technical`, `refs`, `blockedBy`. `--help` gives their types.

Example: `weaver task show t3`

### `weaver task list [--input <json>|-] [--root <dir>] [--json]`

Lists Tasks, or picks the runnable ones.

`runnable` gives the next runnable Task, or up to `max` (at least 1) for free places, plus `waiting` (Tasks in To do with blockers) and, with `heldBack`, those left out. `groups` prints the runnable Tasks in groups of Tasks that concern the same thing, for one sub-agent each. `proposals` restricts the Tasks to those of these Proposals and `notProposals` excludes them.

Input:

- `column`, `todo` | `doing` | `done`, optional: Only the Tasks of this column.
- `tasks`, list of string (not blank, one line), optional: Only these Tasks.
- `runnable`, boolean, optional: Pick the runnable Tasks.
- `max`, integer >= 1, optional: At most this many Tasks (or groups with `groups`); with `runnable`.
- `groups`, boolean, optional: Print the runnable Tasks in groups.
- `proposals`, list of string (not blank, one line), optional: Only the Tasks of these Proposals.
- `notProposals`, list of string (not blank, one line), optional: Not the Tasks of these Proposals.
- `heldBack`, boolean, optional: Also list the runnable Tasks left out and why.

Output with `--json`: `tasks`, `waiting`, `groups`, `heldBack`. `--help` gives their types.

Example: `weaver task list --input '{"runnable":true,"max":3,"groups":true}'`

### `weaver task move <id> doing|todo|done [--input <json>|-] [--root <dir>] [--json]`

Takes a Task, releases it or finishes it.

Bound to a job. `doing` takes the Task (refused naming its blocker), `todo` releases it and `done` finishes it. `unchanged`, a non-blank note, with `done` closes the fix Task of a Drift when the code already does what should happen. Only the job that holds a Task moves it out of `doing`.

Who: Only the agent of a job Weaver started can run it (Weaver sets `WEAVER_JOB` in the environment of that agent). Any other caller is refused with exit 1 and nothing is written.

- Argument `<id>`: the card, by identity, unique identity prefix or short number (`P3`, `t3`: a letter and digits is always a short number, and its letter must match the type).
- Argument `doing|todo|done`: the state to move to: `doing`, `todo`, `done`.

Input:

- `unchanged`, string (not blank), optional: Closes the fix Task of a Drift without changes; with `done`.

Output with `--json`: `id`, `number`, `title`, `column`. `--help` gives their types.

Example: `weaver task move t3 doing`

### `weaver task delete <id> [--input <json>|-] [--root <dir>] [--json]`

Deletes a Task in To do.

A decision of the developer: the agent runs it only when the developer explicitly asked for that decision in the conversation.

- Argument `<id>`: the card, by identity, unique identity prefix or short number (`P3`, `t3`: a letter and digits is always a short number, and its letter must match the type).

Output with `--json`: `id`, `number`, `title`. `--help` gives their types.

Example: `weaver task delete t3`

## drift

### `weaver drift new [--input <json>|-] [--root <dir>] [--json]`

Files a Drift: the code and the Model or the Requirements disagree.

`options` are 2 to 5 fix options, the recommended first; they may be left out only with `proposal` (a Draft or Open Proposal no other Drift holds). `task` is the Task the Drift was found in; `previous` is set automatically when that Task is a fix Task. `merge` is only `true` and only for the agent of a scan job started after a merge (the instruction says so), for a Drift whose difference lies in the files the merge brought; from any other caller it is refused and nothing is written, and no command changes it afterwards. `announcement` is the identity printed by `weaver announcement new`. The job that files it is recorded when `WEAVER_JOB` is set.

Input:

- `title`, string (not blank, one line), required: The title.
- `refs`, list of string (not blank, one line) (at least 1), required: What the Drift concerns: Model elements, Requirement sections, Specs fields.
- `happens`, string (not blank), required: What the code does.
- `should`, string (not blank), required: What the Model or the Requirements say it should do.
- `where`, string (not blank), required: Where in the code.
- `options`, list of object (2 to 5), optional: The fix options, the recommended first.
  - `title`, string (not blank, one line), required: The option.
  - `text`, string (not blank), required: What fixing the code this way means.
- `task`, string (not blank, one line), optional: The Task the Drift was found in.
- `proposal`, string (not blank, one line), optional: A Draft or Open Proposal that resolves it.
- `previous`, string (not blank, one line), optional: The Drift this one follows.
- `merge`, boolean, optional: `true` when the difference lies in the files the merge brought; only from a scan after a merge.
- `announcement`, string (not blank, one line), optional: The announcement of the card.

Output with `--json`: `id`, `number`, `title`, `status`, `merge`. `--help` gives their types.

Example: `weaver drift new --input -`

### `weaver drift show <id> [--input <json>|-] [--root <dir>] [--json]`

Prints a Drift with its options, Task, Proposal and prepared question.

`previous` lists the Drifts this one repeats, the nearest first, with the decision taken on each. `prepared` is the prepared question with `valid`; a block that does not read is reported in `problem`.

- Argument `<id>`: the card, by identity, unique identity prefix or short number (`P3`, `t3`: a letter and digits is always a short number, and its letter must match the type).

Output with `--json`: `id`, `number`, `title`, `status`, `merge`, `refs`, `happens`, `should`, `where`, `options`, `created`, `origin`, `previous`, `proposal`, `task`, `job`, `decided`, `prepared`. `--help` gives their types.

Example: `weaver drift show d3`

### `weaver drift list [--input <json>|-] [--root <dir>] [--json]`

Lists Drifts.

Open Drifts only, unless `all`. `refs` lists the Drifts that concern those references, as the Drifts that block a Task, and marks (`unchanged`) the Dismissed Drifts that concern them whose text is unchanged since the dismissal: the agent never files those differences again. `currentJob` uses `WEAVER_JOB` and lists nothing outside a job. `unprepared` lists the open Drifts that have no valid prepared question and are in no resolution being written or open.

Input:

- `all`, boolean, optional: Every Drift, decided ones included.
- `refs`, list of string (not blank, one line), optional: Only the Drifts that concern these references.
- `job`, string (not blank, one line), optional: Only the Drifts filed by this job.
- `currentJob`, boolean, optional: Only the Drifts filed by the job of the caller.
- `unprepared`, boolean, optional: Only the open Drifts without a valid prepared question.

Output with `--json`: `drifts`. `--help` gives their types.

Example: `weaver drift list --input '{"currentJob":true,"unprepared":true}'`

### `weaver drift edit <id> [--input <json>|-] [--root <dir>] [--json]`

Records the prepared question of a Drift, or attaches a Proposal to it.

`prepared` is `{question, decisions}`: `question` is a question of `weaver question new` whose `id` is the Drift identity and whose `multi` is false, and `decisions` has one decision per option, in the same order: every fix option once as `fix-code` with its `option`, plus one `proposal` or `dismissed` (both when the Drift has no fix option). Weaver adds the fingerprint and the instant. `proposal` attaches a Proposal to a Drift that has none.

- Argument `<id>`: the card, by identity, unique identity prefix or short number (`P3`, `t3`: a letter and digits is always a short number, and its letter must match the type).

Input:

- `prepared`, object, optional: The prepared question.
  - `question`, object, required: The question.
    - `id`, string (not blank, one line), required: Unique in the round, non-empty.
    - `question`, string (not blank), required: The question, in Markdown, self-contained: the developer sees only the round.
    - `context`, string (not blank), optional: What the developer needs to answer, in Markdown.
    - `options`, list of object (at least 2), required: At least two. Never an "Other" option: a free answer is always offered.
      - `label`, string (not blank, one line), required: The answer; plain text, not Markdown.
      - `description`, string (not blank), required: What choosing it means, in Markdown.
      - `details`, string (not blank), optional: More detail, in Markdown.
    - `recommended`, integer >= 0, required: The index of the recommended option, from 0.
    - `multi`, boolean, required: Whether several options may be chosen.
  - `decisions`, list of object (at least 1), required: One decision per option of the question.
    - `decision`, `fix-code` | `proposal` | `dismissed`, required: The decision the option stands for.
    - `option`, integer >= 1, optional: The fix option, from 1; with `fix-code`.
- `proposal`, string (not blank, one line), optional: A Proposal to attach.

Output with `--json`: `id`, `number`, `title`, `status`, `prepared`, `proposal`. `--help` gives their types.

Example: `weaver drift edit d3 --input -`

### `weaver drift move <id> code-to-be-fixed|proposal-accepted|dismissed [--input <json>|-] [--root <dir>] [--json]`

Decides a Drift.

The developer's decisions: the agent runs them only when the developer explicitly asked for that decision in the conversation. `code-to-be-fixed` needs `option` (from 1) when the Drift has options; `proposal-accepted` and `dismissed` take none.

- Argument `<id>`: the card, by identity, unique identity prefix or short number (`P3`, `t3`: a letter and digits is always a short number, and its letter must match the type).
- Argument `code-to-be-fixed|proposal-accepted|dismissed`: the state to move to: `code-to-be-fixed`, `proposal-accepted`, `dismissed`.

Input:

- `option`, integer >= 1, optional: The fix option, from 1; with `code-to-be-fixed`.
- `note`, string (not blank), optional: A note for the fix Task.

Output with `--json`: `id`, `number`, `title`, `status`, `task`. `--help` gives their types.

Example: `weaver drift move d3 dismissed`

## history

### `weaver history new [--input <json>|-] [--root <dir>] [--json]`

Records a History entry for finished Tasks.

`tasks` needs at least one Task, each Done and not closed without changes. Every problem is listed and nothing is written.

A Task cited for the first time is recorded in the entry at once: its copy is written into every entry that cites it and its file is deleted.

`commit` is always given, whether Atomic commits is on or off. While it is on, Weaver commits with git the files named, what the developer already staged and, outside Solo mode, the entry and the removal of the files of the Tasks it records, with your message; it never amends, pushes or fetches. A path outside the project, ignored by git, or missing and not tracked by git refuses the whole command and nothing is written. The entry is written whatever happens to the commit, and `commit.status` says how it ended. While it is off, `commit` is only checked for its shape.

Input:

- `title`, string (not blank, one line), required: The title, on one line.
- `summary`, string (not blank, one line, at most 280 characters), required: One line of 1 to 280 characters.
- `body`, string, optional: More detail, in Markdown.
- `tasks`, list of string (not blank, one line) (at least 1), required: The Done Tasks the entry records.
- `commit`, object, required: The commit of the code of this entry's feature.
  - `files`, list of string (not blank, one line), required: Paths of the code of this entry's feature, relative to the project root with `/`, without duplicates; may be empty.
  - `message`, string, required: The commit message in English: a first line of at most 72 characters, optionally followed by a blank line and a body.
  - `checksFailed`, boolean, optional: `true` when the final checks of the Run failed; default `false`.

Output with `--json`: `id`, `number`, `title`, `file`, `recorded`, `kept`, `commit`. `--help` gives their types.

Example: `weaver history new --input -`

### `weaver history edit <id> [--input <json>|-] [--root <dir>] [--json]`

Changes a History entry: replaces fields and adds Tasks.

Replaces the fields given and adds the Tasks given; it never removes a Task. The date of the entry becomes the moment of the edit.

`commit` is required when `tasks` adds at least one Task the entry did not cite, and ignored otherwise. An edit that adds Tasks makes a new commit while Atomic commits is on and never changes the earlier one; an edit that adds none makes no commit.

- Argument `<id>`: the card, by identity, unique identity prefix or short number (`P3`, `t3`: a letter and digits is always a short number, and its letter must match the type).

Input:

- `title`, string (not blank, one line), optional: The title, on one line.
- `summary`, string (not blank, one line, at most 280 characters), optional: One line of 1 to 280 characters.
- `body`, string, optional: More detail, in Markdown.
- `tasks`, list of string (not blank, one line), optional: Tasks to add.
- `commit`, object, optional: The commit of the code of the feature of the Tasks added; required when the edit adds Tasks.
  - `files`, list of string (not blank, one line), required: Paths of the code of this entry's feature, relative to the project root with `/`, without duplicates; may be empty.
  - `message`, string, required: The commit message in English: a first line of at most 72 characters, optionally followed by a blank line and a body.
  - `checksFailed`, boolean, optional: `true` when the final checks of the Run failed; default `false`.

Output with `--json`: `id`, `number`, `title`, `file`, `recorded`, `kept`, `commit`. `--help` gives their types.

Example: `weaver history edit <id> --input -`

### `weaver history show <id> [--input <json>|-] [--root <dir>] [--json]`

Prints a History entry with the Tasks it cites.

- Argument `<id>`: the card, by identity, unique identity prefix or short number (`P3`, `t3`: a letter and digits is always a short number, and its letter must match the type).

Output with `--json`: `id`, `number`, `title`, `summary`, `body`, `created`, `updated`, `tasks`. `--help` gives their types.

Example: `weaver history show <id>`

### `weaver history list [--input <json>|-] [--root <dir>] [--json]`

Lists the History entries, newest first.

Output with `--json`: `entries`. `--help` gives their types.

Example: `weaver history list`

## announcement

### `weaver announcement new [--input <json>|-] [--root <dir>] [--json]`

Announces a card the agent will file, so that the Board shows a ghost of it.

Prints the `id` to pass as `announcement` when creating the card. An unknown or foreign identity still creates the card, with a note.

Who: Only the agent of a job Weaver started can run it (Weaver sets `WEAVER_JOB` in the environment of that agent). Any other caller is refused with exit 1 and nothing is written.

Input:

- `kind`, `proposal` | `task` | `drift`, required: The kind of card.
- `title`, string (not blank, one line, at most 60 characters), required: The title, on one line of at most 60 characters.

Output with `--json`: `id`. `--help` gives their types.

Example: `weaver announcement new --input '{"kind":"task","title":"Cancel an order"}'`

## job

### `weaver job new [--input <json>|-] [--root <dir>] [--json]`

Starts a job of Weaver.

`design` needs `request` (a written, unsent, non-empty design request), `scan` covers the whole System, `run` takes one `task` (or runs all the runnable Tasks without it) and `resolve` needs `drift`. Weaver applies the developer's refusals. Run it only when the developer explicitly asked for that job in the conversation.

Who: Only the agent of the project's General or of an Ask more conversation that Weaver started can run it (Weaver sets `WEAVER_GENERAL` or `WEAVER_ASK` in the environment of that agent). Any other caller is refused with exit 1 and nothing is written.

Input:

- `kind`, `design` | `scan` | `run` | `resolve`, required: The kind of job.
- `request`, string (not blank, one line), optional: The design request; with `design`.
- `task`, string (not blank, one line), optional: The Task to run; with `run`.
- `drift`, string (not blank, one line), optional: The Drift to resolve; with `resolve`.

Output with `--json`: `id`, `kind`. `--help` gives their types.

Example: `weaver job new --input '{"kind":"scan"}'`

### `weaver job show [<id>] [--input <json>|-] [--root <dir>] [--json]`

Prints the job of the caller, or the job `<id>`.

Without `<id>` it reads only the environment and the registry record of the job of the caller: `id`, `kind`, `target`, `merged` (only for a scan after a merge: the files the merge brought, relative to the project root, sorted), whether the session is a Run (`WEAVER_RUN`) and its `task` (`WEAVER_TASK`); outside a job it says so, with exit 0. `<id>` asks Weaver.

- Argument `<id>`: a job identity. Without it, the job of the caller is shown.

Output with `--json`: `job`. `--help` gives their types.

Example: `weaver job show`

### `weaver job list [--input <json>|-] [--root <dir>] [--json]`

Lists the jobs of the project.

Input:

- `all`, boolean, optional: Every job, finished ones included.

Output with `--json`: `jobs`. `--help` gives their types.

Example: `weaver job list`

### `weaver job edit [--input <json>|-] [--root <dir>] [--json]`

Declares what the job is doing now, as a short note.

Line breaks become spaces and the ends are trimmed; an empty note is invalid input. A note over 120 characters is cut to 119 plus `…`, and the output says so. The last 6 notes are kept with their instants; a repeat of the last one only renews its instant.

Who: Only the agent of a job Weaver started can run it (Weaver sets `WEAVER_JOB` in the environment of that agent). Any other caller is refused with exit 1 and nothing is written.

Input:

- `note`, string (not blank), required: What the job is doing.

Output with `--json`: `note`, `cut`. `--help` gives their types.

Example: `weaver job edit --input '{"note":"Reading the order Machine"}'`

## question

### `weaver question new [--input <json>|-] [--root <dir>] [--json]`

Asks the developer a round of questions and waits for the answers.

Prints only JSON on standard output, always. The input is validated whole before Weaver is contacted; then the command waits with no time limit until a result exists: run it with no time limit and wait until it ends.

Write every text so that it stands alone: the developer sees only the round, not the conversation. Texts are Markdown except labels. Use no first person and no "Other" option: a free answer is always offered.

In Chat, use it only when the developer asked to be asked with a round; otherwise ask in your replies.

Results: `{"status":"answered","answers":{<id>:{"status":"answered","selected":[...],"text":...}|{"status":"skipped"}}}` (exit 0); `{"status":"unavailable","reason":...}` (exit 0) when the caller is neither in a working Weaver job nor in the project's Chat during a turn, or no Weaver answers before the round is submitted: stop and tell the developer; `{"error":{"code":"invalid-input"|"interrupted"|"cancelled","message":...}}`, exit 2 for `invalid-input` and 1 for the others.

Who: Only the agent of a job Weaver started or the agent of the project's Chat during a turn can use it (Weaver sets `WEAVER_JOB` or `WEAVER_GENERAL` in the environment of that agent). For any other caller the outcome is `unavailable`, with exit 0.

Input:

- `questions`, list of object (at least 1), required: The questions, at least one.
  - `id`, string (not blank, one line), required: Unique in the round, non-empty.
  - `question`, string (not blank), required: The question, in Markdown, self-contained: the developer sees only the round.
  - `context`, string (not blank), optional: What the developer needs to answer, in Markdown.
  - `options`, list of object (at least 2), required: At least two. Never an "Other" option: a free answer is always offered.
    - `label`, string (not blank, one line), required: The answer; plain text, not Markdown.
    - `description`, string (not blank), required: What choosing it means, in Markdown.
    - `details`, string (not blank), optional: More detail, in Markdown.
  - `recommended`, integer >= 0, required: The index of the recommended option, from 0.
  - `multi`, boolean, required: Whether several options may be chosen.

Output with `--json`: `status`, `answers`, `reason`. `--help` gives their types.

The output is always JSON, with or without `--json`.

Example: `weaver question new --input -`

## request

### `weaver request new [--input <json>|-] [--root <dir>] [--json]`

Writes a design request with the given text and sends it, starting a design job on it (a setup job when the Model is empty).

Run it only when the developer explicitly asked Chat for a Proposal. Weaver then starts a job that explores, asks the developer its own question rounds and hands the Proposal over; the agent of Chat does not wait for it and writes no Proposal.

`text` is the whole request and must stand alone, because the job sees only the request and never the conversation: an H1 with a short title, then what to add or change, every decision agreed with the developer, and the alternatives discussed with the developer's choice. Name cards only by permanent identity, never by short number. Send no images: a request sent this way has none.

Weaver writes the request as the developer's Send does: the text as given, with LF line endings and a final newline, and recognises the tags of assets in it at that moment. The request and the job are created together: when the job cannot start, nothing is written and the reason is printed (exit 1).

Who: Only the agent of the project's Chat during one of its turns can run it (Weaver sets `WEAVER_GENERAL` in the environment of that agent). Any other caller, an Ask more conversation and a job included, is refused with exit 1 and nothing is written.

Input:

- `text`, string (not blank), required: The whole request, in Markdown; not blank.

Output with `--json`: `request`, `job`, `kind`, `title`. `--help` gives their types.

Example: `weaver request new --input '{"text":"# Cancel an order\n\nLet a customer cancel an order that has not shipped."}'`
