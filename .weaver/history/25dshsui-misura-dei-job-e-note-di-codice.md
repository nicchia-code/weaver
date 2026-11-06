---
created: 2026-11-05T20:00:44.398Z
updated: 2026-11-05T20:00:44.398Z
summary: Weaver misura la durata dei job di design, rework, refine e Run; le skill installate chiedono note di codice nei testi dei Task.
tasks:
- i7osguyb
- lyhvs3xn
cleared:
  i7osguyb:
    title: Misura dei job
    refs:
    - job
    - requirements/data#job
    - requirements/data#General
    - requirements/tech#job
    - requirements/cli#job
    - requirements/docs#General
    technical: false
    proposal: lhn2o96u
    label: Misura dei job
    finished: 2026-11-05T19:49:16.858Z
    body: |-
      Quando un job di design, rework, refinement o un Run finisce, in qualunque modo (Done, Failed o Stopped), Weaver lo misura sul computer dello sviluppatore, così lo sviluppatore può confrontare i job prima e dopo un cambio del modo di lavorare dell'agente. Gli altri tipi di job (setup, import, scan, resolve, merge) non vengono misurati.

      Quando il Task è fatto:

      - **Durate.** Ogni job misurato registra la durata totale (da `created` a `ended`) e la durata attiva: solo il tempo passato in Running, senza Queued, Waiting for the developer e Suspended. Il record del job nel registro locale tiene i millisecondi passati in Running e, mentre è in Running, l'istante in cui ci è entrato (`running`), aggiornati a ogni cambio di stato (oggi il record ha solo `created`, `started` ed `ended`), così un job sospeso e ripreso continua a contarli. Se Weaver si è fermato di colpo (crash o chiusura forzata) mentre il job era Running o Waiting for the developer, alla riapertura il tempo dall'ultimo cambio di stato è ignoto: non si conta, il record prende `runningIncomplete` e la riga del job `activeIncomplete: true`.
      - **Letture, ricerche e token.** A fine job Weaver legge una volta i file di sessione di omp: il file principale (nome che finisce con `_<sessionId>.jsonl`) è l'agente principale; ogni transcript nella sua cartella e nelle cartelle annidate, a qualunque profondità, è un sub-agente, tranne quelli il cui nome inizia con `__advisor.`, che sono gli advisor di un Run. Per ogni parte somma i token dell'`usage` di ogni `message` con `role: assistant` (input, output, cache letta, cache scritta), conta le chiamate dello strumento `read` (letture) e i percorsi distinti letti (senza selettore di righe, relativi al progetto con `/` dentro il progetto, come dati altrimenti), e le chiamate di `grep` e `glob` (ricerche). I comandi di shell non contano mai. I file distinti si contano anche per tutto il job.
      - **Riga in `.weaver/timings/jobs.jsonl`.** Una riga JSON per job, aggiunta una sola volta alla fine, mai riscritta, con i campi e l'ordine di `requirements/data#job`. Weaver non cancella né accorcia mai il file e non legge né tocca gli altri file della cartella (vi restano vecchi file di Trama). `timings/` è una cartella solo locale con il suo `.gitignore` `*`, creata con la prima riga e riscritta quando manca, come le altre di `requirements/data#General`.
      - **Sessione illeggibile.** Senza id di sessione (per esempio un job fermato in coda), con il file principale mancante o illeggibile o con una riga completa malformata in un file, la riga c'è comunque, con durate ed esito, e `unavailable` con il motivo che nomina il file al posto di letture e token. Un'ultima riga incompleta si ignora.
      - **Mai d'intralcio.** La lettura avviene dopo che la fine del job è stata applicata e mostrata, fuori dai thread della UI e dell'IPC, leggendo ogni file riga per riga senza tenerlo intero in memoria. Non ritarda la fine, il rilascio di ciò che il job tiene o il job successivo, e un suo errore non cambia nulla del job.
      - **Riga di comando.** `weaver job show <id>` aggiunge `measure`, la riga del job quando è stata scritta: in `--json` la riga com'è; in testo le due durate e poi una riga per parte (agente principale, sub-agenti con il loro numero, advisor) con letture, file, ricerche e token, oppure il motivo di `unavailable`. `weaver job show` senza id e `weaver job list` non cambiano.
      - **Testi per l'agente.** Nessun testo installato per l'agente parla della misura: l'agente non fa nulla di diverso.
      - **Test.** Con file di sessione finti in cartelle temporanee: somma dei token e conteggi per parte, transcript annidati, advisor separati, file distinti con selettori di righe diversi, comandi di shell ignorati, ogni motivo di `unavailable`, ultima riga incompleta; durata attiva con attese, coda, una sospensione ripresa e una ripresa dopo un crash con `activeIncomplete` (orologio iniettato); job non misurati senza riga; `.gitignore` scritto con la cartella; output di `weaver job show <id>` in testo e `--json`, con e senza misura.
      - **CHANGELOG.** Una riga sotto `## [Unreleased]`, `Added`: Weaver registra durata, letture e token di design e run in `.weaver/timings`, leggibili con `weaver job show`.

      Punti nel codice già noti dall'esplorazione:

      - registro dei job e stati: `crates/core/src/jobs/record.rs` (`JobRecord`, `KEPT_FINISHED`), `src-tauri/src/jobs/registry.rs` (`Jobs::dispatch_in`, che vede stato prima e dopo e salva a ogni cambio; `Jobs::finish`, lavoro dopo la fine fuori dal lock, punto per la misura; `Jobs::adopt_records`, che dopo un crash porta Running/Waiting a Suspended senza passare dalla macchina), `crates/core/src/jobs/machine.rs`;
      - cartella delle sessioni e file principale: `src-tauri/src/jobs/host.rs` (`Host::sessions_root`), `src-tauri/src/jobs/registry.rs` (`locate_agents`), `crates/core/src/jobs/subagents.rs` (`find_main_file`, `agent_file`);
      - lettura dei transcript, che oggi ignora `usage`: `src-tauri/src/subagents/reader.rs`, `src-tauri/src/subagents/records.rs` (`apply`);
      - output di `weaver job show <id>`: `src-tauri/src/jobs/authority.rs` (`job_show`, che legge la tabella dei job in memoria con `Jobs::list_for`, job finiti compresi), `crates/cli` (comando `job`);
      - cartelle solo locali: `crates/core/src/project/layout.rs` (`LOCAL_ONLY_DIRS`, `ensure_local_folder`);
      - parole vietate nei testi installati (non vanno toccate): `crates/core/src/agent_files.rs`, test `OUT_OF_SCOPE_WORDS`, `OUT_OF_SCOPE_PHRASES`.
  lyhvs3xn:
    title: Note di codice nelle skill
    refs:
    - task
    - requirements/data#task
    - requirements/data#proposal
    - requirements/data#General
    - requirements/rules#project
    - requirements/docs#General
    technical: false
    proposal: 4k5hy92m
    label: Note di codice
    finished: 2026-11-05T19:41:23.768Z
    body: |-
      Una Task scritta da una design, da uno scan o da un compare può portare note di codice per chi la implementa, fatte solo di ciò che l'agente ha già letto del codice: dove guardare, le convenzioni da seguire, le insidie viste e cosa si può riusare, con il commit su cui il codice è stato letto. Sono suggerimenti, mai parte di ciò che deve essere vero. Chi implementa la Task parte da lì e riesplora solo i file cambiati da quel commit, o tutti i file citati quando non si può sapere; legge sempre un file prima di cambiarlo, e quando una nota contraddice il codice vince il codice. Una Task senza note, e ogni fix Task, si implementa come oggi. Weaver non legge né controlla le note: il cambio è tutto nei testi installati per l'agente, sorgenti in `crates/core/assets/` (le copie in `.agents/skills/` e `.omp/agents/` si aggiornano con Update skills, mai a mano).

      Quando il Task è fatto:

      - **Formato** (`requirements/data#task`, **Code notes**). La sezione `## Code notes` chiude il corpo della Task. Prima riga `Commit: <hash completo>`, assente se il progetto non è in un repository git; poi Markdown breve: file tra backtick, relativi al progetto con `/`, e nomi nel codice; convenzioni; insidie; cosa riusare. Circa 1.500 caratteri al massimo, come guida, mai controllata. Le fix Task non l'hanno mai. Il riferimento di formato della design (`FORMAT.md`, sezione dei Task di una Proposal) dice che il corpo può finire con `## Code notes` invece di "Body: prose" e basta.
      - **Chi scrive le note.** `weaver-design`, `weaver-scan`, `weaver-compare` e il sub-agente `weaver-analysis` dicono all'agente di registrare `git rev-parse HEAD` una volta, prima di esplorare, e di chiudere ogni Task che scrive o crea (con `weaver proposal edit` o `weaver task new`), tranne le fix Task, con `## Code notes`, fatte solo di ciò che la sua esplorazione e i resoconti dei suoi sub-agenti hanno già letto: mai leggere codice solo per scrivere le note. Il corpo prima della sezione resta in parole di dominio, come oggi.
      - **Design.** I sub-agenti di esplorazione riportano, oltre ai punti nel codice, simboli, convenzioni, insidie e cosa riusare; i sub-agenti che scrivono le Task li ricevono (sezione "Sub-agents context sharing", voce dello step 5). La rifinitura di una Proposal stale lascia note e commit come sono; la rielaborazione di una Proposal rifiutata scrive note nuove dalla propria esplorazione; lo scan leggero dopo la consegna non aggiunge note.
      - **Implement.** `weaver-implement` (step 3 "Read the code", voce "Implementing a Task" e frase "It always reads the code, also at the places it received" di "Sub-agents context sharing") e `weaver-implementer` (step 4) dicono, per una Task con `## Code notes`: partire dalle note; eseguire `git diff <commit> -- <file>` sui file citati, contro i file sul disco; un file citato non cambiato non si riesplora, e di quelli si leggono solo i file da cambiare, prima di cambiarli; un file citato cambiato si riesplora come senza note; senza riga del commit, fuori da un repository git o con un commit sconosciuto, ogni file citato conta come cambiato (si parte comunque dalle note); ogni file si legge prima di cambiarlo, citato o no. Una nota che contraddice il codice si ignora e quella parte si esplora come senza note, senza Drift; il report del sub-agente e il report finale del run dicono quale nota era sbagliata. Una Task senza `## Code notes`, e ogni fix Task (che rilegge Where in the code), si implementa come oggi.
      - **Vincoli dei testi.** In inglese, senza prima persona, ogni definizione di sub-agente sotto 6 KB (`weaver-analysis.md` e `weaver-implementer.md`); nessun testo installato usa le parole vietate dai test di `agent_files.rs` (`timing`, `weaver timing` e le altre). Ogni comando citato esiste nel registro della CLI.
      - **Requisiti già scritti in questa Proposal**, senza codice da cambiare: il fragment `requirements/data/project-files.md` diviso in `project-files` e `card-files` (testo delle sezioni invariato) e "keys in the order listed in this Area" in `requirements/data#General`.
      - **Test.** I test esistenti dei testi installati restano verdi (`crates/core/src/agent_files.rs`: inglese, prima persona, 6 KB, comandi nel registro, sezioni nominate dalle istruzioni); il test che lega `FORMAT.md` al parser resta verde con la nuova riga sul corpo dei Task.
      - **CHANGELOG.** Una riga sotto `## [Unreleased]`, `Changed`: le Task scritte da design, scan e compare portano note di codice con il commit, e chi le implementa riesplora solo i file cambiati da quel commit.

      Punti nel codice già noti dall'esplorazione:

      - `crates/core/assets/skills/weaver-design/SKILL.md`: step 1 (resoconto dei sub-agenti di esplorazione), step 5 (formato delle Task), "Refining a stale Proposal" passi 1 e 4, "Sub-agents context sharing" voce "Writing the Tasks (step 5)";
      - `crates/core/assets/skills/weaver-design/FORMAT.md`: sezione "Tasks of a Proposal and Already present";
      - `crates/core/assets/skills/weaver-implement/SKILL.md`: step 1 "With sub-agents", step 3 e controllo della premessa delle fix Task, "Sub-agents context sharing";
      - `crates/core/assets/omp/agents/weaver-implementer.md`: step 3 e 4, paragrafo del report;
      - `crates/core/assets/skills/weaver-scan/SKILL.md` step 6 "File Tasks" (oggi il corpo dice "not which files to touch"); `crates/core/assets/skills/weaver-compare/SKILL.md` (creazione di Task con `weaver task new`); `crates/core/assets/omp/agents/weaver-analysis.md`;
      - vincoli: `crates/core/src/agent_files.rs` (`SUBAGENT_DEFINITION_LIMIT`, test `OUT_OF_SCOPE_WORDS`, `OUT_OF_SCOPE_PHRASES`, `every_installed_text_is_english`, `no_installed_text_uses_the_first_person`, `every_quoted_command_is_in_the_registry`), `crates/core/src/job_instruction.rs` (frase fissa su "Sub-agents context sharing");
      - il corpo arriva intatto all'approvazione e alla History: `crates/core/src/project/tasks.rs` (`approved_task_file`, `body_after_title`), `crates/core/src/project/history.rs` (`TaskCopy`); nessun cambio lì.
---
# Misura dei job e note di codice

Ogni job di design, rework, refinement e Run registra la sua misura, visibile in `job show`. Le skill di design, scan, compare e implement e gli agenti hanno le istruzioni sulle note di codice.
