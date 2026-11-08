---
created: 2026-11-07T20:53:36.306Z
updated: 2026-11-07T20:53:36.306Z
summary: 'Le domande dell''agente di Chat sono una card nella conversazione che non trattiene il turno: niente modale, attesa o notifica; Ask more funziona sulla card e il codice dello stato di attesa è rimosso.'
tasks:
- bjtki7m6
- c8a6r19t
- l2nebby3
- pb2emp8h
- tyst21xz
- 78zv72h5
- r3es4uyc
- 2py2i1n6
- 6vpxagjw
cleared:
  2py2i1n6:
    title: Remove code of `general-chat/Waiting for answers`
    refs:
    - general-chat
    technical: false
    proposal: htsbjhwo
    label: Chat question cards
    finished: 2026-11-07T20:11:48.616Z
    body: ''
  6vpxagjw:
    title: Remove code of `notification/Not shown/general-chat.WAITING`
    refs:
    - notification/Not shown
    technical: false
    proposal: htsbjhwo
    label: Chat question cards
    finished: 2026-11-07T20:08:28.064Z
    body: ''
  78zv72h5:
    title: Remove code of `general-chat/Working/question-round.OPENED`
    refs:
    - general-chat/Working
    technical: false
    proposal: htsbjhwo
    label: Chat question cards
    finished: 2026-11-07T20:10:33.070Z
    body: ''
  bjtki7m6:
    title: 'Card di domande: backend e CLI'
    refs:
    - question-round
    - agent.ASK_QUESTIONS
    - developer.SEND_ANSWERS
    - requirements/cli#question-round
    - requirements/rules#question-round
    - requirements/tech#question-round
    - requirements/data#general-chat
    technical: false
    proposal: htsbjhwo
    label: Chat question cards
    finished: 2026-11-07T20:37:10.290Z
    body: |-
      L'agente di Chat non apre più round bloccanti: pubblica una card di domande nella conversazione e non aspetta. Per i job il round resta identico (bloccante, in coda, `answered`/`interrupted`/`cancelled`), come le domande preparate dei Drift. Restano fuori da questo Task: l'aspetto e l'uso della card nell'interfaccia, il badge della sidebar, la rimozione dello stato `Waiting for answers` di Chat e del suo evento (Weaver crea da sé i Task di rimozione per `general-chat/Waiting for answers`, `general-chat/Working/question-round.OPENED`, `general-chat.WAITING`, `notification/Not shown/general-chat.WAITING`) e il testo delle istruzioni dell'agente.

      ## Pubblicazione da Chat
      - `weaver question new` valida tutto l'input come oggi; da Chat legge una sola risposta ed esce, senza timeout.
      - La richiesta è accettata solo per la Chat viva di un progetto aperto durante un turno; altrimenti `unavailable` (exit 0) con il motivo di sempre.
      - Prima card del turno: diventa **Active in General**, compare subito nella conversazione, il comando stampa `{status:"published", card:<id>, message}` (exit 0); `message` dice all'agente di chiudere ora il turno senza ripetere le domande nella risposta e che le risposte arriveranno come un successivo messaggio dello sviluppatore.
      - Seconda card nello stesso turno: rifiutata intera, nulla pubblicato, `{error:{code:"card-already-published", message}}` exit 1; il messaggio chiede di mettere tutte le domande del turno in una sola card. Il turno conta dal messaggio (o dalle risposte) che lo ha avviato fino a fine, stop o errore.
      - La card non entra mai nella coda dei round del progetto, non aspetta round né li fa aspettare, non registra attese né gestori di chiusura, non cambia lo stato di Chat, non solleva notifiche. `interrupted` e `cancelled` non toccano mai una card pubblicata: la card resta.
      - Ogni card riceve alla pubblicazione un'identità di 8 caratteri `[a-z0-9]`, unica nella conversazione.

      ## Archivio delle card
      - Per progetto, le card della conversazione in ordine di pubblicazione: identità, turno che l'ha pubblicata, domande, risposte confermate e skip, stato (attiva, answered, answered with a message, removed) e, solo a finestra aperta, bozze e posizione.
      - Si riusa il modello del round (visita, preselezione alla prima visita, conferma, skip, bozze), senza riduzione né `Skip all`.
      - Attiva solo l'ultima: al massimo una card attiva per progetto, vince la più recente. Resta attiva a fine turno, dopo stop, turno fallito e uscita di omp.

      ## Invio e chiusura
      - `developer.SEND_ANSWERS` → Answered solo se ogni domanda ha risposta confermata o skip; il backend rifiuta l'invio altrimenti, qualunque cosa mandi l'interfaccia. Nessun messaggio dello sviluppatore mostrato: la card diventa il riepilogo in sola lettura.
      - `developer.SEND_MESSAGE` in Chat con card attiva → Answered with a message, nel momento dell'invio: vanno risposte confermate e skip; ogni altra domanda va come `not-answered` (scelte o testo non confermati e preselezione mai toccata compresi).
      - In entrambi i casi la card si chiude sotto lo stesso lock che prende il messaggio, poi segue il percorso di un messaggio dello sviluppatore (trattenuto durante il caricamento, steer durante un turno, avvio di omp se non è attivo).
      - Testo inviato a omp: testo dello sviluppatore, poi l'eventuale blocco assets, poi una riga vuota (nessuna se il testo è vuoto), la riga `Answers to the card of questions <card id> (added by Weaver, not written by the developer):` e alla riga dopo il JSON `{"status":"answered","answers":{<id>:{"status":"answered","selected":[...],"text"?}|{"status":"skipped"}|{"status":"not-answered"}}}`, una voce per ogni id.
      - Il blocco non si mostra mai, né dal vivo né in replay; un messaggio fatto solo del blocco non mostra alcun messaggio dello sviluppatore.

      ## Persistenza
      - Nel record di Chat `{project, sessionId, updated, cards?}`, `cards` in ordine di pubblicazione, ognuna `{id, questions, answers, state}`: `questions` come l'input del comando, `answers` solo confermate e skip, `state` `active` | `answered` | `answered-with-message`. Mai bozze, domanda in vista o testi di messaggi. Scritto a ogni cambio di una card; nulla di una card nei log.
      - `cards` assente o illeggibile vale nessuna card.
      - `New chat` (developer.CLEAR_GENERAL) e una conversazione conservata non ricaricabile (omp.SESSION_NOT_RESUMED) → Removed: card cancellate in memoria e nel record (riscritto senza `cards`), nulla arriva all'agente.
      - Dopo la riapertura le card tornano; quella attiva riparte dalla prima domanda senza risposta.

      ## Posizione
      - Dal vivo la card appartiene al turno che l'ha pubblicata e sta dopo l'ultima voce di quel turno.
      - In replay ogni messaggio con blocco risposte è abbinato per identità: la card appare subito prima, il messaggio senza blocco (o per nulla se resta vuoto). La card attiva va in fondo, dopo la conversazione e le righe `byChat`; una card chiusa non nominata da alcun messaggio va in fondo, chiusa.

      ## Ask more
      - `developer.ASK_MORE` su una domanda della card attiva: la richiesta nomina la card per identità e la domanda per posizione; il backend legge domande e opzioni e l'origine Chat, mai le bozze; nulla nella card cambia. Se la card non è più attiva non si apre nulla.

      ## `--help`
      - Testi autosufficienti, niente prima persona né opzione "Other"; in un job eseguirlo senza limite di tempo e attendere le risposte; in Chat usarlo per ogni domanda con opzioni distinte, una card per turno, poi chiudere il turno senza ripetere le domande, e leggere nel messaggio successivo i tre stati `answered`, `skipped`, `not-answered`. Sparisce "solo quando lo sviluppatore lo chiede".

      ## Code notes
      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - CLI: `crates/cli/src/round.rs` (`run`, `outcome`, `answered()` oggi rifiuta tutto tranne `answered`: aggiungere `published` e `card-already-published`); help in `crates/cli/src/commands/question.rs:11-28`, `crates/cli/src/cli.rs:505`; rigenerare `crates/core/assets/skills/weaver-design/COMMANDS.md` (e le copie `.agents/skills`); aggiornare `tests/cli-contract/contract.test.mjs:510-516`, `crates/cli/tests/question_round.rs:208`.
      - App: `src-tauri/src/rounds/service.rs` `Rounds::handle` — ramo Chat (`General::chat_of`, turno in corso) che risponde subito, senza `Queue`, waiter né `peer.on_close`; togliere `chat_round_opened/closed` da `carry_out`. Vedi `rounds/host.rs`, `rounds/view.rs` (ts-rs, mai editare `ui/src/bindings`), `cli_handler.rs`, `protocol.rs`.
      - Core: `crates/core/src/question/round.rs` (`visit`, `apply`, `Answer` + not-answered, `answered_json`, commento stale su `Origin::Chat`); macchine pure Events→Effects.
      - Invio: come `General::send_message` (`src-tauri/src/general/service.rs:549-571`, `tags::with_block`); replay in `replayed_user` (`src-tauri/src/jobs/conversation.rs`), `Entry` senza variante card. Precedente: righe `byChat` in `session_opened`/`started_jobs`.
      - Record: `GeneralRecord`/`GeneralStore` in `crates/core/src/jobs/general.rs:670-710`.
      - Ask more: `src-tauri/src/ask/build.rs` `question()`, `QuestionOrigin::Chat` in `crates/core/src/jobs/ask.rs`.
      - Test: `src-tauri/src/rounds/service/tests.rs:240-370`, fake `RoundsHost` in `src-tauri/src/drifts.rs` e `src-tauri/src/ask/tests.rs`.
  c8a6r19t:
    title: Chat senza attesa del round
    refs:
    - general-chat
    - developer.SEND_MESSAGE
    - agent
    - agent-work
    - requirements/rules#general-chat
    technical: false
    proposal: htsbjhwo
    label: Chat question cards
    finished: 2026-11-07T20:18:13.788Z
    body: |-
      La Chat (General) non apre più round di domande bloccanti: ogni domanda con opzioni distinte diventa una card di domande nella conversazione, che non trattiene mai il turno. Questo Task porta la macchina di Chat, la sua istruzione e le descrizioni dell'attore agent e della Feature Agent work al nuovo comportamento.

      Fuori da questo Task: l'archivio delle card, il comando `weaver question new` in Chat (risposta `published`, rifiuto `card-already-published`) e il formato del blocco delle risposte sono del Task `question-round`; la card nella vista, il badge e la riga sopra la casella del messaggio sono del Task dell'interfaccia. La rimozione dello stato `Waiting for answers`, della sua transizione `question-round.OPENED` in Working e dell'evento `WAITING` di Chat (con la notifica relativa) è coperta dai Task di rimozione che Weaver crea all'approvazione: qui non va duplicata, ma nessun comportamento nuovo deve dipenderne.

      ## Cosa deve valere

      **Idle**
      - `developer.SEND_MESSAGE` → Working: Weaver avvia omp se non gira, ricaricando la conversazione conservata se non è ancora tornata, e invia il messaggio, con le risposte della card di domande attiva quando ce n'è una.
      - `developer.SEND_ANSWERS` → Working: solo le risposte della card attiva di Chat; Weaver avvia omp se non gira, ricaricando la conversazione conservata se serve, e invia le risposte come messaggio dello sviluppatore.
      - `developer.CLEAR_GENERAL` → Idle: la conversazione riparte vuota, senza le sue card di domande.

      **Loading**
      - `developer.SEND_ANSWERS` → Loading: le risposte della card attiva aspettano e vengono inviate quando la conversazione è tornata (come i messaggi in attesa).
      - `developer.CLEAR_GENERAL` → Idle: il caricamento si ferma, conversazione vuota senza card.
      - Reloading: `omp.SESSION_RESUMED` mostra la conversazione com'era, con le card al loro posto (poi Idle, o Working inviando ciò che aspetta). `omp.SESSION_NOT_RESUMED` avvia una conversazione vuota con l'avviso: le card della conversazione conservata spariscono con essa.

      **Working**
      - `developer.SEND_MESSAGE` → Working: l'agente lo riceve mentre lavora, con le risposte confermate della card attiva quando c'è.
      - `developer.SEND_ANSWERS` → Working: l'agente riceve le risposte della card attiva come messaggio dello sviluppatore mentre lavora (come uno steer).
      - `agent.ASK_QUESTIONS` → Working: la card compare nella conversazione; il turno prosegue finché l'agente non lo chiude. Nessuna attesa, nessuna notifica.
      - `omp.TURN_FAILED`, `omp.EXITED`, `developer.STOP_AGENT` → Idle: una card attiva resta attiva.
      - `developer.CLEAR_GENERAL` → Idle: turno fermato, conversazione vuota senza card.
      - `window.CLOSED` → Idle: turno annullato; la conversazione è conservata con le sue card (anche da Loading, la card attiva resta attiva).

      Un messaggio scritto mentre una card è attiva la chiude: vanno le risposte confermate e i salti; ogni altra domanda va come `not-answered`. È attiva solo la card dell'ultima risposta dell'agente.

      **Istruzione di Chat, punto 8** riscritto: l'agente chiede ogni domanda con opzioni distinte con `weaver question new`, come dice `--help`, in qualunque momento e senza che lo sviluppatore lo chieda; tutte le domande di un turno in una sola card, che il comando pubblica subito; poi chiude il turno senza ripetere quelle domande nella risposta e senza aspettare. Le domande aperte, senza opzioni, restano nella risposta in testo semplice. Le risposte tornano come messaggio successivo dello sviluppatore con un blocco di risposte, di cui spiega i tre stati: `answered`, `skipped` (usa la raccomandazione e la elenca come decisione da verificare), `not-answered` (domanda ancora aperta: nessuna raccomandazione, non la decide). Form e strumento di domande di omp sono rifiutati. Sparisce la regola «round solo quando lo sviluppatore lo chiede». Il test verifica: card per domande con opzioni, una card per turno, chiusura del turno senza ripetere le domande, i tre stati; più punti 3, 4, 5, righe aggiunte in ordine e nessuna prima persona. La riga di istruzione dei job resta invariata.

      **Descrizioni**: l'attore agent dice che solo un agente di un job o di General può fare domande (round in un job, card di domande in General); la Feature Agent work dice che l'agente di General pone le domande con opzioni come card nella conversazione, che non trattengono mai il turno, e lo sviluppatore risponde lì o continua a scrivere.

      ## Code notes
      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Macchina: `crates/core/src/jobs/general.rs`, `GeneralMachine` (Events→Effects puri). Togliere l'uso di `State::WaitingForAnswers`, `Event::RoundOpened/RoundClosed`, `Effect::Waiting/EndRound`, `round_ends_without_answers` e la trattenuta dei messaggi in `send` (~300); `turn_in_progress`/`accepts_agent_calls` oggi includono WaitingForAnswers. Aggiungere l'evento delle risposte (Idle/Loading/Working, come `send`). Rivedere `turn_ended`, `stop`, `clear`, `window_closed`. Doc/diagramma di modulo `general.rs:1-36` superati; test macchina ~1068-1138.
      - Istruzione: `INSTRUCTION` `general.rs:644-664`, punto 8 ~655; test `the_instruction_of_chat_holds_the_rules_in_order_and_the_lines_of_the_settings` (~1428-1460) confronta frasi esatte (`points[7]`). Non toccare `crates/core/src/job_instruction.rs:157`.
      - Servizio: `src-tauri/src/general/service.rs` `round_opened`/`round_closed`/`chat_of` (~1250-1300), `Slot::state`, `GeneralState.waiting` (~234), `Notice::Waiting/RoundEnded/RoundClosed`, `ChatObserver::chat_waiting/round_ended/round_closed` (~97-117), `send_message` (~549-571), `session_opened` (~931-956); `general/wire.rs:27-29` `waiting`; doc `general/mod.rs:3`; replay in `general/actor.rs:487-619`.
      - Test da riscrivere: `src-tauri/src/general/round_tests.rs` (fake-acp non esegue la CLI: guida il servizio direttamente), `general/tests.rs:2515`.
      - Convenzioni: l'app esegue gli effetti fuori dal lock; observer sul proprio thread. Archivio card e blocco risposte vengono dal Task `question-round`.
  l2nebby3:
    title: Card domande nella Chat
    refs:
    - requirements/ui#question-round
    - requirements/ui#question-round/Active in General
    - requirements/ui#general-chat
    - requirements/ui#window
    technical: true
    proposal: htsbjhwo
    label: Chat question cards
    finished: 2026-11-07T20:44:55.382Z
    body: |-
      Le domande dell'agente di Chat non aprono più una finestra modale: ogni round di Chat è una **card di domande** dentro la conversazione, sotto la risposta dell'agente, e non trattiene mai il turno. Questo Task copre solo l'interfaccia; la memorizzazione delle card, il comando e la macchina di Chat sono altri Task: la vista legge le card dal backend (nuovi eventi e stato iniziale) e invia le azioni.

      ## Aspetto e comportamento della card
      - Cornice della card di Proposal della Chat; testata con l'icona `MessagesSquare`, `Questions from Weaver` e `Question <n> of <N>` oppure `Summary`.
      - Una domanda alla volta, riga di pallini numerati (l'indice stretto dell'overlay), `Back`/`Next`, Summary con `Edit` e `Send to agent`.
      - Restano: opzioni a riga intera con `Recommended` e preselezione alla prima visita, `Show details`, risposta libera per domanda, `Skip`, `Ask more` su una domanda. L'invio richiede ogni domanda risposta o saltata, come nel modale.
      - Tolti: oscuramento, trappola del focus, `Minimize` e barra in basso, `Close` con `Skip all`, `+<k> more rounds waiting`, colonna indice.
      - La card non prende mai il focus da sola; i tasti `1`–`9` e `Ctrl/Cmd+Enter` valgono solo con il focus dentro la card.
      - Solo l'ultima card è attiva; le precedenti sono in sola lettura.
      - Dopo l'invio dalla card nessun fumetto dello sviluppatore: la card diventa il Summary in sola lettura, con la pillola `Answered` / `Partly answered` / `Not answered`, e sotto compare `Starting Weaver…` finché il messaggio non è consegnato.
      - La card si può inviare anche durante il turno (arriva come un messaggio inviato mentre l'agente lavora).
      - Un messaggio scritto mentre una card è attiva la chiude: diventa il Summary in sola lettura.
      - Stop, turno fallito o uscita di omp: la card resta attiva.
      - Posizione: sotto tutto il lavoro del turno che l'ha pubblicata, mai dentro il riepilogo del lavoro; dopo una riapertura ogni card chiusa appare subito prima del messaggio dello sviluppatore che l'ha chiusa, quella attiva in fondo; l'ultima non risposta riparte dalla sua prima domanda senza risposta. Il blocco di risposte non si mostra mai.
      - Da `Ask more` su una domanda della card, `Back to questions` mostra la Chat con la card in vista, finché la card è attiva.

      ## Vista Chat
      - Spariscono la riga di attesa con `Answer`, `Queued` sotto i messaggi e lo stato occupato dovuto all'attesa.
      - Con una card attiva, sopra la casella del messaggio, una riga secondaria: `Your message also sends the answers chosen so far.`
      - Il controllo del modello non è più disabilitato da una card.

      ## Finestra
      - Con una card attiva la barra laterale mostra sulla Chat il badge `1` in Peach con tooltip `Chat has questions for you`; nessuna notifica di sistema.
      - L'intestazione dell'overlay non ha più una variante per la Chat.

      Weaver crea da sé i Task di rimozione per `general-chat/Waiting for answers` e `general-chat.WAITING`: non duplicarli.

      Aggiungere in `CHANGELOG.md` sotto `## [Unreleased]` una riga (Changed): le domande della Chat diventano una card nella conversazione.

      ## Code notes
      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - Riusare da `ui/src/components/questions/`: `QuestionPane`/`Options` (`question-pane.tsx`), `SummaryPane` (`summary-pane.tsx`), `QuestionIndex compact` e `StatusIcon` (`question-index.tsx`), `question-model.ts` invariato. `DialogBody`/`DialogFooter` sono div, usabili fuori dal dialog.
      - Da cambiare: togliere prop `askSkipAll`/pulsanti `Skip all`; il focus del titolo al mount (`useEffect(() => title.current?.focus())`) non deve agire nella card; `askMore()` oggi minimizza il round.
      - Resta solo modale in `question-overlay.tsx` (`QuestionOverlay`, `MinimizedBar`, `QuestionRoundLayer`, `waitingOf`); `onKeyDown` con `handlers: KeyHandlers` riusabile sul contenitore della card. Togliere il ramo Chat di `headerOf`.
      - Bozze nel backend; patch ottimistica come `withDraft` in `use-question-round.tsx`, azioni concatenate in ordine, gli eventi vincono. Sorgente propria per card, non la coda `Rounds`; schema di `use-chat-proposals.ts` (ascolta l'evento, poi chiedi, scarta risposte più vecchie).
      - Vista: `general-view.tsx` (riga d'attesa, `busy`, `MODEL_REASON`), `chat-conversation.tsx` (`Queued`), `chat-model.ts` `chatItems` (card mai dentro `WorkSummary`), cornice da `chat-proposal-card.tsx`.
      - Badge: `use-general.tsx` `useChatWaiting` → `project-content.tsx` → `views.ts` `ShellCounts.chatWaiting` → `sidebar.tsx` `countOf`.
      - Mai modificare `ui/src/bindings` (ts-rs). Test: `question-overlay.test.tsx`, `general-view.test.tsx`, `project-content.test.tsx`, `sidebar.test.tsx`.
  pb2emp8h:
    title: Ask more sulla card
    refs:
    - ask-conversation
    - developer.ASK_MORE
    - requirements/rules#ask-conversation
    - requirements/ui#ask-conversation
    technical: false
    proposal: htsbjhwo
    label: Chat question cards
    finished: 2026-11-07T20:42:32.257Z
    body: |-
      Ask more deve funzionare anche sulla domanda in vista della card di domande attiva di Chat, non solo sulla domanda in vista di un round.

      ## Cosa deve valere

      - **Pulsante:** sulla domanda in vista della card di domande attiva di Chat c'è un `Button` `ghost` piccolo, solo icona `MessageCircleQuestion`, tooltip `Ask an agent about this question`, nome accessibile `Ask more about this question`. È sempre abilitato. Le card chiuse (riepilogo in sola lettura) non lo hanno.
      - **Apertura (`developer.ASK_MORE`, da Not opened):** se la card è ancora attiva, Weaver prepara l'istruzione dai file in quel momento, apre la conversazione nella vista Jobs (nome `Ask question`, segnaposto `Ask about the question…`), la mostra e mette il focus nella sua casella di messaggio. Non invia nulla e omp non parte. Se la card non è più attiva (inviata, chiusa da un messaggio, sostituita da una card più recente o cancellata con `New chat`), non si apre nulla e nulla cambia.
      - **La card resta com'era:** premendo Ask more la card di Chat non viene minimizzata né alterata: stessa domanda in vista, scelte e testo scritto invariati. (Per un round invece resta il comportamento di oggi: il round va nella sua barra.)
      - **Parte Question dell'istruzione:** la domanda in vista per intero (testo, contesto, ogni opzione con etichetta, descrizione e dettagli, l'opzione raccomandata, se se ne possono scegliere più), poi testo e opzioni (etichetta e descrizione) delle altre domande della card nel loro ordine, segnando quella in vista. Come chi ha chiesto indica `Chat`. Dice che la card è ancora attiva e che l'agente non deve rispondere al posto dello sviluppatore. Mai le scelte, il testo scritto, le risposte o i salti dello sviluppatore. Nessun comando di lettura è elencato per una card di Chat.
      - **Back to questions:** una conversazione aperta da una domanda della card mostra in cima, finché quella card è attiva, il `Button` `Back to questions` (`ArrowLeft`), che mostra la vista Chat scorsa in modo che la card sia visibile, alla domanda che aveva. Scompare quando la card smette di essere attiva, comunque accada.
      - Ask more non riceve mai card di domande propria: condivide la conversazione con Chat ma non pubblica card.

      La lettura lato backend della domanda della card (per costruire l'istruzione) è a carico del Task di `question-round`: questo Task la usa.

      ## Code notes
      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - `src-tauri/src/ask/build.rs` (~295-325) `question()` legge oggi solo `Rounds::question_in_view` (round presentato): deve leggere anche la domanda della card attiva fornita dal Task `question-round`; se la card non è attiva → nulla si apre.
      - `crates/core/src/jobs/ask.rs` (~131, ~451) `QuestionOrigin::Chat`: usarla per scrivere `Chat` e "the card is still active".
      - Binding `AskTarget` (`question {round, index}`) e `AskRow.round` sono generati da ts-rs: cambiare i tipi Rust, mai `ui/src/bindings` a mano.
      - UI: in `ask-conversation.tsx` (~47-66) `Back to questions` oggi fa `round.act({kind:"minimize", minimized:false})`; per una card deve invece mostrare la vista Chat e scorrere fino alla card, alla sua domanda. `askMore()` nei pannelli minimizza il round: non farlo per la card.
      - Test: `ask.test.tsx` (~240-261), `src-tauri/src/ask/tests.rs` (~31, fake `RoundsHost`).
      - Insidia: Ask more condivide `Conversation`/codice attore con Chat; non deve mai ottenere card.
      - Esistono già Task di rimozione creati da Weaver per `Waiting for answers`/`WAITING` di Chat: non duplicarli.
  r3es4uyc:
    title: Remove code of `general-chat.WAITING`
    refs:
    - general-chat
    technical: false
    proposal: htsbjhwo
    label: Chat question cards
    finished: 2026-11-07T20:11:05.928Z
    body: ''
  tyst21xz:
    title: Chat senza notifiche
    refs:
    - notification
    - requirements/ui#notification
    technical: false
    proposal: htsbjhwo
    label: Chat question cards
    finished: 2026-11-07T20:11:06.130Z
    body: |-
      Chat (General) non solleva mai una notifica di sistema: le sue card di domande si vedono solo nella sua conversazione e nella sidebar (badge `1` in Peach, tooltip `Chat has questions for you`). Le notifiche restano solo per i job: un job finito o fermato in modo anomalo (`job.ENDED`), un job in attesa di risposte o di un permesso (`job.WAITING`), mostrate subito e solo se la finestra del progetto non è in primo piano; e `System Certificates turned on`, sempre.

      Quando è fatto:
      - Non esiste più un tipo di notifica "Question round" per Chat: il tipo `Question round` (`<project> – <job> asks a question`, `The agent is waiting for your answers.`) riguarda solo i round dei job, che mantengono la loro notifica, il ritiro alla chiusura del round (`question-round.CLOSED` → Withdrawn) e il clic che apre l'overlay del round.
      - Nessun titolo contiene `Chat`; `<project>` è il nome della cartella, `<job>` il nome fisso del job. Mai messaggi, domande, comandi, percorsi, risposte o card.
      - L'ID di ogni notifica è solo progetto + job + tipo, per sostituirla o ritirarla; nessun ID dedicato a Chat.
      - Non resta nulla del percorso di notifica di Chat: né le voci del registro delle notifiche mostrate, né il bersaglio del clic verso una domanda di Chat, né gli osservatori di apertura/chiusura dei round di Chat, né i testi, né i test che li verificano. I test dei round dei job restano e passano.

      La rimozione della transizione `notification/Not shown/general-chat.WAITING` (e dell'evento `general-chat.WAITING`) è coperta da Task di rimozione codice che Weaver crea all'approvazione: questa Task non la duplica e copre tutto il resto del percorso.

      ## Code notes
      Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

      - `src-tauri/src/notifications.rs`: rimuovere `chat_waiting` e `chat_round_closed` (righe ~9-11, 161-212) e la registrazione di `round_opened`/`round_closed` come `JobObserver` per Chat (~383), se servono solo a Chat; test ~693-790 con testi e ID di Chat da eliminare.
      - `crates/core/src/notification.rs`: rimuovere `Book::chat_question`, `chat_round_closed`, `Target::ChatQuestion` e la variante di `Destination::Round` usata solo da Chat (righe ~19, 139-161, 208, 390-448, 491); test ~854-919.
      - I chiamanti in `src-tauri/src/general/service.rs` (`Notice::Waiting/RoundEnded/RoundClosed`, `ChatObserver::*`) sono della Task sulla general chat: coordinarsi, togliendo qui solo le API e lasciando lì gli usi (o viceversa nello stesso commit) perché compili.
      - Attenzione a non toccare il percorso dei round dei job (stesso `Book`, stessi testi `asks a question`).
---
# Domande della Chat come card nella conversazione

Ogni round di domande della Chat è una card salvata dal backend (comandi general_cards, general_card_act, general_send_answers e CLI di question) e mostrata sotto il lavoro del turno, con Summary in sola lettura dopo l'invio e la pillola Answered, Partly answered o Not answered. La Chat non ha più lo stato di attesa del round, la riga con Answer, i messaggi Queued per l'attesa né la notifica di sistema; la sidebar mostra il badge 1 sulla Chat finché una card è attiva. Ask more su una domanda della card apre la conversazione con Back to questions che riporta alla card. Rimosso il codice di Working/question-round.OPENED, Waiting for answers, general-chat.WAITING e della notifica relativa.
