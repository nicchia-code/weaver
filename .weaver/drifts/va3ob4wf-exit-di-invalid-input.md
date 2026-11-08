---
status: proposal-accepted
refs:
- requirements/cli#question-round
- requirements/tech#question-round
proposal: mqlcdh4r
job: v8j5o6fi
created: 2026-11-07T20:55:09.640Z
decided: 2026-11-07T21:01:58.391Z
---
# Exit di invalid-input

## What happens

Il codice e il test di contratto della CLI terminano con **exit 2** quando `weaver question new` rifiuta la richiesta con `invalid-input`, come dice il Requirement della CLI.

## What should happen

Il Requirement della CLI dice **exit 2** per `invalid-input`; il Requirement tecnico dei round di domande dice **exit 1**. Nessuna scelta del codice rispetta entrambi, quindi a decidere deve essere una correzione dei Requirement.

## Where in the code

crates/cli/src/commands/question.rs, crates/cli/src/round.rs, tests/cli-contract/contract.test.mjs

## Prepared question

```json
{
  "question": {
    "id": "va3ob4wf",
    "question": "Quale codice di uscita deve avere `invalid-input` di `weaver question new`?",
    "context": "Due Requirement dicono cose diverse sullo stesso esito: quello della CLI dice exit 2, quello tecnico dei round di domande dice exit 1. La Proposal allegata corregge il Requirement tecnico portandolo a exit 2, che è già ciò che fanno il codice e il test di contratto della CLI.",
    "options": [
      {
        "label": "Adotta la Proposal",
        "description": "Il Requirement tecnico passa a exit 2 e i due Requirement tornano d'accordo; il codice non cambia."
      },
      {
        "label": "Chiudi senza cambiare",
        "description": "Nessuna correzione dei Requirement: la contraddizione tra exit 1 ed exit 2 resta scritta."
      }
    ],
    "recommended": 0,
    "multi": false
  },
  "decisions": [
    {
      "decision": "proposal"
    },
    {
      "decision": "dismissed"
    }
  ],
  "fingerprint": "sha256:68da2f122a372c92f3f8615297cf9ebb21132bc8bd3af2bda0ac2a4f09b1df36",
  "prepared": "2026-11-07T20:55:40.791Z"
}
```
