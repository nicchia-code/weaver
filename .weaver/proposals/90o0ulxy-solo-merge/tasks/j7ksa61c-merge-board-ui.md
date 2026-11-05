---
refs:
  - developer.MERGE_BRANCH
  - developer.ABORT_MERGE
  - requirements/ui#solo-merge
  - requirements/ui#work
  - requirements/ui#task
  - requirements/docs#solo-merge
---
# Merge sulla Board

Quando è fatto, il developer può avviare dalla Board il merge di un altro branch nel branch corrente (`developer.MERGE_BRANCH`) e annullare un merge in corso (`developer.ABORT_MERGE`), con tutta l'interfaccia di `requirements/ui#solo-merge`. L'interfaccia usa i comandi del backend e il campo `ProjectStatus.merge` realizzati da un altro Task di questa Proposal: `merge_refresh` (chiamato quando la Board viene mostrata), `merge_branches`, `merge_start`, `merge_abort` e `resolve_merge_conflicts`; questo Task aggiunge i loro binding in `ui/src/lib/backend.ts` se mancano e non ricalcola nulla lato interfaccia (disponibilità, motivi, conteggio dei conflitti arrivano dal backend). Il merge esiste solo nell'app, non nella riga di comando.

## Pulsante `Merge`
- `outline` Button con icona `GitMerge`, nell'intestazione della Board a sinistra di `Scan System`, presente solo con Solo mode acceso; tooltip `Merge another branch into <current>`.
- Disabilitato con il primo motivo che vale, nell'ordine: `The agent is working` (job dell'agente non concluso: in coda, al lavoro, in attesa o sospeso), `A merge is in progress`, `Merge needs git: <reason>`, `Git is in the middle of a merge, rebase or cherry-pick`, `Not on a branch`, `Commit or stash your changes first` (modifiche non committate ai file tracciati, esclusi quelli di Weaver). Il motivo è il tooltip e la descrizione accessibile del pulsante disabilitato.
- La disponibilità si aggiorna quando la Board viene mostrata, quando la finestra torna in primo piano, quando un job finisce e dopo ogni azione di merge; mai a intervalli.

## Dialog
- Titolo `Merge a branch`; campo `Search branches` che filtra una lista `Command` con i branch locali e poi i remoti già noti a git (nessun `git fetch`), sotto le intestazioni `Local` e `Remote`; frecce per muoversi, Enter per scegliere. Senza altri branch la lista mostra `No other branch`.
- Scelto il branch: `Merge <branch> into <current>?` e, se git traccia ancora file di Weaver, un `Alert` (variante warning) `Git still tracks <N> of Weaver's files: their changes go into the merge commit.` (Merge resta comunque disponibile); pulsanti `Merge` (primario) / `Cancel`.
- Mentre git lavora `Merge` mostra uno spinner e entrambi i pulsanti sono disabilitati; il Dialog si chiude quando git risponde.

## Avvisi
In cima alla Board (`requirements/ui#work`, **Details dialogs and notices**), ognuno con pulsante di chiusura:
- `<current> is already up to date with <branch>.` (il branch non porta nulla, non si avvia nulla);
- `Could not merge <branch>: <first line of git's error>.` (nulla cambia);
- `Could not merge <branch>: it would overwrite files that Solo mode keeps out of git.` seguito dai percorsi in monospace;
- `Could not merge <branch>: git did not finish the merge within 2 minutes.` (Weaver ha interrotto e annullato il merge);
- `The merge of <branch> ended without a commit: Weaver forgot it and did not scan.` (merge sparito fuori da Weaver);
- `Could not abort the merge: <first line of git's error>.`

## Banner del merge
- Con un merge in corso, un `Alert` sopra le due colonne e sopra gli avvisi, icona `GitMerge`, senza chiusura: `Merging <branch> into <current>: <n> files with conflicts. Resolve them here or by hand, then commit the merge in git.`, oppure, senza conflitti rimasti, `Merging <branch> into <current>: no conflicts left. Commit the merge in git to finish.`
- Azioni a destra: `Resolve with the agent` (primario, icona `Sparkles`, solo finché qualche file ha conflitti; disabilitato con `The agent is resolving the conflicts` finché il job `Merge conflicts` non è concluso), che chiama `resolve_merge_conflicts`; `Abort merge` (outline, testo destructive), disabilitato con lo stesso motivo.
- `Abort merge` chiede conferma: `Abort the merge of <branch>? Every change of the merge, conflict resolutions included, is lost.`, `Abort merge` (destructive) / `Cancel`. Confermato, Weaver annulla il merge in git, lo dimentica e non avvia scan.
- Weaver non fa mai il commit del merge: il developer lo fa in git.

## Altri controlli durante il merge
- `Run all` (`requirements/ui#work`): primo motivo `A merge is in progress`, prima di `Drifts are being resolved`.
- `Scan System`: disabilitato con `A merge is in progress`.
- `Run` di un Task (`requirements/ui#task`): `A merge is in progress` dopo `The agent is still finishing` e prima di `Drifts are being resolved`.
- Il conteggio `· <n> done since last scan` conta i Task finiti (non chiusi senza modifiche) dall'avvio dell'ultimo scan dell'intero System, chiesto dal developer o avviato dopo un merge, compresi quelli cancellati e dimenticati; nascosto se 0.

## Accessibilità
Il banner è una `region` chiamata `Merge in progress`; un cambio del suo testo è annunciato (`aria-live="polite"`); i motivi di disabilitazione sono la descrizione del loro controllo (`aria-describedby`).

## Test e documentazione
- Test Vitest: pulsante presente solo con Solo mode acceso e i suoi motivi, Dialog (filtro, intestazioni, `No other branch`, conferma con e senza file tracciati, spinner), avvisi, banner nei due testi e con le azioni disabilitate, conferma di annullamento, `A merge is in progress` su `Run all`, `Scan System` e `Run`.
- `README.md`: dentro `## Solo mode` una sottosezione `### Merge` (`requirements/docs#solo-merge`): a cosa serve (portare il lavoro degli altri nel branch corrente e trovare con uno scan cosa ha cambiato rispetto al Modello); dove sta (la Board, solo con Solo mode acceso) e quando è disponibile; che Weaver esegue `git merge` ma non fa mai fetch e non fa mai il commit di un merge con conflitti (il developer li risolve a mano o con l'agente, poi fa il commit); che lo scan dopo il merge parte da solo e i suoi Drift dal merge raccomandano di adattare il Modello.
- `CHANGELOG.md`: sotto `## [Unreleased]`, `Added`, una riga che descrive Merge in Solo mode.

Punti nel codice: intestazione in `ui/src/components/board/board-view.tsx` (`ViewHeader view="board"`, `Scan System`, conteggio; `BoardFooterButtons` per `Run all`, i cui motivi arrivano da `snapshot.runAllReasons`); i motivi di `Run all` e di `Run` sono calcolati nel core, in `crates/core/src/project/board.rs` (costanti `REASON_*`) e `crates/core/src/project/tasks.rs:run_reasons`, dove va aggiunto `A merge is in progress`; `ui/src/components/board/board-actions.tsx`, `NoticeStack` in `ui/src/components/notices`; banner accanto a `OpeningCheckToast`/`SkillsBanner` in `ui/src/components/project-content.tsx`; binding in `ui/src/lib/backend.ts` (vicino a `set_solo_mode`, `solo_tracked_files`); test del conteggio in `ui/src/components/board/board-view.test.tsx`; `README.md` `## Solo mode`, `CHANGELOG.md` `## [Unreleased]`.
