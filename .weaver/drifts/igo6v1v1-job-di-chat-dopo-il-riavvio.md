---
status: proposal-accepted
refs:
- general-chat
- requirements/ui#general-chat
- requirements/data#job
proposal: 0jj3e29s
job: 8ssxwv0i
created: 2026-11-07T11:22:43.982Z
decided: 2026-11-07T11:24:01.927Z
---
# Job di Chat dopo il riavvio

## What happens

La Chat ricorda quali job ha avviato con un flag nel record del job. Dopo un riavvio le righe `Started <job>` e le card delle Proposal tornano in ordine di creazione in fondo alla conversazione riletta, non dove erano comparse.

## What should happen

La conversazione ricordata torna com'era e i Requirements chiedono che la riga `Started` e le card tornino dopo la rilettura. Il record del job però elenca i suoi campi e non ne ha uno per la Chat, e nulla dice dove le righe compaiono: Weaver non salva il testo della conversazione, quindi non conosce la posizione originale.

## Fix options

### Tenere il codice com'è

Il flag resta nel record del job e le righe tornano in fondo, dopo la cronologia. Serve solo che i Requirements lo dicano, con la Proposal allegata. Nessun costo sul codice; dopo un riavvio la posizione delle righe cambia.

### Collocare le righe per istante

Dopo la rilettura ogni riga `Started` va tra i messaggi in base all'istante di creazione del job, se la cronologia riletta porta gli istanti. Mantiene l'ordine originale ma costa codice in più e non vale se gli istanti non ci sono. Il flag nel record serve comunque.

## Where in the code

crates/core/src/jobs/record.rs (campo `by_chat`); src-tauri/src/jobs/registry.rs (`chat_jobs`); src-tauri/src/general/service.rs (ricostruzione delle righe Started)

## Prepared question

```json
{
  "question": {
    "id": "igo6v1v1",
    "question": "Dove devono tornare le righe `Started` della Chat dopo un riavvio?",
    "context": "La Chat deve mostrare di nuovo, dopo un riavvio, la riga `Started <job>` e le card delle Proposal dei job che ha avviato. Per ricordare quali job sono suoi, il record del job ha un campo che i Requirements non prevedono, e nulla dice dove le righe compaiono: Weaver non salva il testo della conversazione, quindi non conosce la posizione originale.",
    "options": [
      {
        "label": "Tenere il flag e le righe in fondo",
        "description": "Il flag resta nel record del job e le righe tornano in fondo, dopo la cronologia. Nessun costo sul codice; la posizione delle righe cambia dopo un riavvio."
      },
      {
        "label": "Collocare le righe per istante",
        "description": "Ogni riga `Started` va tra i messaggi in base all'istante di creazione del job, se la cronologia riletta porta gli istanti. Mantiene l'ordine originale ma costa codice in più; il flag nel record serve comunque."
      },
      {
        "label": "Adottare la Proposal",
        "description": "I Requirements dicono che il record del job ha il flag `byChat` e che dopo un riavvio le righe tornano in ordine di creazione dopo la cronologia riletta."
      }
    ],
    "recommended": 2,
    "multi": false
  },
  "decisions": [
    {
      "decision": "fix-code",
      "option": 1
    },
    {
      "decision": "fix-code",
      "option": 2
    },
    {
      "decision": "proposal"
    }
  ],
  "fingerprint": "sha256:6693c3ef16c44bf36fb249a115e43309cd95b9f1c19ad2a6cbf29c9344278d95",
  "prepared": "2026-11-07T11:23:00.642Z"
}
```
