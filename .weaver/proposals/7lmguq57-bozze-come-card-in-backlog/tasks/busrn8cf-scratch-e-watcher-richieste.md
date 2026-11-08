---
refs:
  - project
  - requirements/ui#project
  - requirements/tech#project
technical: false
---
# Start from Scratch e watcher

Le richieste di design non ancora inviate diventano card nella colonna Backlog della Board; le nuove richieste nascono solo dal pulsante `+` della Backlog (`New Request`) e da `Start from Scratch`. Questo Task copre il comportamento di `Start from Scratch` e il modo in cui le modifiche ai file delle richieste aggiornano la Board.

## Start from Scratch

Con il Modello vuoto, la card `Start from Scratch` apre nell'editor delle richieste la richiesta in scrittura (stato Writing) più vecchia, quando ce n'è almeno una; in quel caso non ne viene creata nessuna. Solo se non c'è alcuna richiesta in scrittura viene creata una nuova richiesta di setup, vuota, e aperta per la scrittura. "Più vecchia" segue l'ordine di creazione delle richieste (lo stesso con cui le bozze sono ordinate in fondo alla Backlog, dalla più vecchia). Il pulsante `+` della Backlog (`New Request`) invece crea sempre una richiesta nuova, anche quando ne esistono già in scrittura. Nel Modello della Design request è la Transizione `developer.NEW_REQUEST` → Writing: si crea una richiesta vuota aperta per la scrittura, tranne quando lo sviluppatore parte da zero mentre una richiesta è già in scrittura, nel qual caso si apre la più vecchia.

Il resto della schermata del Modello vuoto (testi delle due card, avviso sui file già presenti nella cartella, card nascoste mentre lavora un job di setup o di import o è aperta una Proposal, riquadro di stato di omp) resta com'è.

## Watcher e richieste

Le modifiche ai file sotto la cartella delle richieste di `.weaver/`, comprese quelle fatte fuori da Weaver (altri editor, CLI, agente), aggiornano le bozze della Board: toccano solo la parte `board` della fotografia del progetto, con l'avviso di progetto cambiato, e nient'altro (né Modello, né Requirement, né altro). La cancellazione del file di una richiesta resta l'evento `workspace.REQUEST_DELETED`, come oggi (una richiesta inviata il cui file sparisce viene dimenticata e l'editor viene avvisato).

La Board va toccata solo quando una richiesta compare, sparisce, oppure cambia il suo titolo o il fatto di essere vuota; una modifica che non cambia nessuna di queste cose non tocca nulla. L'editor salva circa ogni 500 ms, quindi toccare la Board a ogni salvataggio va evitato.

Gli eventi sotto `assets/` continuano ad aggiornare solo la lista degli asset e quelli sotto `notes/` raggiungono solo la vista Notes: né l'una né l'altra cartella tocca mai la fotografia. Le altre cartelle locali di `.weaver/` restano ignorate, come i file temporanei delle scritture atomiche.

## Code notes
Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Start from Scratch: `ui/src/components/project/empty-model.tsx` (~61-69), `Paths` chiama `useRequests().newRequest`. Stato richieste in `ui/src/components/requests/use-requests.tsx` (`newRequest`, `openEditor`). Backend `request_new` in `src-tauri/src/requests.rs`; l'ordine di creazione viene da `list()` dello store con `id::compare` (`crates/core/src/project/requests/store.rs`). Il `+` della Backlog deve continuare a creare sempre. Test: `ui/src/components/project/empty-model.test.tsx`.
- Watcher: `src-tauri/src/requests.rs` `files_changed`/`touches_requests` (deletion → REQUEST_DELETED, evento `requests-changed`), chiamato da `windows/tauri_shell.rs`. Per toccare la Board riusare il pattern di `board.rs` `publish_agent_change` (`pictures.touch(id, &[SLICE_BOARD])` + `emit_project_changed`).
- `pictures.rs`: la mappa cartella→slice non deve includere `requests/` (la Board si tocca da `requests.rs`, solo su cambio di presenza/titolo/vuotezza); il test `slices_follow_the_folder…` che asserisce che `requests/` non tocca slice va aggiornato coerentemente.
- Insidia: per confrontare titolo/vuotezza serve ricordare per progetto l'ultimo stato noto delle richieste, altrimenti ogni salvataggio esterno ripubblica la Board.
