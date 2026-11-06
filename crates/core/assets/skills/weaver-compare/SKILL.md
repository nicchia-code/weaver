---
name: weaver-compare
description: Use when comparing one Machine of a project with a `.weaver/` folder against its code, as a `weaver-analysis` sub-agent of weaver-scan or on its own. Reads the Machine, its Requirements and the existing cards, compares them with the code, and files its Tasks and Drifts with `weaver task new` and `weaver drift new`.
---

# Compare one Machine

You compare one **Machine** with its code. You read it yourself, and you file the result with `weaver task new` and `weaver drift new`. You change neither code, nor the Model, nor the Requirements.

Read nothing else about the project: not the General Requirements of other Areas, not any `weaver-req-*` skill, not `weaver-scan`.

## Read

1. Run `weaver model show <machine>`: it prints the Machine (the State tree, the Events with their receivers, the Transitions) followed by the Requirement sections that apply to it. When the instruction gives `refs`, compare only those references, each of which must belong to the Machine. If the command refuses (unknown Machine or reference), report it and stop.
2. Run `weaver task list` once for the open Tasks and `weaver drift list --input -` with `{"all":true,"refs":["<ref>"]}` for the references of the Machine, to know what is already filed. A Drift the developer dismissed, whose text has not changed since, is marked in that list.
3. Before reading the code, record the commit it is read on: run `git rev-parse HEAD` once and keep the hash for the Code notes of the Tasks you file; when the instruction already gives the commit, use that one. Outside a git repository the command fails and there is no commit to record.

## Compare

Compare State by State and Transition by Transition, then each Requirement section that applies. Decide with this table:

| You find | You file |
|---|---|
| A State, Transition or Event in the Model that the code lacks | a **Task** |
| A Requirement the code does not implement yet | a **Task** referencing the Requirement |
| Code that does something the Model says differently, or something extra that would add or change a State, Transition or Event of the Model | a **Drift** |
| Code that contradicts a Requirement | a **Drift** referencing the Requirement |
| Code that handles a case the Model forgot, and it is **important** (user-visible or data-affecting, not a detail) | a **Drift with a Proposal** |
| Anything below the level of the Model's States, Transitions and Events that no Requirement mentions | nothing |

- Find the code yourself: handlers, reducers, components, routes, jobs, database transitions. The Model carries no code hints.
- Unsure whether something is a Drift or only an implementation detail? Ask: would it add or change a State, Transition or Event of the Model? If not, skip it.
- If you cannot tell whether code implements a Transition, say so in your reply instead of guessing.
- No source code at all: the Machine is still to be built. File one Task per coherent unit (usually a State with its Transitions), no Drifts, and say in your reply that the project had no code.

## Notes on the job

The developer follows the work through notes: `weaver job edit --input -` with `{"note":"<one line>"}`, passed as `weaver job edit --input - <<'JSON'`, the object and `JSON`. Write one when you start reading the code and one when you start filing: two notes in all, never one per command. A note is one sentence of at most 120 characters saying what the agent does and why, for the developer: no first person, no `weaver` command names, no short numbers of cards, in the language of the instruction when it asks for one (for example "Compares the order Machine with the code, State by State").

## Announcing cards

Announce every Task and Drift you file, as soon as you know its title: `weaver announcement new --input -` with `{"kind":"task","title":"<title>"}` or `{"kind":"drift","title":"<title>"}` prints an announcement ID; pass it as `announcement` when you file the card. For a Drift with a Proposal, announce only the Drift and pass the same ID to `weaver proposal new --input -` too.

## Tasks

File each with `weaver task new --input -` and `{"title":"<what to implement>","refs":["order/Paid"],"body":"<what must be true when done>","technical":false,"announcement":"<id>"}`.

- One Task per coherent unit of work, as specific as possible in `refs`: Model elements and Requirement sections (`order/Paid/user.CANCEL`, `requirements/ui#order/Paid`).
- Skip what an open Task already covers.
- The body says *what* must be true when done, in domain words, not which files to touch, with the conditions and consequences as the Model states them. The files to touch go only in the Code notes below.
- The body of a Task is understood without the conversation that produced it.
- A Task that only applies Requirements without changing behaviour is technical: `technical: true`.
- End every Task you file, a fix Task excepted, with a section `## Code notes` for whoever implements it, made only of what you already read of the code while comparing: never read code only to write them. First line `Commit: <full hash>`, the hash recorded in Read (no such line when the project is not in a git repository); then short Markdown, about 1,500 characters at most: where to look (files in backticks, relative to the project with `/`, and names in the code), the conventions to follow, the pitfalls seen and what can be reused. They are hints, never part of what must be true. In the `body` JSON string the section is written after the domain text, with `\n\n## Code notes\n\nCommit: <hash>\n\n…`.

## Drifts

- Re-read the code at the place you will cite in `where` before filing. File only if that code really does what `happens` says.
- A difference that a Drift closed (fixed in the code, or its Proposal accepted) stays closed while the Model or Requirement says the same. If it is back after a fix, set `previous` to that resolved Drift's ID. A dismissed Drift whose text has not changed is never filed again.
- `happens` and `should` say what the System does and what it should do, in domain words. Name no variable, function, class, field or file unless unavoidable (then as `inline code`). Code locations go only in `where`.
- Give two to five `options`, the recommended first, each a different way to reach what should happen without changing the Model or any Requirement. Each option has a short `title` and about 60 words of `text` (what it changes and costs), in short Markdown without headings or tables.
- Drift titles: at most five words, about 40 characters.
- File each with `weaver drift new --input -` and `{"title":"<title>","refs":["<ref>"],"happens":"<what the code does>","should":"<what the Model or Requirement says>","options":[{"title":"<title>","text":"<text>"},{"title":"<title>","text":"<text>"}],"where":"<where in the code>","announcement":"<id>"}`. A Drift filed by a Scan has no `task`.

### Drift with a Proposal

Only for an important case the Model forgot, or when no fix of the code respects the Model and the Requirements together (then the Drift takes no `options`):

1. Create a Draft: `weaver proposal new --input -` with `{"title":"<title>","body":"<the choices, and the Drift it answers>","label":"<short label>","announcement":"<id of the Drift's announcement>"}`.
2. Write its files as the `weaver-design` skill describes, without its interview.
3. File the Drift once with `proposal` set to the Draft's ID, then run `weaver proposal move <draft id> open` and read its output. If it is refused, fix the reported problems with `weaver proposal edit <id>` and hand over again.

Never approve, reject, discard or resolve any card, never delete a Task (`weaver task delete`) and never choose a resolution: decisions belong to the developer, unless the developer explicitly asks you in the conversation.

## After a merge

When the scan that sent you here follows a merge, it passes you the files the merge brought, relative to the project folder; without that list, nothing here applies. Before filing a Drift, look at where the difference lies: when the code you cite in `where` is in one of those files, add `"merge":true` to the Drift, and never in any other case. The Drift is otherwise filed as always, and the rules for Drifts already filed, dismissed or closed do not change. Only a scan after a merge may set `merge`: `weaver drift new` refuses it from any other job.

## Reply

Name each card by short number and title, as `Task T3 – title` or `Drift D2 – title`, but write only permanent identities in files and commands. Report: the cards created with their identities; references you found no code for; any doubt you could not settle.

## Language

When the instruction asks for a language, the texts listed below go in that language. Without a language in the instruction, nothing here changes.

- **In the language of the instruction:** the titles and bodies of Tasks; the titles, bodies and Fix options of Drifts; the titles, bodies and reasons of Proposals; the text, context and options of questions; the notes of the job and the titles of announcements; the titles, summaries and texts of History entries; the replies during the work and the final report.
- **Always in English**, whatever the language: the whole Model (the name of the System; the names of Features, Machines, Actors and States; IDs and Event names; the descriptions in Model files); the Requirements (Areas, titles and bodies of fragments); the Specs, also their `notes`; code and its comments, commit messages and the project's own documentation; the headers Weaver reads in files (`States`, `Events`, `General`, `What happens`, `What should happen`, `Where in the code`, `Fix options`, `Already present`), the `(initial)` and `(final)` markers and the front matter fields; technical terms (commands and tools such as `git pull` or `npm install`, names of products, services, libraries and languages) and Weaver's own terms (Proposal, Task, Drift, Board, Run, Scan). Weaver's interface and the `weaver` command line stay in English.
- **A language changes nothing already written.** When you edit a file written in another language, keep its existing text and write only the new or rewritten text in the chosen language; never translate the whole file.
- **No first person in Weaver's texts.** In what you write for Weaver (questions, contexts and options of rounds, titles and bodies of Proposals, Tasks and Drifts, History entries), Weaver is always "Weaver" and you are "the agent": no first-person pronouns or first-person verbs, in any language. Your replies in the conversation are not Weaver's texts and do not change. Text already written is not rewritten.
- **Self-contained Tasks.** The body of a Task is understood without the conversation that produced it: it says what must be true when done, with the conditions and consequences as the Model says them, and every decision of the interview that concerns it, because whoever implements it does not see that conversation.

## Sub-agents context sharing

The agent that gave you the Machine may also pass you, besides the Machine and its `refs`, the places in the code it already knows for those references, one line per reference, ``- `<ref>` — <where in the code>``.

- **Use the places** as a starting point to find the code: read the code at them, always, and never trust a place for what the code does. A place that no longer matches the code is ignored: search by yourself.
- **Take nothing from them for the cards.** You still run `weaver model show`, `weaver task list` and `weaver drift list` as written above.
- **Do not run again** a read-only `weaver` command for a text you received.
- **Report back**, in your reply, where you found the code of what you saw, one line per reference as above, so that the agent that gave you the Machine can pass them on.

## Command reference

Every `weaver` command, with its verbs and input fields, is in [COMMANDS.md](../weaver-design/COMMANDS.md), generated from the command registry. `weaver <object> <verb> --help` prints the usage, the fields with their types and limits, an example and the output fields.

## PowerShell

The examples use POSIX syntax. In PowerShell, pass JSON with a pipe and `--input -`, for example `Get-Content -Raw round.json | weaver question new --input -`, instead of `<` or a heredoc. On Windows PowerShell 5.1, set `$OutputEncoding` and `[Console]::OutputEncoding` to UTF-8 first, or non-ASCII characters arrive as `?`. Never pass the JSON as the text of `--input`: Windows PowerShell 5.1 strips the inner quotes of arguments to external programs.
Write the `body` and `happens` values in single quotes, because PowerShell expands `$` and the backtick inside double quotes.
