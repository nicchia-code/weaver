---
refs:
  - app
---
# Palette dei comandi

Da qualunque punto della finestra di un progetto una palette trova una vista, un'azione, un progetto, una card, una sezione dei Requirements o una voce di History e ci va o la esegue esattamente come il suo controllo, con le stesse conferme e gli stessi motivi di rifiuto; non cerca mai nei file del progetto, negli asset o nelle note.

Quando la Task è finita:
- `Cmd+K` su macOS e `Ctrl+K` su Windows e Linux aprono la palette ovunque nella finestra, anche con il focus nell'editor della vista Files, nelle Notes o in un campo di testo; anche il pulsante `Search or run…` della sidebar la apre. Premuto di nuovo, la chiude.
- È un Dialog senza barra del titolo, largo 560 px, nel terzo superiore della finestra: un campo di ricerca con il segnaposto `Search or run…` e i risultati nella stessa lista con ricerca del dialog di Merge. Le frecce muovono, Invio sceglie, Esc chiude e restituisce il focus a dove era.
- I risultati sono a gruppi, in quest'ordine: `Views` (ogni voce della sidebar e `Settings`), `Actions` (`Scan System`, `Run all`, `New Work Item`, `Open Folder…`, `Usage`), `Projects` (come gli elenchi dello switch di progetto), `Cards` (Proposal, Task e Drift della Board, numero breve e titolo), `Requirements` (sezioni, con Area e frammento in Muted), `History` (voci, titolo e data). Con il campo vuoto si vedono solo `Views`, `Actions` e `Projects`.
- La ricerca non distingue maiuscole e minuscole e guarda nomi, titoli e numeri brevi; prima ciò che inizia con il testo; al massimo 8 righe per gruppo.
- Scegliere una vista la mostra; un'azione si esegue come il suo controllo, con la stessa conferma (per esempio `Scan System`), mentre un'azione disattivata mostra i suoi motivi in Muted e non si può scegliere; un progetto si apre come dallo switch; una card mostra la Board e apre il suo dialog di dettaglio; una sezione mostra i Requirements scorsi fino a lei; una voce di History mostra la History, sul periodo `All` se quello scelto la nasconde, e apre la voce. La palette si chiude a ogni scelta.
- Test unitari coprono apertura da tastiera anche dentro l'editor, ordine dei gruppi, filtro e scelta di un'azione disattivata.

Riferimento grafico negli Assets: `redesign-sidebar.png`/`.html` (pulsante `Search or run…`); la palette non è disegnata e segue le Foundations.

## Code notes

Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Nessun binding `Mod-k` in `ui/src`; CodeMirror (`files/editor-state.ts:57`) lega `Shift-Mod-k`, non `Mod-k`. TipTap usa `Mod-Enter` (`notes-extensions.tsx:239`); `isModifier(event)` in `notes/notes-model.ts:97`. Un listener `keydown` in cattura su `ProjectWindow` arriva prima degli editor. Attenzione: `files/file-tabs-context.tsx:643-667` ignora i tasti mentre è aperto un `[role="dialog"]`.
- Lista: `components/ui/command.tsx` (lista con ricerca fatta in casa, usata dal dialog di Merge).
- Dati già caricati: card da `useBoard().snapshot` (`board/board-state.tsx`, `BoardCard.head{id, shortNumber, title}`; Task in `active`, Proposal e Drift in `backlog`); Requirements da `useViews().snapshot.data.requirements` (`views/views-state.tsx`, disponibile solo dentro `around`); History da `useHistory(projectId)` (`history/use-history.ts`, hook locale, non contesto); job da `useJobs()`.
- Navigazione e azioni: `useProject()` (`showView`, `openJob`) in `project-context.tsx`; Scan e Run all in `jobs/board-actions.tsx` (`scan_system`, `run_all`); New Work Item da `requests/requests-layer.tsx` (`useRequests().newRequest`); apertura del dettaglio di una card con `useBoardUi().openDetails`.
- Progetti: usare lo stesso comando ed evento dello switch di progetto (Task della stessa Proposal).
