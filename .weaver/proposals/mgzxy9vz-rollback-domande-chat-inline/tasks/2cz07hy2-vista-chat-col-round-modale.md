---
technical: true
refs:
  - requirements/ui#question-round
  - requirements/ui#general-chat
  - requirements/ui#window
---

# Vista Chat col round modale

Nella conversazione della Chat non compare più la card delle domande: spariscono il titolo `Questions from Weaver`, i puntini, le pill `Answered`/`Partly answered`/`Not answered`, il riepilogo in sola lettura, `Starting Weaver…` sotto la card, `Back to questions` verso la card e la riga `Your message also sends the answers chosen so far.`.

Quando lo sviluppatore chiede esplicitamente di essere interrogato, il round della Chat si apre nell'overlay modale come prima (sfondo attenuato, focus intrappolato, `Minimize` e barra, `Close`/`Skip all`, `+<k> more rounds waiting`, colonna indice), con intestazione `Questions from the agent · Chat`; Ask more su una domanda funziona di nuovo nel modale. I round dei job non cambiano.

Mentre la Chat aspetta le risposte, la conversazione termina con la riga cerchiata in Lavender con `?`, il testo `Weaver is waiting for your answers.` e il pulsante secondario `Answer`, che riporta in vista l'overlay minimizzato. I messaggi inviati in quell'attesa mostrano sotto `Clock` + `Queued`.

Nella barra laterale la voce Chat mostra il badge `1` in Peach con tooltip `Chat is waiting for your answers` mentre la Chat aspetta il suo round (il tooltip `Chat has questions for you` sparisce). Il controllo del modello della Chat è disabilitato con il motivo `Change the model when Weaver is not working` anche mentre la Chat aspetta il suo round.

Il resto del nuovo design resta com'è: stile, barra laterale, selettore di progetto, card delle Proposal nella Chat e bozze delle design request.

## Code notes

Commit: bcb006abc30b7b93d0223e6a5d5faf43d3e2eb18

Revert SELETTIVO da `git show d267932^:<file>`; mai `git revert d267932` intero (rompe `tsc` e `src-tauri` col redesign).

- Badge: in `ui/src/components/sidebar.tsx` `countOf()` (~34-35) riscrivere sul posto `counts.chatCard` → `counts.chatWaiting` con tooltip `Chat is waiting for your answers`, tono `peach`. NON prendere il vecchio file della sidebar. In `views.ts` `ShellCounts.chatCard` → `chatWaiting` (tenere `jobsRunning`); `project-content.tsx` lo alimenta con `useChatWaiting(projectId)` (vecchio in `use-general.tsx`).
- Riga d'attesa: il blocco pre-d267932 di `general-view.tsx` è già nello stile nuovo (`?` lavender, `Button variant="secondary"`, `round?.act({kind:"minimize",minimized:false})`); l'hunk torna pulito.
- Modello: `general-view.tsx:~93` `busy = working || loading || state?.waiting` e commento su `MODEL_REASON`; `model-control.tsx` invariato.
- `chat-conversation.tsx` e `chat-draft.tsx` restano (contengono `Clock`+`Queued` in `DeveloperMessage` su `delivery.status === "queued"`); togliere solo il ramo di `ChatEntry` per messaggio vuoto con risposte di una card, e il commento.
- Test: `sidebar.test.tsx` e `general-view.test.tsx` mescolano redesign e card: sostituire i test card con badge, riga d'attesa e modello disabilitato nello stile nuovo; in `project-content.test.tsx` togliere il mock `general_cards` (~84).
- Binding ts-rs in `ui/src/bindings` si rigenerano, mai a mano.
