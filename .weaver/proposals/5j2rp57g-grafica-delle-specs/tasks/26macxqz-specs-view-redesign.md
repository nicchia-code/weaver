---
technical: false
refs:
  - app
  - requirements/ui#app
  - specs#frameworks
---
# Loghi e icone delle Specs

La vista Specs viene rivista graficamente come dice la sezione "Specs view" di `requirements/ui#app`; header, filtri `All`/`Agent's choice`, ordine dei campi, box delle note e overlay restano come sono.

Quando il Task è finito:
- Ogni intestazione di campo ha un'icona lucide-react di 16 px, nel colore del titolo e decorativa, prima del titolo: languages `Code`, frameworks `Layers`, data `Database`, testing `FlaskConical`, CI `Workflow`, repository `GitBranch`, deployment `Rocket`, cloud `Cloud`, infrastructure `Server`, notes `NotebookPen`.
- L'intestazione e il contenuto di ogni campo partono dallo stesso bordo sinistro del titolo dell'intestazione della vista; il bordo sinistro e la tinta del focus non li spostano di lato.
- Il riquadro di 26 px di ogni voce (Ink 2 su Paper, raggio 6 px) non mostra più l'iniziale: mostra il logo monocromo della tecnologia, 16 px in Ink 2 e decorativo, oppure l'icona del campo quando la voce non ha logo. Nessun logo prende il colore del marchio.
- I loghi vengono da `simple-icons`, l'intera raccolta inclusa nell'app e mai scaricata dalla rete; la raccolta si carica la prima volta che la vista Specs si apre in una finestra. La vista non la aspetta: finché non è caricata ogni riquadro mostra l'icona del campo e ogni logo la sostituisce appena la raccolta c'è. Se la raccolta non si carica restano le icone dei campi, senza alcun avviso.
- Una voce ha un logo solo quando il suo nome intero, senza distinguere maiuscole, spazi e punteggiatura, coincide con il titolo o lo slug di un logo di `simple-icons` confrontato allo stesso modo (`Tailwind CSS`, `Node.js`); nessuna corrispondenza parziale (`Tauri plugins` e `react-markdown` restano senza logo).
- Ogni card ha l'anello Line, raggio 12 px, l'ombra delle card della Board (`0 2px 6px rgba(24,24,27,0.05)`) e la stessa altezza minima, circa 92 px.
- Prima riga della card: riquadro, nome (14 px semibold) centrato verticalmente con il riquadro, e all'estremità destra la versione come tag monospace su Paper con raggio 6 px; un nome lungo va a capo senza spostare riquadro e versione.
- Sotto: la nota in Ink 2 a 13 px; su una voce cambiata il toggle `Show before` e il vecchio testo `Before: <text>`, entrambi a 13 px come la nota.
- In fondo alla card, su una sola riga: l'etichetta dell'overlay (`added`, `changed`, `removed`) e, per una voce scelta dall'agente, la pill Lavender `Agent's choice` con `Sparkles` e il motivo nel tooltip.
- `simple-icons` è una dipendenza dell'interfaccia, come dice il campo frameworks delle Specs.

## Code notes
Commit: 0b94932067beeaa5a3e8371caa857af77dd0249f

- Vista: `ui/src/components/views/specs-view.tsx` (`SpecsPage`, `Field`, `Entry`, `FilterPill`, `fieldTitle`); test in `specs-view.test.tsx`, fixture in `view-fixtures.ts:53-68`.
- Il riquadro oggi è uno `<span aria-hidden>` con `name.charAt(0)`; la sezione del campo ha sempre `border-l-4 border-transparent pl-4` (da qui il rientro): usare margine negativo o un bordo che non sposti il contenuto, mantenendo `focusClass` di `change-badge.tsx`.
- `ChangeBadge` oggi sta su una riga sotto il nome; `Agent's choice` è un `Badge variant="lavender"` con `mt-auto self-start` e `Tooltip`.
- Mappa campo→icona: seguire il pattern `TONE_DOT` di `ui/src/components/board/column.tsx` (mappa con classi/componenti scritti per intero). Le 10 chiavi sono `SPECS_FIELDS`/`NOTES_FIELD` di `crates/core/src/project/specs.rs`; nessun cambio lato Rust: l'entry ha solo `name`, `version`, `note`, `chosenReason`, `change`, `before`.
- Icone lucide importate per nome da `lucide-react`, sempre `aria-hidden`; dimensione dal contenitore (`Button` 16 px, `Badge` 12 px).
- `simple-icons`: dati SVG (`path`, `title`, `slug`); caricare con `import()` dinamico per avere un chunk separato, come mermaid. Rendere il path con `fill="currentColor"`, mai `hex`.
- Solo token (`bg-paper`, `text-ink-2`, `ring-line`, `shadow-card`), mai hex; il tema scuro funziona scambiando variabili in `ui/src/index.css`.
- I test fissano l'iniziale (`getByText("T")`), il `font-mono` della versione, la griglia `minmax(250px,1fr)`, `max-w-[1180px]` come genitore diretto della sezione, il conteggio `text-muted-foreground`, i nomi delle `li` (`"TypeScript, changed"`) e il testo delle pill (`["All2","Agent's choice1"]`): aggiornare solo ciò che il nuovo aspetto cambia.
