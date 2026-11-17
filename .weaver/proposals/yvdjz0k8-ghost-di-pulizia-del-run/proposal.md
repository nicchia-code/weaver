---
status: approved
kind: change
base:
  model/agent-work/job.md: 0f6b8d920fb6b3adefafd8630b011d521b77b30be95da95b9b0859b4e42713a6
  requirements/data/settings.md: d7db902bcba23892bfb55f6256abeb20069a029a5330cb5e1a1e9904c11b389d
  requirements/ui/board-cards.md: 9d65f6735022b252ec7df480171c7f2586d92d2574f80ea7945021294a5d379f
  requirements/ui/board.md: 518903e80db57370ba012cb34dfcf685bd670c4168baa09bd4b89039d6bd7782
created: 2026-11-09T07:25:18.753Z
request: 2026-11-09T08-13-31
label: Ghost di pulizia
job: xkc1qvvk
---
# Ghost di pulizia del Run

Quando un Run ha finito i suoi Task ma il job lavora ancora (controlli finali, voce di History, domande sui Drift, revisione dell'advisor), la colonna Active mostra `Nothing active` e lo sviluppatore non vede che l'agente sta ancora lavorando. Weaver mostra allora in Active un proprio ghost di pulizia del Run.

Decisioni dell'intervista:
- Solo i job Run (Run di un Task e ogni Run di Run all) hanno il ghost di pulizia.
- Compare quando il Run ha già preso almeno un Task e non ne tiene più nessuno; sparisce se ne prende un altro (in Run all può comparire brevemente tra un gruppo di Task e il successivo).
- Compare solo quando Active non ha card reali e nessun ghost annunciato; i ghost di pulizia di più Run non si escludono tra loro.
- Un ghost per Run: ogni gruppo di Run all ha il proprio.
- Si nasconde finché un ghost annunciato dal suo stesso job è sulla Board, come il ghost proprio di un design.
- Resta mentre il Run è sospeso; il fatto che il Run abbia preso un Task è registrato nel record locale del job, così sopravvive al riavvio di Weaver.
- Aspetto di un ghost di Task (tipo `Task`, segnaposti di un Task).
- Titolo `Cleaning up: <nome del job>` (per esempio `Cleaning up: Run T3`).
- Riga di fase `Finishing up`, e `Waiting for your answers` mentre il job attende lo sviluppatore.
- Nome accessibile proprio: `Agent cleaning up after its Tasks: <nome del job>, <fase>`.
- Posizione tra i ghost di Active come se fosse stato annunciato alla creazione del job.

Assunzioni dell'agente (da verificare):
- Il ghost sparisce alla fine del job, per qualunque motivo, nella stessa immagine degli altri ghost del job, e come ogni ghost apre la conversazione del job e non conta mai nei numeri.
- Il frammento `board` dei Requirements `ui` supererebbe 12 KB: la sezione `task` passa invariata nel frammento `board-cards`, che raccoglie ora le card di Task, Proposal e Drift; le descrizioni dei due frammenti sono aggiornate.
- Il ghost compare nella stessa immagine in cui l'ultimo Task tenuto dal Run lascia Active, così `Nothing active` non si vede mai nel mezzo.
