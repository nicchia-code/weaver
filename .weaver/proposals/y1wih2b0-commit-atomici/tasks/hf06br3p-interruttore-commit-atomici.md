---
technical: false
refs:
  - atomic-commits
  - developer.CHANGE_ATOMIC_COMMITS
  - requirements/data#atomic-commits
  - requirements/data#General
  - requirements/ui#atomic-commits
  - requirements/ui#developer.CHANGE_SETTINGS
  - requirements/tech#atomic-commits
---

# Interruttore Atomic commits

## Obiettivo

Ogni copia di lavoro di un progetto ha un interruttore Atomic commits, spento per impostazione predefinita, che il developer accende e spegne solo in Settings → `Project`, senza conferma, con effetto immediato. La scelta vive solo nella copia di lavoro, come Solo mode: mai nel repository e mai nelle impostazioni di Weaver, quindi un altro clone dello stesso repository parte con l'interruttore spento.

Questa Task riguarda solo l'interruttore (stato, transizioni, comando dell'app, controllo in Settings). Il commit vero e proprio dopo ogni voce di History, il controllo dei percorsi, la coda dei commit e l'avviso sulla Board sono realizzati da un'altra Task della stessa Proposal, su `history-entry`; qui basta che il loro codice possa leggere lo stato dell'interruttore dal modulo di `weaver-core` descritto sotto.

## File di stato

- Lo stato è `.weaver/commits/commits.json` nella copia di lavoro, contenuto `{"enabled": true}`, UTF-8, LF con newline finale, scritto in modo atomico come ogni file di progetto (file temporaneo `.<name>.<random>.tmp` nella stessa cartella, flush, rename; su Windows retry ogni 50 ms per al massimo 1 s).
- Atomic commits non è un'impostazione: non compare mai in `settings.json` né nello schema delle impostazioni (`crates/core/src/settings/`).
- Nessun file = spento. Spegnere cancella l'intera cartella `commits/`.
- Un file che non si riesce a leggere o che contiene un altro valore conta come spento ed è segnalato come diagnostica con percorso e motivo (nella finestra del progetto e nell'output del comando CLI che lo incontra); non viene mai riscritto automaticamente, solo al prossimo cambio di Atomic commits.
- Nulla dei commit fatti da Weaver viene memorizzato: né nella voce di History, né nel registro dei job, né in alcuno stato locale (nessun hash).
- Layout (`requirements/data#General`): `commits/commits.json` è una voce local-only del layout di `.weaver/`; `commits/` si aggiunge alle cartelle local-only (`requests/`, `short-numbers/`, `task-images/`, `notes/`, `solo/`, `merge/`, `commits/`), ognuna con il proprio `.gitignore` che contiene `*`, scritto quando la cartella viene creata e riscritto ogni volta che manca. In codice: aggiungere una costante accanto a `SOLO_DIR`/`MERGE_DIR` e includerla in `LOCAL_ONLY_DIRS` in `crates/core/src/project/layout.rs`, con i test relativi.
- Un solo modulo di `weaver-core`, accanto a quelli di Solo mode (`crates/core/src/solo_mode.rs`) e Merge (`crates/core/src/solo_merge.rs`), legge e scrive `commits/commits.json`; la UI non esegue mai git.

## Stati e transizioni

- **Off (iniziale):** Weaver non fa commit per questa copia di lavoro e non ricorda nulla. Fuori da un repository git, o quando git non si trova, non si può accendere e Settings dice perché.
  - `developer.CHANGE_ATOMIC_COMMITS` → On: quando il developer lo accende in un progetto la cui cartella è in un repository git; Weaver scrive il file di stato.
  - `developer.CHANGE_ATOMIC_COMMITS` → Off: quando la cartella non è in un repository git, git non si trova o la scelta non si può memorizzare; nulla cambia e Settings dice perché. I motivi sono quelli di Solo mode (**Not available**): `git was not found`, `this folder is not in a git repository`, `git did not answer within 10 s`, `git failed: <first line of its error output>`; una scrittura fallita dà `could not write <path>: <system error>`. Il controllo git usa il runner di Solo mode (`Git` in `crates/core/src/solo_mode.rs`: nessuna shell, cwd la radice del progetto, `GIT_OPTIONAL_LOCKS=0` per le letture, 10 s per chiamata), mai sul thread della UI o dell'IPC.
- **On:** ogni voce di History scritta dall'agente, e ogni aggiornamento che le aggiunge Task, è seguito da un commit del suo codice (realizzato dall'altra Task).
  - `developer.CHANGE_ATOMIC_COMMITS` → Off: quando il developer lo spegne; Weaver cancella la cartella `commits/`. Spegnere non richiede git. I commit già fatti restano.
  - `developer.CHANGE_ATOMIC_COMMITS` → On: quando la scelta non si può cancellare; nulla cambia e Settings dice perché.
  - `workspace.PROJECT_FOLDER_REMOVED` → Off: lo stato se n'è andato con la cartella `.weaver`; non si fa nient'altro.

## App (Tauri)

- Comando Tauri `set_atomic_commits {projectId, enabled}`, eseguito nella coda di mutazioni del progetto sotto il lock del progetto, che restituisce il nuovo stato o l'errore; va affiancato a `set_solo_mode` in `src-tauri/src/project_setup.rs`. Non esiste alcun comando né campo della riga di comando per cambiarlo, e non compare in Get Started.
- `ProjectStatus` porta `atomicCommits`: `enabled` e `unavailable` (il motivo per cui non si può accendere, assente quando si può), inviato con `project-state` come `solo`; `unavailable` è calcolato all'apertura del progetto e ogni volta che viene mostrata la vista Settings.

## Settings

- Ordine del gruppo `Project`: `Solo mode`, poi `Atomic commits`, poi `Update skills` (gruppi di Settings in `ui/src/components/settings-view.tsx`).
- **Controllo:** titolo `Atomic commits`, uno `Switch` shadcn come `System Certificates` (`SystemCertificatesSetting`), spento per impostazione predefinita, con sotto la nota `Applies only to this working copy: after each History entry, Weaver commits the code the agent wrote for it.` Non è nello schema delle impostazioni: mostra lo stato del progetto e invia i cambi al backend.
- **Salvataggio:** subito al cambio, senza conferma. Mentre un cambio è in corso lo switch è disabilitato. Un cambio fallito mostra `Could not change Atomic commits: <reason>.` sotto lo switch nel colore di errore e rimette il valore del progetto.
- **Non inizializzato:** disabilitato e spento, con `Initialise the project to use Atomic commits.` sotto la nota.
- **Senza git:** se spento, disabilitato con `Atomic commits needs git: <reason>.` sotto la nota, con i motivi elencati sopra. Se acceso resta usabile, così che si possa spegnere.
- **Accessibilità:** la nota e il motivo per cui lo switch è disabilitato sono la sua descrizione per le tecnologie assistive (`aria-describedby`).

## Fatto quando

Lo switch compare nel gruppo `Project` tra `Solo mode` e `Update skills` con i testi sopra; accenderlo in un repository git crea `.weaver/commits/commits.json` e `.weaver/commits/.gitignore`, spegnerlo cancella `commits/`; fuori da git o senza git l'accensione è rifiutata con il motivo; un file illeggibile conta come spento con diagnostica; `ProjectStatus.atomicCommits` riflette lo stato in ogni finestra del progetto.
