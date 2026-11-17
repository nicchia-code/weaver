---
technical: false
refs:
  - app
  - shell-command
  - developer.INSTALL_COMMAND
  - developer.REMOVE_COMMAND
  - requirements/tech#shell-command
  - requirements/ui#shell-command
  - requirements/ui#developer.CHANGE_SETTINGS
  - requirements/docs#shell-command
---
# Installa comando weaver

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
