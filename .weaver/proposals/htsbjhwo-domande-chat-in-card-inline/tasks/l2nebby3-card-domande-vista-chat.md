---
technical: true
refs:
  - requirements/ui#question-round
  - requirements/ui#question-round/Active in General
  - requirements/ui#general-chat
  - requirements/ui#window
---
# Card domande nella Chat

Le domande dell'agente di Chat non aprono più una finestra modale: ogni round di Chat è una **card di domande** dentro la conversazione, sotto la risposta dell'agente, e non trattiene mai il turno. Questo Task copre solo l'interfaccia; la memorizzazione delle card, il comando e la macchina di Chat sono altri Task: la vista legge le card dal backend (nuovi eventi e stato iniziale) e invia le azioni.

## Aspetto e comportamento della card
- Cornice della card di Proposal della Chat; testata con l'icona `MessagesSquare`, `Questions from Weaver` e `Question <n> of <N>` oppure `Summary`.
- Una domanda alla volta, riga di pallini numerati (l'indice stretto dell'overlay), `Back`/`Next`, Summary con `Edit` e `Send to agent`.
- Restano: opzioni a riga intera con `Recommended` e preselezione alla prima visita, `Show details`, risposta libera per domanda, `Skip`, `Ask more` su una domanda. L'invio richiede ogni domanda risposta o saltata, come nel modale.
- Tolti: oscuramento, trappola del focus, `Minimize` e barra in basso, `Close` con `Skip all`, `+<k> more rounds waiting`, colonna indice.
- La card non prende mai il focus da sola; i tasti `1`–`9` e `Ctrl/Cmd+Enter` valgono solo con il focus dentro la card.
- Solo l'ultima card è attiva; le precedenti sono in sola lettura.
- Dopo l'invio dalla card nessun fumetto dello sviluppatore: la card diventa il Summary in sola lettura, con la pillola `Answered` / `Partly answered` / `Not answered`, e sotto compare `Starting Weaver…` finché il messaggio non è consegnato.
- La card si può inviare anche durante il turno (arriva come un messaggio inviato mentre l'agente lavora).
- Un messaggio scritto mentre una card è attiva la chiude: diventa il Summary in sola lettura.
- Stop, turno fallito o uscita di omp: la card resta attiva.
- Posizione: sotto tutto il lavoro del turno che l'ha pubblicata, mai dentro il riepilogo del lavoro; dopo una riapertura ogni card chiusa appare subito prima del messaggio dello sviluppatore che l'ha chiusa, quella attiva in fondo; l'ultima non risposta riparte dalla sua prima domanda senza risposta. Il blocco di risposte non si mostra mai.
- Da `Ask more` su una domanda della card, `Back to questions` mostra la Chat con la card in vista, finché la card è attiva.

## Vista Chat
- Spariscono la riga di attesa con `Answer`, `Queued` sotto i messaggi e lo stato occupato dovuto all'attesa.
- Con una card attiva, sopra la casella del messaggio, una riga secondaria: `Your message also sends the answers chosen so far.`
- Il controllo del modello non è più disabilitato da una card.

## Finestra
- Con una card attiva la barra laterale mostra sulla Chat il badge `1` in Peach con tooltip `Chat has questions for you`; nessuna notifica di sistema.
- L'intestazione dell'overlay non ha più una variante per la Chat.

Weaver crea da sé i Task di rimozione per `general-chat/Waiting for answers` e `general-chat.WAITING`: non duplicarli.

Aggiungere in `CHANGELOG.md` sotto `## [Unreleased]` una riga (Changed): le domande della Chat diventano una card nella conversazione.

## Code notes
Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Riusare da `ui/src/components/questions/`: `QuestionPane`/`Options` (`question-pane.tsx`), `SummaryPane` (`summary-pane.tsx`), `QuestionIndex compact` e `StatusIcon` (`question-index.tsx`), `question-model.ts` invariato. `DialogBody`/`DialogFooter` sono div, usabili fuori dal dialog.
- Da cambiare: togliere prop `askSkipAll`/pulsanti `Skip all`; il focus del titolo al mount (`useEffect(() => title.current?.focus())`) non deve agire nella card; `askMore()` oggi minimizza il round.
- Resta solo modale in `question-overlay.tsx` (`QuestionOverlay`, `MinimizedBar`, `QuestionRoundLayer`, `waitingOf`); `onKeyDown` con `handlers: KeyHandlers` riusabile sul contenitore della card. Togliere il ramo Chat di `headerOf`.
- Bozze nel backend; patch ottimistica come `withDraft` in `use-question-round.tsx`, azioni concatenate in ordine, gli eventi vincono. Sorgente propria per card, non la coda `Rounds`; schema di `use-chat-proposals.ts` (ascolta l'evento, poi chiedi, scarta risposte più vecchie).
- Vista: `general-view.tsx` (riga d'attesa, `busy`, `MODEL_REASON`), `chat-conversation.tsx` (`Queued`), `chat-model.ts` `chatItems` (card mai dentro `WorkSummary`), cornice da `chat-proposal-card.tsx`.
- Badge: `use-general.tsx` `useChatWaiting` → `project-content.tsx` → `views.ts` `ShellCounts.chatWaiting` → `sidebar.tsx` `countOf`.
- Mai modificare `ui/src/bindings` (ts-rs). Test: `question-overlay.test.tsx`, `general-view.test.tsx`, `project-content.test.tsx`, `sidebar.test.tsx`.
