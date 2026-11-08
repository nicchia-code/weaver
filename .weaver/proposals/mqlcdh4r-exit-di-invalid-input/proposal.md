---
status: discarded
kind: change
base:
  requirements/tech/architecture.md: f2a5bc026bd98f63346820b906199d556c5e15e5821b2ac51bb74757d6d7d383
created: 2026-11-07T20:54:38.903Z
label: question CLI
job: v8j5o6fi
---
# Exit di invalid-input

I Requirement `requirements/cli#question-round` e `requirements/tech#question-round` danno due codici di uscita diversi allo stesso esito `invalid-input` di `weaver question new`: 2 nel primo, 1 nel secondo. Il codice e il test di contratto della CLI usano già 2. La Proposal allinea il Requirement tecnico a 2 e non cambia nient'altro.

## Already present

- `requirements/tech#question-round` — `crates/cli/src/round.rs` e `crates/cli/src/commands/question.rs` terminano già con exit 2 per `invalid-input`
