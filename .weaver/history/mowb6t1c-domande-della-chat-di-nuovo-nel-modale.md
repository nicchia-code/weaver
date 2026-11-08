---
created: 2026-11-07T22:32:05.097Z
updated: 2026-11-07T22:32:05.097Z
summary: Le domande dell'agente di Chat tornano nell'overlay modale e il turno resta in attesa delle risposte; la card nella conversazione, il suo codice e il relativo Ask more sulla card spariscono.
tasks:
- 2cz07hy2
- 86sx5kmm
- 90c255j4
- 9i9ed90c
- 9oh9bb1h
- b52j0r4c
- cubdkp3d
- hxbsijv7
- mrrvm3ms
- rfswqdiz
- yld5qwe0
- yz50ecb8
cleared:
  2cz07hy2:
    title: Vista Chat col round modale
    refs:
    - requirements/ui#question-round
    - requirements/ui#general-chat
    - requirements/ui#window
    technical: true
    proposal: mgzxy9vz
    label: Rollback Chat question cards
    finished: 2026-11-07T22:22:56.881Z
    body: |-
      Nella conversazione della Chat non compare più la card delle domande: spariscono il titolo `Questions from Weaver`, i puntini, le pill `Answered`/`Partly answered`/`Not answered`, il riepilogo in sola lettura, `Starting Weaver…` sotto la card, `Back to questions` verso la card e la riga `Your message also sends the answers chosen so far.`.

      Quando lo sviluppatore chiede esplicitamente di essere interrogato, il round della Chat si apre nell'overlay modale come prima (sfondo attenuato, focus intrappolato, `Minimize` e barra, `Close`/`Skip all`, `+<k> more rounds waiting`, colonna indice), con intestazione `Questions from the agent · Chat`; Ask more su una domanda funziona di nuovo nel modale. I round dei job non cambiano.

      Mentre la Chat aspetta le risposte, la conversazione termina con la riga cerchiata in Lavender con `?`, il testo `Weaver is waiting for your answers.` e il pulsante secondario `Answer`, che riporta in vista l'overlay minimizzato. I messaggi inviati in quell'attesa mostrano sotto `Clock` + `Queued`.

      Nella barra laterale la voce Chat mostra il badge `1` in Peach con tooltip `Chat is waiting for your answers` mentre la Chat aspetta il suo round (il tooltip `Chat has questions for you` sparisce). Il controllo del modello della Chat è disabilitato con il motivo `Change the model when Weaver is not working` anche mentre la Chat aspetta il suo round.

      Il resto del nuovo design resta com'è: stile, barra laterale, selettore di progetto, card delle Proposal nella Chat e bozze delle design request.

      ## Code notes

      Commit: bcb006abc30b7b93d0223e6a5d5faf43d3e2eb18

      Revert SELETTIVO da `git show d267932^:<file>`; mai `git revert d267932` intero (rompe `tsc` e `src-tauri` col redesign).

      - Badge: in `ui/src/components/sidebar.tsx` `countOf()` (~34-35) riscrivere sul posto `counts.chatCard` → `counts.chatWaiting` con tooltip `Chat is waiting for your answers`, tono `peach`. NON prendere il vecchio file della sidebar. In `views.ts` `ShellCounts.chatCard` → `chatWaiting` (tenere `jobsRunning`); `project-content.tsx` lo alimenta con `useChatWaiting(projectId)` (vecchio in `use-general.tsx`).
      - Riga d'attesa: il blocco pre-d267932 di `general-view.tsx` è già nello stile nuovo (`?` lavender, `Button variant="secondary"`, `round?.act({kind:"minimize",minimized:false})`); l'hunk torna pulito.
      - Modello: `general-view.tsx:~93` `busy = working || loading || state?.waiting` e commento su `MODEL_REASON`; `model-control.tsx` invariato.
      - `chat-conversation.tsx` e `chat-draft.tsx` restano (contengono `Clock`+`Queued` in `DeveloperMessage` su `delivery.status === "queued"`); togliere solo il ramo di `ChatEntry` per messaggio vuoto con risposte di una card, e il commento.
      - Test: `sidebar.test.tsx` e `general-view.test.tsx` mescolano redesign e card: sostituire i test card con badge, riga d'attesa e modello disabilitato nello stile nuovo; in `project-content.test.tsx` togliere il mock `general_cards` (~84).
      - Binding ts-rs in `ui/src/bindings` si rigenerano, mai a mano.
  86sx5kmm:
    title: Remove code of `general-chat/Working/agent.ASK_QUESTIONS`
    refs:
    - general-chat/Working
    technical: false
    proposal: mgzxy9vz
    label: Rollback Chat question cards
    finished: 2026-11-07T22:24:45.693Z
    body: ''
  90c255j4:
    title: Ask more sul round di Chat
    refs:
    - ask-conversation
    - developer.ASK_MORE
    - requirements/rules#ask-conversation
    - requirements/ui#ask-conversation
    technical: false
    proposal: mgzxy9vz
    label: Rollback Chat question cards
    finished: 2026-11-07T22:22:47.294Z
    body: |-
      Ask more su una domanda torna a funzionare solo sulla domanda in vista di un round di domande, anche di un round di Chat, e mai su una card di domande nella conversazione di Chat, che non esiste più. Ask more apre una conversazione libera su una cosa che lo sviluppatore sta leggendo: una Proposal, una Task, una Drift, una voce della History con le Task che cita, una domanda di un round o i Requirements nel loro insieme.

      Sulla domanda in vista del round c'è un piccolo pulsante solo icona, tooltip `Ask an agent about this question`, nome accessibile `Ask more about this question`. Premendolo, la finestra mostra la vista Jobs con la nuova conversazione selezionata e il focus nel suo campo messaggio; il round viene ridotto alla sua barra, con la domanda in vista, le scelte e il testo scritto invariati, e nient'altro nel round cambia. All'agente della nuova conversazione arrivano solo le domande, mai le risposte.

      L'istruzione di Weaver per una domanda riporta per intero la domanda in vista, poi testo e opzioni delle altre domande del round nel loro ordine, segnando quella in vista; poi nome e identità del job che ha chiesto il round, `Chat` per un round di Chat o, per una domanda preparata, l'identità della Drift. Dice che il round è ancora aperto e che l'agente non deve rispondere al posto dello sviluppatore; mai scelte, testo scritto, risposte o salti dello sviluppatore.

      La richiesta di Ask more nomina il round per la sua identità e la domanda per la sua posizione; non esiste più un bersaglio "card". Se è presentato un altro round, o nessuno, non si apre nulla.

      `developer.ASK_MORE` → Closed quando la cosa scelta non esiste più, tra cui una domanda di un round che si è chiuso: non si apre nulla.

      Una conversazione aperta da una domanda mostra in cima, solo finché quel round è aperto, il pulsante `Back to questions` (`ArrowLeft`) che riapre il round dove era; scompare quando il round finisce, comunque finisca. Non porta più alla vista Chat scorrendo fino a una card.

      ## Code notes

      Commit: bcb006abc30b7b93d0223e6a5d5faf43d3e2eb18

      Revert SELETTIVO: prendere da `git show d267932^:<file>` solo il codice di Ask more legato alle card; mai `git revert d267932` per intero (commit misto: romperebbe `tsc` e `src-tauri`, es. `bindings.rs` `requests::RequestSummary`, comandi `usage_status`, `request_leave`, ecc.).

      - `src-tauri/src/ask/*`: codice puro di card, tornare alla versione pre-d267932 (richiesta che nomina round + posizione; niente target card, niente "card non più attiva").
      - Binding `AskRow`, `AskTarget` (variante card) vanno via: rigenerare i binding ts-rs in `ui/src/bindings` da `bindings.rs` `export_all`, mai a mano.
      - `src-tauri/src/rounds/*`: tornare alla versione pre-d267932 (`RoundsHost` con `chat_round_opened`/`chat_round_closed`); la minimizzazione del round all'Ask more e `Back to questions` usano il round, come prima.
      - Rimuovere ogni `BackToCard` / scroll verso la card nella vista Chat.
      - `.agents/skills/*` sono copie generate.
  9i9ed90c:
    title: Remove code of `question-round/Removed`
    refs:
    - question-round
    technical: false
    proposal: mgzxy9vz
    label: Rollback Chat question cards
    finished: 2026-11-07T22:25:00.445Z
    body: ''
  9oh9bb1h:
    title: Remove code of `question-round/Answered with a message`
    refs:
    - question-round
    technical: false
    proposal: mgzxy9vz
    label: Rollback Chat question cards
    finished: 2026-11-07T22:26:01.421Z
    body: ''
  b52j0r4c:
    title: Remove code of `general-chat/Idle/developer.SEND_ANSWERS`
    refs:
    - general-chat/Idle
    technical: false
    proposal: mgzxy9vz
    label: Rollback Chat question cards
    finished: 2026-11-07T22:26:01.532Z
    body: ''
  cubdkp3d:
    title: Round della Chat di nuovo bloccante
    refs:
    - question-round
    - agent
    - agent.ASK_QUESTIONS
    - developer.SEND_ANSWERS
    - requirements/cli#question-round
    - requirements/rules#question-round
    - requirements/tech#question-round
    technical: false
    proposal: mgzxy9vz
    label: Rollback Chat question cards
    finished: 2026-11-07T22:26:27.851Z
    body: |-
      Le domande della Chat tornano a essere un round, come quelle di un job, e la card di domande nella conversazione scompare del tutto.

      Quando termina deve valere:

      - Solo l'agente di un job avviato da Weaver o della Chat del progetto durante un turno può fare un round di domande; un comando lanciato fuori da questi è rifiutato (`Unavailable`), come quando Weaver non è in esecuzione. L'agente della Chat apre un round solo quando lo sviluppatore ha chiesto esplicitamente di essere interrogato con un round.
      - `agent.ASK_QUESTIONS`: l'agente, di un job o della Chat, chiede un round e attende le risposte senza limite di tempo. Se un altro round dello stesso progetto è presentato, il round va in coda (Queued), anche quello della Chat, in ordine FIFO per istante di invio insieme alle domande preparate dei Drift; altrimenti è presentato (Presented) e la finestra del progetto viene in primo piano. I progetti non si aspettano mai tra loro.
      - `developer.SEND_ANSWERS`: lo sviluppatore invia le risposte del round, ogni domanda risposta o saltata; l'agente riceve ogni risposta, le saltate marcate come saltate (usa la raccomandazione e la elenca come decisione da verificare).
      - `weaver question new --input -` stampa solo JSON su stdout; l'input è validato per intero prima di contattare l'app, poi il comando resta bloccato senza limite di tempo finché esiste un risultato, sia da un job sia dalla Chat. Esiti: `answered` (exit 0), `unavailable` con motivo (exit 0), errori `invalid-input` (exit 2, nomina domanda e campo), `interrupted` e `cancelled` (exit 1); Stop, Clear e un'uscita dell'omp della Chat terminano il comando in questo modo. Non esistono più l'esito `published`, l'errore `card-already-published`, lo stato di risposta `not-answered` né il blocco di risposte aggiunto ai messaggi dello sviluppatore.
      - L'app accetta un round solo per un job che ha lanciato, in esecuzione o in attesa dello sviluppatore, oppure per la Chat viva di un progetto aperto mentre un turno è in corso. Un round della Chat porta la Chat in "Waiting for answers" e la riporta indietro quando si chiude.
      - Il testo di `--help` dice: i testi sono autosufficienti, perché lo sviluppatore vede solo il round, senza prima persona e senza opzione "Other" (una risposta libera è sempre offerta); va eseguito senza limite di tempo; nella Chat, solo quando lo sviluppatore ha chiesto di essere interrogato con un round.
      - Gli stati della card (`Not asked`, `Active in General`, `Answered with a message`, `Removed`) e le regole sulle card di domande della Chat (identità, una per turno, chiusura con un messaggio) non esistono più.
      - Il testo dei comandi generato per la skill di design torna al testo del round.
      - I comandi dell'app dedicati alle card (elenco delle card, azione su una card, invio delle risposte di una card) sono rimossi, con i loro permessi e binding.
      - I round dei job non cambiano.

      ## Code notes
      Commit: bcb006abc30b7b93d0223e6a5d5faf43d3e2eb18

      Revert **SELETTIVO**: prendere il codice da `git show d267932^:<file>` solo per card/attesa; **mai** `git revert d267932` intero (rompe `tsc` e `src-tauri`: `requests_list`, `RequestSummary`).
      - Da riportare interi a `d267932^` (quelli che d267932 ha creato si eliminano): `crates/cli/src/commands/question.rs` (help), `crates/cli/src/round.rs` (test `("invalid-input", 2)` resta), `crates/cli/tests/question_round.rs`, `crates/core/src/question/{card.rs,mod.rs,round.rs}`, `src-tauri/src/rounds/*` (`chat_round_opened`/`chat_round_closed` in `RoundsHost`), `src-tauri/src/general/{cards.rs,card_tests.rs,chat_tests.rs,commands.rs}`, `general/round_tests.rs` (torna), `tests/cli-contract/contract.test.mjs`, `crates/core/assets/skills/weaver-design/COMMANDS.md`.
      - `general/service.rs`: solo gli hunk card (torna `ChatObserver`/`Notice`); NON toccare `analysis_model_changed`, arrivals (`arrivals`, `push_at`, `chat_jobs`).
      - Togliere `general_cards`, `general_card_act`, `general_send_answers` da `src-tauri/build.rs`, `capabilities/default.json`, `commands.rs`, `lib.rs`, `bindings.rs`, `ui/src/bindings/CommandName.ts`, `ui/src/lib/backend.ts`; tenere `usage_status`, `open_folder_from_window`, `project_switcher`, `project_git`, `general_proposals`, `request_leave`, `pub mod proposals`.
      - Exit 2 di `invalid-input`: `weaver_core::command::exit_code` (da `crates/cli/src/spec.rs:286`), invariato.
      - Rigenerare (mai a mano) `ui/src/bindings` (`export_all`) e `src-tauri/permissions/autogenerated/*.toml`; `.agents/skills/weaver-design/COMMANDS.md` torna al testo di HEAD o si rigenera.
      - `general/tests.rs`: non perdere il test del cambio modello.
  hxbsijv7:
    title: Remove code of `general-chat/Loading/developer.SEND_ANSWERS`
    refs:
    - general-chat/Loading
    technical: false
    proposal: mgzxy9vz
    label: Rollback Chat question cards
    finished: 2026-11-07T22:26:28.047Z
    body: ''
  mrrvm3ms:
    title: Remove code of `question-round/Active in General`
    refs:
    - question-round
    technical: false
    proposal: mgzxy9vz
    label: Rollback Chat question cards
    finished: 2026-11-07T22:27:18.010Z
    body: ''
  rfswqdiz:
    title: Notifica di Chat in attesa
    refs:
    - notification
    - requirements/ui#notification
    technical: false
    proposal: mgzxy9vz
    label: Rollback Chat question cards
    finished: 2026-11-07T22:26:27.647Z
    body: |-
      Torna la notifica di sistema quando Chat aspetta le risposte del suo giro di domande, come prima delle card di domande nella conversazione.

      A lavoro finito deve valere:

      - Una Notification in `Not shown` riceve `general-chat.WAITING` e passa a `Shown` quando la finestra del progetto non è in primo piano; se la finestra è in primo piano non viene mostrato nulla. È mostrata subito.
      - General notifica soltanto l'attesa delle risposte del proprio giro (round): nessun'altra cosa di Chat genera notifiche. Chat genera solo il tipo `Question round`: titolo `<project> – Chat asks a question`, messaggio `The agent is waiting for your answers.`, dove `<project>` è il nome della cartella del progetto.
      - La notifica nomina il progetto, `Chat` e cosa è successo, mai ciò che l'agente ha scritto, chiesto o fatto (nessun messaggio, domanda, comando, percorso, risposta o card).
      - Ogni notifica ha un ID composto da progetto + job (oppure Chat) + tipo, usato per sostituirla o ritirarla. Quella di Chat passa a `Withdrawn` con `question-round.CLOSED` quando il giro che annunciava si chiude; dove il sistema non può rimuovere una notifica consegnata, il clic su una ritirata porta solo avanti la finestra.
      - Il clic (`Clicked`) porta lo sviluppatore al progetto, sull'overlay del giro di domande.
      - Non è un tipo raggruppato: una notifica per evento. I giri dei job non cambiano.

      ## Code notes

      Commit: bcb006abc30b7b93d0223e6a5d5faf43d3e2eb18

      - Revert **selettivo**: riprendere solo il codice della notifica di Chat da `git show d267932^:<file>`; mai `git revert d267932` intero (commit misto: romperebbe `tsc` e `src-tauri`, es. `requests_list`, `usage_status`, `project_switcher`, ecc.).
      - Dove: la variante `ChatQuestion` in `crates/core/src/notification.rs` e in `src-tauri/src/notifications.rs` (le parti legate alle card vanno tolte, quelle di prima ripristinate).
      - L'evento nasce dall'osservatore di Chat: `ChatObserver`/`Notice` in `src-tauri/src/general/service.rs` e `chat_round_opened`/`chat_round_closed` in `RoundsHost` (versioni pre-d267932); il ritiro alla chiusura del giro passa da lì.
      - Test: `src-tauri/src/general/round_tests.rs` (eliminato da d267932) torna; `general/tests.rs` va ripristinato senza perdere il test del cambio di modello.
      - Convenzioni: binding ts-rs in `ui/src/bindings` e `src-tauri/permissions/autogenerated/*.toml` si rigenerano, mai a mano; commit con `Co-authored-by: Weaver`.
  yld5qwe0:
    title: Chat attende il round
    refs:
    - general-chat
    - developer.SEND_MESSAGE
    - agent-work
    - requirements/rules#general-chat
    - requirements/data#general-chat
    technical: false
    proposal: mgzxy9vz
    label: Rollback Chat question cards
    finished: 2026-11-07T22:27:39.712Z
    body: |-
      Chat torna ad attendere le risposte del proprio round dentro lo stesso turno, come prima delle card domande.

      - **Working → Waiting for answers:** `question-round.OPENED` porta Chat in `Waiting for answers` solo per un round di Chat, che l'agente ha aperto perché lo sviluppatore ha chiesto di essere interrogato così. Quando Chat inizia ad attendere viene emesso `general-chat.WAITING`.
      - **Waiting for answers:** l'agente attende le risposte del round nello stesso turno; Chat conta come in attesa dello sviluppatore.
        - `developer.SEND_MESSAGE`: il messaggio attende e non risponde mai al round; raggiunge l'agente quando il round finisce.
        - `question-round.CLOSED` → Working.
        - `omp.TURN_FAILED` → Idle: il motivo è mostrato; il round finisce senza risposte.
        - `omp.EXITED` → Idle: il motivo è mostrato; il messaggio successivo riavvia omp; il round finisce senza risposte.
        - `developer.STOP_AGENT` → Idle: il turno è annullato e il round finisce senza risposte.
        - `developer.CLEAR_GENERAL` → Idle: il turno si ferma, il round finisce senza risposte e la conversazione riparte vuota.
        - `window.CLOSED` → Idle: il turno è annullato e il round finisce senza risposte; la conversazione è conservata.
      - **Niente card:** non esiste più una card domande nella conversazione di Chat, né al ricaricamento né al Clear; un messaggio dello sviluppatore non porta più le risposte scelte di una card (sparisce l'invio di risposte di card con `developer.SEND_MESSAGE`). `developer.SEND_MESSAGE` nei job e in Ask more resta invariato: spariscono solo le risposte alle card di Chat.
      - **Istruzione dell'agente di Chat, punto 8:** chiede allo sviluppatore nella risposta, in testo semplice, e attende; usa `weaver question new`, eseguito con bash `timeout: 0`, solo quando lo sviluppatore ha chiesto di essere interrogato con un round. Form e strumento domande di omp sono rifiutati qui. I test dell'istruzione verificano questo testo.
      - **Record di Chat:** `general/<project hash>.json` nella cartella dei dati locali è di nuovo `{project, sessionId, updated}`, senza campo `cards`; scritto quando omp apre o ricarica la sessione, rimosso da Clear; senza record la Chat parte vuota.
      - **CHANGELOG:** in Unreleased si tolgono solo le due voci delle card (sotto Added "Ask more also works on the question in view of the active card…", sotto Changed "The questions of Chat are now a card…"); nessuna nuova voce.

      ## Code notes
      Commit: bcb006abc30b7b93d0223e6a5d5faf43d3e2eb18

      Revert **SELETTIVO** da `git show d267932^:<file>`, mai `git revert d267932` intero (commit misto: rompe `tsc` e `src-tauri`, es. `usage_status`, `project_switcher`, `general_proposals`, `request_leave`, model change `ModelChanged`/`analysis_model_changed`/`watch_analysis_model`, arrivals in `jobs/conversation.rs`, `general/wire.rs`).
      - Codice puro card da togliere: `src-tauri/src/general/{cards.rs,card_tests.rs,chat_tests.rs,commands.rs}`, hunk card di `general/service.rs`, la variante `card` del binding `Entry`, i binding `ChatCardState`, `ChatCardView`, `ChatCards`, `CardActionInput`; permessi `general_cards`. Il campo `waiting` di `GeneralState` invece torna, come prima di d267932.
      - Da riprendere da `d267932^`: `ChatObserver`/`Notice` in `general/service.rs`, `chat_round_opened`/`chat_round_closed` in `RoundsHost`, `src-tauri/src/general/round_tests.rs`; `general/tests.rs` ripristinato senza perdere il test del model change (`pub mod proposals` in `general/mod.rs` resta).
      - Istruzione: `INSTRUCTION` in `crates/core/src/jobs/general.rs` (~586, riga ~597), test ~1308-1343: cambia solo quella riga e le asserzioni.
      - `chat-conversation.tsx` resta: togli solo il ramo `ChatEntry` per messaggio con testo vuoto che porta risposte di card (e il commento). Mock `general_cards` in `ui/src/components/project-content.test.tsx` (~84).
      - Binding ts-rs (`ui/src/bindings`) e `src-tauri/permissions/autogenerated/*.toml` si rigenerano, mai a mano; tieni in sync `build.rs`, `capabilities`, `commands.rs`.
      - Commit con `Co-authored-by: Weaver`.
  yz50ecb8:
    title: Remove code of `general-chat/Working/developer.SEND_ANSWERS`
    refs:
    - general-chat/Working
    technical: false
    proposal: mgzxy9vz
    label: Rollback Chat question cards
    finished: 2026-11-07T22:27:16.659Z
    body: ''
---
# Domande della Chat di nuovo nel modale

Quando lo sviluppatore chiede di essere interrogato, il round della Chat si apre di nuovo nel modale (intestazione `Questions from the agent · Chat`) e Chat attende le risposte nello stesso turno. La conversazione termina con la riga `Weaver is waiting for your answers.` e il pulsante `Answer`; la voce Chat della barra laterale mostra il badge `1` in Peach, il controllo del modello è disabilitato durante l'attesa e i messaggi inviati nel frattempo restano in coda. Ask more funziona di nuovo sul round di Chat nel modale. Rimosso il codice della card di domande (stati della card, invio delle risposte da Chat, comandi e binding correlati).
