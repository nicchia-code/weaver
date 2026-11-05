---
refs:
  - job
  - developer.RESOLVE_MERGE_CONFLICTS
  - work
  - requirements/rules#job
  - requirements/data#job
  - requirements/cli#job
  - requirements/ui#job
---
# Job del merge

Quando il lavoro è finito, i job di Weaver sanno lavorare con un merge in corso di Solo mode (`solo-merge`): c'è un nuovo tipo di job per i conflitti, lo scan dopo un merge è un normale scan con i file portati dal merge, e l'ammissione dei job tiene conto del merge in corso.

**Nuovo tipo `merge`.** `JobKind` ha il tipo `merge` (in `ALL`, `as_str` = `merge`, binding TS `ui/src/bindings/JobKind.ts` aggiornato). Il job si chiama sempre `Merge conflicts` (`requirements/ui#job`, **Fixed names**) e nella riga dei Jobs il tipo in testo secondario è `merge`. Usa il ruolo del modello Analysis delle Settings (`role()`), senza `--slow` e senza `--advisor`, con l'ambiente di ogni job (`weaver_env`). Non è fra i tipi di `job new` né di `agent.START_JOB`: si avvia solo con `developer.RESOLVE_MERGE_CONFLICTS`, cioè dall'avviso del merge in corso (`requirements/cli#job`). Nella registry il `kind` è `merge` (`requirements/data#job`).

**Istruzione.** La prima frase del job `merge` è ``Use the weaver-resolve-conflicts skill on the merge in progress recorded in `.weaver/merge/merge.json`.``, seguita dalle frasi comuni come per ogni job (`requirements/rules#job`). L'agente risolve i conflitti, chiede allo sviluppatore con un Question round quando la scelta fra le due versioni non è ovvia, segna in git i file risolti, non fa mai il commit e non annulla mai il merge; se il job finisce e restano conflitti, il merge resta in corso. Questo comportamento sta nella skill `weaver-resolve-conflicts`, che è un altro Task.

**Scan dopo un merge.** Su `solo-merge.CONCLUDED` Weaver avvia da solo, senza conferma, uno scan sul System intero: tipo `scan`, nome `Scan`, avviato dallo stesso percorso di Scan System (`start_scan`). La prima frase è ``Use the weaver-scan skill on the whole System after a merge: read the files the merge brought with `weaver job show`.`` Il record di registry ha `merged`: i file portati dal merge (diversi fra il commit di partenza e il commit del merge), percorsi relativi con `/`, ordinati, solo per questo scan. `weaver job show` senza id lo mostra come `merged`. Lo scan conta come l'ultimo Scan System e azzera il numero dei Task fatti, come ogni scan (`work`: la scan è chiesta dallo sviluppatore o avviata da Weaver a merge concluso). Il calcolo dei file e il rilevamento della conclusione appartengono al motore del merge, che è un altro Task: qui serve il modo di avviare lo scan con `merged`.

**Ammissione.** Mentre un merge è in corso (esiste `.weaver/merge/merge.json`) sono rifiutati, con il motivo `A merge is in progress`: Run di un Task, Run all, resolve (anche quelli di `developer.RESOLVE_DRIFT` e di `question-round.ANSWERED`), scan e import. Design, rework, refinement e il job `merge` restano possibili. Il job `merge` è rifiutato quando non c'è un merge in corso con file ancora in conflitto, o quando un altro job `merge` del progetto non è finito. `agent.START_JOB` ha gli stessi rifiuti di quando chiede lo sviluppatore. Il fatto "merge in corso" (e "conflitti presenti") entra in `AgentFacts` per `check_start`.

Punti nel codice: `crates/core/src/jobs/kinds.rs:JobKind` (`ALL`, `as_str`, `role()`, `uses_analysis_agents()`, `is_run()`), `ui/src/bindings/JobKind.ts`; `crates/core/src/job_instruction.rs` (`enum Job`, `compose`, `first_sentence`); `crates/core/src/jobs/admission.rs:check_start`, `StartPlan`; `crates/core/src/project/board.rs:AgentFacts`; `src-tauri/src/jobs/starts.rs` (`Jobs::start`, `start_scan`, `start_resolve`); `src-tauri/src/jobs/registry.rs:create` (azzeramento del conteggio quando parte uno scan); `crates/core/src/jobs/launch.rs` (`weaver_env`, `args`, `role_model`), `crates/core/src/jobs/names.rs`; `src-tauri/src/jobs/authority.rs:start_jobs`; `crates/cli/src/commands/job.rs` (`job new`, `job show`).
