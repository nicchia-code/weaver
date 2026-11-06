---
name: weaver-req-cli
description: "Use when building or changing the `weaver` command line: commands, inputs, outputs, help and exit codes."
---

`weaver` is the command built from the CLI crate, as the binary `weaver-cli`, on top of the same Rust core as the app. It ships inside the app bundle as `weaver-cli`; Weaver copies it as `weaver` into the folder it puts first on the PATH of every omp it starts, and that is how the agent reaches Weaver. It also works without the app for every command not bound to a job.

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

## Agent commands

Use when changing the `weaver` commands of agents in jobs, Chat and Ask more: question rounds, announcements, notes, job starts and who may run what.

Sections: `question-round`, `job`, `agent`

File: `agent-commands.md`

## weaver commands

Use when adding or changing a `weaver` command, its input, output, help or its link with the app.

Sections: `General`, `project`, `proposal`, `task`, `drift`

File: `commands.md`

## weaver history commands

Use when adding or changing the `weaver history` commands, their input with the files and message of a commit, or their output.

Sections: `history-entry`

File: `history.md`
