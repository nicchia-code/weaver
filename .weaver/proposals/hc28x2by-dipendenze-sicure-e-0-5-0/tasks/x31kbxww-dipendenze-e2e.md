---
technical: true
refs:
  - requirements/release#General
---
# Dipendenze e2e corrette

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
