---
created: 2026-11-08T07:47:23.851Z
updated: 2026-11-08T07:47:23.851Z
summary: Weaver passa alla versione 0.5.0 con il CHANGELOG aggiornato; dipendenze dei test e2e corrette (glob, minimatch) e rischio glib su Linux accettato su GitHub.
tasks:
- 0u2h87jt
- x31kbxww
- 7b3jw1cy
cleared:
  0u2h87jt:
    title: Versione 0.5.0
    refs:
    - requirements/release#General
    - requirements/docs#General
    technical: true
    proposal: hc28x2by
    label: Dipendenze 0.5.0
    finished: 2026-11-08T07:29:41.322Z
    body: |-
      Porta Weaver da 0.4.0, già pubblicata con il tag `v0.4.0`, a 0.5.0, versione minore perché `dev` contiene funzioni nuove non ancora rilasciate.

      Quando il Task è finito:

      - La versione è 0.5.0 in `src-tauri/tauri.conf.json`, in `[workspace.package]` del `Cargo.toml` alla radice, da cui la prendono `src-tauri`, `crates/core` e `crates/cli`, e in `ui/package.json`; `Cargo.lock` registra 0.5.0 per ogni crate del workspace, nello stesso commit. Il controllo delle versioni di `ci` passa e i comandi con `--locked` riescono. Il controllo stesso non cambia.
      - Il `CHANGELOG.md` ha una sezione `## [0.5.0] - <data>` sotto `## [Unreleased]`, con la data del giorno in cui questo Task porta la versione a 0.5.0 su `dev`; `## [Unreleased]` resta in cima, vuota. Prima dello spostamento:
        - una riga `Fixed` dice che nelle quattro caselle di testo con tag (Chat, la risposta di un job nella vista Jobs, Ask more, l'editor delle richieste) il cursore e la selezione stanno sul testo disegnato, e che Invio o Tab premuti per confermare una composizione con metodo di input non inviano più il messaggio né scelgono un suggerimento;
        - la riga che dice che Ask more risponde senza dettagli interni di Weaver vale anche per Chat;
        - righe `Security`, come chiedono i Requirements della documentazione: le dipendenze dei test e2e `glob` e `minimatch` aggiornate a versioni corrette (GHSA-3g3m-2fjr-6pw4, GHSA-g3hv-75wg-m249), e `glib` 0.18 (GHSA-wrw7-89jp-8q8g) come rischio noto della sola app per Linux, che arriva da Tauri 2 e resta finché Tauri non passa a GTK4.
      - Tutte le righe di `[Unreleased]` passano alla sezione 0.5.0, nei loro gruppi, senza id di Task o Proposal; le sezioni delle versioni precedenti non cambiano.
      - Il Task si ferma al commit su `dev`: non apre la pull request verso `main`, non crea tag e non pubblica rilasci; il rilascio resta allo sviluppatore.

      ## Code notes

      Commit: e26f72108607520f3c19db3c51b7ec8413c73a78

      - Versione: `src-tauri/tauri.conf.json:4` (riferimento; `release.yml` la legge con jq), `Cargo.toml:6` (`[workspace.package]`), `ui/package.json:4`. `src-tauri/Cargo.toml`, `crates/core/Cargo.toml`, `crates/cli/Cargo.toml`, `crates/fake-acp/Cargo.toml` hanno `version.workspace = true`: nessuna modifica.
      - `Cargo.lock`: voci `fake-acp-agent`, `weaver`, `weaver-cli`, `weaver-core` (~righe 1180, 5290, 5319, 5331). Le altre `0.4.0` (`is-wsl`, `winapi-*-gnu`) sono di altri pacchetti: non toccarle. Aggiornare con un comando cargo offline o a mano, non con `cargo update` generale.
      - Non sono la versione di Weaver e restano 0.1.0: `package.json` alla radice e `e2e/package.json`. `pnpm-lock.yaml` non registra la versione di `ui` (l'unico `0.4.0` è un `engines` di un altro pacchetto).
      - Controllo: job `versions` in `.github/workflows/ci.yml:29-41` esegue `node --test scripts/` e `node scripts/check-versions.mjs` (risolve `version.workspace` dal `Cargo.toml` alla radice); i test usano fixture 1.2.3.
      - `CHANGELOG.md`: `[Unreleased]` ha oggi `Added` (5 righe) e `Changed` (12); nessun `Fixed` né `Security`. Segue `## [0.4.0] - 2026-11-06`. Nessun blocco di link di confronto in fondo. La riga incompleta è quella sulle risposte di Ask more (~riga 21). La correzione del cursore è la voce di History `qg2wshej` (2026-11-07).
  7b3jw1cy:
    title: Rischio glib accettato
    refs:
    - requirements/release#General
    technical: true
    proposal: hc28x2by
    label: Dipendenze 0.5.0
    finished: 2026-11-08T07:31:05.347Z
    body: |-
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
  x31kbxww:
    title: Dipendenze e2e corrette
    refs:
    - requirements/release#General
    technical: true
    proposal: hc28x2by
    label: Dipendenze 0.5.0
    finished: 2026-11-08T07:30:10.229Z
    body: |-
      Chiude gli avvisi Dependabot GHSA-3g3m-2fjr-6pw4 (`glob` 13.0.6, corretto in 14.0.2) e GHSA-g3hv-75wg-m249 (`minimatch` 10.2.6, corretto in 11.0.1), che arrivano solo dalle dipendenze di sviluppo dei test e2e.

      Quando il Task è finito:

      - In `e2e/package.json` tutti e cinque i pacchetti WebdriverIO (`@wdio/cli`, `@wdio/local-runner`, `@wdio/mocha-framework`, `@wdio/spec-reporter`, `webdriverio`) chiedono `^10.2.0`; `@wdio/config` 10.2.0 porta già `glob ^14.0.2`.
      - `pnpm-workspace.yaml` ha una sezione `overrides:` con `glob: ">=14.0.2"` e `minimatch: ">=11.0.1"`, con minimo aperto come deciso dallo sviluppatore. Servono perché Mocha 12.0.3, l'ultimo, chiede ancora `glob ^13` e `minimatch ^10.2.2`, e `readdir-glob` 3.0.0 (via `webdriverio` → `archiver`) chiede `minimatch ^10.2.2`. Le sostituzioni non vanno in un campo `pnpm` di un `package.json`: il pnpm fissato le ignorerebbe senza avvisi.
      - `pnpm-lock.yaml` è rigenerato e committato: registra le sostituzioni e non contiene più nessuna copia di `glob` sotto 14.0.2 né di `minimatch` sotto 11.0.1. `pnpm install --frozen-lockfile` riesce senza modificarlo.
      - Nessuna sostituzione forzata riguarda `glib` o altre dipendenze Rust.
      - I test e2e girano e passano in `ci` su Linux e su Windows come prima, contro la build di release; Vitest e la build dell'interfaccia non cambiano comportamento.
      - Gli avvisi su GitHub si chiudono da soli quando il lockfile corretto arriva su `main`, il ramo predefinito: il Task non apre la pull request verso `main` e non tocca gli avvisi a mano.

      La riga `Security` del `CHANGELOG.md` su queste dipendenze è scritta dal Task della versione 0.5.0.

      ## Code notes

      Commit: e26f72108607520f3c19db3c51b7ec8413c73a78

      - `package.json` alla radice: `packageManager: pnpm@12.16.1`, nessun campo `pnpm`. `pnpm-workspace.yaml` elenca `ui` e `e2e` e ha già `allowBuilds:`: aggiungere `overrides:` accanto. pnpm 11+ ignora `pnpm.overrides` nel `package.json` (pnpm#11536).
      - `pnpm-lock.yaml` contiene due documenti YAML: il primo è l'auto-pin di pnpm (`packageManagerDependencies`), il secondo il lock vero; le sostituzioni compaiono come `overrides:` nel secondo.
      - Lock attuale: `@wdio/*` e `webdriverio` 10.1.0 (`@wdio/protocols` 10.0.2, `@wdio/repl` 10.0.0), `mocha` 12.0.3, una sola copia di `glob` 13.0.6 e di `minimatch` 10.2.6. `ui` non porta né `glob` né `minimatch`.
      - `glob` 14.0.2 dipende da `minimatch ^11.0.0` e `path-scurry ^3`; `minimatch` 11.0.1 aggiunge `@isaacs/nonrecursive-glob-match`. Engines `18 || 20 || >=22`; `.nvmrc` è 26.13.0, compatibile anche con l'`engines` di WebdriverIO 10.2.0.
      - Rischio: Mocha usa `glob.hasMagic`, `glob.sync` e `minimatch(file, pattern, {windowsPathsNoEscape})`, tutti presenti nelle nuove major; la modalità watch di Mocha usa internals di `Glob` ma WebdriverIO non la usa. `readdir-glob` usa solo `new Minimatch(...)`.
      - `e2e/wdio.conf.mjs`: `framework: "mocha"` e glob delle spec, nessuna modifica attesa.
      - `ci.yml`: chiave della cache dello store pnpm basata sull'hash del lockfile (~riga 101), install con `--frozen-lockfile` (~113), e2e Linux (~180) e Windows (~199).
      - Nessun `.github/dependabot.yml`.
---
# Versione 0.5.0 e dipendenze

Versione 0.5.0 in tauri.conf.json, Cargo.toml, ui/package.json e Cargo.lock; il CHANGELOG sposta le righe di Unreleased nella sezione 0.5.0 con le righe Fixed e Security. I pacchetti WebdriverIO dei test e2e passano a ^10.2.0 e pnpm-workspace.yaml forza glob >=14.0.2 e minimatch >=11.0.1, con il lockfile rigenerato. L'avviso Dependabot su glib 0.18 (solo Linux, da Tauri 2) è chiuso su GitHub come rischio tollerato, dopo aver verificato che nessuna versione di Tauri 2 o di drag porta glib 0.20 o successiva.
