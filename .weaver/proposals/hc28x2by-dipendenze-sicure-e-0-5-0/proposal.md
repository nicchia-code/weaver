---
status: approved
kind: change
base:
  requirements/docs/main.md: 22908301ef284cce92fb6dab7f3c4c1d704a4de2cf6a611738764acfc3e51911
  requirements/release/pipeline.md: b7344404bc38ff0f335962867a9ec5f395a6a915d3a2b051200045918ec8e0ac
created: 2026-11-08T07:24:12.973Z
request: 2026-11-08T08-19-03
label: Dipendenze 0.5.0
job: mi3lxdlw
---
# Dipendenze sicure e 0.5.0

Chiude i tre avvisi Dependabot aperti su `nicchia-code/weaver` (GHSA-g3hv-75wg-m249 `minimatch`, GHSA-3g3m-2fjr-6pw4 `glob`, GHSA-wrw7-89jp-8q8g `glib`) e porta Weaver da 0.4.0 a 0.5.0.

## Decisioni

- WebdriverIO dei test e2e passa alla 10.2.0 (tutti e cinque i pacchetti), il cui `@wdio/config` chiede già `glob ^14.0.2`.
- `glob` e `minimatch` sono forzati con minimo aperto, `>=14.0.2` e `>=11.0.1`: Mocha 12.0.3 (ultima) e `readdir-glob` 3.0.0 (ultima, via `webdriverio` → `archiver`) chiedono ancora le versioni vulnerabili.
- Le sostituzioni forzate vanno in `pnpm-workspace.yaml` sotto `overrides:`, non in `pnpm.overrides` del `package.json` come diceva la richiesta: il pnpm fissato (12.16.1) ignora quel campo senza avvisi.
- Le sostituzioni sono temporanee: si tolgono quando Mocha e `readdir-glob` chiedono da soli versioni corrette.
- `glib`: nessuna versione di Tauri 2 porta `glib >= 0.20` (`tauri` 2.12.5, `wry` 0.57.1, `tao` 0.37.2 e `drag` 2.1.1, tutte ultime, chiedono `gtk ^0.18`; gtk-rs 0.18 è fuori supporto; GTK4 arriva con Tauri 3, oggi in alpha). Nessuna sostituzione forzata di `glib`. L'avviso diventa un rischio accettato: l'agente lo chiude con `gh` come `tolerable_risk` con un commento, e i Requirements di release registrano il rischio finché Tauri non passa a GTK4.
- Versione 0.5.0 (minore). Nel Rust il valore sta in `[workspace.package]` del `Cargo.toml` alla radice e `Cargo.lock` cambia insieme: il Requirement di release ora lo dice; il controllo di `ci` resta com'è.
- `CHANGELOG.md`: la sezione `## [0.5.0]` ha la data del commit che porta la versione a 0.5.0 su `dev`; prima dello spostamento `[Unreleased]` riceve una riga `Fixed` per il cursore allineato al testo e la riga sulle risposte senza dettagli interni di Weaver viene estesa a Chat; una riga `Security` racconta le dipendenze dei test aggiornate e il rischio noto di `glib` su Linux.
- Regola generale nei Requirements della documentazione: ogni avviso di sicurezza chiuso o accettato aggiunge una riga `Security`, anche quando tocca solo i test o la CI.
- La pull request da `dev` a `main` per il rilascio resta allo sviluppatore: gli avvisi di `glob` e `minimatch` si chiudono da soli quando il lockfile corretto arriva su `main`, il ramo predefinito.
