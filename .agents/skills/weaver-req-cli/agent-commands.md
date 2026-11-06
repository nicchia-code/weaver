# Agent commands

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
- `invalid-input` (exit 2) names question and field. `interrupted` (exit 1): the job ended, the window closed, or the connection was lost after submission (reported at once). `cancelled` (exit 1): SIGINT/SIGTERM (Ctrl+C on Windows) or stdout closed; the round is withdrawn. Stop, Clear and an exit of Chat's omp end the command this way.
- `unavailable`: neither in a working Weaver job nor in the project's Chat during a turn, or no app answers before submission. Reason: "Weaver cannot show questions for this command. Do not ask in any other way: stop and tell the developer."
- `--help` says: texts are self-contained, since the developer sees only the round, with no first person and no "Other" option (a free answer is always offered); run it with no time limit; in Chat, only when the developer asked to be asked with a round.

## job

- `announcement new {kind, title}`: `kind` `proposal`, `task` or `drift`; `title` one line, ≤ 60 characters. Prints the `id` to pass as `announcement` when creating the card; an unknown or foreign one still creates the card, with a `note`.
- `job edit {note}`: line breaks become spaces, ends trimmed; empty → exit 2; over 120 characters cut to 119 plus `…`, said in the output. The last 6 notes are kept with their instants; a repeat of the last only renews its instant.
- `job show` without id reads only the environment and the job's registry record (`id`, `kind`, `target`, `merged` for a scan after a merge, `run` from `WEAVER_RUN`, `task` from `WEAVER_TASK`); outside a job it says so, exit 0. `job show <id>` and `job list {all?}` ask the app; `job show <id>` adds `measure`, the job's line of `requirements/data#job` once written (text: durations, then one line per part). No command starts, aborts or shows a merge.
- `job new {kind, request?, task?, drift?}`: `design` with `request` (written, unsent, not empty), `scan` (whole System), `run` (one `task`, or Run all), `resolve` with `drift`. Accepted only from General's agent (`WEAVER_GENERAL`, recognised by the app as this project's working General) or from an Ask more agent (`WEAVER_ASK`, recognised by the app as a working Ask more conversation of this project), with the developer's refusals; its `--help` says: only when the developer explicitly asked for that job.

## agent

- `WEAVER_JOB` is the job identity Weaver sets on each job's omp; the app accepts it only for a job of that project that is Running or Waiting for the developer.
- `WEAVER_GENERAL` is the identity Weaver sets on Chat's omp; the app recognises it only as the live Chat of that project, and for `question new` only while a turn of Chat is in progress. An identity the app does not recognise counts as no identity.
- `announcement new`, `job edit` and `task move` are refused (exit 1, nothing written) for any caller but a job, saying only the agent of a Weaver job can do it; `question new` answers `unavailable` for any caller but a job or Chat; `job new` needs General's or an Ask more conversation's identity.
- `proposal new`, `task new` and `drift new` work without `WEAVER_JOB` and record it as `job` when set. `proposal new` from Chat records `chat: true`, and `proposal new` and `proposal edit` from Chat mark the Proposal as written by Chat (`requirements/data#proposal`).
- Decision commands (`proposal move approved|rejected|discarded`, `drift move`, `task delete`) accept any caller and count as the developer's Events with the same conditions. Skills, Chat's instruction and `--help` allow the agent to run them only when the developer explicitly asked for that decision in the conversation. Choosing a Proposal's model and answering a prepared question exist only in the app.
