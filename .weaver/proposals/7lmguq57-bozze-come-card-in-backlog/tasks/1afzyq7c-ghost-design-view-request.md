---
refs:
  - job
  - requirements/ui#job
technical: false
---
# Ghost design e View request

Dopo l'invio di una richiesta, il job che la riceve si vede subito sulla Board anche quando è un job di design, e dalla sua conversazione lo sviluppatore può riaprire la richiesta inviata.

## Ghost di Weaver anche per i job di design

Weaver mostra il proprio ghost non solo per i job di setup e di import, ma anche per i job di design, compresi quelli avviati dall'agente di Chat su richiesta esplicita dello sviluppatore.

- Il ghost compare dal momento in cui il job è creato (in coda) fino alla sua fine, sospensione compresa, ogni volta che nessuno dei ghost annunciati dal job è sulla Board. Non serve alcun annuncio.
- Sta in fondo alla Backlog, dopo le card vere (bozze di richiesta comprese), nel posto che avrebbe se fosse stato annunciato alla creazione del job. Non conta mai nel numero della colonna.
- Tipo `Proposal`; riga di fase `Waiting for your answers` mentre il job aspetta un round di domande o un permesso, `Preparing` altrimenti (mai `Writing`); sotto, l'ultima nota del job come sugli altri ghost. Stesso scheletro, stesso pulsante (apre la conversazione del job in Jobs) e stesso nome accessibile degli altri ghost, per esempio `Proposal being prepared by the agent: Checkout redesign, Preparing`.
- Titolo: per un job di setup o di design, il titolo della richiesta ricevuta così come lo mostrava la sua card di bozza (primo titolo o prima riga, altrimenti `Untitled request`). Weaver lo legge quando crea il job e lo tiene con il job in memoria: non viene mai scritto nel registro del job né nel file della richiesta. Il ghost di un job di import resta `Import from Code`.
- Nella stessa fotografia in cui compare il primo ghost annunciato dal job, il ghost di Weaver sparisce; ritorna nella stessa fotografia in cui se ne va l'ultimo ghost annunciato, se il job non è finito. Il ghost di una Proposal resta mentre la sua Draft è scritta, quindi il ghost di Weaver resta assente finché quella Proposal non è consegnata.
- Sparisce quando il job finisce, per qualunque motivo, nella stessa fotografia degli altri ghost del job.
- La sua identità deriva dal job, resta la stessa tra una fotografia e l'altra, non coincide mai con quella di un annuncio e non è mai salvata nel registro del job.

## `View request` nell'intestazione del job

- Nell'intestazione della conversazione del job, a destra, compare `View request` (pulsante secondario, icona `FileText`) prima di `Stop job`.
- Compare solo per un job di setup o di design, quelli avviati da Chat compresi, e solo finché la richiesta che ha ricevuto esiste ed è ancora inviata a quel job: sparisce quando la richiesta è consumata, scartata, cancellata, torna in scrittura o viene inviata di nuovo a un altro job (in quel caso il pulsante appare solo sul job a cui è inviata ora, non su quello vecchio).
- Perché l'interfaccia lo sappia, la riga del job comunica, per un job di setup o di design, quale richiesta ha ricevuto e se quella richiesta è ancora inviata a lui.
- Il clic apre quella richiesta in sola lettura nell'editor delle richieste (la vista di oggi con il banner `Sent to the agent · <job name>`), passando alla vista della Board dove l'editor vive; il controllo per tornare indietro dell'editor dice `← Jobs` e riporta a questa conversazione. Aperto dalla Board, l'editor mostra invece `← Board`. La vista a tutto schermo dell'editor è oggetto di un altro Task: questo Task collega il pulsante e la navigazione (origine `Jobs` e ritorno al job).

## Code notes
Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Ghost: `src-tauri/src/jobs/registry.rs` — `weaver_ghost` (≈1590) restituisce `None` per tutto tranne Setup/Import; `ghosts_of` (463) lo usa ordinato per `live.order`. Estendere a `JobKind::Design` e prendere il titolo da un campo in memoria di `Live` (non di `record`), riempito alla creazione in `src-tauri/src/jobs/starts.rs` (`start_design`/`start_setup`, che già mettono `target = Some(request id)`). Titolo: `title::title_of` in `crates/core/src/project/requests/store.rs` ricade sull'id; in quel caso usare `Untitled request` come la card di bozza.
- Riga del job: `src-tauri/src/jobs/wire.rs` `JobRow.target` (binding `ui/src/bindings/JobRow.ts`, oggi mai letto). Serve un campo nuovo "richiesta ancora inviata a questo job", calcolato da `src-tauri/src/requests.rs` (`.state.json` `sent: {id: jobId}`): confrontare il job id, non basta `target` (richiesta rinviata a un job più recente; richiesta consumata = file cancellato).
- UI: `ui/src/components/jobs/jobs-view.tsx` `JobHeader` (529-560). Navigazione: `ui/src/components/requests/use-requests.tsx` `openEditor(id?)`; l'editor esiste solo sotto la Board (`project-content.tsx:44-50`), quindi `openEditor(target)` + `showView("board")` e un'origine "jobs/<jobId>" per `← Jobs`; per tornare usare l'equivalente di `openJob` in `project-window.tsx:71-79`.
- Test: `jobs-view.test.tsx` monta senza `RequestsProvider`: aggiungere `useOptionalRequests` sul modello di `useOptionalAsks()`.
