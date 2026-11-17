---
created: 2026-11-11T11:15:11.069Z
updated: 2026-11-11T11:15:11.069Z
summary: Lo sviluppatore installa il comando weaver dalle Settings e con weaver open apre una cartella nel Weaver in esecuzione o ne mostra la finestra di benvenuto.
tasks:
- 2j0tkevs
- 50papxry
cleared:
  2j0tkevs:
    title: Installa comando weaver
    refs:
    - app
    - shell-command
    - developer.INSTALL_COMMAND
    - developer.REMOVE_COMMAND
    - requirements/tech#shell-command
    - requirements/ui#shell-command
    - requirements/ui#developer.CHANGE_SETTINGS
    - requirements/docs#shell-command
    technical: false
    proposal: vdxthvvg
    label: weaver open
    finished: 2026-11-11T11:02:06.848Z
    body: |-
      Lo sviluppatore può rendere il comando `weaver` raggiungibile dal proprio terminale, e toglierlo, solo dalle impostazioni: Weaver non lo installa né lo rimuove mai da solo. Ciò che viene reso raggiungibile è sempre la copia stabile della command line che Weaver tiene nella propria cartella di dati locali (`bin/weaver`, `weaver.exe` su Windows) e rinnova a ogni avvio, mai un file dentro il bundle dell'app o dentro il mount dell'AppImage: così un aggiornamento di Weaver lascia funzionante un comando installato.

      La descrizione della Feature `app` ora dice che lo sviluppatore raggiunge Weaver anche dal proprio terminale: una volta installato il comando `weaver` dalle impostazioni, apre una cartella nel Weaver in esecuzione o ne mostra la finestra di benvenuto; Weaver non installa mai il comando da solo e l'agente non lo usa mai per aprire qualcosa.

      ## Macchina `shell-command`

      Stato iniziale `Not installed`: il terminale non raggiunge la command line di Weaver tramite l'installazione di Weaver.

      - `developer.INSTALL_COMMAND` → `Installed`: Weaver rende la command line raggiungibile dai terminali aperti da quel momento e dice dove; su macOS il sistema può prima chiedere una password di amministratore; su Linux, se la cartella usata può non essere tra i posti in cui il terminale cerca i comandi, Weaver avvisa.
      - `developer.INSTALL_COMMAND` → `Not installed`, quando al suo posto c'è già qualcosa che non è il comando di Weaver: non si tocca nulla e Weaver dice quale file c'è.
      - `developer.INSTALL_COMMAND` → `Not installed`, quando lo sviluppatore annulla la richiesta di password di amministratore: non cambia nulla e non si mostra nulla.
      - `developer.INSTALL_COMMAND` → `Not installed`, quando l'installazione fallisce: non resta nulla dietro e Weaver dice perché.

      Stato `Installed`: il terminale raggiunge la command line di Weaver tramite la sua installazione.

      - `developer.REMOVE_COMMAND` → `Not installed`: Weaver disfa ciò che la sua installazione ha fatto; i terminali aperti da quel momento non trovano più il comando tramite essa.
      - `developer.REMOVE_COMMAND` → `Not installed`, quando ciò che occupa il posto non è più il comando di Weaver: non si tocca nulla e Weaver dice quale file c'è.
      - `developer.REMOVE_COMMAND` → `Installed`, quando lo sviluppatore annulla la richiesta di password di amministratore: non cambia nulla e non si mostra nulla.
      - `developer.REMOVE_COMMAND` → `Installed`, quando la rimozione fallisce: non cambia nulla e Weaver dice perché.

      Lo stato è quello che Weaver trova sul computer ogni volta che le impostazioni lo mostrano, quindi anche un cambiamento fatto fuori da Weaver si vede lì.

      ## Per sistema operativo

      - **macOS:** un link simbolico `/usr/local/bin/weaver` verso la copia. Se `/usr/local/bin` manca o non è scrivibile, la cartella e il link si creano con diritti di amministratore tramite `/usr/bin/osascript` con `do shell script … with administrator privileges`; i percorsi passano come argomenti (`on run argv`) quotati con `quoted form of`, mai dentro il testo dello script. Una richiesta di password annullata (errore AppleScript `-128`) non è un fallimento: nulla cambia e nulla si mostra. La rimozione funziona allo stesso modo.
      - **Linux:** un link simbolico `~/.local/bin/weaver` verso la copia, creando `~/.local/bin` se manca; non chiede mai privilegi. Dopo l'installazione, se `~/.local/bin` non è nel `PATH` dell'app stessa, il risultato porta l'avviso sul PATH. I pacchetti restano come sono: con il `.deb`, `/usr/bin/weaver` resta l'eseguibile dell'app, e il link vince solo dove `~/.local/bin` viene prima di `/usr/bin` nel `PATH` del terminale.
      - **Windows:** aggiunge la cartella `<app local data>\bin` al `Path` dell'utente in `HKEY_CURRENT_USER\Environment`, mantenendo il tipo del valore (`REG_EXPAND_SZ` resta espandibile) e ogni altra voce, poi trasmette `WM_SETTINGCHANGE` con `Environment`; i terminali aperti da quel momento trovano `weaver`. La rimozione cancella solo quella voce. Il `Path` di macchina non si legge né si scrive mai.

      ## Stato e file estranei

      - Su macOS e Linux è installato quando il posto è un link simbolico che risolve alla copia; non installato quando il posto è vuoto o contiene altro. Su Windows è installato quando il `Path` dell'utente contiene la cartella, confrontata dopo l'espansione delle variabili, senza distinzione di maiuscole e ignorando un `\` finale. Lo stato si legge fuori dal thread principale ogni volta che Settings si mostra e dopo ogni azione.
      - Su macOS e Linux l'installazione rifiuta quando il posto contiene un file, una cartella o un link che non risolve alla copia, e non tocca nulla; la rimozione cancella il link solo se risolve ancora alla copia, altrimenti non tocca nulla; entrambe riportano il percorso trovato.
      - Lo stato, l'installazione e la rimozione restituiscono ciascuno un risultato strutturato: installato o no, il posto, l'esito (fatto; rifiutato con il percorso trovato; annullato; fallito con il motivo) e su Linux l'avviso sul PATH. Una sola azione alla volta in tutta l'app: un'altra richiesta nel frattempo è rifiutata. Un fallimento si registra nel log come warning, come gli altri fallimenti dell'app.

      ## Settings

      Il gruppo `Command line` sta in Settings subito dopo `Editor` e prima di `Appearance` (ordine: `omp`, `Models`, `Agents`, `Editor`, `Command line`, `Appearance`, `Project`), con l'intestazione come gli altri gruppi. Contiene:

      - la riga `Open a folder in Weaver from a terminal with weaver open <folder>.`, con `weaver open <folder>` in Geist Mono;
      - lo stato: `Installed at <place>` (il posto in Geist Mono: `/usr/local/bin/weaver`, `~/.local/bin/weaver`, oppure su Windows la cartella aggiunta al PATH dell'utente) oppure `Not installed`, riletto ogni volta che Settings si mostra;
      - un pulsante: `Install weaver command` (icona `Terminal`) quando non installato, `Remove` quando installato; mentre l'azione è in corso gira ed è disabilitato.

      Il risultato appare sotto il pulsante e resta fino all'azione successiva, come per `Update skills`:

      - rifiuto all'installazione: `<path> already exists and is not Weaver's command; nothing was changed.`;
      - rifiuto alla rimozione: `<path> is not Weaver's command; nothing was removed.`;
      - fallimenti, nel colore d'errore: `Could not install the weaver command: <reason>.`, `Could not remove the weaver command: <reason>.`;
      - su Linux, dopo l'installazione, se `~/.local/bin` può non essere nel PATH, nel colore di avviso: `~/.local/bin may not be in your PATH; see the README.`;
      - una richiesta di password di amministratore annullata su macOS non mostra nulla.

      ## Documentazione

      `README.md` ha una sezione `## Command line` subito dopo `## Install`, che dice:

      - che `weaver open <folder>` apre una cartella nel Weaver in esecuzione e `weaver open` da solo mostra la sua finestra di benvenuto; Weaver deve essere già in esecuzione, il comando non lo avvia mai;
      - come installare il comando `weaver`: Settings › Command line › `Install weaver command`; su macOS un link in `/usr/local/bin`, che può chiedere una password di amministratore; su Linux un link in `~/.local/bin`, che deve essere nel PATH e, con il `.deb`, prima di `/usr/bin`, dove `weaver` è l'app stessa; su Windows la cartella di Weaver viene aggiunta al PATH dell'utente, per i terminali aperti da quel momento;
      - che `Remove` nello stesso posto lo disfa, e che nessuno dei due tocca un `weaver` che non è di Weaver.

      `CHANGELOG.md` aggiunge sotto `## [Unreleased]`, `Added`, una riga per `weaver open` e una per l'installazione del comando `weaver` da Settings.

      ## Code notes

      Commit: f183f7c85d3fb27844c508df3fd421fac74fee1f

      - Copia stabile: `src-tauri/src/sidecar.rs` (`install`, `FILE_NAME`) la copia in `<app local data>/bin/weaver` (0755) a ogni avvio; percorso da `AppDirs::bin_dir` in `crates/core/src/paths.rs`.
      - Processi figli solo con `weaver_core::process::command` (`crates/core/src/process.rs`, niente shell): usarlo per `osascript`.
      - Windows: `crates/core/Cargo.toml` ha già `windows-sys` 0.61 sotto cfg(windows); servono le feature `Win32_System_Registry` e `Win32_UI_WindowsAndMessaging`.
      - Nessun plugin `shell`/`fs`: solo `std`. Lavoro su disco in `spawn_blocking`. I comandi Tauri restituiscono errori tipizzati; i tipi TS vengono da Rust via ts-rs (la CI fallisce se sono vecchi).
      - UI: `ui/src/components/settings-view.tsx:539-569` contiene i `Group` (omp, Models, Agents, Editor, Appearance, Project); riusare il pattern del risultato sotto il pulsante di `Update skills`.
      - Docs: `README.md:5-14` sezione Install; `CHANGELOG.md` in formato Keep a Changelog.
      - Messaggi mai in prima persona.
  50papxry:
    title: Comando weaver open
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
    technical: false
    proposal: vdxthvvg
    label: weaver open
    finished: 2026-11-11T11:10:54.695Z
    body: |-
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
---
# Comando weaver da terminale

Il gruppo Command line nelle Settings installa e rimuove il comando weaver su macOS, Linux e Windows, senza mai toccare un file che non sia di Weaver. Il comando weaver open apre una cartella nel Weaver già in esecuzione, oppure mostra la finestra di benvenuto se non gli si passa nulla; non avvia mai Weaver e un agente non può usarlo. README e CHANGELOG descrivono entrambe le funzioni.
