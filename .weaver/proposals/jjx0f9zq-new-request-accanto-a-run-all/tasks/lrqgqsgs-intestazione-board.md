---
technical: false
refs:
  - developer.SCAN_SYSTEM
  - requirements/ui#work
  - requirements/ui#solo-merge
  - requirements/docs#General
---
# Intestazione della Board

Nell'intestazione della Board, a destra, i comandi compaiono in quest'ordine: `Merge` (solo con Solo mode attivo, come da `requirements/ui#solo-merge`), il menu `More actions`, `New Request`, `Run all`. `Run all` resta l'ultimo, con testo e motivi di disabilitazione invariati.

`New Request` sta subito a sinistra di `Run all`: pulsante secondario con icona `Plus`, testo `New Request`, tooltip `Write a request in Markdown and send it to an agent`, sempre abilitato; apre una nuova design request come prima.

`Scan System` non è più un pulsante della riga: è l'unica voce, `Scan System…` con icona `ScanSearch`, di un menu aperto dal pulsante `More actions`, secondario e solo icona (`Ellipsis`, tooltip e nome accessibile `More actions`, sempre abilitato, alto come i vicini). La voce chiede ancora conferma (`Compare the whole System with the code?`, `Scan System` / `Cancel`); annullando non parte nulla e il conteggio non cambia; confermando, un job di scan sull'intero System passa da Not started a Queued. Mentre un merge è in corso la voce è disabilitata e mostra `A merge is in progress` sotto il suo testo in Muted (le voci di menu non hanno tooltip); lo stesso motivo è la sua descrizione accessibile.

Il pulsante solo icona `+` in fondo all'intestazione della colonna Backlog sparisce: nessuna intestazione di colonna contiene un comando; Backlog e Active mostrano solo pallino, titolo e numero. Il Backlog vuoto conserva i pulsanti `Request` e `Ask in Chat`.

La palette (⌘K) resta identica, ordine delle azioni compreso (`Scan System`, `Run all`, `New Request`, `Open Folder…`, `Usage`), e il suo `Scan System` apre la stessa conferma.

Documentazione: in `CHANGELOG.md`, sotto `[Unreleased]`, una riga `Changed` descrive la nuova posizione di `New Request` (nell'intestazione della Board, accanto a `Run all`) e di `Scan System` (nel menu `More actions`); nel README la frase su dove sta `Merge` non nomina più `Scan System` come vicino (ora sta a sinistra del menu `More actions`).

## Code notes
Commit: 36c46de1361d7597826cb4f86d991198128f3698

- Intestazione: `ui/src/components/board/board-view.tsx` (~232-257, figli di `ViewHeader`: `MergeButton`, `GuardedButton` Scan, Run all); aggiornare il commento ~145-149.
- `+` del Backlog: prop `action` di `Column` (~288-299); in `ui/src/components/board/column.tsx` (:19-20, :44) l'unico utente è la Board: rimuovere la prop.
- Conferma: `ConfirmDialog` (~320-327), stato `scanning`, `useReveal("scan", …)`: devono restare montati, la palette ne dipende.
- `useBoardActions().newRequest` (`board/board-ui.tsx`), `scanSystem` (`jobs/board-actions.tsx`).
- Menu: modello unico è lo split Rework in `proposal-card.tsx:415-431` (`GuardedButton` come `DropdownMenuTrigger asChild`). Stile del motivo in Muted: riusare quello di `command-palette.tsx:45-51`. Usare `aria-describedby` sulla voce.
- `GuardedButton` (`guarded-button.tsx`): `reasons` come `cond && "..."`, `label` per i solo icona; `size="icon"` in `ui/button.tsx`.
- Larghezza: finestra minima 960 px, controlli `shrink-0`, il sottotitolo si tronca.
- Test da riscrivere: `board-view.test.tsx` (:247-258, :301-315, :345-371, :375-383), `merge/merge.test.tsx` (:175-185, :460-472), titolo in `command-palette.test.tsx:160`.
- Docs: `README.md:49`; in `CHANGELOG.md` riga 11 dice ancora `New Work Item` (non pertinente).
