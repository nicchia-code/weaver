---
created: 2026-11-05T20:00:44.262Z
updated: 2026-11-05T20:00:44.262Z
summary: La Chat scrive Draft di Proposal su ordine dello sviluppatore, tiene le richieste di design, apre round di domande, attende le risposte e notifica; la vista e la Board la mostrano.
tasks:
- 481k4qsb
- 6hmq5a3u
- c9x4aah0
- ohlbzgm4
- opn6ixht
- q7v4up7r
- v2oomx1j
- yncrtvz9
cleared:
  481k4qsb:
    title: Sezione Chat nelle skill
    refs:
    - requirements/rules#project
    technical: true
    proposal: cqffevyy
    label: Chat Proposals
    finished: 2026-11-05T19:30:14.026Z
    body: |-
      L'istruzione della Chat, su ordine esplicito del developer, manda l'agente a caricare `weaver-design` (Proposal di modifica) o `weaver-resolve-drifts` (risoluzione di Drift aperti). Le due skill devono quindi dire come si scrive dalla Chat.

      ## Cosa cambia (`requirements/rules#project`, punto Chat texts)

      Nei sorgenti `crates/core/assets/skills/weaver-design/SKILL.md` e `crates/core/assets/skills/weaver-resolve-drifts/SKILL.md` (installati da `crates/core/src/agent_files.rs`; le copie in `.agents/skills/` si aggiornano con l'installazione, non a mano):

      - Nuova sezione `## Writing from Chat` in entrambe, che vale quando l'istruzione della Chat manda lì l'agente su ordine esplicito del developer. Dice:
        - la conversazione vale come intervista: ciò che developer e agente hanno concordato è deciso e non si richiede; una decisione ancora aperta si chiede nella risposta, o con un solo round `weaver question new` solo se il developer ha chiesto di essere interrogato così; allo stesso modo si decide se legare le immagini di una richiesta;
        - nessun annuncio e nessuna nota sul job; la Proposal si apre subito, una modifica con `weaver-design` o una risoluzione con `weaver-resolve-drifts`, legata con `request` a una richiesta non inviata, o con `reworks` a una Proposal rifiutata, solo quando il developer l'ha nominata;
        - poi file, Task di una modifica, `## Already present`, consegna e, per una modifica, la scansione leggera del codice, come dicono i passi della skill, sotto-agenti inclusi; una contraddizione trovata dalla scansione si chiede come sopra;
        - dopo la consegna l'agente riassume la Proposal in poche righe e si ferma; non decide nulla a meno che il developer chieda esplicitamente quella decisione.
      - `## Hard rules` (e la `description` del front matter di `weaver-design`, che oggi dice "never in Chat"): non dicono più che la Chat non le segue mai o non scrive mai Proposal; continuano a tenere fuori ogni altra conversazione (Ask more). In `weaver-design` togliere il rimando obbligato a `+ Work Item` per la Chat.
      - Il testo resta in inglese, senza prima persona, rispettando le altre regole di `requirements/rules#project` (lunghezza e formato delle skill).

      ## Test

      - `crates/core/tests/format_md.rs` e i test di `agent_files.rs` che controllano le skill incorporate: aggiornare le asserzioni che fissano la frase "never in Chat" o l'assenza della sezione; aggiungere un controllo che entrambe le skill abbiano `## Writing from Chat`.
  6hmq5a3u:
    title: Scheda Draft della Chat
    refs:
    - requirements/ui#proposal
    technical: true
    proposal: cqffevyy
    label: Chat Proposals
    finished: 2026-11-05T19:33:25.164Z
    body: |-
      Una Draft che l'agente della Chat sta scrivendo (front matter `chatWriting: true`) ha una propria scheda sulla Board; ogni altra Draft, salvo quella lasciata a metà da un refinement, continua a non averne (per quelle scritte da un job resta solo il ghost del job).

      ## Cosa deve valere (`requirements/ui#proposal`)

      - La scheda sta in Backlog, nell'ordine delle schede, con la struttura di una Proposal Open del suo tipo (riga dei file, riga dei Task con la label, righe della risoluzione), aggiornata man mano che la Chat scrive, più i badge outline `draft` e `Chat` per primi.
      - Unici controlli: `Open Chat` (outline, icona `MessageSquare`), che mostra la vista Chat, e `Discard` (destructive), che chiede `Discard this Draft?` / `Its files and Tasks are thrown away.` con `Discard` (destructive) e `Cancel`, focus su `Cancel`. Niente `Approve` né `Reject`.
      - Mentre un turno della Chat è in corso, `Discard` è disabilitato con `The agent is still finishing`.
      - Il dialogo dei dettagli mostra gli stessi controlli, poi `Ask more`.
      - La Draft lasciata a metà da un refinement mantiene la sua scheda come oggi (`refinement interrupted`, `Refine`, `Discard`).

      ## Dove

      - Dati della Board: `crates/core/src/project/board.rs` (`Board::compute` l.221, `is_being_written` l.200, `proposal_state`) deve esporre la Draft con `chatWriting` come scheda con i suoi contenuti, e se un turno della Chat è in corso (stesso fatto usato per il motivo 1); i tipi verso la UI di conseguenza.
      - UI: `ui/src/components/board/board-view.tsx`, `column.tsx`, la scheda Proposal (`proposal-card`) e il suo dialogo dettagli; `Open Chat` usa la navigazione già esistente verso la vista Chat; lo scarto usa la stessa azione di `Discard` della Draft interrotta. Il conteggio della voce Board nella barra laterale, che esclude queste schede, è in un altro Task (`requirements/ui#General`).

      ## Test

      - `ui/src/components/board/proposal-card.test.tsx` e `board-view.test.tsx`: scheda con badge `draft`/`Chat`, solo `Open Chat` e `Discard`, dialogo di conferma con i testi e focus su `Cancel`, `Discard` disabilitato con tooltip durante il turno; i test che oggi affermano che nessuna Draft non interrotta ha scheda vanno cambiati.
      - Test di `board.rs` sulla presenza della scheda per `chatWriting`.
  c9x4aah0:
    title: Notifica del round della Chat
    refs:
    - notification
    - requirements/ui#notification
    technical: false
    proposal: cqffevyy
    label: Chat Proposals
    finished: 2026-11-05T19:49:03.803Z
    body: |-
      Quando la Chat (General) di un progetto aspetta le risposte al proprio round di domande, il developer deve essere avvisato come per un job, se la finestra del progetto non è in primo piano.

      ## Cosa deve valere

      **Model `notification`**, stato `Not shown`:
      - `general-chat.WAITING` → `Shown`: quando la finestra del progetto non è in primo piano; altrimenti non si mostra nulla. La notifica è mostrata subito.
      - Da `Shown`, `question-round.CLOSED` → `Withdrawn`: il round che annunciava si è chiuso (comunque si chiuda: risposte, Skip all, abbandono, interruzione per Stop, Clear, uscita di omp o chiusura della finestra).
      - `desktop.NOTIFICATION_CLICKED` → `Clicked`: il developer è portato al progetto.
      - La Chat notifica soltanto l'attesa delle risposte del suo round: mai la fine di un turno, mai altro. Una notifica non cambia nulla nel progetto, nei job o nei round.

      L'evento `general-chat.WAITING` (Chat che entra in `Waiting for answers` per un round) è prodotto dal Task di `general-chat` e dal Task di `question-round`; qui si consuma.

      **`requirements/ui#notification`:**
      - Tipo Question round: titolo `<project> – Chat asks a question`, messaggio `The agent is waiting for your answers.`. `<project>` è il nome della cartella; un round della Chat genera solo il tipo Question round.
      - Mai alcun messaggio, domanda, comando, percorso, risposta o scheda.
      - ID di ogni notifica: progetto + job (o Chat) + tipo, per sostituirla o ritirarla; dove il sistema non può rimuovere una notifica consegnata, il clic su una ritirata porta solo la finestra in primo piano.
      - Clic su una notifica di domanda → l'overlay del round (finestra del progetto in primo piano).
      - Nessun raggruppamento per il tipo Question round.

      ## Dove nel codice
      - `crates/core/src/notification.rs` (`Book`, l.250): `question`/`round_closed` oggi prendono l'identità del job; servono le stesse operazioni per la Chat del progetto, con chiave progetto + Chat + tipo distinta da qualunque job.
      - `src-tauri/src/notifications.rs`: oggi `round_opened`/`round_closed` arrivano solo come `JobObserver` (l.200–209) con un `JobInfo`. Aggiungere i metodi per la Chat (attesa del round e chiusura), usando `in_background(project_id)` e `waiting` come per i job; il nome del progetto come in `project_name`.
      - Chi li chiama: il servizio General (`src-tauri/src/general/service.rs`) quando la Chat entra/esce da `Waiting for answers`, oppure `src-tauri/src/rounds/service.rs::carry_out` per gli effetti `Opened`/`Closed` con origine Chat; collegamento in `notifications::install` / setup dell'app.

      ## Test
      - Test in `src-tauri/src/notifications.rs` (modulo tests, vicino a `in_the_background_the_exact_texts_appear_without_anything_the_agent_said`): in secondo piano appare esattamente `<project> – Chat asks a question` / `The agent is waiting for your answers.`; in primo piano nulla; la chiusura del round la ritira; una notifica di un job e una della Chat dello stesso progetto hanno ID diversi.
      - Test di `Book` in `crates/core/src/notification.rs` per la chiave della Chat.
  ohlbzgm4:
    title: Richiesta tenuta dalla Chat
    refs:
    - design-request
    - agent.OPEN_PROPOSAL
    - requirements/data#design-request
    - requirements/ui#design-request
    technical: false
    proposal: cqffevyy
    label: Chat Proposals
    finished: 2026-11-05T19:47:33.778Z
    body: |-
      Su ordine esplicito del developer, l'agente della Chat (General) può scrivere la Proposal di una richiesta di design scritta e non inviata, legandola alla richiesta. La richiesta allora resta bloccata finché la bozza esiste e torna modificabile se la bozza viene scartata.

      ## Actor Event `agent.OPEN_PROPOSAL`

      L'agente apre una nuova Proposal, come Draft, eventualmente legata a una richiesta scritta o al posto di una Proposal rifiutata. Per la Chat questo è `weaver proposal new` con `request` quando il chiamante è la Chat (`WEAVER_GENERAL`). Il controllo di chi può legare cosa (un job lega solo la richiesta della sua istruzione, la Chat una richiesta in Writing, altri nulla; vincolo non permesso → Proposal creata senza vincolo con una nota) appartiene al Task di `requirements/cli#proposal`; qui conta l'effetto sulla richiesta.

      ## Machine `design-request`: nuova State `Held by Chat`

      - In `Writing`: `agent.OPEN_PROPOSAL` → `Held by Chat`, solo per una Proposal che l'agente della Chat apre legata a quella richiesta. Una Proposal di un job non porta mai a `Held by Chat`.
      - `Held by Chat`: l'agente della Chat sta scrivendo una Proposal legata alla richiesta. Il developer può leggerla ma non cambiarla, inviarla né scartarla, e può aprire la Chat da lì.
        - `proposal.SUBMITTED` → `Consumed`: solo la Proposal legata; file e immagini della richiesta cancellati in un passo (come oggi per `Sent`).
        - `proposal.DISCARDED` → `Writing`: solo la Proposal legata, scartata prima della consegna (con `Discard` sulla scheda Draft della Chat o con Clear della Chat, che scarta le bozze create dalla Chat); il developer può cambiarla e inviarla di nuovo. Nessun banner "job ended" in questo caso.
        - `workspace.REQUEST_DELETED` → `Discarded`.
      - Le altre State e transizioni restano come sono.

      ## Dati (`requirements/data#design-request`)

      - Lo stato `Held by Chat` è derivato, non registrato: una richiesta è tenuta dalla Chat mentre una Proposal `draft` con `chatWriting: true` la nomina in `request`. Nulla viene scritto in `.weaver/requests/.state.json` (che resta `{sent, consumed}`).
      - Quindi alla consegna (`chatWriting` tolto, Proposal non più `draft`) la richiesta è consumata come per `Sent` (il consumo avviene alla consegna della Proposal legata); quando la bozza viene scartata la richiesta torna in `Writing` da sola.
      - Consumare o scartare una richiesta cancella `<id>.md` e `<id>.images/` in un passo.

      ## UI (`requirements/ui#design-request`)

      - Lista dell'editor delle richieste: la richiesta tenuta compare con badge `Chat` al posto di `Draft`.
      - Vista: sola lettura renderizzata con banner `Chat is writing a Proposal from this request` e pulsante `Open Chat` (apre la vista Chat), senza `Send` né `Discard`; `Ctrl/Cmd+Enter` non invia; l'autosave non scrive.
      - Quando la bozza viene scartata la richiesta è di nuovo modificabile come prima.

      ## Dove nel codice

      - `crates/core/src/project/requests/machine.rs`: `RequestState` riceve `HeldByChat` con le transizioni sopra.
      - `crates/core/src/project/requests/store.rs`: `state_of`/`list` derivano `HeldByChat` leggendo le Proposal `draft` con `chatWriting: true` e `request` = id; `save`, `discard` e `send` rifiutano una richiesta tenuta; consumo alla consegna come `consume`.
      - `src-tauri/src/requests.rs`: `RequestStatus` riceve `held-by-chat` (`status_of`, `summary`, `view`); `send_locked` e i comandi `request_save`/`request_discard` rifiutano; `proposal_submitted` consuma anche una richiesta tenuta; `emit_changed` quando una bozza della Chat legata nasce o viene scartata (`src-tauri/src/proposals.rs` vicino a l.458–466 e il percorso di discard, `crates/core/src/project/proposal/transitions.rs:plan_discard`), così la lista si aggiorna. I tipi TS generati (`ts-rs`) vanno rigenerati.
      - `ui/src/components/requests/request-editor.tsx` (badge, l.46) e `request-pane.tsx` (banner accanto a `Sent to the agent`, l.405; pulsanti Send/Discard l.386).

      ## Test

      - `src-tauri/src/requests.rs` (test in fondo al file): nuovi casi — una Proposal `draft` con `chatWriting: true` e `request` rende la richiesta tenuta e rifiuta save/send/discard; scartarla la riporta a `Writing`; consegnarla la consuma; file cancellato → `Discarded`.
      - `crates/core/src/project/requests/`: test della macchina per le nuove transizioni.
      - `ui/src/components/requests/request-editor.test.tsx`: badge `Chat`, banner e `Open Chat`, assenza di `Send`/`Discard`.
  opn6ixht:
    title: Istruzione e attesa della Chat
    refs:
    - general-chat
    - developer.CLEAR_GENERAL
    - requirements/rules#general-chat
    - requirements/tech#general-chat
    - requirements/docs#General
    technical: false
    proposal: cqffevyy
    label: Chat Proposals
    finished: 2026-11-05T19:53:44.821Z
    body: |-
      La Chat (General) smette di essere una conversazione libera che non scrive mai Proposal e diventa la conversazione di brainstorming del developer con l'agente sul progetto e su Weaver, guidata da un'istruzione di Weaver. L'agente legge liberamente; non scrive nulla, né in Weaver né nei file del progetto, senza un ordine esplicito del developer (un desiderio, un problema o un'idea non sono un ordine); quando il ragionamento arriva a qualcosa da mettere per iscritto lo dice in una riga e aspetta; non modifica mai a mano i file di `.weaver/`. Su ordine esplicito scrive una Proposal di modifica con i suoi Task (caricando `weaver-design`) o una risoluzione di Drift aperti (caricando `weaver-resolve-drifts`), con le stesse regole e gli stessi controlli del job che la scriverebbe, la conversazione al posto dell'intervista; mai setup né import. Su ordine esplicito può anche creare un singolo Task o Drift, modificare una Proposal esistente, prendere una decisione del developer o avviare un job. Fa domande nelle risposte; con un round (`weaver question new`) solo se il developer lo chiede. Non è un job: non aspetta altro lavoro e nessun lavoro la aspetta, non tiene Task, non annuncia schede, nulla sulla Board finché parla soltanto. Modello: quello di analisi, come oggi. Nessuna riga nella conversazione per la Proposal scritta: basta la scheda sulla Board.

      ## Istruzione (`requirements/rules#general-chat`)

      In `crates/core/src/jobs/general.rs` riscrivere `INSTRUCTION` e `instruction(...)`:

      - Testo fisso in inglese, prima riga `# Weaver Chat` (non più `# Weaver General`), mai mostrato nella conversazione, senza numeri brevi, testi di schede o di richieste, senza prima persona.
      - Il testo fisso dice, in quest'ordine, i dieci punti di `requirements/rules#general-chat` (frammento nuovo `rules/chat-instructions`): 1) cos'è la Chat (brainstorming su progetto e Weaver, `.weaver/` con Model, Requirements, Specs, Proposals, Tasks, Drifts; nessuna richiesta scritta né skill allegata); 2) legge liberamente file, comandi di lettura `weaver` e le skill `weaver-req-*`; 3) non scrive nulla senza ordine esplicito, un desiderio/problema/idea non è un ordine, lo dice in una riga e aspetta; 4) non modifica mai a mano `.weaver/`, solo i comandi `weaver`; 5) su ordine esplicito di Proposal di modifica carica `weaver-design` e ne segue la sezione per la Chat, per una risoluzione `weaver-resolve-drifts` e la sua sezione per la Chat; niente setup/import; mai caricare o seguire le altre skill dei job (`weaver-setup`, `weaver-import`, `weaver-scan`, `weaver-compare`, `weaver-implement`, `weaver-resolve-conflicts`) se non per leggere le regole del punto 6; 6) singolo Task o Drift con `weaver task new` / `weaver drift new` come dice il loro `--help`, Task autosufficiente come dice `weaver-design`, Drift come dice `weaver-compare`; 7) decisioni del developer (approvare, rifiutare, scartare una Proposal, decidere un Drift, cancellare un Task in To do) solo se il developer la chiede esplicitamente in questa conversazione; 8) domande nella risposta in testo semplice e attesa; `weaver question new`, lanciato con bash `timeout: 0`, solo se il developer ha chiesto di essere interrogato con un round; form e strumento di domanda di omp rifiutati; 9) la Chat non è un job: `weaver task move`, `weaver announcement new`, `weaver job edit` sono rifiutati (non più `weaver question new`); 10) `weaver job new` solo su richiesta esplicita, con gli esempi di oggi (scan, run di un `task`, resolve di un `drift`, design di una `request`); al rifiuto riferisce il motivo dall'output e non avvia il job in altro modo; dice quale job è partito e non ne aspetta la fine.
      - Va rimossa la frase di oggi che vieta di scrivere Proposal e rimanda alla richiesta `+ Work Item`.
      - Righe aggiunte dopo il testo fisso, costruite dalle Settings a quell'avvio, in quest'ordine: `Run at most <N> sub-agents at once.` (N = limite di sotto-agenti); `Pass each sub-agent what you have already read that concerns its part, as the "Sub-agents context sharing" section of your skill says.`, con il testo fisso che aggiunge che senza skill caricata l'agente passa ciò che ha già letto che riguarda la parte del sotto-agente; solo con una lingua impostata `Write the texts for the developer in <language>.` (resta `clean_language`). `instruction` riceve quindi anche il limite di sotto-agenti; riusare le stesse frasi già costruite per le istruzioni dei job, senza una seconda convenzione.

      ## Avvio di omp (`requirements/tech#general-chat`)

      - Riga di comando invariata: `<omp> [--model <analysis>] --approval-mode <mode> --append-system-prompt <file> acp`, niente `--advisor`, `--slow`, overlay.
      - `<file>` è l'istruzione della Chat costruita a ogni avvio con limite di sotto-agenti e lingua delle Settings di quel momento e scritta in `<app local data>/general.md` (`instruction_file`) prima di lanciare omp: in `src-tauri/src/general/actor.rs:launch` passare a `general::instruction` anche il limite letto da `host.settings()`.
      - Ambiente: `WEAVER_GENERAL` e `WEAVER_ENDPOINT`, mai `WEAVER_JOB` (`env`). Eccezioni accettate dall'app: `weaver job new` dall'identità General viva di un progetto aperto, e `weaver question new` dalla stessa identità solo mentre è in corso un turno della Chat. L'app conosce l'identità General viva di ogni progetto aperto (`General::identity` in `src-tauri/src/general/service.rs`), così che `proposal new`/`proposal edit` registrino `chat`/`chatWriting` e la Board sappia se un turno della Chat è in corso: questa parte la usano i Task di `proposal` e `question-round`, qui basta che l'identità e lo stato "turno in corso" (incluso Waiting for answers) siano esposti in modo affidabile.
      - Sessione: invariata (record in `<app local data>/general/<project hash>.json`, `session/load`, Clear chiude la sessione e cancella la voce).

      ## Stato `Waiting for answers` ed Eventi

      Nella Machine `GeneralMachine` (`crates/core/src/jobs/general.rs`, enum `State`, `Event`, `Effect`) e nel servizio `src-tauri/src/general/service.rs`:

      - Working: `question-round.OPENED` → Waiting for answers, solo per un round della Chat (aperto dall'agente perché il developer ha chiesto di essere interrogato così). All'ingresso viene emesso l'Evento `WAITING` (`general-chat.WAITING`), che la Machine `notification` usa per notificare a finestra in secondo piano come per `job.WAITING`; la Chat conta come in attesa del developer.
      - Waiting for answers, stesso turno:
        - `developer.SEND_MESSAGE` → Waiting for answers: il messaggio aspetta e non risponde mai al round; arriva all'agente quando il round finisce.
        - `question-round.CLOSED` → Working.
        - `omp.TURN_FAILED` → Idle: motivo mostrato; il round finisce senza risposte.
        - `omp.EXITED` → Idle: motivo mostrato; il prossimo messaggio riavvia omp; il round finisce senza risposte.
        - `developer.STOP_AGENT` → Idle: turno annullato, round finito senza risposte.
        - `developer.CLEAR_GENERAL` → Idle: turno fermato, round finito senza risposte, conversazione di nuovo vuota, Draft della Chat scartate.
        - `window.CLOSED` → Idle: turno annullato, round finito senza risposte, conversazione tenuta (`GeneralWindows::window_closed` in `src-tauri/src/general/mod.rs`, `General::window_closed`). Anche l'uscita di omp chiude il round senza risposte.
      - `turn_running()` / `accepts_agent_calls()` devono valere anche in Waiting for answers (un turno è in corso); l'apertura e la chiusura del round arrivano dal Task di `question-round`, qui va il lato della Chat (Eventi d'ingresso/uscita della Machine e la fine del round senza risposte come Effetto).
      - Working, descrizione: mentre un turno è in corso (Working o Waiting for answers) il developer non può approvare, rifiutare o scartare una Proposal che l'agente della Chat ha creato o sta scrivendo. Il blocco sta nel Task di `proposal`; qui va solo il fatto osservabile "turno in corso" per l'identità viva.
      - Tutte le altre Transition (Idle, Loading con Reloading e Waiting for omp, Working) restano come oggi, inclusa `general-chat/Idle/developer.SEND_MESSAGE` e `general-chat/Reloading/*`.

      ## Clear (`developer.CLEAR_GENERAL`)

      Da Idle, Loading, Working e Waiting for answers, Clear porta in Idle con conversazione vuota e scarta le Proposal create dall'agente della Chat che sono ancora Draft (con `chat: true`; non quelle soltanto riaperte dalla Chat). L'Evento `CLEARED` (`general-chat.CLEARED`) è emesso quando il developer pulisce la conversazione, dopo che il turno, se c'era, si è fermato: in `service.rs` (`clear()`, `carry_out` su `Effect::Clear`, dopo `GeneralStore::delete` e `locks.family_ended`) emettere l'Evento verso il backend della Board; lo scarto vero e proprio (`general-chat.CLEARED` → Discarded in `proposal`, sblocco delle richieste legate) è del Task di `proposal`, che va chiamato da qui. Stop, uscita di omp, chiusura della finestra e uscita da Weaver NON scartano le bozze.

      ## Test

      - `crates/core/src/jobs/general.rs`: il test `the_instruction_is_fixed_text_with_the_language_only_when_one_is_set` (l.~1122) va riscritto, non ri-fissato sul testo vecchio: controlla che il testo contenga le regole dei punti 3, 4, 5 e 8, le righe aggiunte nell'ordine con la riga della lingua solo se impostata, e nessuna parola in prima persona. Il test degli argomenti (l.~1080) resta valido; aggiungere test della Machine per Working → Waiting for answers e per ogni Transition di Waiting for answers (messaggi trattenuti, fine round senza risposte, Clear).
      - `src-tauri/src/general/tests.rs` (l.~204–236): il file dell'istruzione scritto prima dell'avvio contiene `# Weaver Chat`, il limite di sotto-agenti e la lingua delle Settings di quell'avvio; Clear emette `CLEARED` dopo l'arresto del turno; il round della Chat emette `WAITING`.

      ## CHANGELOG (`requirements/docs#General`)

      Aggiungere una riga al CHANGELOG: la Chat torna a essere un brainstorming che, su ordine esplicito del developer, scrive Proposal di modifica e risoluzioni di Drift (con una scheda Draft propria sulla Board), Task e Drift singoli, e può fare domande con un round; Clear scarta le bozze scritte dalla Chat.
  q7v4up7r:
    title: Round di domande della Chat
    refs:
    - question-round
    - requirements/cli#question-round
    - requirements/tech#question-round
    - requirements/ui#question-round
    - requirements/rules#ask-conversation
    technical: false
    proposal: cqffevyy
    label: Chat Proposals
    finished: 2026-11-05T19:35:58.542Z
    body: |-
      `weaver question new` oggi è accettato solo dall'agente di un job (`WEAVER_JOB`). Deve essere accettato anche dall'agente della Chat (General) del progetto, mentre un suo turno è in corso. La Chat usa un round solo quando il developer chiede di essere interrogato così: di norma fa le domande nelle risposte.

      ## Cosa deve valere

      **Model `question-round`**, stato `Not asked`, `agent.ASK_QUESTIONS`:
      - → `Unavailable`: quando il chiamante non è né l'agente di un job di Weaver al lavoro né l'agente della Chat del progetto durante un turno, oppure Weaver non è in esecuzione.
      - → `Queued`: quando un altro round dello stesso progetto è presentato.
      - → `Presented`: altrimenti; la finestra del progetto viene in primo piano.

      Un round appartiene al suo job, oppure alla Chat, e al suo progetto; non compare mai nella finestra di un altro progetto. Gli altri stati (Queued, Presented, Answered, Abandoned, Interrupted, Unavailable) restano come sono per entrambe le origini.

      **Instradamento e trasporto (`requirements/tech#question-round`):**
      - La CLI invia `{secret, job, general, request}`, con le identità che ha (`WEAVER_JOB` e/o `WEAVER_GENERAL`).
      - Si instrada solo per `WEAVER_JOB` o, senza di essa, per `WEAVER_GENERAL`; mai per cwd, focus o percorso. I round da sotto-agenti e da git worktree arrivano al progetto del job o della Chat.
      - L'app accetta un round solo per un job che ha lanciato, Running o Waiting for the developer, oppure per la Chat viva di un progetto aperto mentre un turno è in corso. Un'identità della Chat non riconosciuta conta come nessuna identità.
      - `unavailable` immediato quando: mancano sia `WEAVER_JOB` sia `WEAVER_GENERAL`, o manca `WEAVER_ENDPOINT`; il file di runtime manca o indica un altro endpoint; la connessione fallisce; la versione del protocollo differisce; il job è sconosciuto o finito, oppure l'identità della Chat non è quella viva o non ha un turno in corso.
      - Un round della Chat porta la Chat in `Waiting for answers` e la riporta indietro quando il round si chiude: dal lato rounds questo vuol dire avvisare la Chat del progetto all'apertura (`OPENED`) e alla chiusura (`CLOSED`) del round. Lo stato `Waiting for answers` della Chat, il numero `1` sulla voce Chat della barra laterale e la fine del round con Stop, Clear, uscita di omp o chiusura della finestra appartengono al Task di `general-chat`; qui serve solo l'aggancio (un `Effect::Opened`/`Effect::Closed` con origine Chat deve raggiungere la Chat del progetto, non il registro dei job).

      **CLI (`requirements/cli#question-round`):**
      - `unavailable`: "neither in a working Weaver job nor in the project's Chat during a turn, or no app answers before submission". Il reason resta "Weaver cannot show questions for this command. Do not ask in any other way: stop and tell the developer."
      - `cancelled` (exit 1): SIGINT/SIGTERM o stdout chiuso; il round è ritirato. Stop, Clear e un'uscita dell'omp della Chat terminano il comando così.
      - `--help` dice inoltre: in Chat, solo quando il developer ha chiesto di essere interrogato con un round. Anche il testo dei risultati in `--help` e la nota di accesso (oggi "Only the agent of a job Weaver started can use it") devono nominare la Chat durante un turno.

      **UI (`requirements/ui#question-round`):** intestazione `Questions from the agent · Chat` per un round della Chat (invariati `Questions from the agent · <job name>` e `Question about <D3> · <Drift title>`).

      **Ask more (`requirements/rules#ask-conversation`, parte **Question**):** dopo le domande, l'istruzione nomina il job che ha chiesto il round (nome e identità), `Chat` per un round della Chat, o l'identità del Drift per una domanda preparata. Dal lato backend (`requirements/tech#ask-conversation`) si legge dal round presentato anche l'origine Chat. Ask more per il resto non cambia.

      ## Dove nel codice
      - `crates/core/src/question/round.rs`: `Origin` riceve una variante per la Chat (con l'identità del progetto/Chat); `Origin::job()` non la tratta come job.
      - `crates/cli/src/round.rs` (l.54–75): oggi `unavailable` senza `env.job`; deve accettare anche `env.general` e mettere nella richiesta `general`. `crates/cli/src/client.rs` (campo della richiesta), `crates/cli/src/commands/question.rs` (testi `--help`), `crates/cli/src/help.rs` l.107 e `Access::JobOrUnavailable` in `crates/cli/src/spec.rs`.
      - `src-tauri/src/cli_handler.rs` (costruzione della richiesta `question.new`) e `src-tauri/src/rounds/service.rs::handle` (l.266–): senza `request.job`, riconoscere l'identità della Chat tramite il servizio General (`src-tauri/src/general/service.rs`, campo identità) come Chat viva del progetto con turno in corso; `carry_out` instrada `Opened`/`Closed` con origine Chat verso la Chat; `src-tauri/src/rounds/host.rs` riceve gli agganci verso General.
      - `src-tauri/src/rounds/view.rs::OriginView` e `ui/src/components/questions/question-model.ts` (l.14): intestazione `Questions from the agent · Chat`.
      - `src-tauri/src/ask/build.rs` (l.300, `Origin::Agent`): ramo per la Chat che scrive `Chat`.

      ## Test
      - `src-tauri/src/rounds/service/tests.rs`: round accettato dalla Chat con turno in corso; `unavailable` per identità della Chat non viva o senza turno; aperture/chiusure notificate alla Chat.
      - Test della CLI su `round.rs` e `tests/cli-contract/contract.test.mjs` (l.410–473): il caso "senza `WEAVER_JOB` → unavailable" va cambiato in "senza `WEAVER_JOB` né `WEAVER_GENERAL`", e i controlli sul testo di `--help`.
      - `ui/src/components/questions/question-overlay.test.tsx`: intestazione per un round della Chat.
      - `src-tauri/src/ask/tests.rs`: istruzione Ask more di un round della Chat contiene `Chat`.
  v2oomx1j:
    title: Proposal scritte dalla Chat
    refs:
    - proposal
    - agent
    - requirements/cli#proposal
    - requirements/cli#project
    - requirements/cli#agent
    - requirements/data#proposal
    - requirements/rules#proposal
    - requirements/rules#job
    technical: false
    proposal: cqffevyy
    label: Chat Proposals
    finished: 2026-11-05T19:52:33.529Z
    body: "Il Machine `proposal` ora ammette che a scrivere una Proposal sia, oltre all'agente di un job, l'agente della Chat (General), su ordine esplicito del developer, e solo Proposal ordinarie (`change`) e risoluzioni (`resolution`), mai `setup` né `import`.\n\n## Cosa deve valere\n\n- **Identità della Chat sul percorso CLI.** `WEAVER_GENERAL` è l'identità che Weaver mette sull'omp della Chat; l'app la riconosce solo come la Chat viva di quel progetto, e per `question new` solo mentre è in corso un turno della Chat. Un'identità non riconosciuta vale come nessuna identità (`requirements/cli#agent`). Oggi `src-tauri/src/cli_handler.rs` (l.76–82) costruisce il `Caller` copiando `general` ma senza verificarlo per comandi diversi da `job new` e senza copiare `ask`: il `Caller` (`crates/cli/src/spec.rs:218`) deve distinguere in modo verificato tre chiamanti — job (`WEAVER_JOB` accettato da `src-tauri/src/jobs/authority.rs`), Chat (`WEAVER_GENERAL` riconosciuto, vedi `accepts_general`), altro — e i comandi `proposal new`/`proposal edit` devono riceverlo. Quando il comando gira senza app, la Chat non è riconoscibile e il chiamante è «altro».\n- **Actor `agent`.** Solo l'agente di un job tiene un Task o annuncia una scheda (`announcement new`, `job edit`, `task move` rifiutati, exit 1, nulla scritto, per ogni chiamante che non è un job); solo l'agente di un job o della Chat può chiedere un round (`question new` risponde `unavailable` per gli altri); `job new` resta per General o Ask more. Controlli: `crates/cli/src/cli.rs:450–458` e `491–497`, `exec.rs:194–213`, enum `Access` in `crates/cli/src/spec.rs:164–174` (`question.rs:17` `JobOrUnavailable` deve accettare anche la Chat). Il funzionamento del round della Chat stesso non è di questo Task.\n- **Segni nei file (`requirements/data#proposal`).** Front matter di `proposal.md`: `chat: true` quando l'ha creata l'agente della Chat, mai tolto; `chatWriting: true` mentre la Chat la scrive, messo da `proposal new` e da `proposal edit` chiamati dalla Chat e tolto alla consegna. In `crates/core/src/project/proposal/create.rs` (accanto a `job`, l.176–178) e in `crates/cli/src/commands/proposal.rs` (`new` l.251, `edit` l.714/831, `open` l.844–917).\n- **`proposal new` (`requirements/cli#proposal`).** Rifiutati: `setup` su un Model con Feature o Actor; `request` con `reworks`; `setup` e `import` dall'agente della Chat. `request` e `reworks` legano solo come permette **Bindings**; altrimenti la Proposal è creata senza vincolo con una nota che dice perché.\n- **`proposal edit`:** ogni chiamata, anche senza input, riporta una Open a Draft e lo dice; dalla Chat segna anche la Proposal come scritta dalla Chat (`chatWriting: true`) fino alla consegna. Nel Machine: `agent.EDIT_PROPOSAL` da Open → Draft; se la modifica la fa la Chat diventa una Draft che la Chat sta scrivendo finché non la consegna di nuovo. Vale anche per una Proposal non creata dalla Chat (riaperta): ha `chatWriting` ma non `chat`.\n- **`proposal move <id> open`:** consegna, toglie `chatWriting` e stampa avvisi e testo di `show`.\n- **Bindings (`requirements/rules#proposal`), controllati da `proposal new` contro il chiamante** (oggi `create.rs` l.103–148 non controlla il chiamante; il controllo diventa effettivo per tutti):\n  - `request`: un job di setup o design lega solo la richiesta nominata dalla sua istruzione, e solo con la sua prima Proposal; l'agente della Chat lega solo una richiesta scritta, non inviata, non consumata e non tenuta da un'altra Draft, con una Proposal `change`; ogni altro chiamante non lega nulla.\n  - `reworks`: un job di rework lega solo la Proposal nominata dalla sua istruzione; l'agente della Chat lega qualsiasi Proposal `change` o `resolution` rifiutata, con o senza Rework, che nessun'altra Proposal ha sostituito e su cui non lavora un job di rework, con una Proposal dello stesso tipo; ogni altro chiamante non lega nulla. La Proposal legata prende il posto di quella rifiutata nell'ordine della Board e nei `requires` delle Proposal che la aspettavano, come per un rework (stessa logica oggi usata dal rework in `transitions.rs`/`registry.rs`).\n  - Nel Machine, stato Rejected: su ordine del developer la Chat può ancora scrivere una Proposal dello stesso tipo che ne prende il posto, salvo che un'altra l'abbia già presa o un job di rework ci lavori. Le transizioni `developer.REJECT_PROPOSAL`, `developer.REWORK_PROPOSAL` e `drift.CODE_TO_BE_FIXED` → Rejected portano quindi a uno stato da cui la Chat può sostituire.\n- **`requirements/rules#job`:** l'istruzione dei job dice che i vincoli li impone la riga di comando come in **Bindings**: un job lega solo la richiesta nominata, e solo con la prima Proposal; solo un job di rework, o la Chat, lega con `reworks`. Aggiornare il testo dell'istruzione in `crates/core/src/jobs/` (dove oggi c'è la frase sui binding) di conseguenza.\n- **Motivo 1 e turno della Chat (`requirements/rules#proposal`).** `The agent is still finishing`: il job in `job` lavora ancora oppure, per una Proposal con `chat` o `chatWriting`, è in corso un turno della Chat del progetto. Si applica ad approvare, rifiutare, Rework e scartare: \n  - Draft `developer.DISCARD_PROPOSAL` → Discarded solo quando nessun job al lavoro la scrive e, per una creata o scritta dalla Chat, nessun turno della Chat è in corso; le Proposal che la richiedono restano bloccate dicendo che è stata scartata.\n  - Open `developer.APPROVE_PROPOSAL` → Approved solo quando nulla la blocca, il job che l'ha consegnata è finito, nessun turno della Chat è in corso per una creata dalla Chat e nessuna Run lavora.\n  - Draft/Open → Refining rifiutato mentre l'agente che l'ha consegnata sta ancora finendo.\n  - Codice: `AgentFacts::is_finishing` (`crates/core/src/project/board.rs` l.62–66) deve sapere del turno della Chat; i fatti li riempie `Jobs::facts_of` (`src-tauri/src/jobs/registry.rs` l.428–455) e il turno lo conosce `src-tauri/src/general/service.rs`; `gate_of`, `plan_reject`, `plan_discard` in `transitions.rs`, `approve_reasons` in `proposal/analysis.rs`.\n- **Clear.** `general-chat.CLEARED` ricevuto da Draft → Discarded: solo una Draft creata dalla Chat, non una solo riaperta; emette `proposal.DISCARDED`; le Proposal che la richiedono restano bloccate dicendo che è stata scartata. Regola: Clear della Chat scarta, dopo che il suo turno si è fermato, ogni `draft` con `chat: true` che non è `refining`, come uno scarto del developer; ogni richiesta che teneva torna a Writing. Una `draft` con `chatWriting: true` ma senza `chat` resta. Stop, uscita di omp, chiusura della finestra e uscita da Weaver non scartano nulla. Punto d'aggancio: il Clear in `src-tauri/src/general/service.rs` (`Effect::Clear`, dopo lo stop del turno) chiama lo scarto di `transitions.rs:plan_discard`; la richiesta torna modificabile tramite `crates/core/src/project/requests/store.rs`/`src-tauri/src/requests.rs`.\n- **`project show` (`requirements/cli#project`):** `inWriting` elenca le Draft con il loro job, oppure `chat: true` per una Draft che la Chat sta scrivendo (`chatWriting`). `crates/cli/src/commands/project.rs` l.80 (schema) e l.296–323, 434.\n- **Decisioni dalla Chat:** i comandi di decisione accettano ogni chiamante con le stesse condizioni; skill, istruzione della Chat e `--help` permettono all'agente di eseguirli solo quando il developer l'ha chiesto esplicitamente (aggiornare il `--help` interessato).\n- `--help` di `proposal new`/`edit`/`move` e di `question new` allineati a quanto sopra.\n\n## Test\n\n- `crates/core/src/project/proposal/create.rs` e i suoi test: binding per job (solo richiesta nominata, solo prima Proposal), Chat (richiesta libera non tenuta, `reworks` anche senza Rework, stesso tipo), altro chiamante (nessun binding, con nota); i test che oggi legano `request`/`reworks` senza chiamante vanno cambiati, non ripinnati.\n- `setup`/`import` dalla Chat rifiutati; `chat`/`chatWriting` scritti e `chatWriting` tolto da `move open`.\n- `board.rs`/`transitions.rs`/`analysis.rs`: motivo 1 per turno della Chat in corso su approva, rifiuta, Rework, scarta.\n- Clear: scarta le Draft con `chat: true` (non `refining`), lascia quelle solo con `chatWriting`, la richiesta torna a Writing; Stop non scarta (`src-tauri/src/general/tests.rs`).\n- `cli_handler.rs`: `WEAVER_GENERAL` non riconosciuto = nessuna identità; `project show` con `chat: true`; contract test `tests/cli-contract/contract.test.mjs` l.410–473 aggiornato per l'accesso di `question new` e i nuovi rifiuti."
  yncrtvz9:
    title: Vista Chat e barra laterale
    refs:
    - requirements/ui#general-chat
    - requirements/ui#General
    technical: true
    proposal: cqffevyy
    label: Chat Proposals
    finished: 2026-11-05T19:47:27.627Z
    body: |-
      La Chat ora può scrivere Proposal (bozze Draft con una scheda propria sulla Board, segnate `chat: true` quando create dalla Chat) e aprire un round di domande, durante il quale la Machine `general-chat` è nello State `Waiting for answers`. La vista della Chat e la barra laterale devono mostrarlo.

      ## Vista Chat (`requirements/ui#general-chat`)

      In `ui/src/components/general/general-view.tsx`:

      - Testo della Chat vuota (costante `EMPTY`): `Think with omp about this project. Ask it to write a Proposal or start a job of Weaver when you want one.` al posto di quello di oggi.
      - Dialogo di Clear: titolo `Clear Chat?`, descrizione `The conversation is deleted and a new one starts.` (anche durante il caricamento) o, durante un turno, `The agent's turn stops, the conversation is deleted and a new one starts.`; quando esistono Draft create in Chat, la descrizione prosegue con ` Its <n> Draft Proposal(s) are discarded.` (n = numero delle Proposal Draft con `chat: true`; non contano quelle soltanto riaperte dalla Chat, che Clear non scarta). Bottoni `Clear` (distruttivo) e `Cancel`, fuoco su `Cancel`. Il numero viene dal backend: esporlo nello snapshot della Chat (`GeneralSnapshot`, `src-tauri/src/general/service.rs:snapshot`) o leggerlo dalle schede della Board, senza contare a mano nei file.
      - Waiting for answers: mentre un round della Chat aspetta, un messaggio inviato nel frattempo mostra `Clock` + `Queued` sotto di sé finché il round non finisce (riusare la resa già esistente dei messaggi in coda).

      ## Barra laterale (`requirements/ui#General` e `requirements/ui#general-chat`)

      In `ui/src/components/sidebar.tsx` (`ShellCounts` in `ui/src/views.ts`, valori da `ui/src/components/project-content.tsx` l.~107):

      - La voce Chat mostra `1` nel colore di avviso (ambra), posizionato come il numero della voce Jobs, con tooltip `Chat is waiting for your answers`, mentre la Chat è in Waiting for answers; nessun numero quando è 0. Serve un nuovo campo in `ShellCounts` (e in `NO_COUNTS`) alimentato dallo stato della Chat.
      - Il numero della voce Jobs non conta mai la Chat.
      - La voce Board mostra il numero delle schede reali in Backlog senza le schede Draft della Chat (oggi `snapshot?.backlog.length`): escludere le schede Proposal Draft della Chat come le marca la Board.

      ## Test

      - `ui/src/components/general/general-view.test.tsx`: le asserzioni su `Talk with omp…` (l.~86, 94, 101) vanno cambiate sul nuovo testo; i test del dialogo di Clear (l.~137, 172) restano e se ne aggiunge uno con Draft della Chat che controlla la frase ` Its <n> Draft Proposal(s) are discarded.`; un test per `Queued` mentre la Chat aspetta le risposte.
      - `ui/src/components/sidebar.test.tsx`: numero `1` ambra e tooltip sulla voce Chat in attesa, nessun numero altrimenti.
      - `ui/src/components/project-content.test.tsx`: il conteggio Board esclude le schede Draft della Chat.
---
# Chat scrive Proposal e fa domande

La Chat può scrivere Proposal come Draft marcate come sue, legarle a una richiesta di design scritta o a una Proposal respinta, e Clear scarta le sue Draft. Su richiesta dello sviluppatore apre round di domande con `weaver question new` e resta in attesa delle risposte (nuovo stato Waiting for answers): il round genera una notifica, che si ritira alla chiusura. La vista Chat e la barra laterale mostrano l'attesa, la scheda Draft della Chat compare sulla Board e le skill installate hanno la sezione dedicata alla scrittura da Chat.
