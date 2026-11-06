---
technical: false
refs:
  - job
  - requirements/data#job
  - requirements/data#General
  - requirements/tech#job
  - requirements/cli#job
  - requirements/docs#General
---
# Misura dei job

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
