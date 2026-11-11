---
created: 2026-11-11T14:28:02.597Z
updated: 2026-11-11T14:28:02.597Z
summary: La Board mette in ogni colonna prima ciò su cui si può agire, poi il resto, e le card e i ghost scorrono, appaiono e spariscono in 150 ms.
tasks:
- xvu9yfdt
- zaj1k9a6
cleared:
  xvu9yfdt:
    title: Ordine delle colonne
    refs:
    - work
    - task
    - requirements/rules#work
    - requirements/tech#design-request
    - requirements/ui#design-request
    - requirements/cli#project
    technical: false
    proposal: z1e7gudh
    label: Ordine Board
    finished: 2026-11-11T14:19:08.185Z
    body: |-
      Al termine, Weaver calcola l'ordine di Backlog e Active a ogni immagine della Board, in un'unica funzione del core, come dice `requirements/rules#work`: in ogni colonna ciò su cui il developer può agire viene prima di ciò su cui non può, e in ciascuno dei due gruppi le card che sbloccano altre card vengono per prime; i ghost stanno tra i due gruppi; in Active i Task In progress vengono subito dopo i Task avviabili.

      Deve valere:

      - **Azionabile** si decide solo con i motivi propri della card, mai con quelli che fermano tutte le card insieme (l'agente che sta ancora finendo, una Run in corso, un merge in corso, Drift in risoluzione). Task in To do: non bloccato da un Drift e non in attesa di un fix Task. Proposal o risoluzione Open non stale: nessuno dei motivi 3, 5, 6, 7 di Approve; stale: il suo refinement non è rifiutato; una Draft lasciata a metà da un refinement è azionabile; una Proposal in refinement mai. Drift aperto: nessuno dei motivi propri di `Resolve` (`Being refined`, `This Drift is being resolved`, `Another resolution is being written`, `Proposal <id> resolving Drifts awaits a decision`). Bozza di request: non vuota. Un Task In progress non è né l'uno né l'altro.
      - **Sblocca** una card su cui ora aspetta almeno un'altra card: Drift aperto che blocca un Task in To do o In progress; fix Task il cui Drift è in Code to be fixed e il cui Task di origine lo aspetta; Proposal non approvata che una Proposal in Draft, Open o Refining elenca tra le richieste; risoluzione Open (o Draft lasciata a metà da un refinement) mentre un Drift aperto fuori da essa ha `Resolve` disabilitato per causa sua.
      - **Backlog**, dall'alto: azionabili che sbloccano; altre azionabili; bozze di request non vuote; ghost; non azionabili che sbloccano; altre non azionabili, bozze vuote comprese.
      - **Active**, dall'alto: Task avviabili che sbloccano; altri Task avviabili; Task In progress; ghost; Task non avviabili che sbloccano; altri Task in To do.
      - Dentro ogni gruppo: ordine di creazione, più vecchia prima, parità per identità; per una bozza conta l'id della request. Una Proposal di rework prende ancora il posto di quella rifiutata, una Proposal raffinata conserva il proprio. I ghost restano in ordine di annuncio, i ghost di Weaver di un job al posto che avrebbero se annunciati alla creazione del job.
      - L'immagine della Board porta le card di ogni colonna in quest'ordine, bozze di request comprese (aggiunte dall'app con la stessa funzione), e il punto in cui vanno i ghost; l'interfaccia non ordina nulla. Quando cambia il motivo di una card, la nuova posizione è nella stessa immagine.
      - `weaver project show` elenca `toDecide`, `todo` e `doing` nell'ordine che le card hanno sulla Board.
      - Il blocco non cambia mai lo stato di un Task: cambia solo il suo posto sulla Board.
      - Run all, gli slot, i gruppi e `weaver task list` con `runnable` restano dal più vecchio: l'ordine di lavoro non cambia.

      ## Code notes

      Commit: dabb34a48989d772ac1d5eae7a445886c6f1c4c4

      - Order today: `crates/core/src/project/board.rs : compute` (Backlog `place` key `(created,id)` with `sort_by_cached_key`; Active `active.sort_by(compare(created…))` ~313). `ActiveEntry { doing, blocked_by, waiting_for }`, `ActiveEntry::runnable`. `compute` already gets `AgentFacts`.
      - Reasons: Task `crates/core/src/project/tasks.rs : run_reasons`, `Blocking`, `BlockingIndex::{new, of}` (`waits` = (origin, fix) pairs). Proposal `crates/core/src/project/proposal/analysis.rs : Analysis::{approve_reasons, refinement_refusals}`. Drift `crates/core/src/project/drift/reasons.rs : resolve_reasons`. Reuse them in core; don't re-derive in `src-tauri/src/proposal_view.rs` / `drift_view.rs`.
      - Reverse indexes don't exist: Drift → Tasks by inverting `blocked_by`; fix → origin by inverting `waiting_for`; `requires` via `data::requires_of`.
      - Drafts are added in `src-tauri/src/board.rs : snapshot` (`.chain(drafts…)`) from `src-tauri/src/requests.rs : RequestsService::drafts`; ghosts come from `src-tauri/src/jobs/registry.rs : ghosts_against`. The wire needs the ghost place per column.
      - CLI: `crates/cli/src/commands/project.rs : show`.
      - `plan_run_all` (`crates/core/src/jobs/admission.rs`) reuses `compute` only for reasons; keep slots in `tasks.rs : pick_slots` oldest first.
      - Tests pinning old order to rewrite: `board.rs` `active_holds_todo_and_doing_tasks_oldest_first_blocked_ones_included`, `the_backlog_holds_open_proposals_and_open_drifts_in_one_creation_order`; `src-tauri/src/board.rs` `the_drafts_come_last_in_backlog_oldest_first_…`; `requests.rs` `the_drafts_are_oldest_first_…`. Keep `proposal/tests.rs::a_rework_takes_the_place_of_the_rejected_proposal_on_the_board` passing.
  zaj1k9a6:
    title: Resa ordine e animazioni
    refs:
    - work
    - requirements/ui#work
    - requirements/ui#window
    technical: false
    proposal: z1e7gudh
    label: Ordine Board
    finished: 2026-11-11T14:24:11.231Z
    body: |-
      Al termine, la Board mostra le card di Backlog e Active nell'ordine che arriva dal backend (`requirements/rules#work`), senza riordinarle: prima ciò su cui il developer può agire, poi i ghost, poi ciò su cui non può. I ghost stanno nel punto che l'immagine della Board indica per ogni colonna, non più in fondo. Le card non azionabili hanno lo stesso aspetto delle altre: nessun separatore, intestazione o stile diverso.

      Deve valere:

      - Una card o un ghost che resta nella sua colonna e cambia posto scorre al nuovo posto in 150 ms, ease-out; una card o un ghost che entra appare in dissolvenza e uno che esce svanisce in dissolvenza, negli stessi 150 ms. Il cambio parte nell'immagine che lo causa; numeri delle colonne, sottotitolo dell'intestazione, dialog dei dettagli e focus cambiano subito, come senza animazione.
      - Quando arriva la card di un ghost, nella stessa immagine il ghost inizia a svanire e la card inizia ad apparire al suo posto nell'ordine.
      - Una card o un ghost che esce è `inert` dall'inizio della dissolvenza: niente clic, niente focus, nascosto ai lettori di schermo.
      - Una card che si sposta conserva il focus.
      - Quando esce l'ultima card o l'ultimo ghost di una colonna, lo stato vuoto (`Nothing to decide`, `Nothing active`) appare in dissolvenza mentre quella card svanisce.
      - Con la riduzione del movimento ogni cambio è istantaneo.
      - Nella palette il gruppo `Cards` elenca Proposal, Task, Drift e bozze di request nell'ordine della Board, Backlog poi Active; i nomi che iniziano con il testo cercato restano per primi.

      ## Code notes

      Commit: dabb34a48989d772ac1d5eae7a445886c6f1c4c4

      - `ui/src/components/board/board-view.tsx : CardList` renders `snapshot.active` / Backlog as given; `GhostList` renders ghosts after the cards today; `ready` count ~236. The ghost place in each column comes from the Board snapshot built in `src-tauri/src/board.rs : snapshot` (ordering in `crates/core/src/project/board.rs : compute`); the UI never sorts (module doc of `src-tauri/src/board.rs`).
      - Cards: `task-card.tsx`, `proposal-card.tsx`, `drift-card.tsx`, `request-card.tsx` use `ui/src/components/guarded-button.tsx : GuardedButton`; nothing there changes for the order.
      - Palette: `ui/src/components/command-palette.tsx` (Cards group).
      - Tests: `board-view.test.tsx` "keeps the backend's order of the cards" (~188) and the ghosts-after-cards tests pin today's placement; `request-card.test.tsx` (First/Second).
      - Pitfalls: the Board's "same picture" rules (ghost ↔ card, request draft ↔ job ghost, last card ↔ empty state) must still start in one render; a leaving element kept only for its fade must not count, take focus or be read. Details dialogs follow their card by identity and must not lose focus when the card moves. Reduced motion: no transition at all.
---
# Ordine e animazioni della Board

Un'unica funzione del core ordina Backlog e Active dagli stessi fatti dei motivi di disabilitazione: prima le card azionabili che sbloccano altre, poi le altre azionabili, le bozze di richieste, i ghost e le card non azionabili. Weaver project show e la palette elencano le card nello stesso ordine. L'interfaccia non riordina mai: mostra l'ordine ricevuto, con scorrimento, dissolvenza in entrata e in uscita (inert durante l'uscita), focus mantenuto e nessun movimento con la riduzione del movimento.
