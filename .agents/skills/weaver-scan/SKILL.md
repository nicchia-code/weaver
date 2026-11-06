---
name: weaver-scan
description: Use when the instruction asks to scan the whole System of a project with a `.weaver/` folder against its code. Checks behaviour against the Model and construction against the Requirements, one Machine per `weaver-analysis` sub-agent. Produces Tasks for missing work and Drifts for differences. Does not change code, the Model or the Requirements.
---

# Weaver: scan

A **Scan** compares the whole System: the Model (source of truth for behaviour) and its **Requirements** (how it must be built) with the existing code. You produce **Tasks** (work to do) and **Drifts** (differences the developer must decide on). You change neither code, nor Model, nor Requirements.

## Hand-over

A new Proposal starts as a **Draft**, not approvable. Finish every file, then run `weaver proposal move <id> open` without `weaver proposal show` before it: read the changes in the hand-over's output. If they do not match what you meant, reopen the Proposal with `weaver proposal edit <id>`, fix it, and hand it over again. If the hand-over is refused, fix the reported problems and hand it over again. A Proposal meant for a Drift is attached to that Drift when the Drift is filed, before it is handed over, so it never shows up alone. Only after it succeeds tell the developer to review it. Before every later change, call `weaver proposal edit <id>` again: it returns an open Proposal to Draft, so hand it over again when finished.

## Announcing cards

Announce every Task and Drift you file yourself, as soon as you know its title: `weaver announcement new --input -` with `{"kind":"task","title":"<title>"}` or `{"kind":"drift","title":"<title>"}` prints an announcement ID; pass it as `announcement` to `weaver task new --input -` or `weaver drift new --input -`. For a Drift with a Proposal, announce only the Drift and pass the same ID to `weaver proposal new --input -` (field `announcement`) too. A `weaver-analysis` sub-agent announces the cards it files exactly as you do. The Board shows a ghost of each card meanwhile. Never wait for a ghost to show; an unknown ID is only noted in the output.

## Hard rules

- NEVER edit code, `.weaver/model/` or `.weaver/requirements/` in this skill.
- Agents may approve, reject or discard a Proposal, decide a Drift or delete a Task in To do (`weaver task delete`) only when the developer explicitly asks for that decision in the conversation. Without that request, do not decide and do not ask the developer about it: stop and tell them the decision is theirs, on the Board. A request to scan does not by itself authorise any of them.
- Compare behaviour at the level of the Model's States, Transitions and Events, who reacts to what. Compare construction against what the Requirements actually state. Variable names, structure, style and performance that no Requirement mentions are not Drifts.
- The Model matters far more than the code. By default the code is wrong, not the Model.

## Naming cards

Every live Proposal, Task and Drift has a temporary **short number** (`P3`, `T3`, `D3`) besides its **permanent identity** of eight characters.

- Name cards to the developer by short number and title, for example "Task T3 – title", also in the final report. A closed card has no number: use its permanent identity.
- In every command you run and every file you write use **only the permanent identity**, taken from `--json` (field `id`; the number is the field `number`) or from the file or folder name. **Never write a short number in a file**: it changes from one working copy to another.
- Give every card you create, and its announcement, a **short title**: at most five words, about 40 characters, a name for the card rather than a sentence describing the work (for example "Ghost text" instead of "Describe the start screen and guide written requests with a ghost text"). Put the detail in the body, even when the title loses information. Only the title of an announcement is checked (one line, at most 60 characters). Titles that a skill composes from another title stay as they are.

## Notes on the job

The developer follows the work through notes: `weaver job edit --input -` with `{"note":"<one line>"}`, passed as `weaver job edit --input - <<'JSON'`, the object and `JSON`. Declare a note at the start of each stage of the work: the plan of the Machines to compare, and gathering the results. Write 3 to 6 notes for the whole job, never one per command. A note is one sentence of at most 120 characters (a longer one is cut) saying what the agent does and why, for the developer: no first person, no `weaver` command names and no short numbers of cards, in the language of the instruction when it asks for one (for example "Explores the order Machine before asking the first questions"). A note changes nothing else and is never needed to deliver a card.

## Language

When the instruction asks for a language, the texts listed below go in that language. Without a language in the instruction, nothing here changes.

- **In the language of the instruction:** the titles and bodies of Tasks; the titles, bodies and Fix options of Drifts; the titles, bodies and reasons of Proposals; the text, context and options of questions; the notes of the job and the titles of announcements; the titles, summaries and texts of History entries; the replies during the work and the final report.
- **Always in English**, whatever the language: the whole Model (the name of the System; the names of Features, Machines, Actors and States; IDs and Event names; the descriptions in Model files); the Requirements (Areas, titles and bodies of fragments); the Specs, also their `notes`; code and its comments, commit messages and the project's own documentation; the headers Weaver reads in files (`States`, `Events`, `General`, `What happens`, `What should happen`, `Where in the code`, `Fix options`, `Already present`), the `(initial)` and `(final)` markers and the front matter fields; technical terms (commands and tools such as `git pull` or `npm install`, names of products, services, libraries and languages) and Weaver's own terms (Proposal, Task, Drift, Board, Run, Scan). Weaver's interface and the `weaver` command line stay in English.
- **A language changes nothing already written.** When you edit a file written in another language, keep its existing text and write only the new or rewritten text in the chosen language; never translate the whole file.
- **No first person in Weaver's texts.** In what you write for Weaver (questions, contexts and options of rounds, titles and bodies of Proposals, Tasks and Drifts, History entries), Weaver is always "Weaver" and you are "the agent": no first-person pronouns or first-person verbs, in any language. Your replies in the conversation are not Weaver's texts and do not change. Text already written is not rewritten.
- **Self-contained Tasks.** The body of a Task is understood without the conversation that produced it: it says what must be true when done, with the conditions and consequences as the Model says them, and every decision of the interview that concerns it, because whoever implements it does not see that conversation.

## Steps

1. **Scope.** The scan always covers the whole System. Before reading anything, record the commit the code is read on: run `git rev-parse HEAD` once and keep the hash for the Code notes of step 6 (outside a git repository the command fails and there is no commit to record). Run `weaver model show` for the list of Features and Areas, then `weaver model show <feature>` for the Machines of each. Do not read a Machine yourself (`weaver model show <machine>`): the `weaver-analysis` sub-agent of that Machine does. Read the Requirements only through their `## General` sections, one Area at a time: `weaver model show requirements/<area>#General`, and keep them in mind for everything. Do not read a whole Area for its other sections: they reach each Machine's analysis through its own `weaver model show`.
2. **Avoid duplicates.** Read `weaver task list` and `weaver drift list` once, first, for the parts you compare yourself. Do not file what is already there. Before filing a Drift, run `weaver drift list --input -` with `{"all":true,"refs":["<ref>"]}` for each element or Requirement it concerns: a Drift the developer dismissed, whose text has not changed since, is marked in that list, and that difference is never filed again; a difference closed by a fix of the code or by an accepted Proposal stays closed while the Model or the Requirement there still says the same thing. If the difference is back after a fix of the code or an adaptation of the Model, file the new Drift with `previous` set to the ID of that resolved Drift; the command refuses a previous Drift that is open or unknown.
3. **Compare the Machines, then the rest.** Every Machine in the Model is compared directly against its code, following the `weaver-compare` skill. Do not triage Machines or use a classifier.
   - **With sub-agents**: give each Machine to a `weaver-analysis` sub-agent, together with the Machine's name and the commit hash recorded in step 1. The sub-agent follows `weaver-compare`, reads the Machine, the open Tasks and the Drifts on its references by itself, files its own cards with `weaver task new` and `weaver drift new`, and returns the list of cards it created. Pass it the sentences of the instruction that concern a sub-agent, on the same line, as the instruction writes them: the language sentence, if the instruction has one. After a merge, add the list of the merged files (see "After a merge"). Run at most N sub-agents at once, where N is the limit in the instruction ("Run at most <N> sub-agents at once"). If sub-agents are unavailable, compare the Machines yourself, one after the other, following `weaver-compare`.
   The General Requirements, and the sections that name no Machine (a Feature, an Actor, an Event of an Actor, an Area), stay with you: compare them yourself, in the same way. You write the one report.
4. **Find the implementation.** For the parts you compare yourself (the General Requirements and the sections that name no Machine), search the code for where they live: handlers, reducers, components, routes, jobs, database transitions. The Model carries no code hints; do not expect any. If there are no source files at all, see "No code at all" under Judgement.
5. **Compare the parts you keep** and decide:

   | You find | You file |
   |---|---|
   | A State, Transition or Event in the Model that the code lacks | a **Task** |
   | A Requirement the code does not implement yet (a screen without the required layout, a missing limit, a library not adopted) | a **Task** referencing the Requirement |
   | Code that does something the Model says differently, or something extra that would add or change a State or Transition in the Model | a **Drift** |
   | Code that contradicts a Requirement (uses a forbidden library, shows a different layout, applies another formula) | a **Drift** referencing the Requirement |
   | Code that handles a case the Model forgot, and it looks **important** (real user-visible or data-affecting behaviour, not a detail) | a **Drift with an attached Proposal** |
   | Anything below the level of the Model's States, Transitions and Events that no Requirement mentions | nothing |

6. **File Tasks.** One Task per coherent unit of work (typically a State with its Transitions), as specific as possible in `refs`:
   `weaver task new --input -` with `{"title":"<what to implement>","refs":["order/Refunding","order/Paid/user.CANCEL","requirements/ui#order/Paid"],"body":"<in domain words: what must be true when done>","announcement":"<id>"}` in a heredoc.
   Give the Task a short title (see Naming cards).
   Reference the Requirement sections the Task must satisfy next to the Model elements. A Task that only applies Requirements without changing behaviour (for example adopting the required theme everywhere) is technical: `"technical":true` with `"refs":["requirements/ui#General"]`.
   The body says *what*, in domain words, not which files to touch.
   End every Task you file, a fix Task excepted, with a section `## Code notes` for whoever implements it, made only of what you already read of the code in this scan and of what your sub-agents reported: never read code only to write them. First line `Commit: <full hash>`, the hash recorded in step 1 (no such line when the project is not in a git repository); then short Markdown, about 1,500 characters at most: where to look (files in backticks, relative to the project with `/`, and names in the code), the conventions to follow, the pitfalls seen and what can be reused. They are hints, never part of what must be true: the text above the section stays in domain words. In the `body` JSON string the section follows the domain text after a blank line.
7. **File each Drift exactly once, in one of two ways.** `weaver drift new` always creates a new card, so decide which way applies before running it.
   - **Ordinary Drift** (the default): first re-read the code at the place you will cite in `where`, and file only if that code really does what `happens` says. Then file it with `weaver drift new --input - <<'JSON'` and `JSON` (a JSON object on standard input): `{"title":"<title>","refs":["<nearest Model element or Requirement>"],"happens":"<what the code does, in terms of behaviour>","should":"<what the Model or Requirement says>","options":[{"title":"<title of the way you recommend>","text":"<what it changes and costs>"},{"title":"<title of another way>","text":"<…>"}],"where":"<where in the code you saw it>","announcement":"<id>"}` (up to five options). The two texts, the options and `where` are required; `where` is the only place for code locations. Write them as described in "Writing a Drift". Do not set `task`: a Drift filed by a Scan has no origin Task. Do not choose the resolution.
   - **Drift with a Proposal**, only when the code handles an important case the Model forgot: first re-read the code at the place you will cite in `where`, and announce only the Drift, then begin the Proposal by following the `weaver-design` skill without its interview: `weaver proposal new --input -` with `{"title":"<title>","body":"<why>","label":"<short label>","announcement":"<id of the Drift's announcement>"}`, then `weaver proposal edit <id> --input -` with `{"edit":[…]}`, write the files (list your choices in the Proposal body). If the hand-over warns that a Requirement fragment the Proposal writes into is larger than 12 KB, split it in the same Proposal, as `weaver-design` describes under "Good Requirement habits". Do not hand it over yet: run `weaver drift new --input -` with `proposal` (the Draft's ID) and `announcement` once, with the same texts, options and `where`, and right after it run `weaver proposal move <id> open`. If the hand-over is refused, correct the Proposal with `weaver proposal edit <id>`, then hand it over again. Never run `drift new` a second time for the same difference.
   - **Drift without Fix options, with a Proposal**, when no fix of the code respects the Model and the Requirements together (for example, two Requirements contradict each other): the same steps as above, but the Drift takes no `options`, and its Proposal repairs the Model or the Requirements. Pass `proposal` and `where`, and omit `options`; a Drift without Fix options is refused without a Proposal.
   - Give the Drift a short title (see Naming cards).

   Do not open a Proposal for small differences: the default is that the code gets fixed.
8. **Prepare the questions of the Drifts.** Follow "Preparing the questions" below.
9. **Report.** Give the count of Machines compared (every Machine in the Model, each one compared directly, by you or by a sub-agent) and the count of those that produced at least one Task or Drift. List the Tasks and Drifts you and your sub-agents created with their short numbers and titles, Machines you could not find any code for, and the warnings of `weaver model check`. Say which Drifts need a decision. Then stop.

## After a merge

When the instruction says the scan follows a merge (`Use the weaver-scan skill on the whole System after a merge: read the files the merge brought with `weaver job show`.`), the scan is the same, with these additions:

- Run `weaver job show` before the first step: its `merged` field lists the files the merge brought, relative to the project folder. They are the work of the others, plus the resolutions of the conflicts, and the only thing that tells which differences come from the merge. Keep the list.
- Pass the list to every `weaver-analysis` sub-agent, with its Machine, on the same line as the language sentence. A sub-agent does not read the job itself.
- File with `"merge":true` each Drift whose difference lies in those files, that is, whose `where` cites code in one of them, and no other Drift: a difference found anywhere else is filed as always, without `merge`. Only this scan may set `merge`; `weaver drift new` refuses it from any other job.
- Tasks, and the rules for Drifts already filed, dismissed or closed, stay as they are: a dismissed Drift whose text has not changed is never filed again, also when the difference lies in a merged file, and a difference that is back after a fix is filed with `previous`.

## Judgement

- Unsure whether code is a Drift or only an implementation detail? Ask: would it add or change a State, Transition or Event of the Model? If not, skip it.
- If you cannot tell whether the code implements a Transition, say so in the report instead of guessing a Task or a Drift.
- **No code at all is an answer, not a blocker.** If, after step 4, the project has no source files (only `.weaver/`, agent skill folders and editor or tool settings), it is a greenfield: the whole Model is still to be built. Do not stop and ask where the code lives. Treat every State and Transition as missing and go to step 6: skip what an open or in-progress Task already covers, then file one Task per coherent unit (usually one per Machine, or per group of States if the Machine is large), each with a `refs` entry for every State and Transition it covers and for the Requirement sections attached to them. If the `tech` Area or other General Requirements ask for setup (framework, project structure), file that first as a technical Task referencing them. Say in the report that the project had no code. A Drift needs code to differ from, so file none, and list existing Drifts whose code paths do not exist so the developer can dismiss them.
- Ask only when you truly cannot tell whether code exists, for example because the sources may live in another repository: then ask with `weaver question new`, as "Asking" below says, waiting rules included.

## Writing a Drift

The developer reads a Drift at a glance, as a programmer, but not as a code review. Write the two sides, and the fix options, at the level of behaviour.

- **Say what the System does and what it should do**, in the words of the domain ("the cart total ignores the coupon"), not how the code does it.
- **Name no variable, function, class, field or file** in `happens`, `should` or the options, unless the difference cannot be understood without it. Then name only the one or two that matter, as `inline code`.
- **Code locations go only in `where`**, which is required for every Drift: the files, and the code element when it helps. Re-read that code before filing; no line numbers, which go stale, and no long pieces of code.
- **Give two to five fix options**, the one you recommend first: each option is an object `{"title":"<title>","text":"<text>"}`. Each is a different way to reach what should happen, not a step of one plan, and says in terms of behaviour what changes and what it costs. About 60 words each, short Markdown, no headings or tables inside an option. Titles differ. A Drift filed with fewer than two, more than five, or an option missing its title or text is refused.
- **Fix options reach what should happen** without changing the Model or any Requirement, and respect every Requirement that applies to the elements of the Drift, not only those it names. An option that asks for an exception to, or a change of, the Model or a Requirement is not a fix option: that way goes to a Drift with a Proposal.
- **Use short Markdown:** short paragraphs, a bulleted list when there are several separate points, **bold** for the one or two terms that carry the difference. No headings inside a section, no tables, about 100 words per side.

## Asking

Run **`weaver question new --input -`** with the JSON request on standard input, in a heredoc, for every round of questions. It is the only way to ask the developer: never use omp's own question tool and never write the questions in the conversation.

- Send one complete round as `{"questions":[...]}`. Every question has a unique non-empty `id`, `question`, `options` as `{label, description}` (2 to 5 of them), a zero-based `recommended` index, and an explicit `multi` boolean. Two fields are optional: `context` on a question and `details` on an option; when present they must not be blank.
- Keep `question` short: one sentence that reads on its own. Put the background of the decision in `context`, the tradeoff in a few words in `description`, and a longer explanation, only when it is needed, in `details`. `question`, `context`, `description` and `details` are Markdown; labels are plain text. Set `multi: true` only when several answers can genuinely hold together. Do not supply "Other": Weaver always offers a free-text answer.
- The developer sees only the round (each question, its context, its options and their own answers), never your conversation. Make every question self-contained: it says everything needed to decide, with no references to the conversation or to another question, and no first person (write "Weaver" or "the agent"). Omit `context` when the question and the options are enough on their own.
- The result is JSON on stdout. With `status: "answered"`, `answers` is keyed by question id: each entry is `{"status":"answered","selected":[<labels>],"text":<optional free text>}` or `{"status":"skipped"}`. A skipped question is an explicit developer skip: use its recommendation for that decision, report it as an assumption for review, and do not ask it again.
- With `status: "unavailable"`, Weaver cannot show questions: ask in no other way, file no Task and no Drift, and tell the developer. When the developer says to continue, run the same `weaver question new` again: it is a new round.
- An error `interrupted` or `cancelled` (exit 1) stops this work at once: no partial answers, no recommendations, no retry, no fallback, no resuming of the round; tell the developer that the questions were lost.
- An error `invalid-input` (exit 2) means the request was refused before opening a round: correct the field it names and submit the corrected request. A usage error is never unavailability.
- **Wait for `weaver question new` to finish**: it blocks until the developer answers, however long that takes. Run it with bash `timeout: 0`, never a fixed value. Never use a timer, `sleep`, periodic reads of the output or interval checks; never end the turn to be woken up; never treat the command as failed and never run it again.

## Preparing the questions

At the very end of the work (before the final report) prepare the question of every Drift this work filed, so Resolve opens at once. You alone do this, never a sub-agent that compares or implements.

1. **Find the Drifts.** Run `weaver drift list --input -` with `{"currentJob":true,"unprepared":true}`; never read `WEAVER_JOB` by hand: the command reads it itself. It lists the open Drifts of this job, the advisor's included, with no valid prepared question and outside every resolution. When it lists none, skip the rest.
2. **Prepare each.** Follow "Writing a prepared question" in the `weaver-resolve-drifts` skill (read it first) for each Drift, with the sub-agents you have: hand the exploration to at most N of them at once (N is the limit in the instruction), one Drift (or a few that concern the same thing) each, and have each record its question with `weaver drift edit`. Without sub-agents, one Drift at a time. A sub-agent asks the developer nothing and registers no other card.
3. **Report.** The final report says how many Drifts were prepared and which were not (with the reason).

## Sub-agents context sharing

When you hand a part of the work to a sub-agent, in whatever way your agent starts sub-agents, you pass it what you have already read that concerns its part, so it does not look for it again and does not read it again.

- **What you pass.** Of what you have already read, only what concerns that sub-agent's part, plus a short common part (the request and the decisions, when there are any): Weaver's texts quoted as they are (the written request with its images put in words, the output of `weaver model show`, `weaver task show`, `weaver drift show`, `weaver proposal show`, the decisions of the interview) and the places in the code already known, one line per reference, ``- `<ref>` — <where in the code>``. Never code, never everything you gathered, and nothing you read only to pass it on.
- **What the sub-agent does.** It does not reread the Weaver texts it received and reads on its own only the ones that are missing. It always reads the code, also at the places it received, always runs the commands that register its work (`weaver task move <id> doing`, `weaver task new`, `weaver drift new`) and loads its skill as without this feature. A received place that no longer matches the code is ignored: it searches by itself. Context sharing never stops a job.
- **What comes back.** Each sub-agent reports where it found the code of what it saw, one line per reference as above; you reuse those lines in your own work and for the sub-agents that follow.
- **Limits.** No sub-agent runs again a read-only `weaver` command for a text it received, and what you pass to each one concerns only its part: never copy into every sub-agent all that you gathered.

In this skill:

- **Comparing the Machines (step 3).** Each `weaver-analysis` sub-agent receives, with the Machine, only the places in the code already known for its references and the commit hash recorded in step 1. Lists of cards and the General Requirements stay out: it reads the cards itself.
- **Preparing the questions.** For each Drift, pass the output of `weaver drift show`, the Proposal attached to it, the chain of its `previous` Drifts and where the code is. The code at its Where in the code is read again before deciding.

## Command reference

Every `weaver` command, with its verbs and input fields, is in [COMMANDS.md](../weaver-design/COMMANDS.md), generated from the command registry. `weaver <object> <verb> --help` prints the usage, the fields with their types and limits, an example and the output fields.

## PowerShell

The examples use POSIX syntax. In PowerShell, pass JSON with a pipe and `--input -`, for example `Get-Content -Raw round.json | weaver question new --input -`, instead of `<` or a heredoc. On Windows PowerShell 5.1, set `$OutputEncoding` and `[Console]::OutputEncoding` to UTF-8 first, or non-ASCII characters arrive as `?`. Never pass the JSON as the text of `--input`: Windows PowerShell 5.1 strips the inner quotes of arguments to external programs.
