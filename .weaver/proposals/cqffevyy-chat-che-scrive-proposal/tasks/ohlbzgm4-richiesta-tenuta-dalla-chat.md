---
technical: false
refs:
  - design-request
  - agent.OPEN_PROPOSAL
  - requirements/data#design-request
  - requirements/ui#design-request
---

# Richiesta tenuta dalla Chat

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
