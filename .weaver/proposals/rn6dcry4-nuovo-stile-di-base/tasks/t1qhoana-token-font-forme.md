---
technical: true
refs:
  - requirements/ui#General
  - specs#frameworks
---
# Token, font e forme

L'interfaccia di Weaver adotta le fondamenta visive del redesign in tutte le sue superfici, in tema chiaro e scuro, mantenendo i componenti shadcn/ui ridisegnati (nessuna copia al pixel di stili inline).

Quando la Task è finita:
- I neutri sono quelli dei Requirements: Paper `#F6F5F1` come fondo della finestra, Surface `#FFFFFF` per la superficie di lavoro, card, dialog, popover e menu, Lane `#FAF9F6` per i riquadri interni, Line `#E7E5DF` per anelli e bordi dei controlli (`#E0DDD5` su pulsanti secondari e input, `#EFEDE7` per i separatori interni), Ink `#18181B`, Ink 2 `#52525B`, Muted `#6B6B73`.
- Il nero (Ink) è l'unico colore delle azioni: pulsanti primari Ink con testo bianco, secondari bianchi con bordo da 1 px, ghost senza fondo in Ink 2, distruttivi bianchi con testo e bordo Rose. L'anello del focus è Ink. Non resta nessun primario blu.
- Il tema scuro esiste ancora (Auto, Light, Dark, applicato subito a ogni finestra) e ha gli stessi ruoli, ricavati dai valori chiari: grigi caldi scuri per Paper e Surface (Surface il più chiaro), Line grigio caldo a basso contrasto, Ink quasi bianco con pulsanti primari chiari e testo scuro; i fili mantengono la tinta con tint più scure e ink più chiari. In entrambi i temi valgono i contrasti dei Requirements di qualità (4,5:1 per il testo, 3:1 per icone, bordi e focus).
- Raggi 6 px per id, chip e codice inline, 9 px per pulsanti, input, voci di menu e della sidebar, 12 px per card, riquadri e righe di lista, 14 px per superficie di lavoro, colonne della Board e dialog; badge e pill completamente arrotondati. Card e riquadri hanno un anello da 1 px in Line al posto del bordo; la card appoggiata su una corsia aggiunge un'ombra leggera; dialog, popover e menu un'ombra più ampia.
- Il font è Geist a 14 px (pesi 400, 500, 600) e Geist Mono per id, percorsi, comandi e codice, compreso l'editor della vista Files; entrambi inclusi nell'app e mai caricati dalla rete. I titoli hanno una spaziatura leggermente negativa.
- L'header di ogni vista è alto 76 px più il bordo da 1 px (`#EFEDE7`), con 28 px di margine laterale, titolo a 19 px semibold, sottotitolo facoltativo a 13 px Muted e controlli alti 36 px.
- Gli id delle card sono in monospace su un'etichetta Paper.
- Le superfici non disegnate nel redesign (Settings, Files, Assets, Notes, dialog, toast, banner) mantengono contenuti e disposizione e prendono questi token, forme e font.
- I test unitari e i test end-to-end che dipendono da altezze, classi o testi cambiati sono aggiornati; i test end-to-end continuano a trovare le voci della sidebar per nome accessibile.

Riferimento grafico negli Assets del progetto: `redesign-foundations.png` e `redesign-foundations.html` (valori esatti), più le altre tavole `redesign-*.png`/`.html` per vedere i token applicati.

## Code notes

Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Token: `ui/src/index.css` (`:root` 6-29, `.dark` 31-49, `@theme inline` 51-77 con `--font-sans`/`--font-mono` 71-72 e `--radius` 73-76; `body` `font-sans text-sm` a 87). Nessun file di config Tailwind: Tailwind v4 legge `@theme inline`.
- Primario blu consumato da `components/ui/button.tsx:15`, `badge.tsx:13`, `checkbox.tsx:15`, `progress.tsx:13`, `radio-group.tsx:23`, `switch.tsx:14`, `toggle-group.tsx:26`, e da `markdown.tsx:72`, `files/file-tabs.tsx:56`, `notes/notes-view.tsx:114`, `questions/question-index.tsx`, `question-pane.tsx`, `assets/tagged-textarea.tsx:198`.
- Font: nessun `@fontsource` in `ui/package.json`; la CSP in `src-tauri/tauri.conf.json:16-17` ha già `font-src 'self'` (niente `data:`). CodeMirror ha font e palette proprie in `components/files/editor-theme.ts:29-56,88`; `editor-theme.test.ts` controlla il contrasto.
- Tema: `components/theme-provider.tsx:45-47` mette `.dark` su `<html>`; mermaid segue il tema in `mermaid-block.tsx:34`.
- Header: `components/view-header.tsx:11` `VIEW_HEADER_CLASS` (`h-14`, `px-4`), usato anche direttamente in `jobs/jobs-view.tsx:481`; test che fissano `h-14`: `view-header.test.tsx:31`, `jobs/jobs-view.test.tsx:69`, `settings-view.test.tsx:234`.
- e2e: `e2e/specs/smoke.e2e.mjs` trova le voci con `entry(name)` per testo o `aria-label`; non dipende da colori o classi.
