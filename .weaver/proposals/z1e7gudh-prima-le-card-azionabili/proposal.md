---
status: approved
kind: change
base:
  model/model-changes/proposal.md: 0ec76593b5ba8ead752d693765d8504e1cd2b10b7cf743c9896489a6fdec547f
  model/work/feature.md: 13de0ce376465186204a950a6c8fdbf7d34b74725b221d75df9487ca7f7da74a
  model/work/task.md: 0ef0f95f3cb27fe0f89bdb0dea2b6e0f33b0e6a76ddabf993ca040793003c10e
  requirements/cli/commands.md: c4c41f3421655f41473f1c9861c6afaabdb4c13d5b6053e029e3b3eb0c620583
  requirements/rules/work.md: 7a304a4e27af8f436fc74b37c82efb1629986724f8d7dc93130a1d864c3c93b2
  requirements/tech/architecture.md: ebe5d4c21f91b299591cb9b94afd9298c97b3c413cb08b955a3b5bd214377f8a
  requirements/ui/board.md: 3ce8626feeb6b89660c2ce7b83d6a5ac7b9257a31369e6b2e2323daef254eb2f
  requirements/ui/requests.md: 27cfc41b509c13b717476196a1bbbb11a8f772f66c731f6d01edcd5ea1701587
  requirements/ui/window.md: 3c63d7722fedbff782db2b9b2b3def402d22a3b2ab05e8f57da2e085317a765e
created: 2026-11-11T13:35:20.048Z
request: 2026-11-11T14-21-38
label: Ordine Board
job: 5mk3j520
---
# Prima le card azionabili

Sulla Board ciò che il developer può fare deve stare sopra ciò che non può fare, e una card che sblocca altre card deve venire prima. Oggi Backlog e Active sono solo in ordine di creazione, con card bloccate in mezzo a quelle pronte.

## Decisioni

- Ambito: le due colonne della Board; `weaver project show` e il gruppo `Cards` della palette seguono lo stesso ordine. La lista dei Jobs non cambia.
- Azionabile: il comando principale della card (`Run`, `Approve` o `Refine`, `Resolve`, `Send`) è abilitato contando solo i motivi della card stessa; Run in corso, merge in corso, resolve in corso e l'agente che sta ancora finendo non spostano nessuna card.
- Sbloccanti, tutte e quattro le relazioni: Drift aperto che blocca dei Task, fix Task con un Task di origine in attesa, Proposal richiesta da una Proposal non ancora approvata, risoluzione in attesa di decisione mentre altri Drift sono aperti.
- Uno sbloccante non azionabile va in testa ai non azionabili, mai sopra ciò che si può premere.
- Active: Task avviabili (sbloccanti per primi), poi Task In progress, poi ghost, poi bloccati e in attesa (sbloccanti per primi).
- Backlog: Proposal e Drift azionabili (sbloccanti per primi), poi bozze di request non vuote, poi ghost, poi non azionabili (sbloccanti per primi, bozze vuote comprese).
- Dentro ogni gruppo: ordine di creazione, più vecchia prima; il rework prende ancora il posto della Proposal rifiutata e il refinement conserva il posto nell'ordine di creazione. I ghost tra loro in ordine di annuncio, come oggi.
- Nessun separatore, intestazione o stile diverso per le card non azionabili: solo l'ordine.
- Movimento: una card che cambia posto scorre al nuovo posto in circa 150 ms; card e ghost che entrano o escono appaiono e svaniscono in dissolvenza nello stesso tempo; nessuna animazione con riduzione del movimento.
- Una card che svanisce è inerte (niente clic, focus o lettori di schermo); lo stato vuoto di una colonna entra in dissolvenza insieme all'uscita dell'ultima card.
- Run all e `weaver task list` con `runnable` non cambiano: l'ordine di lavoro resta dal più vecchio.

## Assunzioni dell'agente

- Una Proposal in refinement non è azionabile; una Draft lasciata a metà da un refinement lo è (`Refine` per continuare).
- Le card non azionabili di tipo diverso (Proposal, Drift, bozze vuote) stanno in un solo ordine di creazione; per una bozza conta la creazione della request.
- L'animazione è solo visiva: numero della colonna, intestazione, dialog e focus cambiano nello stesso istante di oggi; il focus resta sulla card che si sposta.
- Il motivo «l'agente sta ancora finendo» di un Task appena registrato da un job non lo sposta: conta come gli altri motivi che fermano tutte le card.

## Already present

- `proposal/Open/developer.REWORK_PROPOSAL` — `crates/core/src/project/board.rs : compute` (chiave `place` della Proposal rifiutata), test `proposal/tests.rs::a_rework_takes_the_place_of_the_rejected_proposal_on_the_board`
- `proposal/Refining` — `crates/core/src/project/board.rs : compute` (la chiave `place` resta quella della Proposal raffinata)
- `proposal/Draft/developer.APPROVE_PROPOSAL` — `crates/core/src/project/proposal/transitions.rs : gate_of`
- `proposal/Draft/developer.REFINE_DRIFT_PROPOSAL` — `crates/core/src/project/proposal/transitions.rs : gate_of`
- `proposal/Open/developer.APPROVE_PROPOSAL` — `crates/core/src/project/proposal/transitions.rs : gate_of`
- `proposal/Open/developer.REFINE_DRIFT_PROPOSAL` — `crates/core/src/project/proposal/transitions.rs : gate_of`
- `proposal/Refining/agent.EDIT_PROPOSAL` — `crates/core/src/project/proposal/transitions.rs : gate_of`
