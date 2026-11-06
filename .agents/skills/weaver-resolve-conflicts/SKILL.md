---
name: weaver-resolve-conflicts
description: Use when the instruction asks to resolve the conflicts of the merge in progress recorded in `.weaver/merge/merge.json`. Finds the files in conflict with git, resolves the obvious ones, asks the developer about the others, and marks each resolved file with `git add`. Never commits and never ends the merge.
---

# Weaver: resolve merge conflicts

A **merge** of another branch into the current one is in progress in this working copy: git stopped on conflicts and Weaver remembers the merge in `.weaver/merge/merge.json`. You resolve the conflicts of the files that still have them. The developer, not you, ends the merge: they commit it in git when no conflict is left. You change only the files in conflict, and you never touch the Model, the Requirements or any card.

## Hard rules

- NEVER run `git commit`, and NEVER run `git merge --abort`, nor anything else that ends the merge or loses its work: `git reset`, `git checkout` of a branch, `git switch`, `git stash`, `git rebase`, `git clean`. The merge ends when the developer commits it or aborts it from the Board.
- NEVER run `git fetch`, `git pull` or `git push`, and never create or delete a branch.
- NEVER edit `.weaver/model/` or `.weaver/requirements/`, nor create, change or decide Proposals, Tasks or Drifts: this job resolves code conflicts only.
- Mark a file resolved with `git add <path>` only when no conflict marker is left in it and the file holds the result you or the developer chose. Never `git add` a file that still has conflicts, and never `git add` a file that is not in conflict.
- If conflicts are left when the job ends, the merge stays in progress: say so in the final report, with the files, so that the developer can resolve them by hand or ask again.

## Notes on the job

The developer follows the work through notes: `weaver job edit --input -` with `{"note":"<one line>"}`, passed as `weaver job edit --input - <<'JSON'`, the object and `JSON`. Declare a note at the start of each stage of the work: finding the files with conflicts, the round of questions, resolving the files, marking them. Write 3 to 6 notes for the whole job, never one per command. A note is one sentence of at most 120 characters (a longer one is cut) saying what the agent does and why, for the developer: no first person, no `weaver` command names and no short numbers of cards, in the language of the instruction when it asks for one (for example "Explores the order Machine before asking the first questions"). A note changes nothing else and is never needed to deliver a card.

## Language

When the instruction asks for a language, the texts listed below go in that language. Without a language in the instruction, nothing here changes.

- **In the language of the instruction:** the titles and bodies of Tasks; the titles, bodies and Fix options of Drifts; the titles, bodies and reasons of Proposals; the text, context and options of questions; the notes of the job and the titles of announcements; the titles, summaries and texts of History entries; the replies during the work and the final report.
- **Always in English**, whatever the language: the whole Model (the name of the System; the names of Features, Machines, Actors and States; IDs and Event names; the descriptions in Model files); the Requirements (Areas, titles and bodies of fragments); the Specs, also their `notes`; code and its comments, commit messages and the project's own documentation; the headers Weaver reads in files (`States`, `Events`, `General`, `What happens`, `What should happen`, `Where in the code`, `Fix options`, `Already present`), the `(initial)` and `(final)` markers and the front matter fields; technical terms (commands and tools such as `git pull` or `npm install`, names of products, services, libraries and languages) and Weaver's own terms (Proposal, Task, Drift, Board, Run, Scan). Weaver's interface and the `weaver` command line stay in English.
- **A language changes nothing already written.** When you edit a file written in another language, keep its existing text and write only the new or rewritten text in the chosen language; never translate the whole file.
- **No first person in Weaver's texts.** In what you write for Weaver (questions, contexts and options of rounds, titles and bodies of Proposals, Tasks and Drifts, History entries), Weaver is always "Weaver" and you are "the agent": no first-person pronouns or first-person verbs, in any language. Your replies in the conversation are not Weaver's texts and do not change. Text already written is not rewritten.

## Steps

1. **Read the merge.** Read `.weaver/merge/merge.json`: `branch` is the branch being merged, `current` the branch it goes into, `start` the commit `current` started from and `incoming` the commit of `branch`. These are the facts of the merge: never ask the developer for them.
2. **Find the files in conflict.** Run `git diff --name-only --diff-filter=U` (add `-z` for names with unusual characters): every path it lists still has conflicts. `git status` shows the same files as `both modified`, `added by us` or `deleted by them`. Stop, resolving nothing, when none is listed, and say that no conflict is left.
3. **Understand each file.** For each file read its conflict markers and, to know what each side meant, `git log --oneline --left-right --merge -- <path>` and the version of each branch (`git show :2:<path>` is the version of `current`, `git show :3:<path>` the version of `branch`, `git show :1:<path>` the common ancestor). Read the code around the conflict until the intent of both sides is clear. A file deleted on one side and changed on the other, or added on both sides, is a conflict too.
   **With sub-agents**, give them this reading, at most N at once, where N is the limit in the instruction ("Run at most <N> sub-agents at once"): one file, or a few that concern the same change, each. A sub-agent resolves nothing and marks nothing: it reports what each side changed and what resolution it would propose. Without sub-agents, read the files yourself, one after the other.
4. **Resolve the obvious ones.** A conflict is obvious when both sides can stay (the two changes touch different lines of the same block, two imports or entries added to the same list, a rename on one side and an edit of the old name on the other), when one side only reformats, or when one side contains the other. Edit the file so that the result keeps the intent of both sides, remove every conflict marker (`<<<<<<<`, `=======`, `>>>>>>>`), check that the file is still valid, then mark it with `git add <path>`.
5. **Ask the developer about the rest.** When the choice between the two versions is not obvious (the sides contradict each other, or keeping one drops behaviour the other needs), ask in **one round only** with `weaver question new`, as "Asking" below says: one question per file or conflict. Each question names the file (its path is the question `id`) and shows the two versions: what `current` has and what `branch` has, in a short excerpt, and what each one means for the code. Offer one option for each version and, when you can see it, one that combines them; recommend one when you can say why. Never ask what git or the code can tell you.
6. **Resolve from the answers.** Apply the chosen option to the file as step 4 says, remove every marker, and mark the file with `git add <path>`. A skipped question means the developer left that file to themselves: leave the file as it is, with its conflicts, and do not ask again.
7. **Check and report.** Run `git diff --name-only --diff-filter=U` again. Report, in domain terms, the files you resolved and how (what was kept from each side), and the files that still have conflicts, because the developer skipped their question or because they could not be resolved. Do not commit: tell the developer to review the changes and commit the merge in git when no conflict is left. Stop.

## Asking

Make **one round only**, with one question per file in conflict that needs the developer, all single-choice (`multi: false`), run with **`weaver question new --input -`** and the JSON request on standard input, in a heredoc.

- Send one complete round as `{"questions":[...]}`. Every question has an `id` that is the path of the file it asks about, `question`, `options` as `{label, description}`, a zero-based `recommended` index, and an explicit `multi` boolean. Two fields are optional: `context` on a question and `details` on an option; when present they must not be blank.
- Keep `question` short: one sentence that reads on its own, naming the file. Put the two versions in `context` (a short excerpt of each, in a code block, with the name of its branch), the tradeoff in a few words in `description`, and the pros and cons of each option in `details`. `question`, `context`, `description` and `details` are Markdown; labels are plain text. Do not supply "Other": Weaver always offers a free-text answer.
- The developer sees only the round (each question, its context, its options and their own answers), never your conversation. Make every question self-contained: it says everything needed to decide, with no references to the conversation or to another question, and no first person (write "Weaver" or "the agent").
- The result is JSON on stdout. With `status: "answered"`, `answers` is keyed by question id: each entry is `{"status":"answered","selected":[<labels>],"text":<optional free text>}` or `{"status":"skipped"}`. The free text guides how you write the resolution. A skipped question is an explicit developer skip: apply no recommendation, leave that file as it is, with its conflicts, say so in the report, and do not ask it again.
- When the round is finished, resolve the files at once, without asking the developer to confirm a summary.
- With `status: "unavailable"`, Weaver cannot show questions: ask in no other way, resolve none of the files that needed an answer, and tell the developer. When the developer says to continue, run the same `weaver question new` again: it is a new round.
- An error `interrupted` or `cancelled` (exit 1) stops this work at once: do not resolve any file from partial answers or recommendations, do not retry or resume the round; tell the developer that the questions were lost. Files already resolved and marked stay as they are.
- An error `invalid-input` (exit 2) means the request was refused before opening a round: correct the field it names and submit the corrected request. A usage error is never unavailability.
- **Wait for `weaver question new` to finish**: it blocks until the developer answers, however long that takes. Run it with bash `timeout: 0`, never a fixed value. Never use a timer, `sleep`, periodic reads of the output or interval checks; never end the turn to be woken up; never treat the command as failed and never run it again.
- Never use omp's own question tool, and never write the questions in the conversation: `weaver question new` is the only way to ask the developer.

## Sub-agents context sharing

When you hand a part of the work to a sub-agent, in whatever way your agent starts sub-agents, you pass it what you have already read that concerns its part, so it does not look for it again and does not read it again.

- **What you pass.** Of what you have already read, only what concerns that sub-agent's part, plus a short common part (the request and the decisions, when there are any): Weaver's texts quoted as they are (the written request with its images put in words, the output of `weaver model show`, `weaver task show`, `weaver drift show`, `weaver proposal show`, the decisions of the interview) and the places in the code already known, one line per reference, ``- `<ref>` — <where in the code>``. Never code, never everything you gathered, and nothing you read only to pass it on.
- **What the sub-agent does.** It does not reread the Weaver texts it received and reads on its own only the ones that are missing. It always reads the code, also at the places it received, always runs the commands that register its work (`weaver task move <id> doing`, `weaver task new`, `weaver drift new`) and loads its skill as without this feature. A received place that no longer matches the code is ignored: it searches by itself. Context sharing never stops a job.
- **What comes back.** Each sub-agent reports where it found the code of what it saw, one line per reference as above; you reuse those lines in your own work and for the sub-agents that follow.
- **Limits.** No sub-agent runs again a read-only `weaver` command for a text it received, and what you pass to each one concerns only its part: never copy into every sub-agent all that you gathered.

In this skill, a sub-agent gets for each file the path, the two versions as `git diff` shows them and what the agent already knows of what each branch changed. It resolves nothing by itself: it reports what it found and the agent decides, because only the agent runs `git add`.

## Command reference

Every `weaver` command, with its verbs and input fields, is in [COMMANDS.md](../weaver-design/COMMANDS.md), generated from the command registry. `weaver <object> <verb> --help` prints the usage, the fields with their types and limits, an example and the output fields.

## PowerShell

The examples use POSIX syntax. In PowerShell, pass JSON with a pipe and `--input -`, for example `Get-Content -Raw round.json | weaver question new --input -`, instead of `<` or a heredoc. On Windows PowerShell 5.1, set `$OutputEncoding` and `[Console]::OutputEncoding` to UTF-8 first, or non-ASCII characters arrive as `?`. Never pass the JSON as the text of `--input`: Windows PowerShell 5.1 strips the inner quotes of arguments to external programs.
