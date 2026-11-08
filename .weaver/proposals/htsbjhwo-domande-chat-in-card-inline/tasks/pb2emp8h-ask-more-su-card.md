---
technical: false
refs:
  - ask-conversation
  - developer.ASK_MORE
  - requirements/rules#ask-conversation
  - requirements/ui#ask-conversation
---
# Ask more sulla card

Ask more deve funzionare anche sulla domanda in vista della card di domande attiva di Chat, non solo sulla domanda in vista di un round.

## Cosa deve valere

- **Pulsante:** sulla domanda in vista della card di domande attiva di Chat c'è un `Button` `ghost` piccolo, solo icona `MessageCircleQuestion`, tooltip `Ask an agent about this question`, nome accessibile `Ask more about this question`. È sempre abilitato. Le card chiuse (riepilogo in sola lettura) non lo hanno.
- **Apertura (`developer.ASK_MORE`, da Not opened):** se la card è ancora attiva, Weaver prepara l'istruzione dai file in quel momento, apre la conversazione nella vista Jobs (nome `Ask question`, segnaposto `Ask about the question…`), la mostra e mette il focus nella sua casella di messaggio. Non invia nulla e omp non parte. Se la card non è più attiva (inviata, chiusa da un messaggio, sostituita da una card più recente o cancellata con `New chat`), non si apre nulla e nulla cambia.
- **La card resta com'era:** premendo Ask more la card di Chat non viene minimizzata né alterata: stessa domanda in vista, scelte e testo scritto invariati. (Per un round invece resta il comportamento di oggi: il round va nella sua barra.)
- **Parte Question dell'istruzione:** la domanda in vista per intero (testo, contesto, ogni opzione con etichetta, descrizione e dettagli, l'opzione raccomandata, se se ne possono scegliere più), poi testo e opzioni (etichetta e descrizione) delle altre domande della card nel loro ordine, segnando quella in vista. Come chi ha chiesto indica `Chat`. Dice che la card è ancora attiva e che l'agente non deve rispondere al posto dello sviluppatore. Mai le scelte, il testo scritto, le risposte o i salti dello sviluppatore. Nessun comando di lettura è elencato per una card di Chat.
- **Back to questions:** una conversazione aperta da una domanda della card mostra in cima, finché quella card è attiva, il `Button` `Back to questions` (`ArrowLeft`), che mostra la vista Chat scorsa in modo che la card sia visibile, alla domanda che aveva. Scompare quando la card smette di essere attiva, comunque accada.
- Ask more non riceve mai card di domande propria: condivide la conversazione con Chat ma non pubblica card.

La lettura lato backend della domanda della card (per costruire l'istruzione) è a carico del Task di `question-round`: questo Task la usa.

## Code notes
Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- `src-tauri/src/ask/build.rs` (~295-325) `question()` legge oggi solo `Rounds::question_in_view` (round presentato): deve leggere anche la domanda della card attiva fornita dal Task `question-round`; se la card non è attiva → nulla si apre.
- `crates/core/src/jobs/ask.rs` (~131, ~451) `QuestionOrigin::Chat`: usarla per scrivere `Chat` e "the card is still active".
- Binding `AskTarget` (`question {round, index}`) e `AskRow.round` sono generati da ts-rs: cambiare i tipi Rust, mai `ui/src/bindings` a mano.
- UI: in `ask-conversation.tsx` (~47-66) `Back to questions` oggi fa `round.act({kind:"minimize", minimized:false})`; per una card deve invece mostrare la vista Chat e scorrere fino alla card, alla sua domanda. `askMore()` nei pannelli minimizza il round: non farlo per la card.
- Test: `ask.test.tsx` (~240-261), `src-tauri/src/ask/tests.rs` (~31, fake `RoundsHost`).
- Insidia: Ask more condivide `Conversation`/codice attore con Chat; non deve mai ottenere card.
- Esistono già Task di rimozione creati da Weaver per `Waiting for answers`/`WAITING` di Chat: non duplicarli.
