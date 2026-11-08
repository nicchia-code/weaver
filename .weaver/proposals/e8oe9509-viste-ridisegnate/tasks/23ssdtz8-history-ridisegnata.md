---
refs:
  - history-entry
  - requirements/ui#history-entry
---
# History ridisegnata

La vista History prende il disegno del redesign, con i totali del periodo scelto.

Quando la Task è finita:
- L'header ha il sottotitolo `Everything that shipped, grouped as it was delivered.` e a destra il periodo: `Today`, `Last 7 days` (predefinito a ogni apertura della finestra), `Last 30 days`, `All time` (al posto di `All`), con i limiti di oggi.
- Sotto, tre riquadri calcolati sulle voci del periodo: `Delivered` (numero di voci), `Tasks done` (Task citate), `Last delivery` (`Today, HH:MM`, `Yesterday, HH:MM` o la data scritta per esteso); `—` quando il periodo è vuoto.
- Le voci, dalla più recente, raggruppate per giorno con intestazione `Today`, `Yesterday` o il giorno della settimana, seguita in Muted da data e `· <n> entries` (`· 1 entry`). Ogni gruppo è un elenco con anello: striscia neutra a sinistra, titolo, riassunto su al massimo due righe con il testo intero nel tooltip, a destra l'ora in monospace e una pill Sage `<n> tasks` (`1 task`).
- Vuoto: `Nothing done in this period`. Il dialog della voce resta com'è, con l'etichetta delle Task come testo in un badge outline.
- I test della vista History sono aggiornati.

Riferimento grafico negli Assets: `redesign-history.png` e `redesign-history.html` (le strisce colorate della tavola diventano neutre).

## Code notes

Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Vista: `ui/src/components/history/history-view.tsx` (`HistoryView({now})`, `Select` del periodo nell'header, `DayList`/`EntryRow`, `EntryDialog` con `TaskRow` e la sua copia di `LabelBadge` a 28-41).
- Modello: `history/history-model.ts` (`PERIODS`, con «All» da rinominare `All time`; `DEFAULT_PERIOD = "last-7-days"`; `groupByDay(entries, period, now)` → `DayGroup` con `header` tipo `Today · 2`; `inPeriod`, `entryTime` su `updated`; `plainSummary`).
- Dati: `history/use-history.ts` `useHistory(projectId)`; `ui/src/bindings/HistoryEntry.ts` (`tasks: HistoryTask[]`): i totali si calcolano nella UI dalle voci filtrate (numero di voci, somma di `tasks.length`, massimo di `updated`).
- Test: `history/history-view.test.tsx` (anche `labelTone` a 137-139).
