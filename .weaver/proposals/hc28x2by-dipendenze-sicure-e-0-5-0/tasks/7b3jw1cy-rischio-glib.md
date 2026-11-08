---
technical: true
refs:
  - requirements/release#General
---
# Rischio glib accettato

L'avviso Dependabot GHSA-wrw7-89jp-8q8g (RUSTSEC-2024-0429: unsoundness negli iteratori di `glib::VariantStrIter`, corretto in `glib` 0.20.0) non si può correggere oggi e diventa un rischio accettato su GitHub.

Quando il Task è finito:

- L'avviso numero 1 del repository `nicchia-code/weaver`, quello di `glib` in `Cargo.lock`, è chiuso su GitHub dall'agente con `gh` come `tolerable_risk`, con un commento in inglese che dice: `glib` 0.18.5 arriva solo su Linux da Tauri 2 (`gtk` 0.18 tramite `tauri`, `tray-icon` → `libappindicator`, `muda`, `wry`, `tao`) e da `drag` 2.1.1; nessuna versione di Tauri 2 permette `glib` 0.20 o successiva; la linea 0.18 di gtk-rs non avrà più rilasci; forzare `glib` romperebbe la build Linux; il codice di Weaver non usa `glib` né `VariantStrIter`; il rischio resta accettato finché Tauri non passa a GTK4.
- Prima di chiuderlo, l'agente verifica che sia ancora vero che nessuna versione di Tauri 2 con i suoi plugin e di `drag` porta `glib` 0.20 o successiva. Se invece una tale versione esiste, non chiude l'avviso: si ferma e lo segnala, perché la decisione dello sviluppatore valeva solo nel caso che non esista.
- Nessuna dipendenza Rust cambia e `Cargo.lock` non cambia per questo Task; nessuna sostituzione forzata di `glib`.
- Gli altri due avvisi (`glob`, `minimatch`) non vengono toccati a mano.

La riga `Security` del `CHANGELOG.md` sul rischio noto di `glib` su Linux è scritta dal Task della versione 0.5.0.

## Code notes

Commit: e26f72108607520f3c19db3c51b7ec8413c73a78

- Avvisi aperti letti con `gh api repos/nicchia-code/weaver/dependabot/alerts`: numero 1 `glib` (`Cargo.lock`), 2 `glob`, 4 `minimatch` (`pnpm-lock.yaml`). Il token `gh` locale ha permessi di amministratore.
- Chiusura: `gh api -X PATCH repos/nicchia-code/weaver/dependabot/alerts/1 -f state=dismissed -f dismissed_reason=tolerable_risk -f dismissed_comment='…'` (il commento ha un limite di lunghezza di GitHub, tenerlo breve).
- Lock attuale: `tauri` 2.12.5 (ultima stabile, chiede `gtk ^0.18`, `tray-icon ^0.25`, `muda ^0.20`), `tauri-runtime-wry` 2.12.3, `wry` 0.57.1, `tao` 0.37.2, `tray-icon` 0.25.1, `libappindicator` 0.9.0, `drag` 2.1.1 (`gtk ^0.18.1`), `gtk` 0.18.2, `glib` 0.18.5. `tauri-plugin-dialog` → `rfd` usa solo i crate `-sys`.
- Verifica: `cargo tree --locked -i glib@0.18.5 --target all -e normal`; versioni su crates.io (`cargo info tauri`, API `crates.io/api/v1/crates/<nome>/versions`). Tauri 3.0.0-alpha ha la feature `gtk4` ma porta ancora `glib` 0.18 tramite `tray-icon ^0.25`, e non è adatto alla produzione.
- Riferimenti: gtk-rs-core#1991 (nessun rilascio 0.18), tauri#14684 (migrazione GTK4, aperta).
- L'unico contatto di Weaver con GTK è `src-tauri/src/assets/drag.rs` (`window.gtk_window()` passato a `drag::start_drag`).
