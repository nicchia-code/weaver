---
created: 2026-11-07T20:53:35.890Z
updated: 2026-11-07T20:53:35.890Z
summary: Le richieste di design in scrittura sono card della Backlog con Send e Discard, il ghost del job porta il titolo della richiesta, Start from Scratch riapre la più vecchia e la palette trova le richieste.
tasks:
- 0villj1v
- 1afzyq7c
- busrn8cf
- e1z6342u
- zo33ob7f
cleared:
  0villj1v:
    title: Backlog e nome Request
    refs:
    - work
    - requirements/ui#work
    - requirements/ui#General
    - developer.NEW_REQUEST
    - developer.SEND_REQUEST
    - developer.DISCARD_REQUEST
    - developer.LEAVE_REQUEST
    technical: false
    proposal: 7lmguq57
    label: Bozze richieste
    finished: 2026-11-07T20:08:21.568Z
    body: |-
      Quando il Task è finito, la colonna `Backlog` della Board mostra ciò che aspetta lo sviluppatore: le Proposal e i Drift aperti, in un solo ordine di creazione dal più vecchio, poi le bozze delle richieste di design (le richieste in scrittura, mai quelle inviate), dalla più vecchia, poi i ghost. L'ordine e il raggruppamento arrivano già calcolati dal backend, bozze comprese: l'interfaccia non riordina, non raggruppa e non aggiunge card sue. La card della bozza e il suo comportamento (etichetta `Request`, titolo, `Not sent · <ora>`, `Send`, `Discard`, apertura nell'editor) sono descritti nella sezione `design-request` dei Requirement `ui` e realizzati da un altro Task di questa Proposal; qui la Board li accoglie come card vere.

      - **Numero della colonna:** il numero accanto a `Backlog` conta le card vere; ogni bozza conta una volta, una card di risoluzione conta una volta, i ghost non contano mai.
      - **Clic:** su ogni card, un clic fuori dai controlli o Enter sul titolo apre il dialogo dei dettagli; su una bozza apre invece la richiesta nell'editor, che non ha dialogo dei dettagli.
      - **Backlog vuota:** compare solo quando la colonna non ha nessuna card vera, bozze comprese: con sole bozze `Nothing to decide` non compare. Testi: `Nothing to decide`, poi in Muted `New requests and Proposals from Chat land here for your go-ahead.`, poi il pulsante secondario `Request` (icona `Plus`), che apre una nuova richiesta di design, e `Ask in Chat` (ghost), che mostra la Chat. Compare anche sopra i ghost.
      - **Pulsante dell'intestazione:** l'intestazione della Backlog termina con il pulsante a sola icona `Plus` con nome accessibile `New Request` e tooltip `Write a request in Markdown and send it to an agent`, sempre abilitato, che apre una nuova richiesta di design (sempre nuova, anche se esistono bozze).
      - **Ghost:** stanno in fondo alla colonna, dopo tutte le card vere, bozze comprese. Il ghost che Weaver crea da solo vale ora per i job di setup, di design e di import; per setup e design porta il titolo della richiesta ricevuta (è il backend a fornirlo, con un altro Task di questa Proposal), per import resta `Import from Code`. Il nome accessibile resta `<kind> being prepared by the agent: <title>, <phase>`, per esempio `Proposal being prepared by the agent: Checkout redesign, Preparing`. Una richiesta inviata lascia la Backlog nella stessa fotografia in cui compare il ghost del suo job; quando un job di setup o di design finisce senza consegnare una Proposal legata alla sua richiesta, la bozza torna nella stessa fotografia in cui spariscono i ghost del job.
      - **Nome:** nessun testo della Board dice più `Work item`, `Work Item` o `work item`; nella tabella delle icone dei Requirement la riga `+ Work Item` diventa `New Request` con `Plus`.

      Il Modello lo dice così: la Feature Work mostra sulla Board ciò che aspetta lo sviluppatore (Proposal e Drift da decidere e le richieste che sta ancora scrivendo) e i Task da finire; una richiesta inviata lascia la Board e torna come ghost del suo job, poi come card della sua Proposal, nella stessa colonna. Lo sviluppatore inizia una richiesta dalla Backlog o partendo da zero (`developer.NEW_REQUEST`), la invia o la scarta dalla richiesta stessa o dalla sua card sulla Board (`developer.SEND_REQUEST`, `developer.DISCARD_REQUEST`), e la lascia tornando alla Board o passando a un'altra vista (`developer.LEAVE_REQUEST`, che scarta una richiesta vuota).

      ## Code notes
      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Board: `ui/src/components/board/board-view.tsx` (`BoardView`, `CardList`, `CardOf`, `GhostList`, `EmptyBacklog` at ~118-141 with the `Work item` button and the `New work items…` text; Backlog `+` at ~284-296, a `GuardedButton` with `aria-label="New Work Item"` and the old tooltip, calling `useBoardActions().newRequest`; count `snapshot.backlog.length` at ~281).
      - Lane: `ui/src/components/board/column.tsx` (`Column`: empty state when `count === 0`, then cards, then ghosts; comment at :19 mentions Work Item). `board-ui.tsx:30` comment says `+ Work Item`.
      - The draft card kind arrives from the backend in `snapshot.backlog` (new `BoardCard` kind, binding `ui/src/bindings/BoardCard.ts`); its renderer is registered in `default-cards.tsx` by the design-request Task. Count and empty state then follow from the array length.
      - Click routing: `card-shell.tsx` `CardShell` always calls `useBoardUi().openDetails`; the design-request Task adds the path that opens the editor for drafts.
      - Ghost: `ghost-card.tsx` (`GhostCard`), titles come from the backend (`src-tauri/src/jobs/registry.rs` `weaver_ghost`).
      - Icon table text also lives in the generated skill `.agents/skills/weaver-req-ui*/` (regenerated, do not edit by hand).
      - Tests: `ui/src/components/board/board-view.test.tsx` (281-335: `New Work Item`, empty Backlog).
  1afzyq7c:
    title: Ghost design e View request
    refs:
    - job
    - requirements/ui#job
    technical: false
    proposal: 7lmguq57
    label: Bozze richieste
    finished: 2026-11-07T20:15:07.227Z
    body: |-
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
  busrn8cf:
    title: Start from Scratch e watcher
    refs:
    - project
    - requirements/ui#project
    - requirements/tech#project
    technical: false
    proposal: 7lmguq57
    label: Bozze richieste
    finished: 2026-11-07T20:10:08.427Z
    body: |-
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
  e1z6342u:
    title: Bozze come card
    refs:
    - design-request
    - requirements/ui#design-request
    - requirements/tech#design-request
    technical: false
    proposal: 7lmguq57
    label: Bozze richieste
    finished: 2026-11-07T20:33:16.395Z
    body: |-
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
  zo33ob7f:
    title: Richieste in palette
    refs:
    - window
    - requirements/ui#window
    technical: false
    proposal: 7lmguq57
    label: Bozze richieste
    finished: 2026-11-07T20:14:14.682Z
    body: |-
      Le richieste di design non ancora inviate (bozze) diventano card vere nella colonna Backlog della Board, e la stessa cosa prende ovunque un solo nome, `Request`. Questa Task porta la palette e la sidebar della finestra in linea con questo.

      ## Palette

      - Nel gruppo `Actions` l'azione che oggi si chiama `New Work Item` si chiama `New Request`. L'ordine del gruppo resta `Scan System`, `Run all`, `New Request`, `Open Folder…`, `Usage`. Sceglierla fa esattamente ciò che fa il pulsante `+` della Backlog (`New Request`): crea sempre una richiesta nuova e la apre nell'editor della richiesta; se l'azione è disabilitata mostra le sue ragioni in Muted e non si può scegliere, come ogni azione.
      - Il gruppo `Cards` elenca, oltre alle Proposal, ai Task e ai Drift della Board (numero breve e titolo), anche le bozze di richiesta che stanno in Backlog: per ognuna la riga mostra `Request` al posto del numero breve, seguito dal titolo della bozza (lo stesso della sua card: primo titolo o prima riga, altrimenti `Untitled request`). Le bozze si trovano cercando nel titolo, senza distinzione fra maiuscole e minuscole, con le stesse regole degli altri risultati (prima i titoli che iniziano col testo, al massimo 8 righe per gruppo); come tutto il gruppo `Cards`, non compaiono a campo vuoto. I ghost dei job non sono card e non compaiono.
      - Scegliere una bozza mostra la Board e apre quella bozza nell'editor della richiesta (a tutta vista, con `← Board`), non la finestra di dettaglio delle card. Scegliere una Proposal, un Task o un Drift continua a mostrare la Board e ad aprirne la finestra di dettaglio. La palette si chiude a ogni scelta.

      ## Sidebar

      - Il numero sulla voce `Board` è il numero delle card vere in Backlog, bozze di richiesta incluse, in Muted, allineato a destra, nessuno quando è 0; con sole bozze il numero c'è e le conta. Il numero viene dalla Backlog che il backend manda nella fotografia della Board: nessun conteggio separato delle richieste nell'interfaccia. Anche il badge sull'icona nella sidebar stretta (sotto 1024 px) mostra lo stesso numero.

      ## Code notes
      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Palette: `ui/src/components/command-palette.tsx` — label `"New Work Item"` (azione `new-request`) intorno a riga 47; gruppo Cards intorno a riga 69, costruito da backlog e active di `useBoard().snapshot`; tipi in `ui/src/components/palette-model.ts`. La rivelazione di una card passa da `useReveal("card", …)` / `useBoardUi().openDetails`.
      - Le bozze arrivano in `snapshot.backlog` come nuovo tipo di `BoardCard` prodotto dal backend (lo introduce la Task della design request: dipendenza). Senza un ramo dedicato il gruppo Cards le includerebbe chiamando `openDetails`: per le bozze serve invece mostrare la vista Board e `useRequests().openEditor(id)`. Escludere i ghost.
      - Sidebar: `ui/src/components/project-content.tsx` (~riga 38, `snapshot?.backlog.length`) → `sidebar.tsx` (~righe 25-26, `card(s) in Backlog`): con le bozze nell'array il conteggio è automatico; verificare che i ghost non siano in `backlog`.
      - Test da aggiornare: `command-palette.test.tsx`, `sidebar.test.tsx`, `project-content.test.tsx`.
---
# Richieste di design come card sulla Board

La colonna Backlog mostra, dopo Proposal e Drift, le bozze delle richieste di design come card vere (etichetta Request, titolo, Not sent, Send, Discard), contate nel numero della colonna. Il pulsante + (New Request) apre sempre una richiesta nuova; Start from Scratch apre la più vecchia in scrittura e ne crea una solo se non ce n'è. Il watcher aggiorna la Board solo quando una richiesta compare, sparisce o cambia titolo o vuotezza. Il ghost del job di setup o di design porta il titolo della richiesta inviata e la bozza torna quando il job finisce senza Proposal. La palette e la sidebar conoscono le bozze. Nessun testo dice più Work item.
