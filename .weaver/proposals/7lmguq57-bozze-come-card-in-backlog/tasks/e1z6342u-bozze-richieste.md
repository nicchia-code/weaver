---
refs:
  - design-request
  - requirements/ui#design-request
  - requirements/tech#design-request
technical: false
---
# Bozze come card

Le richieste di design non ancora inviate (stato Writing) diventano card della colonna Backlog della Board; l'editor mostra una sola richiesta a tutta vista. Spariscono la striscia `N requests are open`, la lista delle richieste a sinistra dell'editor, il pulsante `New request` dell'editor e i badge `Draft`/`Sent`. La stessa cosa prende un solo nome: `Request`.

## Bozze nella fotografia della Board

La fotografia della Board calcolata dall'app porta nella Backlog, dopo le Proposal e i Drift aperti, una card per ogni richiesta in Writing, dalla più vecchia (ordine di creazione per identità). Ogni bozza porta: identità, titolo (primo titolo, altrimenti prima riga), ora di creazione, se è vuota e se è tornata da un job finito senza Proposal. Le richieste restano fuori dalla fotografia del core del Modello e delle card: le bozze le aggiunge l'app, dall'archivio delle richieste, quando costruisce la fotografia, così che l'interfaccia non unisca mai due liste.

Un altro Task (`requirements/ui#work`) si aspetta che le bozze siano card vere: contano nel numero della colonna Backlog (e della voce `Board` della sidebar), stanno prima dei ghost e con sole bozze `Nothing to decide` non compare. L'ordine e il contenuto dati dal backend devono permetterlo senza che l'interfaccia riordini nulla.

## Quando la Board si aggiorna

La parte `board` della fotografia viene toccata, con l'avviso di progetto cambiato, quando una richiesta:
- nasce;
- viene inviata: nella stessa fotografia in cui compare il primo ghost del suo job, mai una in cui si vedano insieme la bozza e il ghost;
- viene scartata, cancellata fuori da Weaver o consumata da una Proposal;
- torna in Writing: nella stessa fotografia in cui escono i ghost del suo job;
- viene salvata cambiando il titolo o il fatto di essere vuota. Un salvataggio che non cambia nessuno dei due non tocca niente (il salvataggio automatico avviene ogni 500 ms).

## Ritorno all'apertura

Una richiesta il cui job è finito mentre l'app era chiusa torna in Writing, nella coda delle modifiche del progetto, prima della prima fotografia della Board della finestra: la sua card si vede fin dall'inizio.

## Scarto di una richiesta vuota

Nuova Transizione della Design request: da Writing, `developer.LEAVE_REQUEST` → Discarded solo quando la richiesta è vuota; file e immagini vengono cancellati, senza conferma e senza toast. `developer.LEAVE_REQUEST` è il nuovo Evento: lo sviluppatore lascia la richiesta che sta scrivendo, tornando alla Board o perché un'altra vista della finestra prende il suo posto (scelta da lui, da un link o da una notifica cliccata). Nuova Transizione anche `window.CLOSED` → Discarded: solo quando è vuota e aperta in scrittura in quella finestra; file e immagini vengono cancellati. Vuota significa testo vuoto dopo il trim; una richiesta fatta solo di immagini non è vuota. L'interfaccia chiede al backend di scartare la richiesta aperta quando lascia l'editor o si chiude la finestra a testo vuoto; il backend verifica la vuotezza sul file, sotto il lock del progetto, e non scarta nulla che non sia vuoto o non sia in Writing.

## Card della bozza

Stesso aspetto delle altre card (`requirements/ui#proposal`): Surface, anello Line, ombra morbida della card sulla corsia, raggio 12 px, controlli in un piè dopo una riga `#EFEDE7`. Dall'alto:
1. l'etichetta `Request` dove le altre card mostrano l'id;
2. il titolo, 15 px semibold, al massimo 3 righe poi `…`: primo titolo, altrimenti prima riga, altrimenti `Untitled request`;
3. `Not sent · <ora>` in Muted, l'ora di creazione nel formato della lista Jobs: `HH:MM` oggi, altrimenti `D MMM HH:MM`;
4. il piè: `Send` (primario, `SendHorizontal`), disabilitato finché il testo è vuoto dopo il trim, tooltip `Write the request first.`; poi `Discard` (`Trash2`), con l'AlertDialog `Discard this request?` / `Its file and images are deleted.`, `Discard` (distruttivo) e `Cancel` (con il focus).

Un clic sulla card fuori dai controlli, o Enter sul titolo, apre la richiesta nell'editor; la bozza non ha dialog di dettaglio. Un invio o uno scarto fallito dalla card mostra lo stesso toast d'errore dell'editor (`The request could not be sent.`, `The request could not be discarded.`, o il motivo dato). Uno scarto dalla card non deve far comparire il toast di cancellazione esterna.

## Editor della richiesta

Una sola richiesta su tutta la vista Board, al posto della Board. Intestazione: controllo di ritorno a sinistra, titolo `Setup request` o `Design request`, stato del salvataggio, controlli della richiesta. Niente lista, niente `New request`, niente badge `Draft`/`Sent`; la Board non mostra più alcuna striscia che conti le richieste aperte.

Si apre sempre con una richiesta: nuova da `New Request` della Backlog (o da `Request` della Backlog vuota); da Start from Scratch la bozza più vecchia, altrimenti una nuova (il `+` della Backlog crea sempre una nuova); una bozza dalla sua card o dalla sua riga nella palette; una richiesta inviata da `View request` del suo job. Una richiesta in scrittura si apre con il focus nel testo.

Controllo di ritorno: `← Board`, che mostra la Board; `← Jobs` quando la richiesta è stata aperta da `View request`, che mostra di nuovo la conversazione di quel job. Il pulsante `View request` appartiene a un altro Task, ma l'editor deve poter essere aperto in sola lettura su una richiesta inviata con `← Jobs`.

Restano come oggi: textarea monospazio con a capo morbido; `Preview` affiancata da ~900 px di larghezza, sotto altrimenti, aggiornata entro 150 ms; salvataggio automatico 500 ms dopo l'ultimo tasto e su blur, cambio di vista e chiusura della finestra, con `Saving…`, `Saved`, `Could not save: <reason>.`; cambi esterni (ricarica se nulla è in sospeso, altrimenti `The request changed on disk; Weaver kept your version.`); immagini PNG, JPEG, GIF o WebP fino a 10 MB come `image-<n>.<ext>` nella cartella della richiesta, altrimenti `Only PNG, JPEG, GIF or WebP images up to 10 MB.`

- **Send** (`SendHorizontal`, anche `Ctrl/Cmd+Enter`, disabilitato a testo vuoto con `Write the request first.`): una volta inviata l'editor si chiude e si vede la Board, con il ghost del job dove c'era la bozza; un invio fallito lascia l'editor aperto con il suo toast.
- **Discard**: stessa AlertDialog della card; una volta scartata l'editor si chiude e si vede la Board.
- **Cancellazione esterna**: chiude l'editor, tornando da dove era stato aperto, con il toast di avviso `The request was deleted outside Weaver.`
- **Inviata**: vista renderizzata in sola lettura con il banner `Sent to the agent · <job name>` e `Open conversation`. Se la richiesta viene consumata o scartata mentre è mostrata, la vista si chiude tornando alla conversazione del job, senza toast; se il job finisce senza Proposal mentre è mostrata, la stessa vista diventa la richiesta scrivibile con il banner di ritorno.
- **Tornata**: una richiesta tornata in scrittura dopo un job finito senza Proposal è di nuovo una card normale; aperta mostra il banner `The job ended without a Proposal. Change the request and send it again.`

## Nome

Ogni testo di queste superfici la chiama richiesta (`Request`, `New Request`, `Untitled request`, `Setup request`, `Design request`), mai work item; così anche il testo dato all'agente da Ask more che rimanda lo sviluppatore al controllo della Board: `+ Work Item` diventa `New Request` nella Backlog. Stati vuoti della Backlog, pulsante `+` dell'intestazione e tabella delle icone sono di un altro Task.

## Code notes
Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Macchina: `crates/core/src/project/requests/machine.rs` (`RequestEvent`, `step()`; aggiungere leave/window-closed; doc di `NewRequest` cita `+ Work Item`). Store: `store.rs` (`list()` ha già `empty`, `title`, `created` locale senza fuso; `id::compare` per l'ordine; `discard`, `job_ended`, `reconcile`).
- Bozze in `src-tauri/src/board.rs` (`BoardCard` taggato per `kind`, `snapshot()`), non nel `compute()` del core: le richieste non sono in `ProjectImage`. Rigenerare i binding `ui/src/bindings/`.
- `src-tauri/src/requests.rs`: `send_locked` avvia il job (ghost pubblicato da `board_changed`) e poi `record_sent`: toccare `SLICE_BOARD` (`src-tauri/src/pictures.rs`) dopo `record_sent` e insieme al ghost. Il ritorno a Writing su `JobEnd` è asincrono: va nella stessa fotografia dell'uscita dei ghost. `returned` è solo in memoria. `board_snapshot` gira fuori dalla coda in `blocking::disk`: `return_finished` scrive, quindi va eseguito in coda all'apertura. `requests_list`/`RequestsSnapshot`/`requests-changed` possono diventare codice morto; `modelEmpty` va spostato (in `RequestView` o nella fotografia).
- UI: `requests-layer.tsx` (striscia da togliere), `request-editor.tsx` (lista, badge, `known`/`discarding` per il toast esterno), `request-pane.tsx`; `project-content.tsx` 44-50. Card: `card-shell.tsx` apre `openDetails` (serve apertura editor), `card-registry.ts` richiede `Details`, pattern `GuardedButton` + `ConfirmDialog` con `stopPropagation` in `drift-card.tsx:67-80`. Formato ora: `jobs/jobs-view.tsx`.
- Ask more: `crates/core/src/jobs/ask.rs:201`, test `general.rs:1431`. Test UI: `request-editor.test.tsx`, `board-view.test.tsx`, `project-content.test.tsx`.
