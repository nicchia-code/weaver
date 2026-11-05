---
status: approved
kind: change
base:
  model/actors/desktop.md: 90090cd9af641edffe916ce900fa2acf5c6a40cab5dffea3add157fefa37a852
  model/actors/developer.md: f1a948e35a1a86904d048c0d0f6da7bfce47e763cb4e2a82f89f3e7244e68f07
  model/agent-work/job.md: 3631263dd11863e656eb34ca159aa7782748ebb4ed637d7aa028c523c6ae1082
  model/project-setup/feature.md: 25f235ce671b64dc05fce3ea0d7a8b2329b0ddcc91d061c0bf82ffd5065a8773
  model/project-setup/solo-merge.md: new
  model/project-setup/solo-mode.md: d1d0cd0c54ea09fb419f70bb6343d291c1c94e455acd8859cdf0008d7d1ed96e
  model/work/drift.md: 312316f7d3578c16bbdd9d22eb4217d42c3f76903bb4c651051607d4a2cd02f3
  model/work/feature.md: e3d89e22da0dee20f6333d33884caec1db4dab7effc846b888dfe6d603889b61
  requirements/cli/commands.md: d92f921e2adcbbe4075211599f3934c6d7de940af5d24c01f88d1e7257e159e6
  requirements/data/project-files.md: 05cec9b1608d1e91ddf26f69fecf7e28c6f5f200017aae2474e291e2c3a67a80
  requirements/data/settings.md: b6f365b716e5f5ce442afce2886d87a042a8953a49cdc5c365db2416f51c26c6
  requirements/docs/main.md: 21d075a5af382b83886525323e3d2e1ef7cfe47004b98b514ffd4f8f23d5de65
  requirements/quality/main.md: bda144ddc6b1b58ad9bca02c0cfe6991f731940be7f5a92b0d501048381c5e1c
  requirements/rules/cards.md: af5224304e0e4f530b4aa11309d744efee2b2e2dc09e5a5039678230f27f5893
  requirements/rules/instructions.md: e1f56c6d132f12d4d832f840a37759ab662569e2ce367ca62e568dd857d88e89
  requirements/rules/skills.md: ccccf2336fe25823ac3671025f5ebc5c96463f14cfbfa5a6da6c56c5cd96ff55
  requirements/rules/solo-mode.md: cea34b0049c3a8734d85c0c29ba3fdbb51a24bfc6a9be7bc3a8b59507d25c2b5
  requirements/rules/work.md: new
  requirements/tech/solo-mode.md: ee24eb3f93eeba943db9324c7aa45ead27ac5600f06a6722abd3a4128eaa29dd
  requirements/ui/board-cards.md: caa66559d2cd9131b568d49c4879b0647c8184bde31d787c5ca5fa35f43fbd2a
  requirements/ui/board.md: eb559ab88ff992ac620ac8fd1e3c538438ae2a96a554b103e59014a47327b926
  requirements/ui/conversations.md: 6734d7e795a6c3e3522cb857c0c4a2e7cfb7191de31a6df8c4d19d62f69f4625
  requirements/ui/solo-mode.md: 17890233183c1a636becc577402c8e7aceef0e9a24d149b16a7db836e954148a
created: 2026-11-05T13:49:18.643Z
request: 2026-11-04T09-38-44-2
label: Solo merge
job: xhc3z7cf
---
# Solo merge

Con Solo mode acceso gli altri cambiano il codice senza aggiornare il Modello. Questa Proposal aggiunge un Merge che porta un altro branch nel branch corrente e fa trovare, con uno scan del System, ogni differenza che quel lavoro ha aperto rispetto al Modello, come Drift da merge.

## Decisioni

- Il pulsante `Merge` (outline, icona `GitMerge`) sta nell'intestazione della Board, prima di `Scan System`, e c'è solo con Solo mode acceso.
- Un solo Dialog: lista filtrabile dei branch locali e dei remoti già scaricati (nessun `git fetch`), poi la conferma che nomina i due branch.
- Merge è disponibile solo senza modifiche non committate ai file tracciati (esclusi quelli di Weaver), senza merge, rebase o cherry-pick in corso, su un branch, e senza job dell'agente non conclusi (in coda, al lavoro, in attesa o sospesi); altrimenti è disabilitato con il motivo.
- Con file di Weaver ancora tracciati Merge resta disponibile e la conferma ricorda che le loro modifiche entrano nel commit del merge.
- Un merge che sovrascriverebbe file che Solo mode tiene fuori da git è rifiutato, con il motivo.
- Senza conflitti git conclude il merge come di solito (avanzamento rapido o commit di merge con il messaggio predefinito).
- Un branch che non porta nulla di nuovo non avvia nulla: Weaver dice che il branch corrente è già aggiornato.
- `git merge` ha un limite di 2 minuti; oltre, Weaver lo interrompe e lo annulla.
- Se git rifiuta o fallisce il merge, nulla cambia e Weaver mostra la prima riga dell'errore di git.
- Durante un merge in corso sono rifiutati Run, Run all, resolve, scan e import; design, rework, refinement e il job sui conflitti restano possibili; Solo mode non si può spegnere.
- Il job `Merge conflicts` (tipo `merge`, modello Analysis, nuova skill `weaver-resolve-conflicts`) risolve i conflitti, chiede con un Question round quando la scelta non è ovvia, segna come risolti in git i file sistemati, non fa mai il commit e non annulla mai il merge; si avvia solo dall'avviso del merge in corso.
- Se il job finisce e restano conflitti, il merge resta in corso.
- `Abort merge` chiede conferma ed è rifiutato mentre il job sui conflitti lavora.
- Weaver si accorge della fine del merge controllando git all'apertura, quando la finestra torna in primo piano, quando un job finisce e dopo ogni sua azione; mai a intervalli e senza osservare la cartella di git.
- Un merge sparito senza commit (annullato fuori da Weaver, branch cambiato, reset) viene dimenticato senza scan, con un avviso.
- Il merge in corso è ricordato in `.weaver/merge/merge.json`, locale e mai versionato, cancellato quando il merge finisce in qualsiasi modo.
- A merge concluso Weaver avvia da solo, senza conferma, lo scan del System (nome `Scan`), che conta come l'ultimo Scan System e azzera il numero dei Task fatti.
- I file portati dal merge sono quelli diversi fra il commit di partenza e il commit del merge; l'agente li legge con `weaver job show`.
- Solo lo scan dopo un merge può segnare un Drift con `merge: true`, alla creazione e per sempre; le regole dei Drift ripetuti e scartati restano uguali.
- La domanda di risoluzione di un Drift da merge offre sempre di tenere il codice e adattare il Modello, con la Proposal allegata, e la raccomanda; nei dettagli `recommended` va su `Accept Proposal` e nessuna correzione è preselezionata.
- La card mostra il badge `merge` con il tooltip `Found by the scan after a merge`.
- Il merge esiste solo nell'app, non nella riga di comando.
- Durante un merge in corso anche `Resolve` di un Drift è disabilitato, pure con una domanda preparata.
- Un file del merge in corso illeggibile viene segnalato, il merge è dimenticato senza scan e il file cancellato.
- Dettagli dell'interfaccia confermati: titoli `Local` e `Remote` e `No other branch` nel Dialog, spinner su `Merge` mentre git lavora, avvisi per il limite di tempo e per un annullamento fallito, `Resolve with the agent` primario con `Sparkles`, avviso del merge come regione accessibile.
- Testi dell'interfaccia come proposti nell'intervista; README (sottosezione `### Merge`) e CHANGELOG aggiornati; test con repository git temporanei e git vero, più Vitest.

## Scelte dell'agente

- Il Modello del merge è una nuova Machine `solo-merge` nel Feature `project-setup`, accanto a `solo-mode`.
- Il ritorno della finestra in primo piano è un nuovo Event `desktop.WINDOW_FOCUSED`.
- Weaver scrive il file del merge in corso prima di avviare git, così un merge interrotto da un crash viene ritrovato alla riapertura.
- Durante un merge in corso il controllo di Solo mode è disabilitato del tutto, anche per il cambio di modo, come dice il testo scelto.
- Il fragment `rules/cards.md` supera i 12 KB: le sezioni `task`, `drift` e `work` passano, senza cambiare testo, nel nuovo fragment `rules/work.md`.
