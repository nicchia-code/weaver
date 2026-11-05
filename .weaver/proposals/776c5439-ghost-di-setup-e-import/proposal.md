---
status: approved
kind: change
base:
  model/agent-work/job.md: 0cd9b80ab64b7d930d2ec1d1cbc501fc0895b03c61314b018dbad65bf78b18d0
  requirements/data/settings.md: 149dfa26576234b92caf18010f90db90a275b45ac4a6f9353206c9518575a208
  requirements/ui/board.md: c9d517c79481725badd560cf70eafde07e4d0aceaaadf1b8c48e7accdb5e66b2
created: 2026-11-05T14:12:16.471Z
request: 2026-11-05T15-01-27
label: Ghost setup/import
job: keep6f0p
---
# Ghost di setup e import

# Ghost di setup e import

Mentre lavora un job di setup (Start from Scratch) o di import (Import from Code), oggi la Board non mostra nulla finché l'agente non annuncia una card: nell'import questo avviene solo dopo la lettura del codice e il giro di domande, mentre le card di Empty Model sono già nascoste. Questa Proposal fa mostrare a Weaver un ghost proprio del job quando il job non ha nessuna card annunciata in attesa.

## Decisioni

- Il ghost di Weaver copre tutta la vita del job: dalla coda (Queued) alla fine, anche mentre il job è sospeso.
- Tipo `Proposal` in Backlog; titolo `Setup` per un setup, `Import from Code` per un import.
- Riga di fase: `Waiting for your answers` mentre il job aspetta un giro di domande o un permesso, altrimenti `Preparing` (anche in coda e da sospeso); sotto, l'ultima nota del job.
- Lascia il posto ai ghost annunciati nella stessa picture in cui compare il primo; ricompare quando nessun ghost annunciato del job resta, finché il job non finisce (per esempio mentre l'import prepara le domande dei Drift).
- Tra i ghost sta nell'ordine di avvio del job, come se fosse stato annunciato quando il job è stato creato.
- Come ogni ghost, è un pulsante che apre la conversazione del job in Jobs e non conta nel numero di card.
- In questa Proposal Weaver ricorda con il job, anche dopo un riavvio, quali card annunciate sono già arrivate: oggi lo tiene solo in memoria e dopo un riavvio i ghost di card già arrivate ricompaiono.

## Assunzioni

- Il record del job conserva, per ogni annuncio, la card creata con esso (tipo e identità): è il modo in cui Weaver lo ricorda dopo un riavvio.
- Il ghost di Weaver non è un annuncio: non viene salvato nel record e ha un'identità derivata dal job, stabile tra una picture e l'altra.

## Raffinamento

- Riscritti `job`, `requirements/data#job` e `requirements/ui/board.md` sopra il Model attuale: restano tutte le modifiche del merge in Solo mode approvate nel frattempo (job di merge conflicts, rifiuti durante un merge, scan dopo un merge, campo `merged` del record, sezione `solo-merge`, pulsante `Merge` e motivi `A merge is in progress`).
- Il ghost di Weaver resta solo per i job di setup e di import; il nuovo job di merge conflicts non lo riceve (assunzione: la decisione originale nomina solo setup e import).
- La sezione `requirements/ui#task` non è più toccata: il cambiamento risultava solo dal file obsoleto.
