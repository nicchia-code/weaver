---
created: 2026-11-08T07:47:22.791Z
updated: 2026-11-08T07:47:22.791Z
summary: New Request passa nell'intestazione della Board accanto a Run all; Scan System va nel menu More actions; il + del Backlog sparisce e i testi per l'agente nominano il nuovo posto.
tasks:
- lrqgqsgs
- dv0ovffh
cleared:
  dv0ovffh:
    title: Testi su New Request
    refs:
    - developer.NEW_REQUEST
    - design-request/Not written/developer.NEW_REQUEST
    - requirements/ui#design-request
    technical: false
    proposal: jjx0f9zq
    label: Board header
    finished: 2026-11-08T07:28:27.577Z
    body: |-
      Il comportamento della creazione di una request non cambia: `New Request` crea sempre una request nuova, vuota, aperta per la scrittura; Start from Scratch apre la request in scrittura più vecchia, se ce n'è una, altrimenti ne crea una nuova. Il developer avvia una design request dal Board o partendo da zero: la setup request quando il Model è vuoto, una design request ordinaria altrimenti.

      Cambia solo dove il controllo viene nominato. L'istruzione che Weaver dà all'agente di Ask more dice che il developer scrive la design request con `New Request` nell'intestazione del Board (accanto a `Run all`) e la invia, non più "nel Backlog". Il testo chiama sempre la cosa una request, mai "work item".

      Lo spostamento del pulsante sul Board è oggetto di un'altra Task: questa non tocca la disposizione del Board.

      Documentazione: le due righe sotto `## [Unreleased]` del CHANGELOG che parlano del `+` del Backlog e di `New Request` nel Backlog vanno corrette sul posto, così da nominare l'intestazione del Board.

      ## Code notes
      Commit: 36c46de1361d7597826cb4f86d991198128f3698

      - `crates/core/src/jobs/ask.rs:201`, `const INSTRUCTION`: "…with `New Request` in the Backlog of the Board…" → intestazione del Board, accanto a `Run all`. Il test `every_text_starts_with_the_line_and_holds_the_fixed_rules` (`ask.rs:695-696`) fissa la frase: cambiare testo e `assert!` insieme.
      - `crates/core/src/jobs/general.rs:652,668` (Chat) non nomina il controllo: invariato.
      - Commenti da allineare: `crates/core/src/project/requests/machine.rs:47` ("`New Request` of the Backlog"), `ui/src/components/requests/use-requests.tsx:22` ("the `+` of the Backlog"). `newRequest` (:89) e `startFromScratch` (:111) restano come sono.
      - `ui/src/components/command-palette.test.tsx:160`: titolo del test "runs the Backlog's `+`" da rinominare; l'asserzione d'ordine a :163 resta.
      - `CHANGELOG.md:28-29` in `[Unreleased]`: correggere sul posto, senza id di card.
      - I template delle skill in `crates/core/assets/skills/` non nominano il controllo.
  lrqgqsgs:
    title: Intestazione della Board
    refs:
    - developer.SCAN_SYSTEM
    - requirements/ui#work
    - requirements/ui#solo-merge
    - requirements/docs#General
    technical: false
    proposal: jjx0f9zq
    label: Board header
    finished: 2026-11-08T07:29:10.740Z
    body: |-
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
---
# New Request nell'intestazione

Nell'intestazione della Board a destra compaiono Merge (solo in Solo mode), il menu More actions con la voce Scan System… (conferma invariata, disabilitata durante un merge con il motivo sotto il testo), New Request e Run all. Il pulsante + in fondo all'intestazione del Backlog è rimosso: nessuna intestazione di colonna contiene un comando. La palette dei comandi resta identica. L'istruzione di Ask more, README e CHANGELOG nominano ora New Request nell'intestazione della Board, accanto a Run all; la creazione di una request non cambia.
