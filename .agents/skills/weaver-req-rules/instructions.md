# Agent instructions

## job

- The instruction is one line, in English, built only from fixed text, permanent identities, project-relative paths in backticks, numbers and the language name. It never contains a short number or the text of a request.
- First sentence by job:

  | Job | Sentence |
  |---|---|
  | Setup | ``Use the weaver-setup skill on the setup request in `.weaver/requests/<request>.md`.`` |
  | Design | ``Use the weaver-design skill on the design request in `.weaver/requests/<request>.md`.`` |
  | Import | ``Use the weaver-import skill on this repository.`` then ``Do not import these paths and do not ask which folders to import: `a`, `b/c`.`` or `Import every folder and do not ask which folders to import.` |
  | Scan | `Use the weaver-scan skill on the whole System.` After a merge: ``Use the weaver-scan skill on the whole System after a merge: read the files the merge brought with `weaver job show`.`` |
  | Run of one Task | `Use the weaver-implement skill on Task <id>.` |
  | Run all | ``Use the weaver-implement skill on the runnable Tasks: take them from `weaver task list --input '{"runnable":true,"max":<N>,"groups":true}'`, one batch at a time, until it says there is none.`` A Run of a model group adds `"proposals":[…]` (the Proposals of its chosen model) or, for the execution-model group, `"notProposals":[…]`, identities in card order. |
  | Resolve | `Use the weaver-resolve-drifts skill on Drift <id>.` With an answer already given: `The developer already answered its prepared question: <answer>.`, the answer as one-line JSON `{"selected":[…],"text":…}`. |
  | Rework | `Use the <skill> skill to rework the rejected Proposal <id> with the developer's feedback.` The skill follows the kind: `weaver-design` for a change, `weaver-setup` for a setup while the Model is empty (`weaver-design` otherwise), `weaver-import`, or `weaver-resolve-drifts` adding ``on the Drifts `<d1>`, `<d2>` `` with its Drifts still open. |
  | Refinement | `Use the <skill> skill to refine the stale Proposal <id> on the current Model and Requirements.` The skill follows the kind as for a Rework; `weaver-resolve-drifts` ends the sentence with ``, on the Drifts `<d1>`, `<d2>`.`` naming its Drifts still open. Every skill has a section for refining a stale Proposal of its kind, with the rules of the design refinement (no code read, no announcement, decisions in the body already taken, at most one round and only on a contradicted decision, `Remove`/`Keep` when the whole change is already in the Model); it adds that an attached Proposal goes back Open, and that the agent asks the developer when something of a requirement dropped by the rebase was needed. |
  | Merge conflicts | ``Use the weaver-resolve-conflicts skill on the merge in progress recorded in `.weaver/merge/merge.json`.`` |

- Setup and Design: right after the first sentence, on the same line, one ``Read the file `.weaver/assets/<name>`.`` for each asset the request tags when it is sent (`requirements/rules#assets`), before the sentences below.

- Sentences appended on the same line, in this order:
  1. `Run at most <N> sub-agents at once.`, with N the sub-agent limit setting (for a Run all group, its share; the same number is the `max` of `weaver task list`).
  2. `Pass each sub-agent what you have already read that concerns its part, as the "Sub-agents context sharing" section of your skill says.`
  3. ``Ask the developer only with `weaver question new`, as the skill says, never with your own question tool; run it with no time limit and wait until it ends.``
  4. Only when a language is set: `Write the texts for the developer in <language>, as the Language section of your skill says.`
- Always-on features (advisor, prepared Drift questions, parallel slots) add no sentence: the skills always do them.
- Bindings are enforced by the command line, not by the instruction, as `requirements/rules#proposal` (**Bindings**) says: a job binds only the request its instruction named, and only its first Proposal; only a rework job may bind with `reworks`.

## ask-conversation

- The instruction of an Ask more conversation is a fixed text in English carried by the app, starting with the line `Weaver: Ask more`, followed by the part that describes the chosen thing, read from the files when the conversation opens (and again when a lost session is replaced). Unlike `## job`, it spans several lines and holds texts of cards. It names no skill, except the Requirement skills of the **Requirements** part, and never copies text of the Requirements.
- Added sentence: only, when a language is set, `Write the texts for the developer in <language>.` None of the sentences of `## job` (sub-agent limit, context sharing, question rounds).
- The fixed text asks the agent to: read with the listed commands what it needs; answer the developer's message and explain; change a Task, a Drift or a Proposal only when the developer asks, and only with the `weaver` commands that allow it; never change the Model or the Requirements, which change only through an approved Proposal; never write by hand in files under `.weaver/`; start a job of Weaver only on the developer's explicit request, as in Chat; ask the developer only in its replies.
- Identities: only permanent identities, except the short number of the chosen card and of the overlay Proposal, each stated as the short number the developer sees.
- **Task:** permanent identity, short number, title, full body, references, paths of its images, and, when present, the Proposal whose design it belongs to (identity and title) and the Drift it fixes (identity).
- **Proposal:** permanent identity, short number, title, state, full body, its Tasks (identity and title) and, for a resolution, each Drift it resolves with its decision.
- **Drift:** permanent identity, short number, title, `What happens` and `What should happen` in full, references, and, when present, the attached Proposal, the Task it was born from and the Drift it repeats.
- **History entry:** the entry's identity, title, summary and full text; then every Task it cites, in the order cited, from its copy in the entry: identity, title, body in full, references, Proposal, label and finished time. A cited Task without a copy is read from its file, as the History view shows it; one that cannot be read is listed by its identity alone, marked as not available.
- **Question:** the question in view in full (text, context, every option with label, description and details, the recommended option, whether several can be chosen), then the text and options (label and description) of the other questions of the round in their order, marking the one in view. Then the name and identity of the job that asked the round, `Chat` for a round of Chat or, for a prepared question, the Drift's identity. It says the round is still open and the agent must not answer for the developer. Never the developer's choices, written text, answers or skips.
- **Requirements:** says the developer asks about the project's Requirements as a whole; that they are in the Requirement skills `weaver-req-*` installed in the project, which hold the current Requirements; and that the agent loads by itself the ones that fit the developer's message, as many as it needs. It lists no skill, names no Area, fragment or section and copies no Requirement text; the focus and `Only these` of the view never reach it. With an overlay Proposal still open when the instruction is built: the Proposal's identity, short number and title, saying that the view shows the Requirements with that Proposal applied and that its changes are read with `weaver proposal show <id>`, not in the skills. An overlay Proposal that is no longer open is left out, and the part speaks of the current Requirements only.
- **Read commands**, listed after each part:
  - cards: `weaver task show <id>`, `weaver proposal show <id>` or `weaver drift show <id>` for the chosen card and each linked card, and `weaver model show <ref>` for each reference;
  - History entry: `weaver history show <entry>`, then `weaver proposal show <id>` once for each Proposal of its Tasks that still exists and `weaver model show <ref>` once for each reference of its Tasks, without repetitions;
  - question: `weaver drift show <id>` for a prepared question;
  - Requirements: only with an overlay, `weaver proposal show <id>`; without one, no command is listed.

## assets

- **Tag:** `@` at the start of the text or after whitespace, followed by the name of an asset (`requirements/rules#asset-view`), compared exactly, case included. When several asset names fit, the longest wins, so names with spaces need no quoting. The name must be followed by the end of the text, whitespace, or one of `. , ; : ! ? ) ] } " '`. Anything else after `@` is plain text and tells the agent nothing. Tags inside code spans or code blocks count like any other.
- **When:** tags are recognised against the assets that exist at the moment the developer sends, never later: a message waiting to be delivered keeps the lines it got when sent.
- **Lines:** each asset tagged gets one sentence ``Read the file `.weaver/assets/<name>`.``, once even when tagged several times, in the order of its first tag.
- **Messages** (Chat, a job's conversation, an Ask more conversation): when at least one asset is tagged, Weaver appends to the text sent to omp, after the developer's text, an empty line, the line `Assets tagged by the developer (added by Weaver, not written by the developer):` and the sentences, one per line. In the first message of an Ask more conversation the block follows the developer's message, after the instruction. Without tags the text goes as it is.
- **Requests:** the sentences go into the job's instruction as `## job` says; the request file is never changed.
- **Shown:** the developer's message is always shown without the appended block, live and when the conversation is reloaded from omp.
