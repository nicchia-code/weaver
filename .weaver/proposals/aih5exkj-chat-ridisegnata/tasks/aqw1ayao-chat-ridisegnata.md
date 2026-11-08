---
refs:
  - general-chat
  - requirements/ui#general-chat
---
# Chat ridisegnata

La Chat prende il disegno del redesign e mostra le Proposal consegnate dai job che ha avviato, da decidere sul posto come sulla Board.

Quando la Task è finita:
- Header: titolo `Chat`, la pill `Weaver · <stato>` con lo stato dell'agente per tutta l'app (`ready` Sage; `checking`, `installing`, `updating`, `logging in` Sky; `not installed`, `not connected` Peach), con un tooltip su cosa fare, mai il nome omp; a destra `New chat`, che è il Clear di oggi con la conferma `Start a new chat?`. La Chat resta una sola conversazione persistente.
- Conversazione centrata, larga al massimo 760 px: messaggi dello sviluppatore in bolle su Paper; messaggi dell'agente con logo, `Weaver` e ora `HH:MM`. Ogni sequenza consecutiva di `Thinking`, righe dei tool e righe dei sotto-agenti di un turno è un riquadro comprimibile, chiuso all'inizio, `Worked for <durata> · <n> steps` (`Working · <n> steps` con il punto Sky mentre è l'ultima cosa di un turno in corso); aprendolo si vedono le righe come oggi. Permessi, avvisi e messaggi non finiscono mai dentro il riquadro. Il backend registra l'istante di arrivo di ogni voce, da cui vengono durata e ora.
- Sotto la riga `Started <job name>` di un job avviato dalla Chat, o del job della richiesta che la Chat ha inviato, c'è una card per ogni Proposal che quel job ha consegnato, dal momento della consegna e anche dopo il ricaricamento della conversazione, finché la Proposal esiste; mai per una Proposal attaccata a una Drift. La card segue la Proposal dal vivo: numero, pill di stato, badge `stale` e `blocked`, titolo; i file aggiunti, cambiati e rimossi (al massimo 6, poi `and <n> more`); le Task che crea; i motivi di blocco in un riquadro Peach.
- Il piè della card ha gli stessi controlli, motivi di disattivazione e dialog della card sulla Board: `Approve` o `Refine`, `Reject` con `Rework with feedback…`, e per una Draft lasciata a metà da una rifinitura `Refine` e `Discard`. Niente scelta del modello, niente `View requirements`, niente diff, niente Undo: l'approvazione è definitiva. Decisa la Proposal, il piè diventa una riga (`Approved. <n> Tasks are on the Board.` con `Open Board`, `Rejected.`, `Sent back for rework.` con `Open conversation`, `Discarded.`); una decisione rifiutata quando si applica mostra il motivo sotto i controlli e non cambia nulla.
- Mentre la Chat aspetta le risposte del suo round, la conversazione finisce con la riga `Weaver is waiting for your answers.` e `Answer`, che riporta in primo piano il round ridotto a barra; il badge `1` della Chat nella sidebar resta.
- Campo di testo: riquadro con anello, ombra e raggio 16 px, segnaposto `Message Weaver… use @ to add assets`, tag degli asset come oggi, nessun pulsante per allegare file e nessun suggerimento; a destra il controllo del modello, poi `Send` o, durante un turno, `Stop`. Il controllo del modello mostra il modello Analysis dei Settings (`Default model` se vuoto) e apre la stessa scelta del Settings: è quel cambio di impostazione, salvato subito per tutti i progetti. È disattivato durante un turno, un caricamento o un round. Cambiare il modello Analysis, dalla Chat o dai Settings, mentre omp della Chat è avviato e nessun turno è in corso chiude omp tenendo la conversazione: il messaggio successivo lo riavvia col nuovo modello e ricarica la conversazione.
- Testi senza omp: `Think with Weaver about this project. Ask it for a Proposal or a job when you want one.`, `Starting Weaver…`, `The agent stopped: <reason>.`, `Your next message starts the agent again.`, `The agent is not installed.` con `Install the agent`.
- I test della Chat sono aggiornati, compresi raggruppamento dei passi, card della Proposal con i suoi controlli e il cambio di modello.

Riferimento grafico negli Assets: `redesign-chat.png` e `redesign-chat.html` (i chip di suggerimento, il pulsante per allegare, Undo, View diff e le sezioni «Cosa decide», «Da sapere», «Da chiarire» della tavola non vanno implementati).

## Code notes

Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Vista: `ui/src/components/general/general-view.tsx` (EMPTY 20, LOADING 21, `lastQueued()` 24, `clearButton` `GuardedButton` + `ConfirmDialog` → `general_clear`, composer `TaggedTextarea` con `aria-label` «Message omp», Stop durante `state.working`, `SubAgentConversationView` con `ownerName="Chat"`); stato in `general/use-general.tsx` (`useGeneral`, `general_snapshot`, batch `general-entries`, `general-changed`, `useChatWaiting`). La conversazione usa `EntryView` di `jobs/jobs-view.tsx`, condiviso con Jobs e Ask more: il riquadro comprimibile va costruito nella Chat sulle sequenze di voci `thinking`/`tool`, senza rompere gli altri.
- Le voci non hanno istanti (solo `note` ha `at`): `ui/src/bindings/Entry.ts` e il backend in `src-tauri/src/general/service.rs` / `jobs/conversation.rs` vanno estesi.
- Job avviati: voci `job-started` (`job`, `name`), create in `general/service.rs:1107` `started_jobs()` e, per una richiesta inviata, `requests.rs:394` `agent_send_locked` + `jobs/authority.rs:33`.
- Legame Proposal → job: chiave `job:` nel front matter della Proposal (`crates/core/src/project/proposal/create.rs:238`, letta da `proposal/data.rs:168` `job_of()`); non arriva alla UI: `CardHead` in `src-tauri/src/board.rs` (~62) ha solo `agent_finishing`. Lo snapshot della Board elenca solo Proposal Open, Refining e interrotte: per mostrare anche quelle decise serve un dato proprio della Chat.
- Decisioni: `board/proposal-card.tsx` (`ProposalControls` 335, `RejectDialog` 220, `ReworkDialog` 257, `BlockedPopover` 51, `FilesLine` 103, `TasksLine` 117); azioni in `board/board-state.tsx` (`approveProposal`, `rejectProposal`, `discardProposal`, `decide()` manda i rifiuti tra gli avvisi della Board: in Chat il motivo va mostrato nella card). `BoardProvider` avvolge tutta la finestra (`project-content.tsx:135`). `plan_discard` accetta solo Draft (`crates/core/src/project/proposal/transitions.rs:560`).
- Round: `questions/question-overlay.tsx`, `use-question-round.tsx` (`useOptionalQuestionRound()`, `act({kind:"minimize", minimized:false})` come `ask-conversation.tsx`); `GeneralState.waiting`.
- Modello: la Chat usa `models.analysis` (`crates/core/src/jobs/general.rs:561` `args()`), letto all'avvio in `src-tauri/src/general/actor.rs` `launch()`; scelta dei modelli in `settings-view.tsx` (`ModelSetting settingKey="models.analysis"`, 269) e `omp-status.tsx:330` `modelCatalogOf`; salvataggio `useSettings().save({key, value})` → `settings_set`.
- Stato dell'agente: `hooks/use-omp.tsx` (`OmpPhase`: `checking | missing | installing | updating | notConnected | loggingIn | ready`).
- Test: `general/general-view.test.tsx` (testi con omp e `h-14`).
