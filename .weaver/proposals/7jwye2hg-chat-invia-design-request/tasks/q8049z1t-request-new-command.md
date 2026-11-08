---
refs:
  - design-request
  - job/Not started/agent.SEND_REQUEST
  - agent
  - requirements/cli#design-request
  - requirements/cli#General
  - requirements/data#design-request
  - requirements/ui#design-request
  - requirements/tech#assets
---
# Comando request new

L'agente di Chat può scrivere e inviare una design request con un comando nuovo, `weaver request new {text}`. La request diventa da subito una come le altre, come se l'avesse scritta e inviata lo sviluppatore. Quando è fatto deve valere quanto segue.

- `design-request/Not written`, su `agent.SEND_REQUEST`, va a Sent. Vale solo dall'agente di Chat durante un suo turno e con un testo non vuoto. La request viene creata con quel testo, senza immagini. Weaver avvia un job di design, o di setup con il Model vuoto, esattamente come all'invio dello sviluppatore, compresi gli asset taggati nel testo. Se il job non può partire non viene creata nessuna request.
- `job/Not started`, su `agent.SEND_REQUEST`, va a Queued con gli stessi rifiuti dell'invio dello sviluppatore.
- Da Sent la request segue le stesse Transition delle altre. Se il suo job finisce senza Proposal torna in Writing e lo sviluppatore la può cambiare e reinviare. Nell'editor delle request compare come `Sent`, con il banner `Sent to the agent · <job name>`.
- Lo stato Held by Chat non esiste più. Dalla lista delle request spariscono il badge `Chat`, il banner `Chat is writing a Proposal from this request` e `Open Chat`. Nessuna Proposal di Chat tiene più una request.
- Comando (`requirements/cli#design-request`):
  - `text` non vuoto dopo il trim, altrimenti exit 2 senza inviare nulla.
  - È accettato solo dall'identità di Chat riconosciuta dall'app come Chat live del progetto con un turno in corso. Qualunque altro chiamante, Ask more e job compresi, è rifiutato con exit 1 senza scrivere nulla, con un messaggio che dice che solo l'agente di Chat può inviare una request.
  - Serve l'app in esecuzione.
  - Request e job nascono sotto un unico lock; un avvio rifiutato non scrive nulla e stampa il motivo con exit 1.
  - Output `{request, job, kind, title}`.
  - `--help` come da Requirement: solo quando lo sviluppatore ha chiesto esplicitamente una Proposal in Chat; testo autosufficiente con H1, decisioni e identità permanenti; niente immagini.
- `weaver --help` elenca l'oggetto `request` con il verbo `new`, e il riferimento dei comandi nelle skill installate lo include.
- File (`requirements/data#design-request`): `.weaver/requests/<id>.md` con l'id dell'istante di creazione, il testo con fine riga LF e un newline finale, senza cartella di immagini. È registrato sotto `sent` con il suo job nello stesso passo; con un avvio rifiutato non si scrivono né il file né la registrazione.
- L'Actor `agent` ha l'Event `SEND_REQUEST`. La sua descrizione dice che solo l'agente di un job scrive una Proposal e solo l'agente di General invia una design request.

## Code notes

Commit: 02129bbb382d41a3a2b2b107bf82f809097051bb

- Il percorso c'è quasi tutto: `weaver job new {"kind":"design","request":…}` (`crates/cli/src/commands/job.rs`, `Access::General`) passa dall'hook `RequestSender` (`src-tauri/src/jobs/authority.rs` ~58-88, `start_jobs` ~344-392), implementato da `GeneralSender` in `src-tauri/src/requests.rs:472-482`, che chiama `RequestsService::send_locked` (~300-360: controllo di vuoto, tag degli asset `crates/core/src/assets/tags.rs::tagged`, `store.job_for` Setup/Design, `jobs/starts.rs` `start_design`/`start_setup` ~235-261). Il nuovo comando può fare `RequestStore::create` + `save` + invio sotto lo stesso lock (`crates/core/src/project/requests/store.rs`). Attenzione: `create` scrive il file prima dell'avvio, quindi va tolto se l'avvio è rifiutato.
- Registro CLI: un file per oggetto in `crates/cli/src/commands/`, aggiunto in `commands::all()` nell'ordine della tabella; il test `TABLE` in `crates/cli/src/registry.rs` deve elencare `("request", &["new"])`. Comandi solo-app: `app_only`, `.place(in_app)` e un ramo in `AppAuthority::run` (`authority.rs` ~138). `Access::General` ammette anche Ask more (`WEAVER_ASK`): serve un controllo su `Who::Chat` o una nuova variante di `Access`, con il testo in `help.rs:106-111` e `exec.rs` `access_failure`.
- `COMMANDS.md` non si modifica a mano: `cargo run -p weaver-cli --example write_command_reference`, altrimenti fallisce `the_command_reference_of_the_skills_is_the_one_the_registry_renders`.
- Held by Chat: `requests/machine.rs` (`HeldByChat`, `OpenProposal{by_chat}`, ~9-39, 125-144), `store.rs` `held_by_chat()` (~226), `consume(id, from_chat)` (~450), `src-tauri/src/requests.rs` `RequestStatus::HeldByChat`/`status_of` (~121-196), `proposal_submitted(…, from_chat)`, `touches_requests` (guarda `proposal.md` solo per le Draft di Chat); UI `ui/src/components/requests/request-pane.tsx` (`HELD_BY_CHAT` :48, banner ~413), `request-editor.tsx:30`, `ui/src/bindings/RequestStatus.ts`. Test: `machine.rs:268-303`, `store.rs:919-1026`, `src-tauri/src/requests.rs:938-1042`, `request-editor.test.tsx:327-364`, `general/tests.rs` `mod job_new`.
