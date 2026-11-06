---
technical: false
refs:
  - task
  - requirements/data#task
  - requirements/data#proposal
  - requirements/data#General
  - requirements/rules#project
  - requirements/docs#General
---
# Note di codice nelle skill

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
