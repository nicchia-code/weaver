---
status: fixed
refs:
- requirements/ui#window
previous: 5bw3q7z9
proposal: 1q8f5nwy
task: 7lpa4uby
option: 1
job: q61cbo0p
created: 2026-11-16T15:47:56.541Z
decided: 2026-11-16T19:25:32.454Z
---
# Provider nascosti con errore

## What happens

Con omp **non collegato a nessun provider** e un messaggio di errore di apertura mostrato nella finestra di benvenuto (640 × 480), l'**elenco dei provider si riduce a 0 px**: scompare del tutto e non si può fare il login.

Intestazione, errore, almeno due righe di recenti e riga di stato di omp restano in vista; il pannello non scorre. Senza il messaggio di errore l'elenco dei provider è alto circa 52 px e scorre da solo.

## What should happen

La finestra di benvenuto tiene sempre in vista intestazione, `Alert` e riga di stato di omp con la sua configurazione, e scorre solo la lista dei recenti. Con omp senza provider l'elenco dei provider fa parte di quella configurazione e deve restare raggiungibile, anche con l'`Alert` di errore mostrato. Nello spazio di 480 px le due richieste non valgono insieme.

## Fix options

### Recenti ridotti col messaggio

Con un messaggio di errore mostrato la lista dei recenti cede spazio e scende a una sola riga, così l'elenco dei provider mantiene un'altezza utilizzabile. Il minimo di due righe vale solo senza messaggio. Cambia solo il layout, nessun comportamento.

### Messaggio di errore compatto

Nella finestra di benvenuto il messaggio di errore occupa una sola riga, con il testo completo in un suggerimento al passaggio del mouse. Lo spazio recuperato resta all'elenco dei provider. Il testo dell'errore si legge meno facilmente.

### Pannello che scorre come ripiego

Solo quando elenco dei provider e messaggio di errore non entrano insieme, scorre l'intero pannello. Tutto è raggiungibile, ma intestazione e stato di omp possono uscire dalla vista in quel caso.

## Where in the code

ui/src/components/welcome-window.tsx, `WelcomeWindow` (`Alert` di errore, `section` dei recenti con altezza minima); `OmpSetup` con `providers="when-needed"` in ui/src/components/omp-status.tsx

## Prepared question

```json
{
  "question": {
    "id": "r6iybmoi",
    "question": "Come deve comportarsi la finestra di benvenuto quando omp non ha provider e compare un errore di apertura?",
    "context": "Nella finestra di benvenuto (640 × 480, non ridimensionabile) con omp senza provider collegato, l'elenco dei provider scorre in una propria area, alta circa 52 px. Se compare anche un messaggio di errore di apertura, l'area dei provider scende a 0 px e il login non è raggiungibile. Il messaggio sparisce alla prossima azione. Drift precedente: `5bw3q7z9` \"Welcome senza provider omp\", risolto con fix-code (provider con scroll proprio).",
    "options": [
      {
        "label": "Recenti ridotti col messaggio",
        "description": "Con il messaggio mostrato i recenti scendono a una riga e i provider restano utilizzabili."
      },
      {
        "label": "Messaggio di errore compatto",
        "description": "Il messaggio occupa una sola riga, con il testo completo nel suggerimento."
      },
      {
        "label": "Pannello che scorre come ripiego",
        "description": "Se non entra tutto scorre l'intero pannello; intestazione e stato di omp possono uscire dalla vista."
      },
      {
        "label": "Provider nascosti col messaggio",
        "description": "Con il messaggio mostrato l'elenco dei provider può sparire finché il messaggio non viene chiuso; Requirements aggiornati di conseguenza."
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
      "decision": "proposal"
    }
  ],
  "fingerprint": "sha256:d1435771e051c5c50dff01fc905c10d530381d21d304c4612707f5e6228fcbcd",
  "prepared": "2026-11-16T15:48:28.661Z"
}
```
