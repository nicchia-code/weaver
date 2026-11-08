---
status: approved
kind: change
requires:
- jvad01rm
base:
  requirements/ui/board-cards.md: e60be3f8fcdc7632d59f9010276dd9388f791a64035b4251c4761cdfd36091d2
  requirements/ui/board.md: 0dd5c65406ac9a1e073e1bd96779a12a938447e6a6329c539bea06ff4a32c00d
  requirements/ui/chat.md: new
  requirements/ui/conversations.md: 5ff3572ea2ff384a7c25c941109934ff832cbe8f282e6cce20eac9776f6bc3c4
  requirements/ui/history.md: new
  requirements/ui/views.md: 1b25fcc525fd5e5489cb84f1a45ee02482039fd639161b7d354db287cef0f0f8
created: 2026-11-07T10:08:20.127Z
label: Redesign
chat: true
---
# Viste ridisegnate

Terza Proposal del redesign (richiede «Sidebar e switch di progetto»). Ridisegna Board, Jobs, Requirements, Specs e History. Riferimento grafico negli Assets: `redesign-board`, `redesign-jobs`, `redesign-requirements`, `redesign-specs`, `redesign-history` (`.png` e `.html`).

## Decisioni prese con lo sviluppatore
- Board: niente colonna Done (restano Backlog e Active) e niente fasi Plan/Implement/Test/Verify sulle Task. Nell'header Scan, Merge (Solo mode) e «Run all · n ready»; il Run resta sulla singola Task. Sulla card della Task l'etichetta come testo, «technical» per le Task tecniche e i frammenti dei Requirements o le Specs che riguarda. «Open job» porta al job che tiene la Task. «+» del Backlog e stato vuoto: «Work item» apre l'editor della design request, «Ask in Chat» porta alla Chat. Card di Proposal e Drift, card fantasma, banner di Merge e avvisi ridisegnati nello stesso stile, con contenuti e azioni di oggi.
- Jobs: filtri All / Active (queued, running, waiting, suspended) / Ended (done, failed, stopped); la conversazione completa, ridisegnata (non una timeline riassunta); niente dati senza fonte (percentuale del sotto-agente, file cambiati finora, «checked n times»); istruzioni di Weaver comprimibili sì, riga del batch no; le conversazioni Ask more restano nell'elenco; «Open sub-agent» apre la conversazione del sotto-agente come l'albero di oggi.
- Requirements: navigazione a sinistra per aree e frammenti con il conteggio delle sezioni; il badge «changed» indica i frammenti toccati dalla Proposal in sovrapposizione; barra di focus e sovrapposizione restano, ridisegnate; «Ask for a change» (porta alla Chat con un testo già pronto) si aggiunge ad «Ask more»; la ricerca filtra sul posto il testo dei Requirements.
- Specs: niente attributo «Local only»; «Edit» del gruppo diventa «Open in editor»; riquadri delle voci neutri; tutti i campi non vuoti, note comprese.
- History: periodi Today, Last 7 days, Last 30 days, All time; riquadri dei totali (voci consegnate, Task fatte, ultima consegna) sul periodo scelto; striscia neutra.

## Assunzioni
- L'header della Board non mostra l'ora dell'ultimo scan: il dato non esiste.
- La card della Task mostra le prime righe del corpo, come nel mockup, e la pill di stato con il tempo del job che la tiene.
- In Specs resta il filtro «Agent's choice» accanto ad «All».
- Il testo già pronto di «Ask for a change» è `Change to the Requirements: ` e non viene mai inviato da solo.
- La Chat passa in un frammento proprio dei Requirements dell'interfaccia e la History pure, senza cambiarne il testo, perché i frammenti restino sotto i 12 KB.
