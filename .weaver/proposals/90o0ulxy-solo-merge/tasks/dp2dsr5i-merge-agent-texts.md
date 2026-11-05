---
technical: true
refs:
  - requirements/rules#project
---
# Testi dell'agente per il merge

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
