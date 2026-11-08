---
refs:
  - work
  - task
  - proposal
  - drift
  - requirements/ui#work
  - requirements/ui#task
  - requirements/ui#proposal
---
# Board ridisegnata

La Board prende il disegno del redesign mantenendo le sue due colonne, Backlog e Active, e tutte le decisioni di oggi.

Quando la Task è finita:
- Le due colonne sono corsie (fondo Lane, anello interno, raggio 14 px) con un'intestazione fatta di un punto da 8 px (Lavender per Backlog, Sky per Active), titolo e numero delle card reali in Muted. Non c'è nessuna colonna Done.
- L'header della Board ha il sottotitolo `<n> active · <m> done since last scan` (ogni parte omessa quando è 0) e a destra, in ordine, `Merge` con la Solo mode attiva, `Scan System` (secondario, con la conferma di oggi) e `Run all` (primario, `Play`) con dentro `<n> ready`, le Task in To do non bloccate. I motivi di disattivazione di `Run all` e di `Scan System` restano quelli di oggi. I piè di colonna con `+ Work Item` e `Run all` non esistono più.
- L'intestazione del Backlog termina con il pulsante a icona `New Work Item` (`Plus`), che apre una nuova design request. Il Backlog vuoto mostra il disegno a fili, `Nothing to decide`, `New work items and Proposals from Chat land here for your go-ahead.`, `Work item` (apre una nuova design request) e `Ask in Chat` (mostra la Chat). Active vuoto mostra `Nothing active`.
- La card della Task: id, etichetta in Muted e, a destra, la pill di stato (`To do` neutra; `In progress` Sky; `Agent working · <tempo del job>` mentre un job la tiene, che scorre ogni secondo); titolo e primo paragrafo del corpo in testo semplice, al massimo 3 righe; chip `technical`, `blocked`, `waiting <id>` e un chip per ogni frammento di Requirements e campo di Specs nominati dai riferimenti (es. `stack.md`, `specs.yaml`), ciascuno apre il focus del primo riferimento che rappresenta; dopo una riga, il piè: mentre un job la tiene il nome di quel job, `View requirements` e `Open job`, che mostra Jobs con quel job selezionato; altrimenti `View requirements` e `Run`. Nessuna fase del lavoro è mostrata. Il bordo rotante resta per la Task tenuta da un job.
- Le card di Proposal e Drift, le card fantasma, il banner di Merge, gli avvisi e i dialog di dettaglio hanno lo stesso aspetto (Surface, anello, ombra leggera, raggio 12 px, id come etichetta, titolo a 15 px, controlli in un piè dopo una riga) con i contenuti, i controlli e i motivi di oggi.
- Il backend fa sapere alla Board quale job tiene una Task, così che la card mostri il nome e il tempo del job e `Open job` lo selezioni.
- I test della Board e delle card sono aggiornati; i test end-to-end trovano ancora `Backlog`, `Active` e la card della Task.

Riferimento grafico negli Assets: `redesign-board.png` e `redesign-board.html` (la colonna Done e le fasi della tavola non vanno implementate).

## Code notes

Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Layout: `ui/src/components/board/board-view.tsx` (`BoardView`, due `Column` in `grid-cols-2`; header con `<MergeButton/>`, Scan in `GuardedButton` + `ConfirmDialog` → `actions.scanSystem`, testo `done since last scan`; sotto `MergeBanner`, `NoticeStack`, avvisi di History); `board/column.tsx` (sezione con `rounded-lg border bg-muted/30`, `ScrollArea`, piè fisso); `BoardFooterButtons` usa `snapshot.runAllReasons`. «n ready» si calcola nella UI come in `crates/core/src/project/board.rs:140` `runnable()` (`!doing && blockedBy` e `waitingFor` vuoti). Nessun istante dell'ultimo scan esiste.
- Nuova design request: `requests/requests-layer.tsx` (`useRequests().newRequest`); mostrare la Chat: `useProject().showView("general")`; aprire un job: `useProject().openJob(jobId)` (`project-context.tsx`, nessun nonce: riaprire lo stesso job non riattiva l'effetto in `JobsView`).
- Card: registro `board/card-registry.ts` (`registerCardRenderer`), `board/default-cards.tsx`, guscio comune `board/card-shell.tsx` `CardShell` (slot `cardId`, `shortNumber`, `title`, `mark`, `accent`, `badges`, `body`, `controls`, `lead`; click fuori dai controlli → `useBoardUi().openDetails`). Task: `board/task-card.tsx` (`StatusDot`, `taskStatus()`, `RefsPopover`, `TaskControls` con `FocusButton` e Run, `TaskDetails` con `RefChip`); Drift `drift-card.tsx`; Proposal `proposal-card.tsx` (`ProposalControls` 335, resolution 475); fantasmi `ghost-card.tsx`; `kind-mark.tsx`.
- Job che tiene la Task: la Task ha solo `heldByAgent: boolean` (`ui/src/bindings/BoardCard.ts`), impostato in `src-tauri/src/board.rs:142,337` da `facts.holds(task)`; il backend conosce il job dalla chiave `holder` del file (`crates/core/src/project/board.rs:69-73` `holds()`). Aggiungere un campo con l'id del job (ts-rs rigenera il binding); nome e tempo si prendono da `useJobs().jobs` (`JobRow`).
- Test: `board-view.test.tsx`, `task-card.test.tsx`; e2e `e2e/specs/smoke.e2e.mjs` cerca `Backlog`, `Active` e il titolo della card.
