---
refs:
  - work
  - requirements/ui#work
  - requirements/ui#window
---
# Resa ordine e animazioni

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
