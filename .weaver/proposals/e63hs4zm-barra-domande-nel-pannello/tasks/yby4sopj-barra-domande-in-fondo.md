---
refs:
  - question-round
  - requirements/ui#question-round
  - requirements/ui#window
  - requirements/ui#General
---

# Barra domande in fondo

Quando un round di domande è minimizzato, la barra `The agent is waiting for your answers (question <n> of <N>)` + `Open questions` sta in fondo al piano di lavoro, dentro il pannello con i suoi margini e angoli arrotondati, a tutta larghezza del piano, sotto la vista selezionata.

- Compare in ogni vista, Chat compresa (anche per un round di Chat, accanto alla riga di attesa e a `Answer`) e nelle conversazioni Ask more (accanto a `Back to questions`).
- Occupa il proprio posto nel layout e non è mai disegnata sopra la vista: la vista si accorcia dell'altezza della barra, così nulla resta coperto — né la casella dei messaggi di Chat, di un job, di un sub-agente o di Ask more, né la fine di una lista, le colonne della Board, un editor o un foglio. La barra laterale resta invariata.
- Aspetto: fondo tinta Peach, testo e icone in inchiostro Peach, bordo superiore di 1 px Peach.
- Una sola riga ad altezza fissa a qualunque larghezza della finestra: il testo si tronca con `…` e il testo completo è nel tooltip; `Open questions` resta sempre intero all'estremità destra.
- Finché la barra è visibile, i toast stanno sopra di essa, mai sopra la barra.
- Riapertura e minimizzazione del round funzionano come prima; la finestra di dialogo delle domande non cambia.

## Code notes

Commit: 0b94932067beeaa5a3e8371caa857af77dd0249f

- Oggi `MinimizedBar` in `ui/src/components/questions/question-overlay.tsx` (~33–50) è `fixed inset-x-0 bottom-0 z-40 … bg-background`, `flex-wrap`, montata da `QuestionRoundLayer` (~148–156) dopo `{children}`, fuori dal layout di `ProjectWindow`: nessuno spazio riservato.
- `ProjectWindow` (`ui/src/components/project-window.tsx` ~186–216): `[data-work-surface]` è una colonna flex con banner in alto e `main.min-h-0.flex-1`. Una barra `shrink-0` in flusso dopo `<main>` accorcia ogni vista senza modifiche per vista; niente padding inferiore per vista.
- Aggiungere a `ProjectWindow` uno slot in fondo (es. `footer`) come la prop `banners` (passata in `project-content.tsx` ~106–110), invece di usare `useQuestionRound` dentro `ProjectWindow` (nei test è senza provider [inferenza]). Stato da `useQuestionRound`.
- Lasciare invariato l'overlay Dialog (portal, `fixed inset-0 z-50`).
- Token Tailwind v4 esistenti (`text-peach-ink`, `border-peach`, tinta Peach), primitive shadcn, `cn`.
- Toast: `Toaster` in `ui/src/components/ui/sonner.tsx` (`bottom-right`), montato in `app-providers.tsx`; `offset` di Sonner può alzarli mentre la barra è visibile.
- Il testo nel codice differisce (`Weaver is waiting…`, `(summary)`): seguire il Requirement.
- `Jump to latest` (`sticky bottom-2`) continua a funzionare.
- Test da tenere verdi: `question-overlay.test.tsx` (barra trovata per regione "Questions waiting"/testo), `ask.test.tsx`, `general-view.test.tsx` (~729–739).
