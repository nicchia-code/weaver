---
technical: false
refs:
  - project-setup
  - requirements/docs#atomic-commits
---
# README e CHANGELOG commit atomici

Quando la Task è finita, il `README.md` e il `CHANGELOG.md` del repository descrivono Atomic commits come lo descrivono il Model e i Requirements, in inglese come tutta la documentazione del progetto.

**README.** Una nuova sezione `## Atomic commits`, subito dopo la sezione `## Solo mode` (e dopo la sua sottosezione `### Merge`), prima di `## Branches`. Dice:

- cosa fa: dopo ogni voce di History scritta dall'agente, o ogni aggiornamento che aggiunge Task a una voce, Weaver fa il commit del codice di quella voce, un commit per voce, con un messaggio in inglese scritto dall'agente; un aggiornamento che non aggiunge Task non fa commit, e un commit già fatto non viene mai modificato;
- che è spento di base, appartiene alla copia di lavoro (non all'utente: un altro clone parte spento) e si accende solo in Settings → Project, con l'interruttore `Atomic commits`; non c'è un comando né un campo della riga di comando per cambiarlo;
- cosa entra in un commit: i file che l'agente indica per la voce, quello che lo sviluppatore ha già messo in stage e, fuori da Solo mode, il file della voce di History e la rimozione dei file delle Task che registra; con Solo mode attivo nessun file di Weaver e nessuna riga `Co-authored-by`; fuori da Solo mode il messaggio chiude con `Co-authored-by: Weaver <weaver@users.noreply.github.com>`, e l'autore è sempre lo sviluppatore come configurato in git;
- che hook e firma girano come configurati nel repository, che Weaver non fa mai amend, push o fetch, e che il commit viene saltato, con un avviso sul Board, quando i controlli finali della Run sono falliti o git non può fare il commit (fuori da un repository, git mancante, merge, rebase o cherry-pick in corso, HEAD staccato, hook o firma che rifiutano); la voce di History resta comunque scritta;
- che richiede git.

Nella sezione esistente `### Where Weaver keeps files`, se elenca le cartelle solo locali di `.weaver/`, va aggiunta `commits/` (lo stato di Atomic commits della copia di lavoro).

**CHANGELOG.** Sotto `## [Unreleased]`, in una sezione `### Added` (da creare se manca), una riga che descrive Atomic commits: l'interruttore per copia di lavoro in Settings → Project, spento di base, che fa un commit git del codice di ogni voce di History.
