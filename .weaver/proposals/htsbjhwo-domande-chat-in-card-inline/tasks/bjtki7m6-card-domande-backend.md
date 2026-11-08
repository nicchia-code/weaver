---
technical: false
refs:
  - question-round
  - agent.ASK_QUESTIONS
  - developer.SEND_ANSWERS
  - requirements/cli#question-round
  - requirements/rules#question-round
  - requirements/tech#question-round
  - requirements/data#general-chat
---

# Card di domande: backend e CLI

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
