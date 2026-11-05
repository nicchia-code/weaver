---
refs:
  - solo-mode
  - requirements/ui#solo-mode
---

# Solo mode durante un merge

Quando il lavoro è finito, mentre un merge è in corso (cioè finché esiste `.weaver/merge/merge.json`, vedi `solo-merge`) Solo mode non si può cambiare in nessun modo: nello stato `On`, `developer.CHANGE_SOLO_MODE` resta in `On` sia quando lo sviluppatore lo spegne sia quando sceglie l'altro modo; non cambia nulla (né il blocco nelle esclusioni né il modo ricordato) e Settings dice di finire o annullare prima il merge. Le transizioni `On` → `Off` e `On` → `Excluded` valgono quindi solo quando nessun merge è in corso. Lo stato `On` ora menziona che la Board offre `Merge`.

Decisione dell'intervista: durante un merge in corso il controllo di Solo mode è disabilitato del tutto, anche per il cambio di modo, non solo per lo spegnimento.

Cosa deve valere:

- **Backend:** `set_solo_mode` rifiuta la richiesta quando `merge.json` esiste, sia per `off` sia per l'altro modo, con il motivo `Finish or abort the merge in progress first.` (`requirements/tech#solo-merge`, **Solo mode**); il controllo avviene prima di qualunque chiamata a git o scrittura delle esclusioni, così nulla cambia.
- **Settings:** mentre un merge è in corso il controllo `Solo mode` è disabilitato, con `Finish or abort the merge in progress first.` sotto la nota `Applies only to this working copy: other clones start with it off.` (`requirements/ui#solo-mode`, **During a merge**). Come ogni motivo di disabilitazione, il testo è la descrizione del controllo per le tecnologie assistive (`aria-describedby`, `requirements/ui#solo-mode`, **Accessibility**). L'informazione arriva dal `ProjectStatus` (campo `merge.inProgress`, presente mentre `merge.json` esiste).
- **Riga di comando:** `project edit {solo}` riceve lo stesso rifiuto con lo stesso motivo quando un merge è in corso, e nulla cambia.
- **Test:** un test Vitest del controllo disabilitato con il testo e la descrizione accessibile durante un merge; un test del backend/core che verifica il rifiuto di `off` e dell'altro modo con `merge.json` presente, lasciando invariate le esclusioni.

Fuori da questa Task: il motore del merge (passi git, `merge.json`, comandi `merge_*`, campo `merge` di `ProjectStatus`, banner e job) è un'altra Task della Proposal; qui si legge solo se un merge è in corso.

Punti nel codice: `src-tauri/src/project_setup.rs` (`set_solo_mode` a :591 e :890, `ProjectStatus`/`SoloStatus` a :114-165); `crates/core/src/solo_mode.rs` (`set_mode`); `ui/src/components/project/solo-mode.tsx` (`SoloModeSetting`, `SoloModeField`); `crates/cli/src/commands/project.rs` (`project edit {solo}`).
