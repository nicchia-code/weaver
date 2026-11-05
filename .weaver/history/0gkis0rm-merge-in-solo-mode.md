---
created: 2026-11-05T14:49:29.072Z
updated: 2026-11-05T14:49:29.072Z
summary: Con Solo mode acceso Weaver unisce un altro branch nel corrente con git, segue il merge con conflitti, li fa risolvere all'agente e a merge concluso avvia da solo lo scan, che segna i Drift da merge.
tasks:
- 9ckiwshv
- adimalgy
- dp2dsr5i
- du4s7bqm
- j7ksa61c
- r1e7delh
cleared:
  9ckiwshv:
    title: Motore del merge
    refs:
    - solo-merge
    - desktop
    - desktop.WINDOW_FOCUSED
    - project-setup
    - requirements/rules#solo-merge
    - requirements/tech#solo-merge
    - requirements/data#solo-merge
    - requirements/data#General
    - requirements/quality#solo-merge
    technical: false
    proposal: 90o0ulxy
    label: Solo merge
    finished: 2026-11-05T14:40:40.281Z
    body: |-
      Quando il Task è fatto, Weaver sa eseguire con git il merge di un altro branch nel branch corrente mentre Solo mode è acceso (Machine `solo-merge` del Feature `project-setup`), ricordare il merge con conflitti in corso, accorgersi di come finisce e, a merge concluso, avviare da solo lo scan del System. Questo Task copre solo il backend: il pulsante `Merge`, il Dialog, l'avviso del merge in corso e gli avvisi sono un altro Task; il tipo di job `merge` (`Merge conflicts`), la skill `weaver-resolve-conflicts` e l'ammissione dei job durante il merge sono altri Task.

      ## Modulo git in `weaver-core`

      Un solo modulo di `weaver-core`, accanto a quello di Solo mode, esegue ogni passo git del merge e legge e scrive `merge/merge.json` (`requirements/tech#solo-merge`). Usa lo stesso runner di Solo mode: niente shell, cwd la radice del progetto, `GIT_OPTIONAL_LOCKS=0` per le chiamate che leggono soltanto, `GIT_EDITOR=true` e `GIT_MERGE_AUTOEDIT=no`, limite di 10 s per chiamata e di 2 minuti per `git merge`. Nessuna chiamata git sul thread della UI o dell'IPC. Il merge esiste solo nell'app: la riga di comando non ha un comando di merge.

      - **Disponibilità** (`requirements/rules#solo-merge`), controllata in quest'ordine, il primo motivo che vale è quello mostrato: un job del progetto in coda, al lavoro, in attesa dello sviluppatore o sospeso (General e Ask more non sono job); un merge di Weaver in corso (`merge.json` esiste); una chiamata git che fallisce o supera il limite (motivi di `requirements/rules#solo-mode`); git a metà di un'operazione (`MERGE_HEAD`, `CHERRY_PICK_HEAD` o `REBASE_HEAD` risolvono con `git rev-parse -q --verify`, oppure esistono `rebase-merge/` o `rebase-apply/` in `git rev-parse --git-path`); HEAD staccato (`git symbolic-ref -q HEAD` fallisce); `git status --porcelain=v1 -z --untracked-files=no` elenca un percorso che nessuna riga del blocco di Solo mode di questa copia di lavoro copre. I file di Weaver sono quindi esclusi dalle modifiche non committate, in entrambi i modi di Solo mode.
      - **Branch:** `git for-each-ref --format=%(refname)` su `refs/heads` e `refs/remotes`, per nome breve, prima i locali poi i remoti, ciascuno in ordine di nome, senza il branch corrente e senza ogni `<remote>/HEAD`. Mai `git fetch`. Insieme ai branch, i file di Weaver ancora tracciati (**Tracked files** di `requirements/rules#solo-mode`): se la lista non è vuota la conferma ricorda che le loro modifiche entrano nel commit del merge.
      - **Avvio**, tutto nella coda delle mutazioni del progetto: si ricontrollano le condizioni (una che vale rifiuta il merge con il suo motivo); `start` = `git rev-parse HEAD`, `current` = nome breve del branch corrente, `incoming` = `git rev-parse <branch>^{commit}`; se `git merge-base --is-ancestor <incoming> <start>` riesce, il branch non porta nulla di nuovo, non si avvia nulla e Weaver dice che il branch corrente è già aggiornato; altrimenti si scrive `merge/merge.json` **prima** di avviare git (così un merge interrotto da un crash viene ritrovato alla riapertura) e poi si esegue `git merge --no-edit --no-overwrite-ignore <branch>` con il limite di 2 minuti.
      - **Esito di `git merge`:** uscita 0 → concluso subito, il commit del merge è HEAD (il commit in arrivo dopo un avanzamento rapido), `merge.json` cancellato; uscita non zero con `MERGE_HEAD` → stato `Merging`, i conflitti sono i percorsi di `git diff --name-only -z --diff-filter=U`; uscita non zero senza `MERGE_HEAD` → rifiutato, nulla cambia, `merge.json` cancellato, il motivo è quello dei file ignorati con i loro percorsi quando l'errore di git elenca file non tracciati che sarebbero sovrascritti e il blocco di Solo mode li copre, altrimenti la prima riga dell'errore di git; limite superato → git viene ucciso, poi `git merge --abort` se `MERGE_HEAD` risolve, `merge.json` cancellato, motivo `git did not finish the merge within 2 minutes`, branch com'era.
      - **Controllo del merge in corso**, finché `merge.json` esiste: `MERGE_HEAD` risolve → ancora `Merging`, conteggio dei file con conflitti ricalcolato; altrimenti, se il branch corrente è `current`, `<start>` e `<incoming>` sono antenati di HEAD e HEAD è diverso da `<start>` → committato, il commit del merge è il più vecchio di `git rev-list --first-parent --ancestry-path <start>..HEAD` (vale anche con commit successivi sopra); qualsiasi altro caso (annullato fuori da Weaver, altro branch, reset) → dimenticato senza scan, con un avviso. `merge.json` viene cancellato a ogni fine. Una chiamata git che fallisce durante il controllo lascia il merge in corso e viene registrata nel log.
      - **Annullamento:** `git merge --abort`, dopo la conferma della UI; rifiutato mentre il job `merge` del progetto non è concluso; se fallisce il merge resta in corso e la Board mostra `Could not abort the merge: <first line of git's error output>.` Riuscito: merge dimenticato, nessuno scan.
      - **File del merge:** `git diff --name-only -z --no-renames --relative <start> <merge commit>` eseguito nella radice del progetto, percorsi relativi a essa con `/`, ordinati per percorso: il lavoro degli altri e le risoluzioni dei conflitti.
      - Weaver non fa mai il commit di un merge con conflitti, non mette mai in stage un file, non esegue mai `git fetch`, non sovrascrive mai un file che Solo mode tiene fuori da git e non lascia mai `merge.json` dietro un merge finito (`requirements/quality#solo-merge`).

      ## `merge/merge.json` e cartella locale

      `.weaver/merge/merge.json` (`requirements/data#solo-merge`) contiene `{"branch", "current", "start", "incoming", "started"}`: il branch scelto come mostrato, il nome breve del branch corrente, gli hash completi dei commit di partenza e in arrivo, l'istante della scrittura; UTF-8, LF, scritto in modo atomico come ogni file di progetto; mai in `settings.json`, nel registro dei job o nel repository. Nessun file significa nessun merge in corso; la cartella `merge/` resta con il suo `.gitignore`. Un file illeggibile o con un campo mancante viene segnalato come diagnostica con percorso e motivo, il controllo lo tratta come merge dimenticato (senza scan) e il file è cancellato. `merge/` è una nuova cartella solo locale (`requirements/data#General`): ha il proprio `.gitignore` con `*`, scritto alla creazione della cartella e riscritto quando manca, come `requests/`, `short-numbers/`, `task-images/`, `notes/` e `solo/`.

      ## Stato nell'app e comandi Tauri

      `ProjectStatus` porta `merge`, assente con Solo mode spento: `available` (booleano), `reason` (quando non disponibile), `inProgress` (`{branch, current, conflicts}`, `conflicts` il numero dei file con conflitti, finché `merge.json` esiste), inviato con `project-state` (`requirements/tech#solo-merge`). Viene ricalcolato quando la vista Board è mostrata (`merge_refresh`), su `WindowEvent::Focused(true)` della finestra del progetto, quando un job del progetto finisce, dopo ogni comando di merge e all'apertura della finestra, nel lavoro in background del progetto, accorpato in modo che giri un solo calcolo alla volta per progetto; mai a intervalli e senza osservare la cartella di git.

      Il ritorno in primo piano della finestra è il nuovo Event `desktop.WINDOW_FOCUSED` dell'attore `desktop` ("A project window came back to the foreground"): nelle transizioni di `Merging` vale come `window.OPENED` (committato → concluso; sparito → dimenticato con avviso; ancora in corso → conteggio aggiornato). Lo stesso vale per `job.ENDED`, anche dopo un job `merge` finito con conflitti rimasti, che lascia il merge in corso. `workspace.PROJECT_FOLDER_REMOVED` porta via la memoria del merge: il merge di git resta com'è e nessuno scan parte.

      Comandi Tauri, ciascuno nella coda delle mutazioni del progetto sotto il lock del progetto, che restituiscono il nuovo stato o l'errore con il motivo: `merge_refresh {projectId}` (disponibilità e merge in corso), `merge_branches {projectId}` (branch e file tracciati, all'apertura del Dialog), `merge_start {projectId, branch}`, `merge_abort {projectId}`, `resolve_merge_conflicts {projectId}`. Quest'ultimo, ammesso solo con file ancora in conflitto e nessun job `merge` non concluso, si limita a chiamare l'avvio del job `merge` definito nel suo Task. I comandi vanno registrati con i loro permessi e nel client della UI.

      `set_solo_mode` è rifiutato mentre `merge.json` esiste, sia per `off` sia per l'altro modo, con il motivo di `requirements/ui#solo-mode`.

      ## Conclusione

      Il passo che trova il merge concluso (senza conflitti all'avvio, o committato al controllo) emette `CONCLUDED` con i file del merge, cancella `merge.json` e nello stesso passo avvia, senza conferma, un job di tipo `scan` chiamato `Scan` attraverso lo stesso percorso di avvio di Scan System; il suo record nel registro riceve `merged` (i file del merge), che l'agente legge con `weaver job show` (`requirements/data#job`). Conta come l'ultimo Scan System e azzera il numero dei Task fatti come ogni scan. Un merge dimenticato e ogni rifiuto arrivano alla Board come avvisi.

      ## Test

      Da `requirements/quality#solo-merge`, con repository git temporanei e git vero, un branch locale e uno remoto: ogni motivo di disponibilità nel suo ordine, file di Weaver esclusi in entrambi i modi, promemoria dei file tracciati; niente da unire; avanzamento rapido e commit di merge conclusi subito; conflitti → `Merging` con il conteggio; rifiuto di git per modifiche locali; rifiuto per file tenuti fuori da git; limite di tempo (iniettato) che annulla il merge; controllo: ancora in corso, committato fuori da Weaver (anche con commit sopra), annullato fuori, altro branch, reset, `merge.json` illeggibile, crash dopo la scrittura di `merge.json`; annullamento e suo rifiuto con il job `merge` non concluso; Solo mode rifiutato durante il merge; file del merge per avanzamento rapido, commit di merge e conflitti risolti, con progetto alla radice del repository e in una sottocartella; scan dopo il merge avviato senza conferma, con `merged` nel record e in `job show`, conteggio azzerato.

      Punti nel codice: runner git di Solo mode `crates/core/src/solo_mode.rs` (`Git::run`, `locate`, `inspect`, `tracked_files`); runner a tempo `crates/core/src/process/timed.rs` `run_command`; `ProjectStatus`/`SoloStatus`, `set_solo_mode`, `follow_solo`, `ProjectHooks::window_opened`, `batch_applied` e i comandi Tauri in `src-tauri/src/project_setup.rs`; focus della finestra in `src-tauri/src/windows/mod.rs` (`WindowEvent::Focused(true)` → `manager.on_focused`); cartelle locali in `crates/core/src/project/layout.rs` (`SOLO_DIR`, `LOCAL_ONLY_DIRS`, `ensure_local_folder`); avvio di Scan System `src-tauri/src/jobs/commands.rs` `scan_system` → `src-tauri/src/jobs/starts.rs` `start_scan`; job non conclusi `src-tauri/src/jobs/registry.rs` (`facts_of`, `active_ids`); permessi dei comandi `src-tauri/build.rs`, `src-tauri/src/commands.rs` (`CommandName`), `ui/src/lib/backend.ts`.
  adimalgy:
    title: Solo mode durante un merge
    refs:
    - solo-mode
    - requirements/ui#solo-mode
    technical: false
    proposal: 90o0ulxy
    label: Solo merge
    finished: 2026-11-05T14:40:40.452Z
    body: |-
      Quando il lavoro è finito, mentre un merge è in corso (cioè finché esiste `.weaver/merge/merge.json`, vedi `solo-merge`) Solo mode non si può cambiare in nessun modo: nello stato `On`, `developer.CHANGE_SOLO_MODE` resta in `On` sia quando lo sviluppatore lo spegne sia quando sceglie l'altro modo; non cambia nulla (né il blocco nelle esclusioni né il modo ricordato) e Settings dice di finire o annullare prima il merge. Le transizioni `On` → `Off` e `On` → `Excluded` valgono quindi solo quando nessun merge è in corso. Lo stato `On` ora menziona che la Board offre `Merge`.

      Decisione dell'intervista: durante un merge in corso il controllo di Solo mode è disabilitato del tutto, anche per il cambio di modo, non solo per lo spegnimento.

      Cosa deve valere:

      - **Backend:** `set_solo_mode` rifiuta la richiesta quando `merge.json` esiste, sia per `off` sia per l'altro modo, con il motivo `Finish or abort the merge in progress first.` (`requirements/tech#solo-merge`, **Solo mode**); il controllo avviene prima di qualunque chiamata a git o scrittura delle esclusioni, così nulla cambia.
      - **Settings:** mentre un merge è in corso il controllo `Solo mode` è disabilitato, con `Finish or abort the merge in progress first.` sotto la nota `Applies only to this working copy: other clones start with it off.` (`requirements/ui#solo-mode`, **During a merge**). Come ogni motivo di disabilitazione, il testo è la descrizione del controllo per le tecnologie assistive (`aria-describedby`, `requirements/ui#solo-mode`, **Accessibility**). L'informazione arriva dal `ProjectStatus` (campo `merge.inProgress`, presente mentre `merge.json` esiste).
      - **Riga di comando:** `project edit {solo}` riceve lo stesso rifiuto con lo stesso motivo quando un merge è in corso, e nulla cambia.
      - **Test:** un test Vitest del controllo disabilitato con il testo e la descrizione accessibile durante un merge; un test del backend/core che verifica il rifiuto di `off` e dell'altro modo con `merge.json` presente, lasciando invariate le esclusioni.

      Fuori da questa Task: il motore del merge (passi git, `merge.json`, comandi `merge_*`, campo `merge` di `ProjectStatus`, banner e job) è un'altra Task della Proposal; qui si legge solo se un merge è in corso.

      Punti nel codice: `src-tauri/src/project_setup.rs` (`set_solo_mode` a :591 e :890, `ProjectStatus`/`SoloStatus` a :114-165); `crates/core/src/solo_mode.rs` (`set_mode`); `ui/src/components/project/solo-mode.tsx` (`SoloModeSetting`, `SoloModeField`); `crates/cli/src/commands/project.rs` (`project edit {solo}`).
  dp2dsr5i:
    title: Testi dell'agente per il merge
    refs:
    - requirements/rules#project
    technical: true
    proposal: 90o0ulxy
    label: Solo merge
    finished: 2026-11-05T14:40:40.625Z
    body: |-
      Quando il lavoro è finito, i testi che Weaver installa per l'agente coprono il merge di Solo mode (`requirements/rules#project`, punto **Merge texts**).

      ## Nuova skill `weaver-resolve-conflicts`
      - Si aggiunge alla lista unica dei file installati: incorporata nel binario condiviso da app e `weaver`, installata in `.agents/skills/weaver-resolve-conflicts/SKILL.md` da Update skills (le copie installate non si scrivono a mano). La lista delle skill in `requirements/rules#project` diventa: `weaver-setup`, `weaver-design`, `weaver-import`, `weaver-scan`, `weaver-compare`, `weaver-implement`, `weaver-resolve-drifts`, `weaver-resolve-conflicts`.
      - È la skill del job `Merge conflicts` (tipo `merge`, modello Analysis), la cui istruzione è ``Use the weaver-resolve-conflicts skill on the merge in progress recorded in `.weaver/merge/merge.json`.`` (`requirements/rules#job`).
      - Il testo, in inglese con fine riga LF, dice all'agente di: leggere `.weaver/merge/merge.json`; trovare con git i file in conflitto; risolvere quelli ovvi; quando la scelta fra le due versioni non è ovvia, chiedere allo sviluppatore con un solo round `weaver question new`, ogni domanda con il nome del file e le due versioni; segnare ogni file risolto con `git add`; non eseguire mai `git commit` né `git merge --abort`, né altro che concluda o perda il merge (`git reset`, `git checkout` di un branch, `git stash`). Se a fine job restano conflitti, il merge resta in corso. Contiene anche le frasi abituali delle altre skill su sub-agenti, lingua e round di domande.

      ## Scan dopo un merge
      - `weaver-scan`, `weaver-compare` e il sub-agente `.omp/agents/weaver-analysis.md` dicono che, quando l'istruzione indica che lo scan segue un merge (``Use the weaver-scan skill on the whole System after a merge: read the files the merge brought with `weaver job show`.``), l'agente legge i file portati dal merge con `weaver job show`, li passa a ogni sub-agente e crea con `"merge": true` ogni Drift la cui differenza sta in quei file, e nessun altro. Solo questo scan può segnare `merge` (`requirements/rules#drift`); le regole di Drift ripetuti e scartati restano uguali.
      - `weaver-analysis.md` resta sotto i 6 KB.

      ## Risoluzione di un Drift da merge
      - `weaver-resolve-drifts` dice che la domanda preparata di un Drift da merge ha sempre l'opzione `proposal` (tenere il codice e adattare il Modello), scritta con la sua Draft allegata, e la raccomanda; le eventuali opzioni di correzione restano, nessuna preselezionata (`requirements/rules#drift`).

      ## Riferimento comandi
      - `weaver-design/COMMANDS.md` è rigenerato dal registro della CLI, così riflette le opzioni nuove (per esempio `merge` di `drift new`).

      Punti nel codice: `crates/core/src/agent_files.rs:SKILL_NAMES` (oggi `[&str; 7]`, da portare a 8; `crates/core/src/job_instruction.rs` sceglie le skill per indice, quindi aggiungere in coda o aggiornare gli indici) e i suoi test (lista attesa, riferimenti a `../weaver-design/COMMANDS.md`); sorgenti in `crates/core/assets/skills/<name>/SKILL.md` (`weaver-scan`, `weaver-compare`, `weaver-resolve-drifts`, nuova cartella `weaver-resolve-conflicts`); `crates/core/assets/omp/agents/weaver-analysis.md`; `crates/core/assets/skills/weaver-design/COMMANDS.md`, generato da `weaver_cli::registry::command_reference()`.
  du4s7bqm:
    title: Job del merge
    refs:
    - job
    - developer.RESOLVE_MERGE_CONFLICTS
    - work
    - requirements/rules#job
    - requirements/data#job
    - requirements/cli#job
    - requirements/ui#job
    technical: false
    proposal: 90o0ulxy
    label: Solo merge
    finished: 2026-11-05T14:33:45.441Z
    body: |-
      Quando il lavoro è finito, i job di Weaver sanno lavorare con un merge in corso di Solo mode (`solo-merge`): c'è un nuovo tipo di job per i conflitti, lo scan dopo un merge è un normale scan con i file portati dal merge, e l'ammissione dei job tiene conto del merge in corso.

      **Nuovo tipo `merge`.** `JobKind` ha il tipo `merge` (in `ALL`, `as_str` = `merge`, binding TS `ui/src/bindings/JobKind.ts` aggiornato). Il job si chiama sempre `Merge conflicts` (`requirements/ui#job`, **Fixed names**) e nella riga dei Jobs il tipo in testo secondario è `merge`. Usa il ruolo del modello Analysis delle Settings (`role()`), senza `--slow` e senza `--advisor`, con l'ambiente di ogni job (`weaver_env`). Non è fra i tipi di `job new` né di `agent.START_JOB`: si avvia solo con `developer.RESOLVE_MERGE_CONFLICTS`, cioè dall'avviso del merge in corso (`requirements/cli#job`). Nella registry il `kind` è `merge` (`requirements/data#job`).

      **Istruzione.** La prima frase del job `merge` è ``Use the weaver-resolve-conflicts skill on the merge in progress recorded in `.weaver/merge/merge.json`.``, seguita dalle frasi comuni come per ogni job (`requirements/rules#job`). L'agente risolve i conflitti, chiede allo sviluppatore con un Question round quando la scelta fra le due versioni non è ovvia, segna in git i file risolti, non fa mai il commit e non annulla mai il merge; se il job finisce e restano conflitti, il merge resta in corso. Questo comportamento sta nella skill `weaver-resolve-conflicts`, che è un altro Task.

      **Scan dopo un merge.** Su `solo-merge.CONCLUDED` Weaver avvia da solo, senza conferma, uno scan sul System intero: tipo `scan`, nome `Scan`, avviato dallo stesso percorso di Scan System (`start_scan`). La prima frase è ``Use the weaver-scan skill on the whole System after a merge: read the files the merge brought with `weaver job show`.`` Il record di registry ha `merged`: i file portati dal merge (diversi fra il commit di partenza e il commit del merge), percorsi relativi con `/`, ordinati, solo per questo scan. `weaver job show` senza id lo mostra come `merged`. Lo scan conta come l'ultimo Scan System e azzera il numero dei Task fatti, come ogni scan (`work`: la scan è chiesta dallo sviluppatore o avviata da Weaver a merge concluso). Il calcolo dei file e il rilevamento della conclusione appartengono al motore del merge, che è un altro Task: qui serve il modo di avviare lo scan con `merged`.

      **Ammissione.** Mentre un merge è in corso (esiste `.weaver/merge/merge.json`) sono rifiutati, con il motivo `A merge is in progress`: Run di un Task, Run all, resolve (anche quelli di `developer.RESOLVE_DRIFT` e di `question-round.ANSWERED`), scan e import. Design, rework, refinement e il job `merge` restano possibili. Il job `merge` è rifiutato quando non c'è un merge in corso con file ancora in conflitto, o quando un altro job `merge` del progetto non è finito. `agent.START_JOB` ha gli stessi rifiuti di quando chiede lo sviluppatore. Il fatto "merge in corso" (e "conflitti presenti") entra in `AgentFacts` per `check_start`.

      Punti nel codice: `crates/core/src/jobs/kinds.rs:JobKind` (`ALL`, `as_str`, `role()`, `uses_analysis_agents()`, `is_run()`), `ui/src/bindings/JobKind.ts`; `crates/core/src/job_instruction.rs` (`enum Job`, `compose`, `first_sentence`); `crates/core/src/jobs/admission.rs:check_start`, `StartPlan`; `crates/core/src/project/board.rs:AgentFacts`; `src-tauri/src/jobs/starts.rs` (`Jobs::start`, `start_scan`, `start_resolve`); `src-tauri/src/jobs/registry.rs:create` (azzeramento del conteggio quando parte uno scan); `crates/core/src/jobs/launch.rs` (`weaver_env`, `args`, `role_model`), `crates/core/src/jobs/names.rs`; `src-tauri/src/jobs/authority.rs:start_jobs`; `crates/cli/src/commands/job.rs` (`job new`, `job show`).
  j7ksa61c:
    title: Merge sulla Board
    refs:
    - developer.MERGE_BRANCH
    - developer.ABORT_MERGE
    - requirements/ui#solo-merge
    - requirements/ui#work
    - requirements/ui#task
    - requirements/docs#solo-merge
    technical: false
    proposal: 90o0ulxy
    label: Solo merge
    finished: 2026-11-05T14:33:45.624Z
    body: |-
      Quando è fatto, il developer può avviare dalla Board il merge di un altro branch nel branch corrente (`developer.MERGE_BRANCH`) e annullare un merge in corso (`developer.ABORT_MERGE`), con tutta l'interfaccia di `requirements/ui#solo-merge`. L'interfaccia usa i comandi del backend e il campo `ProjectStatus.merge` realizzati da un altro Task di questa Proposal: `merge_refresh` (chiamato quando la Board viene mostrata), `merge_branches`, `merge_start`, `merge_abort` e `resolve_merge_conflicts`; questo Task aggiunge i loro binding in `ui/src/lib/backend.ts` se mancano e non ricalcola nulla lato interfaccia (disponibilità, motivi, conteggio dei conflitti arrivano dal backend). Il merge esiste solo nell'app, non nella riga di comando.

      ## Pulsante `Merge`
      - `outline` Button con icona `GitMerge`, nell'intestazione della Board a sinistra di `Scan System`, presente solo con Solo mode acceso; tooltip `Merge another branch into <current>`.
      - Disabilitato con il primo motivo che vale, nell'ordine: `The agent is working` (job dell'agente non concluso: in coda, al lavoro, in attesa o sospeso), `A merge is in progress`, `Merge needs git: <reason>`, `Git is in the middle of a merge, rebase or cherry-pick`, `Not on a branch`, `Commit or stash your changes first` (modifiche non committate ai file tracciati, esclusi quelli di Weaver). Il motivo è il tooltip e la descrizione accessibile del pulsante disabilitato.
      - La disponibilità si aggiorna quando la Board viene mostrata, quando la finestra torna in primo piano, quando un job finisce e dopo ogni azione di merge; mai a intervalli.

      ## Dialog
      - Titolo `Merge a branch`; campo `Search branches` che filtra una lista `Command` con i branch locali e poi i remoti già noti a git (nessun `git fetch`), sotto le intestazioni `Local` e `Remote`; frecce per muoversi, Enter per scegliere. Senza altri branch la lista mostra `No other branch`.
      - Scelto il branch: `Merge <branch> into <current>?` e, se git traccia ancora file di Weaver, un `Alert` (variante warning) `Git still tracks <N> of Weaver's files: their changes go into the merge commit.` (Merge resta comunque disponibile); pulsanti `Merge` (primario) / `Cancel`.
      - Mentre git lavora `Merge` mostra uno spinner e entrambi i pulsanti sono disabilitati; il Dialog si chiude quando git risponde.

      ## Avvisi
      In cima alla Board (`requirements/ui#work`, **Details dialogs and notices**), ognuno con pulsante di chiusura:
      - `<current> is already up to date with <branch>.` (il branch non porta nulla, non si avvia nulla);
      - `Could not merge <branch>: <first line of git's error>.` (nulla cambia);
      - `Could not merge <branch>: it would overwrite files that Solo mode keeps out of git.` seguito dai percorsi in monospace;
      - `Could not merge <branch>: git did not finish the merge within 2 minutes.` (Weaver ha interrotto e annullato il merge);
      - `The merge of <branch> ended without a commit: Weaver forgot it and did not scan.` (merge sparito fuori da Weaver);
      - `Could not abort the merge: <first line of git's error>.`

      ## Banner del merge
      - Con un merge in corso, un `Alert` sopra le due colonne e sopra gli avvisi, icona `GitMerge`, senza chiusura: `Merging <branch> into <current>: <n> files with conflicts. Resolve them here or by hand, then commit the merge in git.`, oppure, senza conflitti rimasti, `Merging <branch> into <current>: no conflicts left. Commit the merge in git to finish.`
      - Azioni a destra: `Resolve with the agent` (primario, icona `Sparkles`, solo finché qualche file ha conflitti; disabilitato con `The agent is resolving the conflicts` finché il job `Merge conflicts` non è concluso), che chiama `resolve_merge_conflicts`; `Abort merge` (outline, testo destructive), disabilitato con lo stesso motivo.
      - `Abort merge` chiede conferma: `Abort the merge of <branch>? Every change of the merge, conflict resolutions included, is lost.`, `Abort merge` (destructive) / `Cancel`. Confermato, Weaver annulla il merge in git, lo dimentica e non avvia scan.
      - Weaver non fa mai il commit del merge: il developer lo fa in git.

      ## Altri controlli durante il merge
      - `Run all` (`requirements/ui#work`): primo motivo `A merge is in progress`, prima di `Drifts are being resolved`.
      - `Scan System`: disabilitato con `A merge is in progress`.
      - `Run` di un Task (`requirements/ui#task`): `A merge is in progress` dopo `The agent is still finishing` e prima di `Drifts are being resolved`.
      - Il conteggio `· <n> done since last scan` conta i Task finiti (non chiusi senza modifiche) dall'avvio dell'ultimo scan dell'intero System, chiesto dal developer o avviato dopo un merge, compresi quelli cancellati e dimenticati; nascosto se 0.

      ## Accessibilità
      Il banner è una `region` chiamata `Merge in progress`; un cambio del suo testo è annunciato (`aria-live="polite"`); i motivi di disabilitazione sono la descrizione del loro controllo (`aria-describedby`).

      ## Test e documentazione
      - Test Vitest: pulsante presente solo con Solo mode acceso e i suoi motivi, Dialog (filtro, intestazioni, `No other branch`, conferma con e senza file tracciati, spinner), avvisi, banner nei due testi e con le azioni disabilitate, conferma di annullamento, `A merge is in progress` su `Run all`, `Scan System` e `Run`.
      - `README.md`: dentro `## Solo mode` una sottosezione `### Merge` (`requirements/docs#solo-merge`): a cosa serve (portare il lavoro degli altri nel branch corrente e trovare con uno scan cosa ha cambiato rispetto al Modello); dove sta (la Board, solo con Solo mode acceso) e quando è disponibile; che Weaver esegue `git merge` ma non fa mai fetch e non fa mai il commit di un merge con conflitti (il developer li risolve a mano o con l'agente, poi fa il commit); che lo scan dopo il merge parte da solo e i suoi Drift dal merge raccomandano di adattare il Modello.
      - `CHANGELOG.md`: sotto `## [Unreleased]`, `Added`, una riga che descrive Merge in Solo mode.

      Punti nel codice: intestazione in `ui/src/components/board/board-view.tsx` (`ViewHeader view="board"`, `Scan System`, conteggio; `BoardFooterButtons` per `Run all`, i cui motivi arrivano da `snapshot.runAllReasons`); i motivi di `Run all` e di `Run` sono calcolati nel core, in `crates/core/src/project/board.rs` (costanti `REASON_*`) e `crates/core/src/project/tasks.rs:run_reasons`, dove va aggiunto `A merge is in progress`; `ui/src/components/board/board-actions.tsx`, `NoticeStack` in `ui/src/components/notices`; banner accanto a `OpeningCheckToast`/`SkillsBanner` in `ui/src/components/project-content.tsx`; binding in `ui/src/lib/backend.ts` (vicino a `set_solo_mode`, `solo_tracked_files`); test del conteggio in `ui/src/components/board/board-view.test.tsx`; `README.md` `## Solo mode`, `CHANGELOG.md` `## [Unreleased]`.
  r1e7delh:
    title: Drift da merge
    refs:
    - drift
    - requirements/rules#drift
    - requirements/data#drift
    - requirements/cli#drift
    - requirements/ui#drift
    technical: false
    proposal: 90o0ulxy
    label: Solo merge
    finished: 2026-11-05T14:33:45.808Z
    body: |-
      Un Drift registrato dallo scan che segue un merge, la cui differenza sta nei file portati dal merge, viene da un merge. È l'agente a deciderlo guardando quei file; viene registrato quando il Drift è creato e non cambia mai più; nessun altro job può registrarlo. Quando il lavoro è finito deve valere quanto segue.

      **File e comando `drift new`.** Il front matter del Drift ha la nuova chiave opzionale `merge`, solo `true`, scritta unicamente da `drift new` e mai cambiata da nessun comando (`requirements/data#drift`; va aggiunta a `DRIFT_KEYS` nella posizione della tabella, prima di `created`). `drift new` accetta il nuovo campo `merge?` (solo `true`) solo dall'agente di un job di scan avviato dopo un merge, riconosciuto tramite `WEAVER_JOB` e il campo `merged` del record del job; da qualsiasi altro chiamante (fuori da un job, altro tipo di job, scan non dopo un merge) è rifiutato con exit 1 e nulla viene scritto (`requirements/rules#drift`, `requirements/cli#drift`). `weaver drift show` e `weaver drift list` stampano `merge`. Le regole dei Drift scartati (fingerprint) e di `previous` valgono per i Drift da merge senza alcuna differenza.

      **Domanda preparata.** La domanda preparata di un Drift da merge (`weaver drift edit <id> {prepared}`) deve sempre contenere la decisione `proposal` per il comportamento attuale del codice, con una Proposal allegata in Draft o Open, e `recommended` deve essere quell'opzione, con o senza opzioni di correzione; una preparazione senza di essa, o che raccomanda un'altra opzione, è rifiutata indicando il motivo. Le opzioni di correzione mantengono il loro ordine, la prima resta elencata per prima. Anche la validità di una domanda già preparata segue questa regola.

      **Risoluzione.** `developer.RESOLVE_DRIFT` è rifiutato, per ogni Drift (anche con una domanda preparata valida), mentre un merge è in corso; sulla Board `Resolve` è disabilitato con il motivo `A merge is in progress`, terzo nell'ordine dei motivi, dopo `A run is in progress` (`requirements/ui#drift`). Il merge in corso è quello ricordato in `.weaver/merge/merge.json`.

      **Board.** La card e il dialog dei dettagli mostrano il badge `merge` (outline) con il tooltip `Found by the scan after a merge`, dopo `recurring`, `resolving` e `prepared`. Nei dettagli di un Drift da merge `recommended` marca `Accept Proposal` invece della prima opzione di correzione; le opzioni restano numerate senza il segno e nessuna è preselezionata, quindi `Fix the code` resta disabilitato finché non se ne sceglie una. La vista del Drift deve portare l'informazione `merge` al frontend; aggiornare le fixture.

      Fuori da questo Task: i testi delle skill (lo scan che decide `merge`, la preparazione della domanda) e l'ammissione dei job durante un merge in corso, che sono altri Task di questa Proposal.

      Punti nel codice: `crates/core/src/project/drift/mod.rs` (`DRIFT_KEYS`); `crates/core/src/project/drift/create.rs` (`NewDrift`, `plan_new`); `crates/core/src/project/drift/prepared.rs` (`shape_problems`, `validate_input`) e `answer.rs`; `crates/cli/src/commands/drift.rs` (spec di `new` e `show`, conversione dell'input in `NewDrift`, `drift_json`); `src-tauri/src/drift_view.rs` (`DriftView`); `ui/src/components/board/drift-card.tsx` (`DriftBadges`, `DriftDetails`) e `drift-fixtures.ts`.
---
# Merge in Solo mode

Con Solo mode acceso il lavoro degli altri entra nella copia di lavoro tramite un merge eseguito da Weaver con git, senza mai fare fetch né il commit di un merge con conflitti.

- Motore: modulo `solo_merge` in `weaver-core` con disponibilità e motivi in ordine, scelta fra branch locali e remoti, avvio con limite di tempo, memoria del merge in `.weaver/merge/merge.json` (solo locale), controllo a ogni apertura, ritorno in primo piano e fine di un job, annullamento e file portati dal merge; comandi Tauri e stato `merge` in `ProjectStatus`.
- Solo mode: durante un merge non si può spegnere né cambiare modo, in Settings e da riga di comando.
- Job: nuovo tipo `merge` (`Merge conflicts`) con la skill `weaver-resolve-conflicts`; a merge concluso parte da solo uno scan sul System intero che legge i file portati con `weaver job show`; durante il merge Run, Run all, resolve, scan e import sono rifiutati.
- Board: pulsante `Merge` con Dialog di conferma, avviso del merge in corso con conteggio dei conflitti, annullamento e avvisi.
- Drift da merge: campo `merge` scritto solo dallo scan dopo un merge, badge `merge`, domanda preparata che raccomanda sempre di tenere il codice e adattare il Modello.
- Testi dell'agente: skill `weaver-resolve-conflicts`, regole di scan, compare e `weaver-analysis`, riferimento comandi rigenerato.
