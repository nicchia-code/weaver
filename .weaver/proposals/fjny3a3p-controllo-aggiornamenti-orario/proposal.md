---
status: approved
kind: change
base:
  requirements/release/pipeline.md: abff613cbfda99643a0060d5609d6b01a34c1a70b233b2ae9ddc7f1d6575b529
created: 2026-11-07T20:00:45.538Z
request: 2026-11-07T20-58-22
label: Aggiornamenti ogni ora
job: 2svugz6n
---
# Controllo aggiornamenti orario

Oggi Weaver controlla se esiste una sua nuova versione all'avvio e poi al massimo una volta ogni 24 ore mentre è aperto: una nuova versione può restare invisibile per quasi un giorno. Il controllo periodico passa a un'ora.

## Decisioni

- Intervallo fisso di 1 ora, senza nessuna impostazione nelle Settings; il controllo all'avvio resta e l'istante dell'ultimo controllo continua a essere salvato nelle settings.
- Later resta com'è: nasconde l'aggiornamento trovato fino al controllo successivo, quindi riappare dopo al massimo un'ora.
- Un controllo fallito continua a essere solo scritto nel log e ritentato alla scadenza successiva, ora entro un'ora.
- Il controllo degli aggiornamenti di omp è separato, già orario, e non cambia.
