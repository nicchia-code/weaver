---
technical: false
refs:
  - proposal
  - agent
  - requirements/cli#proposal
  - requirements/cli#project
  - requirements/cli#agent
  - requirements/data#proposal
  - requirements/rules#proposal
  - requirements/rules#job
---

# Proposal scritte dalla Chat

Il Machine `proposal` ora ammette che a scrivere una Proposal sia, oltre all'agente di un job, l'agente della Chat (General), su ordine esplicito del developer, e solo Proposal ordinarie (`change`) e risoluzioni (`resolution`), mai `setup` né `import`.

## Cosa deve valere

- **Identità della Chat sul percorso CLI.** `WEAVER_GENERAL` è l'identità che Weaver mette sull'omp della Chat; l'app la riconosce solo come la Chat viva di quel progetto, e per `question new` solo mentre è in corso un turno della Chat. Un'identità non riconosciuta vale come nessuna identità (`requirements/cli#agent`). Oggi `src-tauri/src/cli_handler.rs` (l.76–82) costruisce il `Caller` copiando `general` ma senza verificarlo per comandi diversi da `job new` e senza copiare `ask`: il `Caller` (`crates/cli/src/spec.rs:218`) deve distinguere in modo verificato tre chiamanti — job (`WEAVER_JOB` accettato da `src-tauri/src/jobs/authority.rs`), Chat (`WEAVER_GENERAL` riconosciuto, vedi `accepts_general`), altro — e i comandi `proposal new`/`proposal edit` devono riceverlo. Quando il comando gira senza app, la Chat non è riconoscibile e il chiamante è «altro».
- **Actor `agent`.** Solo l'agente di un job tiene un Task o annuncia una scheda (`announcement new`, `job edit`, `task move` rifiutati, exit 1, nulla scritto, per ogni chiamante che non è un job); solo l'agente di un job o della Chat può chiedere un round (`question new` risponde `unavailable` per gli altri); `job new` resta per General o Ask more. Controlli: `crates/cli/src/cli.rs:450–458` e `491–497`, `exec.rs:194–213`, enum `Access` in `crates/cli/src/spec.rs:164–174` (`question.rs:17` `JobOrUnavailable` deve accettare anche la Chat). Il funzionamento del round della Chat stesso non è di questo Task.
- **Segni nei file (`requirements/data#proposal`).** Front matter di `proposal.md`: `chat: true` quando l'ha creata l'agente della Chat, mai tolto; `chatWriting: true` mentre la Chat la scrive, messo da `proposal new` e da `proposal edit` chiamati dalla Chat e tolto alla consegna. In `crates/core/src/project/proposal/create.rs` (accanto a `job`, l.176–178) e in `crates/cli/src/commands/proposal.rs` (`new` l.251, `edit` l.714/831, `open` l.844–917).
- **`proposal new` (`requirements/cli#proposal`).** Rifiutati: `setup` su un Model con Feature o Actor; `request` con `reworks`; `setup` e `import` dall'agente della Chat. `request` e `reworks` legano solo come permette **Bindings**; altrimenti la Proposal è creata senza vincolo con una nota che dice perché.
- **`proposal edit`:** ogni chiamata, anche senza input, riporta una Open a Draft e lo dice; dalla Chat segna anche la Proposal come scritta dalla Chat (`chatWriting: true`) fino alla consegna. Nel Machine: `agent.EDIT_PROPOSAL` da Open → Draft; se la modifica la fa la Chat diventa una Draft che la Chat sta scrivendo finché non la consegna di nuovo. Vale anche per una Proposal non creata dalla Chat (riaperta): ha `chatWriting` ma non `chat`.
- **`proposal move <id> open`:** consegna, toglie `chatWriting` e stampa avvisi e testo di `show`.
- **Bindings (`requirements/rules#proposal`), controllati da `proposal new` contro il chiamante** (oggi `create.rs` l.103–148 non controlla il chiamante; il controllo diventa effettivo per tutti):
  - `request`: un job di setup o design lega solo la richiesta nominata dalla sua istruzione, e solo con la sua prima Proposal; l'agente della Chat lega solo una richiesta scritta, non inviata, non consumata e non tenuta da un'altra Draft, con una Proposal `change`; ogni altro chiamante non lega nulla.
  - `reworks`: un job di rework lega solo la Proposal nominata dalla sua istruzione; l'agente della Chat lega qualsiasi Proposal `change` o `resolution` rifiutata, con o senza Rework, che nessun'altra Proposal ha sostituito e su cui non lavora un job di rework, con una Proposal dello stesso tipo; ogni altro chiamante non lega nulla. La Proposal legata prende il posto di quella rifiutata nell'ordine della Board e nei `requires` delle Proposal che la aspettavano, come per un rework (stessa logica oggi usata dal rework in `transitions.rs`/`registry.rs`).
  - Nel Machine, stato Rejected: su ordine del developer la Chat può ancora scrivere una Proposal dello stesso tipo che ne prende il posto, salvo che un'altra l'abbia già presa o un job di rework ci lavori. Le transizioni `developer.REJECT_PROPOSAL`, `developer.REWORK_PROPOSAL` e `drift.CODE_TO_BE_FIXED` → Rejected portano quindi a uno stato da cui la Chat può sostituire.
- **`requirements/rules#job`:** l'istruzione dei job dice che i vincoli li impone la riga di comando come in **Bindings**: un job lega solo la richiesta nominata, e solo con la prima Proposal; solo un job di rework, o la Chat, lega con `reworks`. Aggiornare il testo dell'istruzione in `crates/core/src/jobs/` (dove oggi c'è la frase sui binding) di conseguenza.
- **Motivo 1 e turno della Chat (`requirements/rules#proposal`).** `The agent is still finishing`: il job in `job` lavora ancora oppure, per una Proposal con `chat` o `chatWriting`, è in corso un turno della Chat del progetto. Si applica ad approvare, rifiutare, Rework e scartare: 
  - Draft `developer.DISCARD_PROPOSAL` → Discarded solo quando nessun job al lavoro la scrive e, per una creata o scritta dalla Chat, nessun turno della Chat è in corso; le Proposal che la richiedono restano bloccate dicendo che è stata scartata.
  - Open `developer.APPROVE_PROPOSAL` → Approved solo quando nulla la blocca, il job che l'ha consegnata è finito, nessun turno della Chat è in corso per una creata dalla Chat e nessuna Run lavora.
  - Draft/Open → Refining rifiutato mentre l'agente che l'ha consegnata sta ancora finendo.
  - Codice: `AgentFacts::is_finishing` (`crates/core/src/project/board.rs` l.62–66) deve sapere del turno della Chat; i fatti li riempie `Jobs::facts_of` (`src-tauri/src/jobs/registry.rs` l.428–455) e il turno lo conosce `src-tauri/src/general/service.rs`; `gate_of`, `plan_reject`, `plan_discard` in `transitions.rs`, `approve_reasons` in `proposal/analysis.rs`.
- **Clear.** `general-chat.CLEARED` ricevuto da Draft → Discarded: solo una Draft creata dalla Chat, non una solo riaperta; emette `proposal.DISCARDED`; le Proposal che la richiedono restano bloccate dicendo che è stata scartata. Regola: Clear della Chat scarta, dopo che il suo turno si è fermato, ogni `draft` con `chat: true` che non è `refining`, come uno scarto del developer; ogni richiesta che teneva torna a Writing. Una `draft` con `chatWriting: true` ma senza `chat` resta. Stop, uscita di omp, chiusura della finestra e uscita da Weaver non scartano nulla. Punto d'aggancio: il Clear in `src-tauri/src/general/service.rs` (`Effect::Clear`, dopo lo stop del turno) chiama lo scarto di `transitions.rs:plan_discard`; la richiesta torna modificabile tramite `crates/core/src/project/requests/store.rs`/`src-tauri/src/requests.rs`.
- **`project show` (`requirements/cli#project`):** `inWriting` elenca le Draft con il loro job, oppure `chat: true` per una Draft che la Chat sta scrivendo (`chatWriting`). `crates/cli/src/commands/project.rs` l.80 (schema) e l.296–323, 434.
- **Decisioni dalla Chat:** i comandi di decisione accettano ogni chiamante con le stesse condizioni; skill, istruzione della Chat e `--help` permettono all'agente di eseguirli solo quando il developer l'ha chiesto esplicitamente (aggiornare il `--help` interessato).
- `--help` di `proposal new`/`edit`/`move` e di `question new` allineati a quanto sopra.

## Test

- `crates/core/src/project/proposal/create.rs` e i suoi test: binding per job (solo richiesta nominata, solo prima Proposal), Chat (richiesta libera non tenuta, `reworks` anche senza Rework, stesso tipo), altro chiamante (nessun binding, con nota); i test che oggi legano `request`/`reworks` senza chiamante vanno cambiati, non ripinnati.
- `setup`/`import` dalla Chat rifiutati; `chat`/`chatWriting` scritti e `chatWriting` tolto da `move open`.
- `board.rs`/`transitions.rs`/`analysis.rs`: motivo 1 per turno della Chat in corso su approva, rifiuta, Rework, scarta.
- Clear: scarta le Draft con `chat: true` (non `refining`), lascia quelle solo con `chatWriting`, la richiesta torna a Writing; Stop non scarta (`src-tauri/src/general/tests.rs`).
- `cli_handler.rs`: `WEAVER_GENERAL` non riconosciuto = nessuna identità; `project show` con `chat: true`; contract test `tests/cli-contract/contract.test.mjs` l.410–473 aggiornato per l'accesso di `question new` e i nuovi rifiuti.
