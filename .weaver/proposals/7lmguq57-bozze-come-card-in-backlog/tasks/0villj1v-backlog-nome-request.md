---
technical: false
refs:
  - work
  - requirements/ui#work
  - requirements/ui#General
  - developer.NEW_REQUEST
  - developer.SEND_REQUEST
  - developer.DISCARD_REQUEST
  - developer.LEAVE_REQUEST
---
# Backlog e nome Request

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
