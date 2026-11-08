---
refs:
  - task
  - proposal
  - drift
  - app
  - requirements/ui#General
  - requirements/ui#task
  - requirements/ui#proposal
  - requirements/ui#drift
  - requirements/ui#app
---
# Colori di stato

In tutta l'interfaccia il colore dice solo uno stato, con i cinque fili del logo, ognuno con base, tint e ink: Sky (`#96BBE0` / `#E6EFF8` / `#2E5F8F`), Sage (`#9DC09A` / `#E9F2E7` / `#3F6B3C`), Peach (`#EDB08F` / `#FBEDE4` / `#9A4A22`), Lavender (`#A996D6` / `#EFEAF8` / `#5B4A99`), Rose (`#E3A3B5` / `#F9E8ED` / `#9B3B57`).

Quando la Task è finita:
- Sky indica in coda, in corso, in rifinitura e ciò che un agente sta facendo ora (un job che tiene una Task, una lettura in corso); Peach indica in attesa dello sviluppatore, stale, blocked, quasi esaurito, una Drift e gli avvisi; Lavender una Proposal, un question round e le scelte dell'agente; Sage fatto, validato, connesso e successo; Rose fallito, fermato, esaurito, errori e controlli distruttivi; sospeso e ogni altro stato restano neutri (Ink 2 su Lane).
- Dove un altro Requirement nomina un colore per uno di questi stati vale il filo dello stato; altrimenti blu e ciano diventano Sky, verde Sage, ambra, giallo e il colore di avviso Peach, viola Lavender, rosso e il colore di errore Rose. Il primario blu diventa Ink, tranne i link nel testo e i tag `@` nei campi di testo, che usano l'ink di Sky. I toast di avviso sono Peach, quelli di errore Rose.
- Le etichette non hanno più colore: sparisce il bordo sinistro colorato delle card delle Task e il badge dell'etichetta (su Task, Proposal e voci di History) è un badge outline col solo testo. Il calcolo del colore dall'etichetta non esiste più.
- Board: il punto di una Task In progress è Sky; i badge `blocked` e `waiting` sono Peach; il bordo rotante della Task tenuta da un job passa per la base e l'ink di Sky (statico Sky con movimento ridotto). Proposal: `stale` e `blocked` Peach, `refining` Sky; la card di una resolution ha bordo sinistro e tint Lavender; `Refine` è un pulsante primario (Ink) con `Sparkles`, non più giallo. Drift: bordo sinistro da 3 px Peach.
- Requirements e Specs: sovrapposizione di una Proposal con aggiunto Sage, cambiato Peach, rimosso Rose (ink sulla sua tint), sempre con l'etichetta testuale; le sezioni a fuoco hanno bordo e tint Sky.
- Nessuno stato è detto dal solo colore: ogni punto, bordo o tint ha testo, badge o icona e un nome accessibile.
- I test che verificano colori o etichette colorate sono aggiornati o rimossi.

Riferimento grafico negli Assets: `redesign-foundations.png`/`.html` (fili e loro significato) e `redesign-board.png`/`.html`, `redesign-jobs.png`/`.html` per gli stati in uso.

## Code notes

Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Il posto naturale della mappa è `ui/src/lib/tones.ts` (`TONES`, `TONE_TEXT`, `TONE_BORDER` scritti per esteso per Tailwind, `SEMANTIC_TONE`). Consumatori: `board/kind-mark.tsx:8-13`, `sidebar.tsx:69`, `omp-status.tsx:137,182,194`, `usage-dialog.tsx:27-28`, `notes/notes-view.tsx:19`, `project/empty-model.tsx:80`.
- Circa 25 file scrivono classi `*-600 dark:*-400` direttamente: `components/ui/alert.tsx:12-14`, `badge.tsx:16-19`, `button.tsx:24` (variante `refine` gialla), `input.tsx:11`, `sonner.tsx:19-20`, `assets/tagged-textarea.tsx:167`, `settings-view.tsx`, `ask/ask-conversation.tsx:81,89`, `general/general-view.tsx:112,120`, `jobs/jobs-view.tsx:63-71` (mappa `STATE` dei badge dei job), `project/*`, `questions/*`, `sub-agents/sub-agent-ui.tsx:28`, `update-banner.tsx:57`, `views/change-badge.tsx:24-28`, `views/focus-bar.tsx:26-27`, `files/file-tabs.tsx:47`, `assets/asset-grid.tsx:314`, `board/drift-card.tsx:97`, `board/proposal-card.tsx:476`, `board/task-card.tsx:45`.
- Etichette: `lib/label-colour.ts` e il duplicato `lib/label-tone.ts`; consumatori `board/task-card.tsx:17,141-146,158` (`LabelBadge`, `leftBorderClass`, riusato da `proposal-card.tsx:124,581`) e `history/history-view.tsx:28-41` (seconda copia di `LabelBadge`). Test da rivedere: `lib/label-colour.test.ts`, `board/task-card.test.tsx:13,154-155`, `history/history-view.test.tsx:14,137-139`.
- Bordo rotante: `.task-held` in `ui/src/index.css:144-162` con `#2563eb`/`#06b6d4` scritti a mano.
