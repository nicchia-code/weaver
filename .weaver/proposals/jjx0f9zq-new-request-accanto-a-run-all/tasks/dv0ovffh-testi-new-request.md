---
technical: false
refs:
  - developer.NEW_REQUEST
  - design-request/Not written/developer.NEW_REQUEST
  - requirements/ui#design-request
---

# Testi su New Request

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
