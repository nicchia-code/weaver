---
technical: false
refs: [job, requirements/ui#work, requirements/data#job]
---
# Ghost di pulizia del Run

Un Run che ha finito i suoi Task ma lavora ancora non lascia più la colonna Active vuota: Weaver vi mostra un proprio ghost di pulizia del Run.

## Cosa deve valere

- **Run in pulizia:** un job Run (Run di un Task oppure ciascun Run di un Run all) è in pulizia quando ha già preso almeno un Task, non ne tiene nessuno adesso e non è terminato; anche se è sospeso. Gli altri tipi di job non hanno mai un ghost di pulizia.
- **Quando compare:** il ghost di un Run in pulizia è in Active solo se valgono insieme queste condizioni:
  - Active non ha card reali (Task in To do o In progress);
  - Active non ha ghost annunciati di nessun job;
  - nessuno dei ghost annunciati dallo stesso Run è sulla Board, in nessuna colonna.
  
  C'è un ghost per Run: ogni gruppo di un Run all ha il proprio, e i ghost di pulizia di più Run non si nascondono mai tra loro.
- **Aspetto:** tipo `Task`, con lo scheletro e i segnaposti di un ghost di Task. Il titolo è `Cleaning up: <job name>` (per esempio `Cleaning up: Run T3`, `Cleaning up: All tasks · <model>`). La riga di fase è `Waiting for your answers` mentre il job attende un round o un permesso, e `Finishing up` negli altri casi. Sotto c'è l'ultima nota del job, come sugli altri ghost. Il nome accessibile è `Agent cleaning up after its Tasks: <job name>, <phase>`, per esempio `Agent cleaning up after its Tasks: Run T3, Finishing up`. Come ogni ghost, apre la conversazione del job e non conta mai nei numeri della colonna.
- **Ordine:** tra i ghost di Active occupa il posto che avrebbe se fosse stato annunciato alla creazione del job.
- **Comparsa nella stessa immagine:** compare nella stessa immagine della Board in cui l'ultimo Task tenuto dal Run lascia Active (con Task finito o chiuso senza modifiche), così `Nothing active` non si vede mai nel mezzo. Compare anche nella stessa immagine in cui l'ultima card reale o l'ultimo ghost annunciato lascia Active, o in cui l'ultimo ghost annunciato dal Run lascia la Board.
- **Scomparsa nella stessa immagine:** sparisce nella stessa immagine in cui il Run prende un Task, in cui una card reale o un ghost annunciato entra in Active, o in cui compare un ghost annunciato dal Run. Sparisce anche quando il job termina, per qualunque motivo, insieme agli altri ghost del job. Tra due gruppi di Task di un Run all può comparire per poco.
- **Identità:** l'identità del ghost è derivata dal job, resta la stessa da un'immagine all'altra, non coincide mai con quella di un annuncio e non è mai salvata nel record del job.
- **Ricordo del Task preso:** la prima volta che un Run prende un Task, Weaver scrive `tookTask: true` nel record locale del job e non lo toglie mai più. Così, dopo un riavvio di Weaver, il ghost di un Run sospeso in pulizia torna. Un Run che non ha ancora preso nessun Task (per esempio appena avviato) non ha il ghost.

## Test esistenti da aggiornare

- Il test che oggi verifica che i job diversi da setup, design e import non abbiano un ghost proprio deve riflettere la nuova regola. Un Run che non ha ancora preso Task resta senza ghost; un Run in pulizia ne ha uno.
- I test sui ghost annunciati di un Run, che oggi contano un numero preciso di ghost, devono tenere conto del ghost di pulizia, che si aggiunge quando le sue condizioni valgono e manca quando un ghost annunciato del Run è sulla Board.
- Nuovi test devono coprire titolo, fase, nome accessibile, segnaposti, ordine, comparsa e scomparsa nella stessa immagine, sospensione e riavvio, e i ghost di più Run insieme.

## Code notes

Commit: 0480b85d2d4b13e938745f2ab2b4719d656a007e

- Ghost: `src-tauri/src/jobs/registry.rs` `ghosts_of`, `weaver_ghost` (Setup/Design/Import only), `Live` (`order`), `card_is_on_the_board`. Add a Run case with id `job:<id>`, `GhostKind::Task`, a title from `crates/core/src/jobs/names.rs` (next to `IMPORT_GHOST_TITLE`).
- `tookTask`: `crates/core/src/jobs/record.rs`. Nothing hooks `move doing` today: add an `Authority` hook with `caller.job` in `crates/cli/src/exec.rs` (`after_success`), implemented in `src-tauri/src/jobs/authority.rs`; `crates/core/src/project/tasks.rs` `plan_take` writes `holder`.
- Phase: new `GhostPhase` variant in `src-tauri/src/agent_work.rs`; regenerate the ts-rs bindings; `PHASES` in `ui/src/components/board/ghost-card.tsx` is exhaustive. Accessible name is built there too.
- Tests: `src-tauri/src/jobs/tests.rs` (the "other kinds of job get none" assertion, 2-ghost Run counts), `ui/src/components/board/board-view.test.tsx`.
- Pitfalls: `board_snapshot` fetches the picture before `bring_up_to_date`, so cards and ghosts may come from different pictures. `task_finished` runs after the Done write, so state set there gives one picture showing `Nothing active`. Decide "holds no Task" from the picture (`doing` + `holder == job`). The Run all filter is not in the record.
