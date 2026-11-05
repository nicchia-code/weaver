---
refs:
  - drift
  - requirements/rules#drift
  - requirements/data#drift
  - requirements/cli#drift
  - requirements/ui#drift
---

# Drift da merge

Un Drift registrato dallo scan che segue un merge, la cui differenza sta nei file portati dal merge, viene da un merge. È l'agente a deciderlo guardando quei file; viene registrato quando il Drift è creato e non cambia mai più; nessun altro job può registrarlo. Quando il lavoro è finito deve valere quanto segue.

**File e comando `drift new`.** Il front matter del Drift ha la nuova chiave opzionale `merge`, solo `true`, scritta unicamente da `drift new` e mai cambiata da nessun comando (`requirements/data#drift`; va aggiunta a `DRIFT_KEYS` nella posizione della tabella, prima di `created`). `drift new` accetta il nuovo campo `merge?` (solo `true`) solo dall'agente di un job di scan avviato dopo un merge, riconosciuto tramite `WEAVER_JOB` e il campo `merged` del record del job; da qualsiasi altro chiamante (fuori da un job, altro tipo di job, scan non dopo un merge) è rifiutato con exit 1 e nulla viene scritto (`requirements/rules#drift`, `requirements/cli#drift`). `weaver drift show` e `weaver drift list` stampano `merge`. Le regole dei Drift scartati (fingerprint) e di `previous` valgono per i Drift da merge senza alcuna differenza.

**Domanda preparata.** La domanda preparata di un Drift da merge (`weaver drift edit <id> {prepared}`) deve sempre contenere la decisione `proposal` per il comportamento attuale del codice, con una Proposal allegata in Draft o Open, e `recommended` deve essere quell'opzione, con o senza opzioni di correzione; una preparazione senza di essa, o che raccomanda un'altra opzione, è rifiutata indicando il motivo. Le opzioni di correzione mantengono il loro ordine, la prima resta elencata per prima. Anche la validità di una domanda già preparata segue questa regola.

**Risoluzione.** `developer.RESOLVE_DRIFT` è rifiutato, per ogni Drift (anche con una domanda preparata valida), mentre un merge è in corso; sulla Board `Resolve` è disabilitato con il motivo `A merge is in progress`, terzo nell'ordine dei motivi, dopo `A run is in progress` (`requirements/ui#drift`). Il merge in corso è quello ricordato in `.weaver/merge/merge.json`.

**Board.** La card e il dialog dei dettagli mostrano il badge `merge` (outline) con il tooltip `Found by the scan after a merge`, dopo `recurring`, `resolving` e `prepared`. Nei dettagli di un Drift da merge `recommended` marca `Accept Proposal` invece della prima opzione di correzione; le opzioni restano numerate senza il segno e nessuna è preselezionata, quindi `Fix the code` resta disabilitato finché non se ne sceglie una. La vista del Drift deve portare l'informazione `merge` al frontend; aggiornare le fixture.

Fuori da questo Task: i testi delle skill (lo scan che decide `merge`, la preparazione della domanda) e l'ammissione dei job durante un merge in corso, che sono altri Task di questa Proposal.

Punti nel codice: `crates/core/src/project/drift/mod.rs` (`DRIFT_KEYS`); `crates/core/src/project/drift/create.rs` (`NewDrift`, `plan_new`); `crates/core/src/project/drift/prepared.rs` (`shape_problems`, `validate_input`) e `answer.rs`; `crates/cli/src/commands/drift.rs` (spec di `new` e `show`, conversione dell'input in `NewDrift`, `drift_json`); `src-tauri/src/drift_view.rs` (`DriftView`); `ui/src/components/board/drift-card.tsx` (`DriftBadges`, `DriftDetails`) e `drift-fixtures.ts`.
