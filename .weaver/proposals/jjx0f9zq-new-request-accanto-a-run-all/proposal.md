---
status: approved
kind: change
base:
  model/actors/developer.md: 52f6906e99f52306627e6c861c1b0a010343f02fef06cf4f7bf2e36fd6b16dc5
  requirements/ui/board.md: e45699a7200685b20f7cb1c92a268135ad38a24d93e5714426f8e9d768653861
  requirements/ui/look.md: cb5c3c06cfdef1424ca2dfd2791a54432123dcb03d5f499403949f13dbf554cf
  requirements/ui/requests.md: b828b1a1165296227e6f5a7cd4e208d3924cf187d294f7db637af0a3cc56bacb
  requirements/ui/solo-mode.md: 86916ddb0f571f2080c3458eaec5cb833c6586841682377c49f3535c8d58e983
created: 2026-11-08T07:18:36.982Z
request: 2026-11-08T08-14-10
label: Board header
job: ix3f18bq
---
# New Request accanto a Run all

La richiesta chiede di portare il `+` di `New Request` accanto a `Run all`, di trovare un altro posto per `Scan System` e di togliere il `+` contestuale in testa alla colonna Backlog.

## Decisioni
- `New Request` va nell'intestazione della Board, subito a sinistra di `Run all`: pulsante secondario con icona `Plus` e testo `New Request`, stesso tooltip (`Write a request in Markdown and send it to an agent`), sempre abilitato; `Run all` resta l'ultimo.
- Il `+` in fondo all'intestazione della Backlog sparisce: nessuna colonna ha più controlli nella sua intestazione.
- `Scan System` lascia la riga dell'intestazione e va in un menu `⋯` (pulsante icona `Ellipsis`, `More actions`) tra `Merge` e `New Request`, con la voce `Scan System…` (`ScanSearch`); la conferma resta la stessa; con un merge in corso la voce è disabilitata e mostra `A merge is in progress` sotto di sé in testo Muted.
- La Backlog vuota tiene il suo pulsante `Request`.
- La palette (⌘K) resta com'è, ordine delle azioni compreso.
- Di conseguenza `Merge` sta a sinistra del menu `⋯`, e i testi che indicano `New Request` "nella Backlog" (Modello, Requirements, testo di Ask more per l'agente) indicano ora l'intestazione della Board.

## Scelte dell'agente da verificare
- Il pulsante `More actions` è un pulsante secondario con sola icona, alto 36 px come i suoi vicini.
- L'icona `Ellipsis` è scritta direttamente nel Requirement della Board e non nella tabella delle icone dell'aspetto generale, il cui fragment supera già 12 KB.
