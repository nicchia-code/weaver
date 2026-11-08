---
refs:
  - job
  - requirements/ui#job
---
# Jobs ridisegnati

La vista Jobs prende il disegno del redesign: elenco filtrabile a sinistra, conversazione completa del job a destra.

Quando la Task è finita:
- In cima all'elenco un controllo a tre parti: `All <n>` (predefinito a ogni apertura della finestra), `Active <n>` (job in coda, in corso, in attesa o sospesi, e conversazioni Ask more il cui agente lavora o attende) e `Ended <n>` (job fatti, falliti o fermati). Le conversazioni Ask more inattive compaiono solo in `All`. Un filtro vuoto dice `No jobs here`; il job selezionato resta selezionato anche se il filtro ne nasconde la riga.
- L'elenco sta su Lane; ogni riga mostra nome, tipo in Muted, pill di stato a destra, l'ultima nota su al massimo due righe, l'ora di avvio e il tempo di lavoro; la riga selezionata sta su Surface con anello e ombra leggera. L'albero dei sotto-agenti sotto la riga selezionata resta com'è oggi.
- Le pill di stato: `queued` e `suspended` neutre, `running` Sky con il punto, `waiting` Peach, `done` Sage, `failed` e `stopped` Rose, sempre con testo e icona.
- A destra, l'header del job: nome, pill di stato con il tempo di lavoro per un job che lavora, `Started today at HH:MM` (o la data) e `Stop job` (distruttivo) finché il job è in coda, in corso, in attesa o sospeso, con la conferma di oggi. Lo `Stop` accanto a `Send` non c'è più.
- La conversazione è quella completa di oggi, ridisegnata: la prima voce è l'istruzione di Weaver in un riquadro comprimibile `Instructions from Weaver`, chiuso all'inizio; messaggi dello sviluppatore in bolle su Paper; messaggi dell'agente, `Thinking`, righe dei tool, sotto-agenti, permessi, note, messaggi in coda e riga di fine con le regole di oggi. Nessuna percentuale di avanzamento, nessun conteggio di file cambiati o di controlli, nessuna riga sul batch.
- Il campo per scrivere al job è un riquadro con anello, ombra e raggio 14 px, segnaposto `Steer the agent while it runs…`, invio con un pulsante primario a icona.
- I test della vista Jobs sono aggiornati, compresi filtro e posizione di `Stop job`.

Riferimento grafico negli Assets: `redesign-jobs.png` e `redesign-jobs.html` (la timeline riassunta, la percentuale del sotto-agente, «3 files changed so far», «checked 5 times» e la riga «batch» della tavola non vanno implementati).

## Code notes

Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Vista: `ui/src/components/jobs/jobs-view.tsx` (`JobsView`, `<nav aria-label="Jobs">` da 300 px, `<section aria-label="Conversation">`; `Row`, `StateBadge` esportato, mappa `BADGES` 63-71; `Conversation` con `useConversation(projectId, jobId)` da `jobs/use-jobs.tsx`, `FollowScroller`; `EntryView` esportato e condiviso con Chat e Ask more: cambiarne l'aspetto tocca anche loro; `MessageBox` con `TaggedTextarea`, Stop con `ConfirmDialog` → `job_stop`; `DisabledMessageBox`).
- Ordine delle righe: `mergeRows(jobs, asks)` in `ask/ask-model.ts`; righe Ask more `AskListRow`. Nessun filtro esiste oggi.
- L'istruzione arriva come `Entry {kind:"instruction", text}` (`ui/src/bindings/Entry.ts`), oggi resa come riquadro etichettato `Weaver` nel caso `instruction` di `EntryView`.
- Sotto-agenti: `sub-agents/sub-agent-ui.tsx` (`SubAgentList`, `SubAgentLine`, `AgentState`), `sub-agents/api.ts`.
- Test: `jobs/jobs-view.test.tsx` (fissa anche `h-14` a 69).
