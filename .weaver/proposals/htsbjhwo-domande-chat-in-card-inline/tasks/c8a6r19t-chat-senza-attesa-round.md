---
technical: false
refs:
  - general-chat
  - developer.SEND_MESSAGE
  - agent
  - agent-work
  - requirements/rules#general-chat
---
# Chat senza attesa del round

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
