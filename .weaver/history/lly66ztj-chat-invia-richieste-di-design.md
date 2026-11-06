---
created: 2026-11-06T12:06:31.853Z
updated: 2026-11-06T12:06:31.853Z
summary: 'La Chat non scrive più Proposal: su ordine esplicito invia una richiesta di design con il nuovo comando per le richieste, che avvia il job di design o di setup.'
tasks:
- 3jzg8pk2
- 85raewc9
- ma26cxlp
- mr6evezq
- ntv9b1ig
- p9qyjiz5
- pfco4h8r
- q8049z1t
- qxumalg4
cleared:
  3jzg8pk2:
    title: Chat invia request
    refs:
    - general-chat
    - requirements/rules#general-chat
    - requirements/tech#general-chat
    - requirements/ui#general-chat
    technical: false
    proposal: 7jwye2hg
    label: Chat request
    finished: 2026-11-06T11:51:39.375Z
    body: |-
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
  85raewc9:
    title: Remove code of `proposal.DISCARDED`
    refs:
    - proposal
    technical: false
    proposal: 7jwye2hg
    label: Chat request
    finished: 2026-11-06T11:50:05.088Z
    body: ''
  ma26cxlp:
    title: Skill senza Chat
    refs:
    - requirements/rules#project
    technical: true
    proposal: 7jwye2hg
    label: Chat request
    finished: 2026-11-06T11:49:57.870Z
    body: |-
      Le skill installate non hanno più una sezione per Chat. Quando è fatto deve valere quanto segue.

      - `weaver-design` e `weaver-resolve-drifts` non hanno più la sezione `Writing from Chat`. La loro descrizione e le loro regole rigide le applicano solo quando l'istruzione di un job di Weaver le nomina e tengono fuori ogni conversazione che non è un job, Chat e Ask more allo stesso modo. Dicono che in Chat una Proposal parte come design request inviata con `weaver request new`.
      - Il resto delle due skill resta com'è: sezione `## Language`, "Sub-agents context sharing", `timeout: 0`, riferimento dei comandi.
      - Le copie installate in questo repository (`.agents/skills/`) vengono riallineate con Update skills, non a mano.

      ## Code notes

      Commit: 02129bbb382d41a3a2b2b107bf82f809097051bb

      - `crates/core/assets/skills/weaver-design/SKILL.md`: descrizione (riga 3: "…or when the instruction of Chat sends you here…", "(from Chat, with the conversation as the interview)"), prima regola rigida (riga 22, rimando a "Writing from Chat"; l'esclusione di Ask more resta), sezione `## Writing from Chat` (righe 50-57) da togliere.
      - `crates/core/assets/skills/weaver-resolve-drifts/SKILL.md`: descrizione (riga 3) e `## Writing from Chat` (righe 46-53); le regole rigide (21-26) non citano Chat.
      - Embedding: `crates/core/src/agent_files.rs` (`ASSETS = include_dir!`, l.40). Il test `the_design_and_resolve_skills_have_the_chat_section_and_no_longer_exclude_chat` (~592-617) fissa la sezione: va tolto o riscritto come controllo di assenza. `every_skill_points_to_the_reference…` (~620+) richiede `## Language`, "Sub-agents context sharing" e `` `timeout: 0` `` in ogni skill.
      - Nessun'altra skill, `omp/agents/*.md` o `omp/WATCHDOG.yml` cita Chat.
  mr6evezq:
    title: Proposal senza Chat
    refs:
    - proposal
    - requirements/rules#proposal
    - requirements/rules#job
    - requirements/data#proposal
    - requirements/cli#proposal
    - requirements/cli#project
    - requirements/cli#agent
    - requirements/ui#proposal
    - requirements/ui#General
    technical: false
    proposal: 7jwye2hg
    label: Chat request
    finished: 2026-11-06T11:57:08.205Z
    body: |-
      Una Proposal la scrive solo l'agente di un job: Chat non ne crea, non ne modifica e non ne consegna più. Quando è fatto deve valere quanto segue.

      - `proposal/Draft`: la scrive l'agente di un job e la Board mostra solo il ghost di quel job. Non esiste più la card "Chat Draft", con i badge `draft` e `Chat`, `Open Chat` e `Discard`.
      - `proposal/Draft/developer.DISCARD_PROPOSAL`: la Draft è scartabile quando nessun job in lavorazione la scrive, senza condizioni legate ai turni di Chat.
      - `proposal/Open/developer.APPROVE_PROPOSAL`: l'approvazione è rifiutata con `The agent is still finishing` solo quando il job che l'ha consegnata lavora ancora, mai per un turno di Chat in corso.
      - `proposal/Open/agent.EDIT_PROPOSAL`: la Proposal torna in Draft e basta, senza segni di scrittura da parte di Chat.
      - `proposal/Rejected`: Chat non può più scrivere una Proposal che ne prenda il posto.
      - Collegamenti (`requirements/rules#proposal`, `requirements/rules#job`): `request` lo lega solo un job di setup o di design alla request della sua istruzione, e solo con la prima Proposal. `reworks` lo lega solo un job di rework alla Proposal della sua istruzione. Ogni altro chiamante non lega nulla e riceve la nota che spiega perché.
      - Comandi:
        - `proposal new`, `proposal edit` e `proposal move <id> open` sono rifiutati (exit 1, nulla scritto) ogni volta che c'è l'identità di Chat, senza interpellare l'app. Il messaggio dice che Chat non scrive Proposal e invia invece una request con `request new`.
        - I comandi di decisione restano permessi a tutti.
        - L'help dei comandi `proposal` non parla più di Chat.
        - `project show` non ha più `chat` in `inWriting`.
      - File (`requirements/data#proposal`): `proposal.md` non ha più le chiavi `chat` e `chatWriting`. Se un file di una versione precedente le contiene, vengono ignorate in lettura e tolte alla prima scrittura di quel `proposal.md`. Una `draft` che le porta è una Draft normale senza job: compare in `weaver proposal list` e si scarta con `weaver proposal move <id> discarded`.
      - L'icona Board nella barra laterale conta le card reali nel Backlog. Non c'è più niente da escludere per Chat.

      ## Code notes

      Commit: 02129bbb382d41a3a2b2b107bf82f809097051bb

      - Dati: `crates/core/src/project/proposal/data.rs` `KEYS` (:27-28), `chat_writing()` (:175-179). Creazione e modifica: `proposal/create.rs` (`Writer::Chat`, `bind_request` rami Chat ~99-160, `bind_reworks` ramo Chat ~194-212, rifiuto setup/import ~256-261, `chat`/`chatWriting` ~306-309, `EditRequest.chat` ~358-360/716-718). Consegna: `proposal/transitions.rs` (rimozione `chatWriting` :179, :397; `chat_drafts_to_clear`/`clear_chat_drafts` ~753-795); doc `proposal/mod.rs:35-39`.
      - Motivo 1: `crates/core/src/project/board.rs` `AgentFacts.chat_working` (:61), `is_finishing` (:70-76), `is_chat_draft`/`ProposalState::ChatDraft` (:85-101, :183); app `src-tauri/src/board.rs:86-96` `ProposalStanding::ChatDraft`; `src-tauri/src/proposals.rs:457-465` (`from_chat` alla consegna).
      - CLI: `crates/cli/src/commands/proposal.rs` (note di help :50-51, :93, :123; Who→`Writer` ~240-262; `chat: who == Who::Chat` :765). `Who::Chat` viene da `crates/cli/src/exec.rs:258-272`/`spec.rs:218-238`: il rifiuto va lì come `Failure::refused` su `caller.who == Who::Chat`, ma senza interpellare l'app. `crates/cli/src/commands/project.rs` (:80-83, :318, :381, :443-445). Rigenerare `COMMANDS.md` con `cargo run -p weaver-cli --example write_command_reference`.
      - UI: `ui/src/components/board/proposal-card.tsx` (:44, badge :90-93, `ChatDraftControls` :337-360, :402), `ui/src/bindings/ProposalStanding.ts`, conteggio `ui/src/components/project-content.tsx:36-37`.
      - Test da aggiornare o togliere: `board.rs:649-694`, `proposal/tests.rs` (~2145-2163, 2328-2590), `cli/commands/proposal.rs:1328-1533`, `src-tauri` `board.rs:625-672`, `cli_handler.rs:354-405`, `proposals.rs:900-930`; UI `proposal-card.test.tsx:394-468`, `board-view.test.tsx:172-184`, `project-content.test.tsx:35-36`.
  ntv9b1ig:
    title: Remove code of `design-request/Held by Chat`
    refs:
    - design-request
    technical: false
    proposal: 7jwye2hg
    label: Chat request
    finished: 2026-11-06T11:51:38.193Z
    body: ''
  p9qyjiz5:
    title: Remove code of `design-request/Writing/agent.OPEN_PROPOSAL`
    refs:
    - design-request/Writing
    technical: false
    proposal: 7jwye2hg
    label: Chat request
    finished: 2026-11-06T11:57:12.638Z
    body: ''
  pfco4h8r:
    title: Remove code of `general-chat.CLEARED`
    refs:
    - general-chat
    technical: false
    proposal: 7jwye2hg
    label: Chat request
    finished: 2026-11-06T11:51:57.547Z
    body: ''
  q8049z1t:
    title: Comando request new
    refs:
    - design-request
    - job/Not started/agent.SEND_REQUEST
    - agent
    - requirements/cli#design-request
    - requirements/cli#General
    - requirements/data#design-request
    - requirements/ui#design-request
    - requirements/tech#assets
    technical: false
    proposal: 7jwye2hg
    label: Chat request
    finished: 2026-11-06T11:59:09.458Z
    body: |-
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
  qxumalg4:
    title: Remove code of `proposal/Draft/general-chat.CLEARED`
    refs:
    - proposal/Draft
    technical: false
    proposal: 7jwye2hg
    label: Chat request
    finished: 2026-11-06T11:59:46.643Z
    body: ''
---
# Chat invia richieste di design

La Chat non scrive più Proposal, né Draft né rielaborazioni: su ordine esplicito dello sviluppatore scrive una richiesta di design autonoma e la invia con il comando per le richieste, riservato all'agente della Chat, e Weaver avvia il job di design (o di setup a Model vuoto). Sono state rimosse la richiesta trattenuta dalla Chat, il Draft di Chat sulla Board, lo scarto dei Draft alla pulizia della Chat e le relative istruzioni, interfaccia, comandi e skill; CHANGELOG e testi dell'interfaccia sono aggiornati.
