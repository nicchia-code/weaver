---
refs:
  - app
  - requirements/ui#app
---
# Requirements e Specs ridisegnati

Le viste Requirements e Specs prendono il disegno del redesign, con navigazione e ricerca nei Requirements e nessun colore che non dica uno stato.

Quando la Task è finita:
- Requirements: l'header ha il sottotitolo `What the app must do, in plain words. Agents build from this.` e a destra il campo `Search requirements`, `Ask more` e `Ask for a change` (primario). `Ask for a change` mostra la Chat con il campo di testo a fuoco che contiene `Change to the Requirements: ` col cursore in fondo; il testo già presente resta e il prefisso va a capo dopo di esso; nulla parte finché lo sviluppatore non invia.
- A sinistra una navigazione di circa 220 px: per ogni Area il titolo come intestazione maiuscola in Muted, poi un'entrata per frammento con il numero delle sue sezioni; una pill Lavender `changed` sui frammenti che la Proposal in sovrapposizione aggiunge, rimuove o cambia; la scelta di un'entrata scorre il frammento in cima, e l'entrata del frammento in cima è evidenziata.
- A destra, larghe al massimo 900 px, le Aree e i frammenti: titolo del frammento a 22 px, percorso in un'etichetta monospace e `Open in editor`; le sezioni in un riquadro con anello, una riga per sezione con il titolo in Geist Mono in una colonna di circa 140 px e il testo accanto (sotto, quando la riga è stretta), `Open in editor` visibile al passaggio e al focus. Nessun colore per Area o sezione.
- La ricerca filtra sul posto, senza distinguere maiuscole e minuscole: restano solo le sezioni il cui titolo o testo contiene le parole, con le corrispondenze evidenziate; frammenti e Aree vuoti spariscono, i conteggi della navigazione contano solo ciò che resta; senza risultati `No sections match.`; Esc o campo vuoto mostrano tutto. Si combina con il focus e `Only these`. Non cerca altro che i Requirements.
- Barra di focus, sovrapposizione, etichette dei cambiamenti e `Show before` restano, nel nuovo stile.
- Specs: sottotitolo `The stack the agents build with · specs.yaml`; a destra le pill `All <n>` e `Agent's choice <n>`, che mostra solo le voci scelte dall'agente. Ogni campo non vuoto, nell'ordine del formato delle Specs, con titolo, numero di voci, una riga e `Open in editor`, che apre il file delle Specs alla riga del campo. Voci in griglia (colonne di almeno 250 px, al massimo 1180 px): riquadro neutro con l'iniziale, nome, versione come etichetta monospace, nota e, per le scelte dell'agente, la pill Lavender `Agent's choice` col motivo nel tooltip. Le note in un riquadro con anello. Nessun attributo «Local only».
- I test delle due viste sono aggiornati; i test end-to-end trovano ancora i testi dei Requirements della fixture.

Riferimento grafico negli Assets: `redesign-requirements.png`/`.html` e `redesign-specs.png`/`.html` (i colori per Area e per voce e il filtro «Local only» delle tavole non vanno implementati).

## Code notes

Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Requirements: `ui/src/components/views/requirements-view.tsx` (`RequirementsView` → `ViewFrame` con `toolbar={<AskRequirementsButton/>}` → `RequirementsPage` su `snapshot.data.requirements`). Tipi in `ui/src/bindings/AreaView.ts`, `FragmentView.ts`, `SectionView.ts` (sezioni per frammento = `fragment.sections.length`; «changed» = `fragment.change` o una `section.change` con `snapshot.overlay` non nullo).
- Focus e sovrapposizione: `views/views-state.tsx` (`useViews()`: `snapshot`, `overlay`, `focus[view]`, `goStep`, `clearFocus`, `setOnly`, `focusOn`), `views/focus-model.ts` (`planFocus`, `sectionKey`), passi trovati con `data-step-key`; `views/focus-bar.tsx`; colori in `views/change-badge.tsx`.
- `Ask for a change`: il testo del campo della Chat è un `useState("")` locale in `general/general-view.tsx:37`; `ProjectContext` (`project-context.tsx`) ha solo `showView`, `openJob`, `openedJob`: serve un canale nuovo (bozza + nonce), sul modello di `openedJob`.
- `OpenInEditor({file, line})` in `components/open-in-editor.tsx` (`open_in_editor`, percorso relativo alla radice del progetto).
- Specs: `views/specs-view.tsx` (`SpecsPage` su `snapshot.data.specs?.fields`; `Field` e `Entry` con `chosenReason`); tipi `bindings/SpecsFieldView.ts` (`file`, `line` per campo) e `SpecEntryView.ts`; il backend omette già i campi vuoti (`crates/core/src/project/view/docs.rs:300-397`).
- Test: `requirements-view.test.tsx`, `specs-view.test.tsx`; e2e cerca `Product`, `Greeting`, `The greeting is always shown in English.`.
