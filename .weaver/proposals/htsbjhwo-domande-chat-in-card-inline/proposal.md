---
status: approved
kind: change
base:
  model/actors/agent.md: 359691ea27b85507d564dcf7a2dc6ed9639176d292d52f3dbf1bf834ea015763
  model/actors/developer.md: 52f6906e99f52306627e6c861c1b0a010343f02fef06cf4f7bf2e36fd6b16dc5
  model/agent-work/ask-conversation.md: f82212aa25950dddfd9fa327c3db209610560b48bb72eeaf867ba5923d8d93ce
  model/agent-work/feature.md: ae26551c8ec378bb582bba8dfac4598191f37f79358ee5ab9e388529e4322588
  model/agent-work/general-chat.md: a8ea453d16de07059361a93eeca54761a58b5fa688a8d8dbf8f43dc2ecdaa4c0
  model/agent-work/question-round.md: 808147695941f0199f3e1dd6c7b7f45619968f19a3841b545b1483fd4e2da78c
  model/app/notification.md: 3fca87b9f58e5a6858daaa537050572a0e50c600c3ba91a6ce8cf762ab330828
  requirements/cli/agent-commands.md: 40441e738c6a38455ce799d3650e8fce1755a120f72bed20f31d1d7f68df8148
  requirements/data/settings.md: 6ea9502480e2658b7ccd790d60826a1a366ffc17f25c956d5dab83ef2f51e19d
  requirements/rules/cards.md: 2c018be9c1e39fb629f952f81bd5159cdec7a817d31a291542e3411056a08807
  requirements/rules/chat-instructions.md: 59a1ebe06f7a1afe5d6b895207ae8cc60011e6332e97a694561c4ffa1ca12166
  requirements/rules/instructions.md: 341910827d7a99ae835770294303883dacfaaa37edebbf531dba9e34c8729bba
  requirements/tech/architecture.md: d0e06ed68ad6e7d97881b2860ecfb8ac1cf44ca342870f58027ce5bd6d9a33fb
  requirements/ui/ask-more.md: ef231443d9cb84939dfa4ebff8ad04e29c7ad7875598be268b63ea5c1b1cc89b
  requirements/ui/chat.md: f8681ea92aaa0860ffcd3d0f879027dc57a2edd0642a7ef406310f6473452791
  requirements/ui/rounds.md: 15237c8accedb2db32b5eb06a30d5907c1f2152c68474f9ebc4cd4144dda296d
  requirements/ui/window.md: c3e9223c4ac70eb467ad5963884c784d0e85f7b6a7da3c12ecec610954d0bf0d
created: 2026-11-07T19:56:29.461Z
request: 2026-11-07T20-46-03
label: Chat question cards
job: krvkmalp
---
# Domande Chat in card inline

Le domande con opzioni dell'agente di Chat diventano una card nella conversazione, sotto la sua risposta, che non blocca il turno: oggi l'agente apre un round solo su richiesta esplicita, il round blocca il turno con Chat in `Waiting for answers` e le domande finiscono spesso scritte due volte, nel testo e nel round.

## Decisioni

- Disposizione: una domanda alla volta, riga di pallini numerati, `Back`/`Next`, riepilogo con `Edit` e invio; restano opzioni a riga intera con `Recommended` e preselezione alla prima visita, `Show details`, risposta libera, `Skip`, tasti `1`–`9` e `Ctrl/Cmd+Enter` con il focus nella card, `Ask more`; spariscono oscuramento, focus intrappolato, `Minimize` e barra, `Close`/`Skip all`, `+<k> more rounds waiting`, colonna indice.
- La card non blocca il turno: il comando ritorna subito, il turno finisce, le risposte arrivano come nuovo messaggio dello sviluppatore. Spariscono `Waiting for answers` di Chat e l'evento `WAITING`; anche il round chiesto espressamente diventa la card. Per i job il round resta com'è.
- Messaggio scritto con la card attiva: parte con le risposte confermate e i salti; il resto, preselezione e scelte non confermate comprese, arriva come "non risposta": resta aperto e l'agente non usa la raccomandazione. La card diventa il riepilogo in sola lettura.
- È attiva solo l'ultima card; le precedenti restano in sola lettura.
- Una seconda card nello stesso turno è rifiutata dal comando, che chiede di mettere tutte le domande in una sola card.
- Fuori da Chat solo il badge `1` in Peach, tooltip `Chat has questions for you`; nessuna notifica di sistema.
- L'agente usa la card per ogni domanda con opzioni distinte, in qualunque momento, senza ripeterla nel testo; le domande aperte restano nel testo; cade la regola "round solo su richiesta".
- `Skip` resta e l'invio dalla card richiede ogni domanda risposta o saltata, come nel modal.
- Inviata dalla card: nessuna bolla dello sviluppatore, la card diventa il riepilogo; sotto, `Starting Weaver…` finché le risposte non sono consegnate.
- La card si può inviare anche durante il turno: le risposte arrivano come ogni messaggio inviato mentre l'agente lavora.
- Stop, turno fallito o uscita di omp: la card resta attiva.
- Le card sono conservate fino a `New chat` in un campo `cards` del record di Chat nei dati locali dell'app, con risposte confermate e salti, mai le bozze; dopo la riapertura ricompaiono al loro posto e l'ultima senza risposta resta attiva. Se la conversazione non si riesce a ricaricare, spariscono con lei.
- Risposte all'agente: blocco in inglese con intestazione che nomina la card, poi lo stesso JSON delle risposte dei job con il terzo stato `not-answered`; nella vista il blocco non si mostra mai.
- Comando in Chat: `{status: "published", card}`, exit 0; seconda card del turno `card-already-published`, exit 1.
- Aspetto: cornice della card Proposal, testata con `MessagesSquare`, `Questions from Weaver` e `Question <n> of <N>` o `Summary`; card chiusa con pill `Answered`, `Partly answered` o `Not answered`.
- La card non prende mai il focus da sola; sopra il box messaggi, con una card attiva, la riga `Your message also sends the answers chosen so far.`
- Da Ask more su una domanda della card, `Back to questions` mostra Chat con la card, finché è attiva.

## Scelte dell'agente

- La card compare sotto tutto quanto il turno che l'ha pubblicata; dopo un ricaricamento ogni card chiusa torna subito prima del messaggio che l'ha chiusa, riconosciuto dall'identità della card nel blocco delle risposte, e quella attiva in fondo.
- Il modello di Chat si può cambiare anche con una card attiva, perché Chat non aspetta più.

## Raffinamento

- Riallineata alle bozze delle richieste nel Backlog, approvate nel frattempo: l'attore developer tiene `LEAVE_REQUEST` e le nuove descrizioni di `NEW_REQUEST`, `SEND_REQUEST` e `DISCARD_REQUEST`; le modifiche della card restano su `SEND_MESSAGE`, `ASK_MORE` e `SEND_ANSWERS`.
- `requirements/tech#design-request` e le righe sulle bozze nella sorveglianza dei file restano come sono; la sezione `question-round` riceve le stesse modifiche della card.
- Nella finestra il conteggio di `Board` include ancora le bozze e la palette tiene `New Request` e le bozze; cambia solo il badge di `Chat`, che segue la card attiva.

## Already present

- `job/Running/developer.SEND_MESSAGE` — `src-tauri/src/jobs/registry.rs` `send_message`; i messaggi ai job non cambiano
- `job/Waiting for the developer/developer.SEND_MESSAGE` — `src-tauri/src/jobs/registry.rs` `send_message`; i messaggi ai job non cambiano
