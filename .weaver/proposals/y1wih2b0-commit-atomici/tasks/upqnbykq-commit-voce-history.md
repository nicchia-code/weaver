---
technical: false
refs:
  - history-entry
  - requirements/cli#history-entry
  - requirements/rules#atomic-commits
  - requirements/rules#project
  - requirements/ui#work
  - requirements/quality#atomic-commits
  - requirements/quality#solo-mode
  - requirements/quality#solo-merge
---
# Commit di ogni voce History

Quando l'agente scrive una voce di History, o aggiunge Task a una voce esistente, e Atomic commits è attivo nella copia di lavoro, Weaver fa un commit git con il codice di quella funzionalità. La voce viene scritta in ogni caso: un commit fallito o saltato non annulla mai la voce. History non mostra mai i commit e nessun hash viene salvato.

L'interruttore `Atomic commits` e il suo file di stato sono costruiti da un altro Task della stessa Proposal (su `atomic-commits`). Questo Task legge solo lo stato: Atomic commits è attivo se e solo se `.weaver/commits/commits.json` contiene `{"enabled": true}`; file assente o illeggibile vale spento.

## Input `commit`

- `history new {title, summary, body?, tasks, commit}`: `commit` è obbligatorio.
- `history edit <id> {title?, summary?, body?, tasks?, commit?}`: `commit` è obbligatorio quando `tasks` aggiunge almeno un Task che la voce non citava, ignorato altrimenti. Una modifica che aggiunge Task fa un commit nuovo (mai `--amend`) e non tocca il commit precedente; una modifica che non aggiunge Task non fa commit.
- `commit` è `{files, message, checksFailed?}`, dato sempre, che Atomic commits sia attivo o no:
  - `files`: percorsi del codice della funzionalità della voce, relativi alla radice del progetto con `/`, senza duplicati; può essere vuoto.
  - `message`: messaggio di commit in inglese, prima riga non vuota di al più 72 caratteri, opzionalmente seguita da una riga vuota e da un corpo.
  - `checksFailed`: `true` quando i controlli finali del Run sono falliti; default `false`.
- La forma di `commit` è sempre controllata insieme agli altri campi. Con Atomic commits spento, `commit` è controllato solo nella forma, non parte nessun git e `status` è `off`.

## Controllo dei percorsi (solo con Atomic commits attivo)

Prima di scrivere qualsiasi cosa, ogni percorso non valido è elencato come problema `files[<i>]: <path> <problem>`; con almeno un problema l'intero comando è rifiutato, tutti i problemi elencati, niente scritto:
- assoluto, vuoto, o che esce dalla radice del progetto una volta risolti `.` e `..`: `is outside the project`;
- ignorato dalle regole di git e non tracciato (`git check-ignore -z --stdin`, senza `--no-index`): `is ignored by git`;
- assente dall'albero di lavoro e non tracciato (`git ls-files -z --cached -- <paths>`): `does not exist and git does not track it`. Un file tracciato e assente è valido: la sua rimozione entra nel commit.
- Con Solo mode attivo, un percorso che una riga del blocco Solo mode di questa copia di lavoro corrisponde viene lasciato fuori dal commit senza problema.

## Ordine e coda

La voce è scritta e i file dei Task registrati sono rimossi come senza Atomic commits; poi, rilasciato il lock del progetto, il commit gira in una coda della copia di lavoro, un commit alla volta, prima che il comando risponda. Dai requisiti tecnici (`requirements/tech/solo-mode.md#atomic-commits`), da rispettare:
- "**Where:** one module of `weaver-core`, beside Solo mode's and Merge's, reads and writes `commits/commits.json` (`requirements/data#atomic-commits`), checks the paths and makes the commit (`requirements/rules#atomic-commits`). It is called by the core function that writes History entries, so the app (through `AppAuthority`) and the command line running alone behave the same; the UI never runs git."
- "**git:** the runner of `requirements/tech#solo-mode` (**git**): no shell, cwd the project root, `GIT_OPTIONAL_LOCKS=0` for the calls that only read, `GIT_EDITOR=true` and `GIT_TERMINAL_PROMPT=0` for `git add`, `git rm` and `git commit`, 10 s per call and 2 minutes for `git commit`. No git call runs on the UI or IPC thread."
- "**Queue:** one commit at a time per working copy, in a queue keyed by the canonical project root, outside the project lock and the project's mutation queue, so that a slow hook never blocks other writes; the command answers after its commit."
- "**Notices:** a History write by a job of the project whose `commit.status` is `not-committed` sends the Board a notice with the entry's title and the reason (`requirements/ui#atomic-commits`). A write made by the command line while the app is not running only prints the result."

## Nessun commit

Controllati in quest'ordine; il primo che vale è il `reason`:
1. `checksFailed` è `true`: `the checks of the Run failed`;
2. un motivo di **Not available** di Solo mode: `git was not found`, `this folder is not in a git repository`, `git did not answer within 10 s`, `git failed: <first line of its error output>`;
3. il merge di Weaver è in corso, o git è nel mezzo di un'operazione come in **Availability** di Merge: `a merge, a rebase or a cherry-pick is in progress`;
4. HEAD è staccato: `HEAD is detached`.

## Staging e commit

- `git add -A -- <paths>` per i percorsi validi nominati, così i file rimossi entrano come rimozioni. Fuori da Solo mode anche il file della voce in `.weaver/history/` e, come rimozioni, i file rimossi dei Task registrati che git traccia (`git rm -q --cached --ignore-unmatch -- <paths>`); mai un altro file di Weaver. Con Solo mode attivo (entrambe le modalità) nessun file di Weaver. Ciò che lo sviluppatore aveva già messo in stage resta in stage ed entra nel commit; Weaver non toglie mai dallo stage nulla.
- Niente da committare: se `git diff --cached --quiet` esce con 0 dopo lo staging, nessun commit, `status` `none`, nessun avviso.
- Commit: `git commit --cleanup=strip -F -`, messaggio su standard input. Fuori da Solo mode il messaggio è quello dell'agente, una riga vuota e il trailer `Co-authored-by: Weaver <weaver@users.noreply.github.com>`; con Solo mode attivo il solo messaggio dell'agente. Autore e committer dalla configurazione di git. Mai `--no-verify`, `--amend`, `-a`, `--allow-empty`, push o fetch: hook e firma girano come configurati.
- Limite: 2 minuti per `git commit` (per gli hook); se raggiunto git viene terminato e il motivo è `git did not finish the commit within 2 minutes`. Le altre chiamate hanno il limite di 10 s.
- Fallimento: un'uscita non zero di `git add`, `git rm` o `git commit` dà `git failed: <first line of its error output>`; ciò che Weaver ha messo in stage resta in stage.
- Hash: dopo un commit, `git rev-parse HEAD`, stampato per intero in `commit.hash`.

## Output

`history new` e `history edit` stampano `id`, `title`, `file`, `recorded` e `commit`: `{status, hash?, reason?}`, con `status` uguale a `off` (Atomic commits spento), `none` (niente da committare, o modifica senza Task aggiunti), `committed` (con `hash` completo) o `not-committed` (con `reason`). L'output testuale termina con `Committed <short hash>.` o `Not committed: <reason>.` e non dice nulla per `off` e `none`.

## Avviso sulla Board

Tra gli avvisi in cima alla Board (`requirements/ui#work`, **Details dialogs and notices**: in alto, sopra le due colonne, più recenti prima, ognuno un `Alert` con pulsante di chiusura, fino alla chiusura o alla chiusura della finestra), ora ci sono anche le voci di History che Weaver non ha committato. Da `requirements/ui/solo-mode.md#atomic-commits`: "**Notices** at the top of the Board (`requirements/ui#work`, **Details dialogs and notices**), each with a close button: `History entry "<title>" was not committed: <reason>.`, with the reasons of `requirements/rules#atomic-commits` (**No commit**, **Limit**, **Failure**)." Con `none` e `off` nessun avviso.

## Testi dell'agente

- La skill `weaver-implement` (`crates/core/assets/skills/weaver-implement/SKILL.md`, passo 7 "Write History") dice all'agente di passare `commit` con ogni `history new` e con ogni `history edit` che aggiunge Task, che Atomic commits sia attivo o no: `files`, i file del codice che l'agente e i suoi sub-agenti hanno creato, modificato o rimosso per i Task di quella voce e nessun altro; `message`, un messaggio di commit in inglese la cui prima riga dice in al più 72 caratteri cosa fa la funzionalità; `checksFailed: true` quando il controllo finale del Run è fallito (la voce si scrive anche in quel caso). Le dice di non eseguire mai `git add` o `git commit` da sé, e di elencare nel report ogni voce non committata con il motivo stampato dal comando.
- Il sub-agente `weaver-implementer` (`crates/core/assets/omp/agents/weaver-implementer.md`) riporta, per ogni Task che ha finito, i file del codice che ha creato, modificato o rimosso. Il file resta sotto 6 KB, testi in inglese, LF.

## Affidabilità

- Solo mode: Weaver non cambia mai una riga del file di esclusioni fuori dal proprio blocco di questa copia di lavoro, non tocca mai l'indice di git per Solo mode, non lascia mai un blocco dopo un'inizializzazione fallita.
- Merge: Weaver stesso non committa mai un merge con conflitti, non mette mai in stage un file per un merge (solo l'agente del job dei conflitti marca i file risolti), non esegue mai `git fetch`, non sovrascrive mai un file che Solo mode tiene fuori da git, non lascia mai `merge/merge.json` dopo un merge concluso.
- Atomic commits è l'unico punto in cui Weaver mette in stage file e committa: solo i percorsi dello staging sopra, mai togliere dallo stage ciò dello sviluppatore, mai amend, riscritture, push o fetch, mai committare un file che Solo mode tiene fuori da git salvo che lo sviluppatore l'abbia messo in stage. Due commit della stessa copia di lavoro non girano mai insieme. git gira solo quando si scrive una voce con Atomic commits attivo (oltre ai momenti dell'interruttore), mai sul thread UI o IPC, mai sotto il lock del progetto durante un commit.

## Test

Contro repository git temporanei con git reale:
- spento: `commit` controllato solo nella forma, nessun git avviato, `status` `off`;
- i percorsi: fuori dal progetto, ignorati, assenti e non tracciati rifiutati tutti insieme senza scrivere nulla; un file tracciato rimosso committato come rimozione; un progetto in una sottocartella;
- il commit di una voce nuova e di una modifica che aggiunge Task (un commit nuovo, il precedente invariato); una modifica senza Task aggiunti che non fa commit; file della voce e file dei Task rimossi nel commit fuori da Solo mode, nessun file di Weaver in entrambe le modalità di Solo mode; i file in stage dello sviluppatore inclusi;
- il messaggio con e senza trailer `Co-authored-by`, l'autore dalla configurazione di git;
- ogni motivo di nessun commit nel suo ordine, niente da committare che dà `none`, un hook pre-commit che fallisce, il limite di tempo (iniettato), la voce scritta in ogni caso;
- due voci scritte insieme da due job committate una dopo l'altra;
- l'avviso della Board in Vitest, con percorso da tastiera e nomi accessibili.

## Punti del codice

- `crates/cli/src/commands/history.rs`: `entry_fields`, `new`, `edit`, `write_planned` (campo `commit`, output).
- `crates/core/src/project/history.rs`: `plan_new`, `plan_edit`, `remove_task_folders`.
- `crates/core/src/solo_mode.rs`: runner `Git` (`Git::exec`, `GIT_LIMIT`), `read_state`, `Mode::lines`; `crates/core/src/solo_merge.rs`: `in_progress`.
- `src-tauri/src/jobs/authority.rs`: `AppAuthority` per l'avviso alla Board.
- `crates/core/src/agent_files.rs`: controllo delle parole nei testi installati.
