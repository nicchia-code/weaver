---
status: fixed
refs:
- requirements/ui#window
task: 38fs730d
option: 1
job: 4uyxt1ts
created: 2026-11-16T15:29:33.759Z
decided: 2026-11-16T15:43:04.107Z
---
# Welcome senza provider omp

## What happens

Con omp **non collegato a nessun provider** la finestra di benvenuto mostra, sotto l'avviso, l'elenco dei provider (7 righe, circa 400 px). Insieme a intestazione, titolo e avviso supera i circa 416 px utili della finestra 640 × 480, non ridimensionabile.

La lista dei progetti recenti si riduce a 0 px, quindi sparisce, e il pannello scorre per intero: la configurazione di omp esce dalla vista.

## What should happen

La finestra misura 640 × 480 px e non è ridimensionabile. Il Task T1 chiede che scorra **solo** la lista dei progetti recenti e che intestazione, titolo, `Alert` e riga di stato di omp con la sua configurazione restano sempre visibili. Con l'elenco dei provider mostrato le due richieste non possono valere insieme.

## Fix options

### Provider con scroll proprio

L'elenco dei provider ha una propria area che scorre nella finestra di benvenuto, con almeno due righe di recenti sempre visibili. Tutto resta in vista, ma il login scorre in un'area molto piccola. Cambia solo il layout, nessun comportamento.

### Scroll del pannello come ripiego

Solo in questo stato, finché omp non ha provider, scorre l'intero pannello e la lista dei recenti ha un minimo di due righe. Il login resta raggiungibile, ma la riga di stato può uscire dalla vista. È ciò che il codice fa oggi, senza il minimo.

### Provider in forma compatta

Nella finestra di benvenuto i provider appaiono in una forma più compatta, per esempio due colonne di righe basse. Occupano circa la metà dello spazio e permettono lista e stato insieme. Cambia l'aspetto dei provider solo lì.

## Where in the code

ui/src/components/welcome-window.tsx, `WelcomeWindow` (`main` e `section` dei recenti); `OmpSetup` con `providers="when-needed"` in ui/src/components/omp-status.tsx

## Prepared question

```json
{
  "question": {
    "id": "5bw3q7z9",
    "question": "Come deve comportarsi la finestra di benvenuto quando omp non ha ancora un provider?",
    "context": "La finestra di benvenuto misura 640 × 480 px e non è ridimensionabile. Con omp senza provider mostra anche l'elenco dei 7 provider, circa 400 px, che insieme a intestazione, titolo e avviso supera lo spazio utile (circa 416 px). La lista dei progetti recenti, che deve scorrere da sola senza spingere fuori vista la riga di stato di omp, non trova spazio: oggi si riduce a zero e scorre il pannello.",
    "options": [
      {
        "label": "Provider con scroll proprio",
        "description": "L'elenco dei provider scorre in un'area propria, con almeno due righe di recenti sempre visibili.",
        "details": "Tutto resta in vista, ma il login avviene in un'area piccola. Cambia solo il layout."
      },
      {
        "label": "Scroll del pannello come ripiego",
        "description": "Finché omp non ha provider scorre l'intero pannello e i recenti hanno un minimo di due righe.",
        "details": "Il login resta raggiungibile, ma la riga di stato può uscire dalla vista."
      },
      {
        "label": "Provider in forma compatta",
        "description": "Nella finestra di benvenuto i provider appaiono più compatti, per esempio su due colonne.",
        "details": "Occupano circa metà dello spazio, quindi lista e stato stanno insieme. Cambia l'aspetto dei provider solo lì."
      },
      {
        "label": "Lasciare il comportamento attuale",
        "description": "I recenti si riducono a zero e scorre il pannello, finché omp non ha un provider.",
        "details": "Nessuna modifica: la lista sparisce in quello stato."
      }
    ],
    "recommended": 0,
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
      "decision": "fix-code",
      "option": 3
    },
    {
      "decision": "dismissed"
    }
  ],
  "fingerprint": "sha256:d1435771e051c5c50dff01fc905c10d530381d21d304c4612707f5e6228fcbcd",
  "prepared": "2026-11-16T15:29:47.770Z"
}
```
