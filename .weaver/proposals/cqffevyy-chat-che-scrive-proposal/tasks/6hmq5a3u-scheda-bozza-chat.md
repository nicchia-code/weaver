---
technical: true
refs:
  - requirements/ui#proposal
---

# Scheda Draft della Chat

Una Draft che l'agente della Chat sta scrivendo (front matter `chatWriting: true`) ha una propria scheda sulla Board; ogni altra Draft, salvo quella lasciata a metà da un refinement, continua a non averne (per quelle scritte da un job resta solo il ghost del job).

## Cosa deve valere (`requirements/ui#proposal`)

- La scheda sta in Backlog, nell'ordine delle schede, con la struttura di una Proposal Open del suo tipo (riga dei file, riga dei Task con la label, righe della risoluzione), aggiornata man mano che la Chat scrive, più i badge outline `draft` e `Chat` per primi.
- Unici controlli: `Open Chat` (outline, icona `MessageSquare`), che mostra la vista Chat, e `Discard` (destructive), che chiede `Discard this Draft?` / `Its files and Tasks are thrown away.` con `Discard` (destructive) e `Cancel`, focus su `Cancel`. Niente `Approve` né `Reject`.
- Mentre un turno della Chat è in corso, `Discard` è disabilitato con `The agent is still finishing`.
- Il dialogo dei dettagli mostra gli stessi controlli, poi `Ask more`.
- La Draft lasciata a metà da un refinement mantiene la sua scheda come oggi (`refinement interrupted`, `Refine`, `Discard`).

## Dove

- Dati della Board: `crates/core/src/project/board.rs` (`Board::compute` l.221, `is_being_written` l.200, `proposal_state`) deve esporre la Draft con `chatWriting` come scheda con i suoi contenuti, e se un turno della Chat è in corso (stesso fatto usato per il motivo 1); i tipi verso la UI di conseguenza.
- UI: `ui/src/components/board/board-view.tsx`, `column.tsx`, la scheda Proposal (`proposal-card`) e il suo dialogo dettagli; `Open Chat` usa la navigazione già esistente verso la vista Chat; lo scarto usa la stessa azione di `Discard` della Draft interrotta. Il conteggio della voce Board nella barra laterale, che esclude queste schede, è in un altro Task (`requirements/ui#General`).

## Test

- `ui/src/components/board/proposal-card.test.tsx` e `board-view.test.tsx`: scheda con badge `draft`/`Chat`, solo `Open Chat` e `Discard`, dialogo di conferma con i testi e focus su `Cancel`, `Discard` disabilitato con tooltip durante il turno; i test che oggi affermano che nessuna Draft non interrotta ha scheda vanno cambiati.
- Test di `board.rs` sulla presenza della scheda per `chatWriting`.
