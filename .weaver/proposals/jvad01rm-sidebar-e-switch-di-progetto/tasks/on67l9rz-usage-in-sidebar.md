---
refs:
  - usage
  - time
  - time.USAGE_REFRESH_DUE
  - developer.OPEN_USAGE
  - project-setup
  - requirements/ui#usage
  - requirements/tech#usage
---
# Usage nella sidebar

Ogni finestra di progetto tiene in vista il limite d'uso più vicino all'esaurimento, e Weaver rilegge i limiti da solo ogni cinque minuti.

Quando la Task è finita:
- Weaver legge i limiti quando lo sviluppatore apre Usage, a ogni `Retry` e da solo con un unico timer per tutta l'app: subito, appena omp è pronto senza letture dall'avvio, poi 5 minuti dopo l'inizio dell'ultima lettura, qualunque l'abbia avviata. Il tick si salta, senza ritardarlo, quando omp non è pronto o nessuna finestra di Weaver è sullo schermo (tutte in background o ridotte a icona). Una lettura alla volta per tutta l'app: apertura, `Retry` o tick durante una lettura aspettano quella. Non si chiama mai un modello e ci si appoggia alla cache di omp, mai ai provider direttamente.
- Con Usage chiusa, la lettura da sola aggiorna il riepilogo; con Usage aperta su limiti mostrati o su un errore, il tick riporta Usage in lettura tenendo visibili i limiti precedenti, segnati come in aggiornamento.
- Il riepilogo nella sidebar è un riquadro bianco con anello, tutto un pulsante che apre Usage: `Usage` e la percentuale usata del limite più vicino all'esaurimento, una barra da 6 px neutra, Peach se quasi esaurito, Rose se esaurito, e sotto nome del limite e reset (`Claude 5 Hour · Resets in 2 h 10 min`). Il limite scelto è quello con lo stato peggiore (esaurito, poi quasi esaurito, poi gli altri) e, a parità, la frazione usata più alta, poi il reset più vicino, poi l'ordine di provider e limiti; i limiti con frazione sconosciuta contano solo se non ce ne sono altri, e allora niente percentuale né barra. Il calcolo sta nel core.
- Senza limiti letti il riepilogo dice `Reading usage…` durante una lettura, altrimenti `Open to read usage`; senza limiti nei provider connessi `No usage limits`; con l'agente non pronto `Usage needs the agent`; dopo un fallimento senza limiti mai letti `Could not read usage`. I limiti letti prima restano sempre visibili. Ogni finestra lo aggiorna appena una lettura finisce, perché l'ultimo risultato è uno solo per tutta l'app.
- Usage si apre dal riepilogo o da `Usage` nella palette; alla chiusura il focus torna dove era. A sole icone il riepilogo è un pulsante `Gauge` con tooltip e nome accessibile che riportano il suo testo.
- Il test che vietava le letture da sole è sostituito da test sul timer (tick saltati senza omp pronto o senza finestre visibili, una lettura alla volta) e sul calcolo del riepilogo.

Riferimento grafico negli Assets: `redesign-sidebar.png`/`.html` (riquadro «Usage this week»: qui mostra il limite più vicino all'esaurimento invece di un totale settimanale, che omp non fornisce).

## Code notes

Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Lettura: `Ops::usage` (30 s) → `weaver_core::omp::usage::read_usage` (`src-tauri/src/omp.rs:639`); stato in `src-tauri/src/usage.rs` (`UsageState`, `last: Option<(omp_path, Usage)>`, `UsageStatus{revision, phase, failure, usage}`); `UsageState::begin(omp_path)` si unisce a una lettura in corso; `OmpSetup::begin_usage()` (`omp.rs:300`) restituisce `None` se omp non è Ready; evento `usage-changed` a tutte le finestre (`events.rs:203`).
- Divieto attuale: test `nothing_is_read_while_omp_is_not_ready_or_by_itself` (`omp.rs:1602-1615`) e il commento di `ui/src/hooks/use-usage.ts`.
- Timer: `Scheduler::repeat(first, period, tick)` con `AbortOnDrop` e `tokio::time::sleep` (`omp.rs:694-721`), `FakeScheduler.live_timer(period)` per i test; altro esempio `updates.rs:489` `run_schedule`.
- Finestre sullo schermo: il `Registry` sa quali sono in background (`has_background`, `is_background`); per «ridotta a icona» `notifications.rs` legge già `is_visible`/`is_minimized` di ogni finestra.
- UI: `components/usage-dialog.tsx` (`STATUS_BAR` 28, testi 23 e 176), apertura e ritorno del focus in `project-window.tsx` (`setUsageOpen`, `focusUsageEntry`).
