---
refs:
  - window
  - requirements/ui#window
technical: false
---
# Richieste in palette

Le richieste di design non ancora inviate (bozze) diventano card vere nella colonna Backlog della Board, e la stessa cosa prende ovunque un solo nome, `Request`. Questa Task porta la palette e la sidebar della finestra in linea con questo.

## Palette

- Nel gruppo `Actions` l'azione che oggi si chiama `New Work Item` si chiama `New Request`. L'ordine del gruppo resta `Scan System`, `Run all`, `New Request`, `Open Folder…`, `Usage`. Sceglierla fa esattamente ciò che fa il pulsante `+` della Backlog (`New Request`): crea sempre una richiesta nuova e la apre nell'editor della richiesta; se l'azione è disabilitata mostra le sue ragioni in Muted e non si può scegliere, come ogni azione.
- Il gruppo `Cards` elenca, oltre alle Proposal, ai Task e ai Drift della Board (numero breve e titolo), anche le bozze di richiesta che stanno in Backlog: per ognuna la riga mostra `Request` al posto del numero breve, seguito dal titolo della bozza (lo stesso della sua card: primo titolo o prima riga, altrimenti `Untitled request`). Le bozze si trovano cercando nel titolo, senza distinzione fra maiuscole e minuscole, con le stesse regole degli altri risultati (prima i titoli che iniziano col testo, al massimo 8 righe per gruppo); come tutto il gruppo `Cards`, non compaiono a campo vuoto. I ghost dei job non sono card e non compaiono.
- Scegliere una bozza mostra la Board e apre quella bozza nell'editor della richiesta (a tutta vista, con `← Board`), non la finestra di dettaglio delle card. Scegliere una Proposal, un Task o un Drift continua a mostrare la Board e ad aprirne la finestra di dettaglio. La palette si chiude a ogni scelta.

## Sidebar

- Il numero sulla voce `Board` è il numero delle card vere in Backlog, bozze di richiesta incluse, in Muted, allineato a destra, nessuno quando è 0; con sole bozze il numero c'è e le conta. Il numero viene dalla Backlog che il backend manda nella fotografia della Board: nessun conteggio separato delle richieste nell'interfaccia. Anche il badge sull'icona nella sidebar stretta (sotto 1024 px) mostra lo stesso numero.

## Code notes
Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Palette: `ui/src/components/command-palette.tsx` — label `"New Work Item"` (azione `new-request`) intorno a riga 47; gruppo Cards intorno a riga 69, costruito da backlog e active di `useBoard().snapshot`; tipi in `ui/src/components/palette-model.ts`. La rivelazione di una card passa da `useReveal("card", …)` / `useBoardUi().openDetails`.
- Le bozze arrivano in `snapshot.backlog` come nuovo tipo di `BoardCard` prodotto dal backend (lo introduce la Task della design request: dipendenza). Senza un ramo dedicato il gruppo Cards le includerebbe chiamando `openDetails`: per le bozze serve invece mostrare la vista Board e `useRequests().openEditor(id)`. Escludere i ghost.
- Sidebar: `ui/src/components/project-content.tsx` (~riga 38, `snapshot?.backlog.length`) → `sidebar.tsx` (~righe 25-26, `card(s) in Backlog`): con le bozze nell'array il conteggio è automatico; verificare che i ghost non siano in `backlog`.
- Test da aggiornare: `command-palette.test.tsx`, `sidebar.test.tsx`, `project-content.test.tsx`.
