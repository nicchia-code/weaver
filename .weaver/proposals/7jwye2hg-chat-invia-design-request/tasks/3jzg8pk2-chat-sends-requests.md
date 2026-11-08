---
refs:
  - general-chat
  - requirements/rules#general-chat
  - requirements/tech#general-chat
  - requirements/ui#general-chat
---
# Chat invia request

In Chat l'agente non scrive più Proposal. Quando lo sviluppatore gliene chiede una in modo esplicito, l'agente scrive una design request con quanto concordato e la invia, come se lo sviluppatore l'avesse scritta e inviata lui. Il job di design che parte (setup con il Model vuoto) esplora, fa i round di domande e consegna la Proposal. Quando è fatto deve valere quanto segue.

- L'istruzione di Chat, punto 5, dice che su ordine esplicito di scrivere una Proposal l'agente non la scrive. Scrive invece una request autosufficiente, perché il job vede solo la request e non la conversazione: un H1 con un titolo breve, poi cosa aggiungere o cambiare, ogni decisione presa e le alternative discusse con la scelta dello sviluppatore, e solo identità permanenti, mai numeri brevi. La invia con il comando di Chat per le request.
- Su ordine esplicito di risolvere Drift aperti, l'agente avvia un job Resolve per ogni Drift.
- L'istruzione dice anche che l'agente non scrive né modifica mai una Proposal e non carica né segue nessuna skill dei job, `weaver-design` e `weaver-resolve-drifts` comprese, salvo leggere le regole di scrittura del punto 6. Dice che riferisce quale job è partito senza aspettarne la fine e che, se Weaver rifiuta, riferisce il motivo senza scrivere la Proposal in altro modo.
- Restano come oggi gli altri punti dell'istruzione: Task e Drift su ordine, decisioni solo su richiesta esplicita, round solo quando chiesti, avvio di job.
- `developer.CLEAR_GENERAL` (da Idle, Loading, Working, Waiting for answers) svuota solo la conversazione. Non scarta più nessuna Draft e non c'è più nessun evento di Chat svuotata verso le Proposal. Il round di Chat ancora aperto si chiude come oggi e la sua notifica viene ritirata.
- In Working, `agent.SEND_REQUEST` resta in Working: Weaver scrive la request e avvia il suo job, che ha una conversazione sua. Nella conversazione di Chat compare la stessa riga `Started <job name>` con `Open conversation` di un job avviato. Un rifiuto mostra `Weaver did not start <kind>: <reason>.`, dove il kind è `design` o `setup`.
- Testi dell'interfaccia di Chat. Vuota: `Think with omp about this project. Ask it for a Proposal or a job of Weaver when you want one.` Il dialogo di Clear non ha più la frase sulle Draft scartate.
- Nell'ambiente di Chat, `weaver request new` è accettato come `weaver question new`: solo dall'identità live di Chat durante un suo turno. Le Proposal di Chat non servono più a nessuno, quindi l'app non le segue più.
- `CHANGELOG.md`, sotto `## [Unreleased]`, `Changed`, ha una riga: chiedere una Proposal in Chat ora invia una design request e avvia un job di design, con i suoi round di domande.

## Code notes

Commit: 02129bbb382d41a3a2b2b107bf82f809097051bb

- Istruzione: `crates/core/src/jobs/general.rs` `INSTRUCTION` (~631-652, punto 5 a ~640; anche i punti su `weaver job new` design). Test `the_instruction_of_chat_holds_the_rules_in_order_and_the_lines_of_the_settings` (~1373-1430): oggi verifica "load `weaver-design`…" e `weaver-resolve-drifts`, va riscritto sui nuovi punti 3, 4, 5, 8.
- Clear: `src-tauri/src/general/service.rs`: `ChatObserver::chat_cleared` (~112), `Effect::Clear` → `notify_cleared` (~739-770), `ClearDiscardsChatDrafts` (~1331-1351), `chat_drafts()`/`snapshot.chat_drafts` (~380-400); registrazione in `general/mod.rs:90`; `wire.rs:37` `GeneralSnapshot.chat_drafts` → `ui/src/bindings/GeneralSnapshot.ts`. Attenzione: `chat_cleared` chiude anche la notifica del round di Chat (`src-tauri/src/notifications.rs:214`, test ~748, `round_tests.rs:45`): va tenuta, ricollegata a un'altra via.
- `chat_working` (`jobs/authority.rs:35`, `jobs/registry.rs:437-452`, `general/service.rs:1226`) serviva solo al motivo 1 delle Proposal: va tolto insieme al Task delle Proposal. `general/tests.rs:277-285` rinvia la Board a inizio/fine turno solo per questo.
- Righe avviate/rifiutate: `general/service.rs` `job_requested` → `started_jobs` (~1146-1186, `conversation.job_started`/`job_refused`); UI `ui/src/components/jobs/jobs-view.tsx:266-302` (`JobStartedLine`).
- UI Chat: `ui/src/components/general/general-view.tsx` `EMPTY` (:20), `chatDrafts` (:36), dialogo di Clear (~178-183); `use-general.tsx`. Test: `general-view.test.tsx:39,89,189-215`, `general/tests.rs:1072-1100,2405-2428`.
