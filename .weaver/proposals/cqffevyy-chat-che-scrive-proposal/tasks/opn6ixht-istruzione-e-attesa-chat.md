---
technical: false
refs:
  - general-chat
  - developer.CLEAR_GENERAL
  - requirements/rules#general-chat
  - requirements/tech#general-chat
  - requirements/docs#General
---
# Istruzione e attesa della Chat

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
