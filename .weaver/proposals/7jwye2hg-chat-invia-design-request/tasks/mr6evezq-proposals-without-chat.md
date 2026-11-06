---
refs:
  - proposal
  - requirements/rules#proposal
  - requirements/rules#job
  - requirements/data#proposal
  - requirements/cli#proposal
  - requirements/cli#project
  - requirements/cli#agent
  - requirements/ui#proposal
  - requirements/ui#General
---
# Proposal senza Chat

Una Proposal la scrive solo l'agente di un job: Chat non ne crea, non ne modifica e non ne consegna più. Quando è fatto deve valere quanto segue.

- `proposal/Draft`: la scrive l'agente di un job e la Board mostra solo il ghost di quel job. Non esiste più la card "Chat Draft", con i badge `draft` e `Chat`, `Open Chat` e `Discard`.
- `proposal/Draft/developer.DISCARD_PROPOSAL`: la Draft è scartabile quando nessun job in lavorazione la scrive, senza condizioni legate ai turni di Chat.
- `proposal/Open/developer.APPROVE_PROPOSAL`: l'approvazione è rifiutata con `The agent is still finishing` solo quando il job che l'ha consegnata lavora ancora, mai per un turno di Chat in corso.
- `proposal/Open/agent.EDIT_PROPOSAL`: la Proposal torna in Draft e basta, senza segni di scrittura da parte di Chat.
- `proposal/Rejected`: Chat non può più scrivere una Proposal che ne prenda il posto.
- Collegamenti (`requirements/rules#proposal`, `requirements/rules#job`): `request` lo lega solo un job di setup o di design alla request della sua istruzione, e solo con la prima Proposal. `reworks` lo lega solo un job di rework alla Proposal della sua istruzione. Ogni altro chiamante non lega nulla e riceve la nota che spiega perché.
- Comandi:
  - `proposal new`, `proposal edit` e `proposal move <id> open` sono rifiutati (exit 1, nulla scritto) ogni volta che c'è l'identità di Chat, senza interpellare l'app. Il messaggio dice che Chat non scrive Proposal e invia invece una request con `request new`.
  - I comandi di decisione restano permessi a tutti.
  - L'help dei comandi `proposal` non parla più di Chat.
  - `project show` non ha più `chat` in `inWriting`.
- File (`requirements/data#proposal`): `proposal.md` non ha più le chiavi `chat` e `chatWriting`. Se un file di una versione precedente le contiene, vengono ignorate in lettura e tolte alla prima scrittura di quel `proposal.md`. Una `draft` che le porta è una Draft normale senza job: compare in `weaver proposal list` e si scarta con `weaver proposal move <id> discarded`.
- L'icona Board nella barra laterale conta le card reali nel Backlog. Non c'è più niente da escludere per Chat.

## Code notes

Commit: 02129bbb382d41a3a2b2b107bf82f809097051bb

- Dati: `crates/core/src/project/proposal/data.rs` `KEYS` (:27-28), `chat_writing()` (:175-179). Creazione e modifica: `proposal/create.rs` (`Writer::Chat`, `bind_request` rami Chat ~99-160, `bind_reworks` ramo Chat ~194-212, rifiuto setup/import ~256-261, `chat`/`chatWriting` ~306-309, `EditRequest.chat` ~358-360/716-718). Consegna: `proposal/transitions.rs` (rimozione `chatWriting` :179, :397; `chat_drafts_to_clear`/`clear_chat_drafts` ~753-795); doc `proposal/mod.rs:35-39`.
- Motivo 1: `crates/core/src/project/board.rs` `AgentFacts.chat_working` (:61), `is_finishing` (:70-76), `is_chat_draft`/`ProposalState::ChatDraft` (:85-101, :183); app `src-tauri/src/board.rs:86-96` `ProposalStanding::ChatDraft`; `src-tauri/src/proposals.rs:457-465` (`from_chat` alla consegna).
- CLI: `crates/cli/src/commands/proposal.rs` (note di help :50-51, :93, :123; Who→`Writer` ~240-262; `chat: who == Who::Chat` :765). `Who::Chat` viene da `crates/cli/src/exec.rs:258-272`/`spec.rs:218-238`: il rifiuto va lì come `Failure::refused` su `caller.who == Who::Chat`, ma senza interpellare l'app. `crates/cli/src/commands/project.rs` (:80-83, :318, :381, :443-445). Rigenerare `COMMANDS.md` con `cargo run -p weaver-cli --example write_command_reference`.
- UI: `ui/src/components/board/proposal-card.tsx` (:44, badge :90-93, `ChatDraftControls` :337-360, :402), `ui/src/bindings/ProposalStanding.ts`, conteggio `ui/src/components/project-content.tsx:36-37`.
- Test da aggiornare o togliere: `board.rs:649-694`, `proposal/tests.rs` (~2145-2163, 2328-2590), `cli/commands/proposal.rs:1328-1533`, `src-tauri` `board.rs:625-672`, `cli_handler.rs:354-405`, `proposals.rs:900-930`; UI `proposal-card.test.tsx:394-468`, `board-view.test.tsx:172-184`, `project-content.test.tsx:35-36`.
