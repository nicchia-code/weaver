---
technical: true
refs:
  - requirements/release#General
  - requirements/docs#General
---
# Versione 0.5.0

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
