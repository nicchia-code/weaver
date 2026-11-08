---
technical: false
refs:
  - ask-conversation
  - developer.ASK_MORE
  - requirements/rules#ask-conversation
  - requirements/ui#ask-conversation
---

# Ask more sul round di Chat

Ask more su una domanda torna a funzionare solo sulla domanda in vista di un round di domande, anche di un round di Chat, e mai su una card di domande nella conversazione di Chat, che non esiste più. Ask more apre una conversazione libera su una cosa che lo sviluppatore sta leggendo: una Proposal, una Task, una Drift, una voce della History con le Task che cita, una domanda di un round o i Requirements nel loro insieme.

Sulla domanda in vista del round c'è un piccolo pulsante solo icona, tooltip `Ask an agent about this question`, nome accessibile `Ask more about this question`. Premendolo, la finestra mostra la vista Jobs con la nuova conversazione selezionata e il focus nel suo campo messaggio; il round viene ridotto alla sua barra, con la domanda in vista, le scelte e il testo scritto invariati, e nient'altro nel round cambia. All'agente della nuova conversazione arrivano solo le domande, mai le risposte.

L'istruzione di Weaver per una domanda riporta per intero la domanda in vista, poi testo e opzioni delle altre domande del round nel loro ordine, segnando quella in vista; poi nome e identità del job che ha chiesto il round, `Chat` per un round di Chat o, per una domanda preparata, l'identità della Drift. Dice che il round è ancora aperto e che l'agente non deve rispondere al posto dello sviluppatore; mai scelte, testo scritto, risposte o salti dello sviluppatore.

La richiesta di Ask more nomina il round per la sua identità e la domanda per la sua posizione; non esiste più un bersaglio "card". Se è presentato un altro round, o nessuno, non si apre nulla.

`developer.ASK_MORE` → Closed quando la cosa scelta non esiste più, tra cui una domanda di un round che si è chiuso: non si apre nulla.

Una conversazione aperta da una domanda mostra in cima, solo finché quel round è aperto, il pulsante `Back to questions` (`ArrowLeft`) che riapre il round dove era; scompare quando il round finisce, comunque finisca. Non porta più alla vista Chat scorrendo fino a una card.

## Code notes

Commit: bcb006abc30b7b93d0223e6a5d5faf43d3e2eb18

Revert SELETTIVO: prendere da `git show d267932^:<file>` solo il codice di Ask more legato alle card; mai `git revert d267932` per intero (commit misto: romperebbe `tsc` e `src-tauri`, es. `bindings.rs` `requests::RequestSummary`, comandi `usage_status`, `request_leave`, ecc.).

- `src-tauri/src/ask/*`: codice puro di card, tornare alla versione pre-d267932 (richiesta che nomina round + posizione; niente target card, niente "card non più attiva").
- Binding `AskRow`, `AskTarget` (variante card) vanno via: rigenerare i binding ts-rs in `ui/src/bindings` da `bindings.rs` `export_all`, mai a mano.
- `src-tauri/src/rounds/*`: tornare alla versione pre-d267932 (`RoundsHost` con `chat_round_opened`/`chat_round_closed`); la minimizzazione del round all'Ask more e `Back to questions` usano il round, come prima.
- Rimuovere ogni `BackToCard` / scroll verso la card nella vista Chat.
- `.agents/skills/*` sono copie generate.
