---
status: proposal-accepted
refs:
- requirements/ui#job
- requirements/ui#ask-conversation
- requirements/quality#General
proposal: obpor0ap
job: 8ssxwv0i
created: 2026-11-07T11:22:20.837Z
decided: 2026-11-07T11:23:49.586Z
---
# Testo di omp contraddittorio

## What happens

Il codice segue la regola di qualità: la card di permesso dice `Weaver asks permission`, il job in coda e il box messaggi dicono `Starting Weaver…` e, con l'agente non pronto, `Waiting for the agent to be ready.`. Nessun testo fuori dalle Settings nomina omp.

## What should happen

Due Requirements si contraddicono. La regola di qualità dice che `omp` compare solo nelle Settings e porta proprio `Weaver asks permission` come esempio. Le sezioni di conversazioni e di Ask more chiedono invece `omp asks permission`, `Starting omp…` e `Waiting for omp to be ready.`. Nessuna scelta del codice rispetta entrambi.

## Where in the code

ui/src/components/jobs/jobs-view.tsx (card di permesso, job in coda); ui/src/components/ask/ask-conversation.tsx (box messaggi)

## Prepared question

```json
{
  "question": {
    "id": "cefti8dz",
    "question": "Quale testo deve vedere lo sviluppatore per permessi e avvio degli agenti?",
    "context": "Due Requirements si contraddicono. La regola di qualità dice che il nome `omp` compare solo nelle Settings e porta come esempio `Weaver asks permission`. Le sezioni sulle conversazioni dei job e su Ask more chiedono invece `omp asks permission`, `Starting omp…` e `Waiting for omp to be ready.`. Nessuna scelta del codice rispetta entrambi.",
    "options": [
      {
        "label": "Adottare la Proposal",
        "description": "I Requirements delle conversazioni e di Ask more passano a `Weaver asks permission`, `Starting Weaver…` e `Waiting for the agent to be ready.`, come dice la regola di qualità."
      },
      {
        "label": "Chiudere senza cambiare",
        "description": "Nessun Requirement cambia e la contraddizione resta com'è."
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
  "fingerprint": "sha256:02d0a22069c06756f317fbcd762e5aff7813db7b80dd2eb6249442d2b3b9fffb",
  "prepared": "2026-11-07T11:23:00.571Z"
}
```
