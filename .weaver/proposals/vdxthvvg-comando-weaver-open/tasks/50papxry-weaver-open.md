---
technical: false
refs:
  - window
  - developer.OPEN_PROJECT
  - developer.SHOW_WELCOME
  - requirements/cli#General
  - requirements/cli#developer.OPEN_PROJECT
  - requirements/cli#developer.SHOW_WELCOME
  - requirements/tech#developer.OPEN_PROJECT
  - requirements/tech#developer.SHOW_WELCOME
  - requirements/tech#window
---
# Comando weaver open

Dal proprio terminale lo sviluppatore apre una cartella in Weaver già in esecuzione con `weaver open <folder>` (`developer.OPEN_PROJECT`), oppure, con `weaver open` senza cartella, chiede di vedere la welcome window (`developer.SHOW_WELCOME`). È l'unica eccezione alla forma `weaver <object> <verb> …`: la forma è `weaver open [<folder>] [--json]`, senza verbo. `weaver` senza parole resta un errore d'uso con l'help (exit 2).

**Argomenti.** Al massimo una cartella: una seconda parola è errore d'uso (exit 2). `--root` e `--input` sono errore d'uso (exit 2, nominando l'opzione); `--json` è accettato. Dopo `open` ogni parola è una cartella, anche se ha il nome di un oggetto: `weaver open model` apre la cartella `model`. `open` non richiede un progetto inizializzato nella cartella corrente.

**Agenti.** Quando è impostata `WEAVER_JOB`, `WEAVER_GENERAL` o `WEAVER_ASK`, il comando è rifiutato (exit 1, code `refused`, nulla inviato) con `` `weaver open` is for the developer's terminal: an agent cannot open folders in Weaver. `` Anche l'app rifiuta, con la stessa ragione e senza aprire nulla, una richiesta `open` che porta un'identità di job o di Chat.

**Controlli della cartella, prima di contattare l'app.** Il percorso è unito alla cartella corrente e reso canonico come identità del progetto (symlink risolti, prefisso `\\?\` tolto su Windows). Percorso inesistente: exit 1, `` `<path>` does not exist. ``; non una cartella: exit 1, `` `<path>` is not a folder. `` In questi casi Weaver non viene contattato. Una cartella senza `.weaver/` è accettata: la sua finestra si apre su Get Started.

**Richiesta.** Il CLI invia all'app in esecuzione il nuovo request kind `open` del canale CLI ↔ app, con lo stesso envelope e la stessa versione di protocollo, payload `{path?}` con il percorso canonico (assente per la welcome window). L'app non viene mai avviata:
- nessuna app risponde: exit 1, code `app-not-running`, `` `weaver open` needs Weaver, and Weaver is not running. ``;
- l'app risponde `unsupported` (versione precedente): exit 1, `This version of Weaver cannot open folders from the command line; update Weaver.`;
- nessuna risposta entro 30 s: exit 1, `Weaver did not answer within 30 s; the folder may still open.`

**Gestione nell'app.** Il dispatcher instrada `open` accanto a `question.new`, sul blocking pool. Con un percorso esegue la stessa apertura del comando `open_project`: si apre una nuova finestra di progetto, oppure quella già aperta viene in primo piano, tornando dal background da sola quando era lì. Nessuna finestra conta come quella da cui la cartella è stata scelta: tutte le altre restano come erano. Senza percorso apre la welcome window, o la porta in primo piano (`show()`, `unminimize()`, `set_focus()`) se è già aperta, anche mentre ci sono finestre di progetto, che restano come sono, comprese quelle in background. La risposta parte quando la finestra è aperta o in primo piano (o la welcome window è mostrata), oppure porta la ragione del fallimento con le stesse frasi delle altre aperture (come "the folder does not exist").

**Fallimenti solo nel terminale.** Nella Machine window, da Closed `developer.OPEN_PROJECT` → Closed quando la cartella non esiste più o il progetto non si può aprire: se lo sviluppatore ha chiesto dal terminale, solo il terminale dice perché e Weaver non mostra nulla. Il fallimento va solo nella risposta: nessun toast, banner, `Alert`, evento `startup-failed` o `menu-open-failed` e nessuna welcome window; è registrato nel log come warning, percorso incluso. Il CLI stampa la ragione (exit 1, code `refused`).

**Successo.** Non stampa nulla (exit 0); con `--json` stampa `{}`.

**Help e riferimento dei comandi.** `weaver --help` elenca `open [<folder>]` nella tabella degli oggetti; `weaver open --help` stampa uso, le due forme e un esempio, come ogni comando; `--help --json` dà i JSON Schemas. `open` sta nel registro dei comandi, nell'help e nei JSON Schemas, ma mai nel riferimento dei comandi delle skill installate, poiché gli agenti non possono eseguirlo; quel riferimento va rigenerato e il test che lo confronta col registro deve continuare a passare.

**Welcome window e ultima finestra.** La welcome window mostrata così si comporta come sempre: si chiude quando una finestra di progetto si apre da essa. Chiudere la welcome window mentre una finestra di progetto è aperta, sullo schermo o in background, la chiude soltanto e non fa uscire Weaver; su Windows e Linux chiuderla esce solo quando nessuna finestra di progetto è aperta. Su macOS l'app resta nel Dock senza finestre, come prima.

## Code notes
Commit: f183f7c85d3fb27844c508df3fd421fac74fee1f

**CLI** (`crates/cli/src/`):
- `cli.rs`: `parse_arguments` (clap, parole libere), `run_with` (help per primo; rami parole vuote/verbo mancante), `fail`/`usage_failure`, `project_root` (join cwd + `paths::canonical_root`), `ask_app`/`app_not_running`, `dispatch`. Saltare `ensure_initialised` per `open`. Test con fake app `app()` + `AppDirs::from_bases`; aggiornare `words_and_options_that_do_not_fit_are_wrong_usage`.
- `registry.rs`/`spec.rs`: oggi solo coppie (object, verb); test `TABLE` (10 oggetti) e `the_command_reference_of_the_skills_is_the_one_the_registry_renders`.
- `help.rs`: `SHAPE`, `top_help`, `command_reference`.
- `client.rs`: `call`, `CallError::Unavailable`.
- Rigenerare `crates/core/assets/skills/weaver-design/COMMANDS.md` con `crates/cli/examples/write_command_reference.rs`.
- Contratto: `tests/cli-contract/contract.test.mjs` (liste `OBJECTS`/`COMMANDS`), `scripts/smoke-weaver-cli.mjs`.

**Core**: `crates/core/src/protocol.rs` (envelope invariato), nuovo modulo kind sul modello di `weaver_core::question::KIND`; `command.rs` `code::*`, `exit_code`. Env in `jobs/launch.rs`, `jobs/general.rs`, `jobs/ask.rs`.

**App**: `src-tauri/src/cli_handler.rs` `handler` (routing per `kind`); `windows/manager.rs` `open_project`/`open_reserved` (`OpenOutcome::Focused`, `focus_project`/`bring_forward`), `canonical_folder` (frasi di errore). NON usare `windows/menu_open.rs` `report`/`FailureRoute` né `report_failure`: i fallimenti non devono comparire in Weaver. Regola ultima finestra: cercare "welcome" in `src-tauri/src/windows/`.

Insidie: `weaver open --help` non deve finire in object_help→unknown; un percorso che inizia con `-` viene preso da clap come opzione; messaggi mai in prima persona (verificato dai test).
