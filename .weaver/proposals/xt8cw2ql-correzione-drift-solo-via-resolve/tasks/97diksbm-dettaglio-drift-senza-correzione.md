---
technical: false
refs:
  - drift/Open/developer.FIX_DRIFT_CODE
  - developer.FIX_DRIFT_CODE
  - requirements/ui#drift
  - requirements/ui#General
  - requirements/docs#General
---

# Dettaglio Drift senza correzione

Nell'app lo sviluppatore non decide più di correggere il codice dal dettaglio di un Drift. Dalla finestra di dettaglio spariscono la scelta dell'opzione di correzione, il campo `Note` e il pulsante `Fix the code`. Nell'app la correzione del codice si sceglie soltanto rispondendo alla domanda posta da `Resolve`, sia quella già preparata sia quella del job di risoluzione.

Le opzioni di correzione non compaiono più nel dettaglio, nemmeno in sola lettura, e non compare più la frase `No code change can respect both the Model and the Requirements.`: entrambe si vedono solo nella domanda di `Resolve`. Restano invece, nell'ordine attuale: id, titolo, badge; elementi e sezioni referenziati come chip; `What happens` e `What should happen` affiancati (impilati sotto i 640 px di larghezza della finestra); `Where in the code` con `Open in editor`; la Task d'origine e il Drift ripetuto quando noti; la Proposal allegata con titolo, stato, badge `stale`, corpo e file.

Le decisioni sulla Proposal allegata restano dove sono oggi, sotto la sezione della Proposal allegata: `Accept Proposal`, disabilitato con i motivi bloccanti di quella Proposal quando non può essere approvata; `Refine` al suo posto quando la Proposal è stale e Open; `Refine` e `Discard` (distruttivo, confermato come sulla scheda della Proposal) quando è una bozza lasciata a metà da un affinamento. Mentre la Proposal è in affinamento, `Accept Proposal`, `Resolve` e `Dismiss Drift` sono disabilitati con `Being refined`; `The agent is still finishing` resta il primo motivo su ogni controllo di decisione. Per un Drift di merge `Accept Proposal` resta marcato `recommended`. Il piè della finestra resta `Resolve`, `Dismiss Drift`, `Ask more`, e la scheda del Drift sulla Board non cambia (`Resolve`, `Dismiss Drift`).

La decisione di correggere il codice continua a esistere fuori dall'app: l'agente può ancora prenderla, se lo sviluppatore glielo chiede esplicitamente in una conversazione, con lo spostamento del Drift in `code-to-be-fixed`, che resta invariato insieme a `proposal-accepted` e `dismissed`. Le sue regole non cambiano: serve un'opzione quando il Drift ne offre; è rifiutata mentre la Proposal allegata è in affinamento; i riferimenti che non esistono più sono sostituiti dal contenitore più vicino, con rifiuto se non resta alcun elemento del Model; viene creata una Task di correzione con entrambi i lati, l'opzione, la nota e il punto del codice. Viene rimosso solo il controllo dell'app e il relativo comando dell'app.

Nella tabella delle icone resta solo `Accept Proposal` con `CheckCheck`; non c'è più una riga per `Fix the code`. Il changelog, nella sezione non ancora rilasciata, riceve sotto "Removed" una frase rivolta all'utente che dice che la correzione del codice non si sceglie più dal dettaglio del Drift ma rispondendo alla domanda di `Resolve`, senza id.

## Code notes

Commit: dabb34a48989d772ac1d5eae7a445886c6f1c4c4

In `ui/src/components/board/drift-card.tsx` togliere da `DecisionForm` la `RadioGroup`, la `Note`, `Fix the code` (icona `Wrench`, `fixDriftCode`) e la frase senza opzioni; tenere `Refine`/`Discard` (con `attached.refine`) e `Accept Proposal` con `CheckCheck` e badge `recommended` sul merge. Togliere `CHOOSE_AN_OPTION` e la sezione `Fix options` di `DriftDetails`; `DriftControls` e `DriftFooter` restano. Rimuovere `fixDriftCode` da `board-state.tsx` (interfaccia, implementazione, valore del contesto) e `fix_drift_code` dalla mappa in `ui/src/lib/backend.ts`. Lato Rust: comando `fix_drift_code` in `src-tauri/src/drifts.rs` e i suoi test (verificare se `a_decision_refused_when_applied_names_the_drift_and_writes_nothing` usa Fix), registrazione in `lib.rs`, `build.rs`, `capabilities/default.json`, `CommandName` in `commands.rs`; `DriftView.fix_reasons` in `drift_view.rs` e relativi test; `fix_reasons` in `crates/core/src/project/drift/reasons.rs` solo se non ha altri chiamanti. Binding ts-rs e file di permessi si rigenerano, mai a mano. Devono restare `Decision::FixCode`/`plan_fix` (usati da `weaver drift move`), `resolution::plan_decision`, il titolo `Fix the code: <title>` e le righe `Fix the code: option <n>` di `ResolutionRows`. Test in `drift-card.test.tsx`: eliminare quelli sul fix con opzione e nota e sull'agente su `Fix the code`, riscrivere quelli su opzioni, merge, frase senza opzioni e `Being refined`; riusare `show()`, `reasonsOf()`, `driftView()`. In `CHANGELOG.md` aggiungere `### Removed` in `## [Unreleased]`.
