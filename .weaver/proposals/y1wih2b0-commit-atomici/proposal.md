---
status: approved
kind: change
base:
  model/actors/developer.md: 438d8e9cb94c066abf4be47cc7b5d61dfbec70e43815ba5da31a459c88d0b31c
  model/project-setup/atomic-commits.md: new
  model/project-setup/feature.md: d0f94459f7112c5763c9f4d45bdc7adf2bd0168ca472ef00b2406ec8c9d95b29
  model/work/history-entry.md: a9b833f265cc962a1e4482499cafcbc0d19a7094d2aafa5ef4bfa8d3b4a70697
  requirements/cli/commands.md: 8a9b5d028dd6b951252a0999e3c7734b5182f4ca27262d1b76421ccf5a1de524
  requirements/cli/history.md: new
  requirements/data/project-files.md: 2f900c6b1cf2cd8611097948a322ab706b5bfdc181fa6015f94f5c61bfa83160
  requirements/data/settings.md: db64b1663d86bbea69f0ac07aa9b1543281435a5eb685511129a37a4ecc98732
  requirements/data/working-copy.md: new
  requirements/docs/main.md: abf24a94a4163aca5bd0552a02b0c66130c8eed51250d6714035123028cc377d
  requirements/quality/git.md: new
  requirements/quality/main.md: dbe5a88d7a62caa5535c766db31704479eeebb317c3cd21178c7af3a9c9db777
  requirements/rules/skills.md: ed1127b57645ad59f07c37c145d2af533c244ca450d31befc86f7a3aed8ba26d
  requirements/rules/solo-mode.md: eb67c8ef798515b580a795543acde870477dd39d859831781049ee4997869168
  requirements/tech/solo-mode.md: 06d87807f3b3bb0f2bfd788ecdadb13443caf7e9f59e0d5cb85afb51809a482d
  requirements/ui/board.md: a47487b383486c1bdd13f1a83d521cad6a53e3bec56618f11efb056d17bf76bd
  requirements/ui/setup.md: 8ae84692bcf9dbc690d67762f83c41aace8121fe24ab88d3a0b88c3566507c90
  requirements/ui/solo-mode.md: 7120832ce855b4e2a7e8f0073c9992258072c4d6e22fbe38894929ba2cc2bde1
created: 2026-11-05T18:58:53.011Z
request: 2026-11-05T19-45-39
label: Atomic commits
job: 04mmi2d4
---
# Commit atomici

Aggiunge i commit atomici: un'opzione per copia di lavoro, spenta di base, con cui Weaver fa un commit git del codice di ogni voce di History.

## Decisioni

- Un commit per ogni voce di History scritta; un aggiornamento della voce che aggiunge Task fa un nuovo commit (mai amend).
- L'agente elenca sempre, con la voce, i file del codice della funzionalità e un messaggio di commit in inglese; Weaver li usa solo quando l'opzione è attiva.
- Weaver fa il commit dei file elencati più quanto lo sviluppatore ha già in stage.
- Fuori da Solo mode il commit contiene anche il file della voce e la rimozione dei file delle Task che registra; con Solo mode attivo solo codice, mai file di Weaver.
- Autore: lo sviluppatore dalla configurazione git; fuori da Solo mode il messaggio chiude con `Co-authored-by: Weaver <weaver@users.noreply.github.com>`, in Solo mode nessun trailer.
- Hook e firma del repository rispettati.
- Commit impossibile o fallito: la voce resta scritta, il comando lo dice all'agente e il Board mostra un avviso col motivo; nulla da committare: niente commit e niente avviso.
- Controlli finali della Run falliti: l'agente lo dichiara, Weaver non fa il commit e il Board lo segnala.
- Percorsi non validi (fuori dal progetto, ignorati da git, inesistenti e mai tracciati) con l'opzione attiva: comando rifiutato, voce non scritta, ogni problema elencato.
- Opzione per copia di lavoro, ricordata in un file locale non versionato di `.weaver/`, come Solo mode; si cambia solo in Settings → Project, interruttore `Atomic commits`, sotto Solo mode.
- La History non mostra i commit.

## Assunzioni dell'agente

- L'interruttore richiede git come Solo mode: fuori da un repository è disabilitato e dice perché.
- Il commit avviene subito dopo la scrittura della voce, fuori dal lock del progetto, un commit alla volta per copia di lavoro, con un limite di 2 minuti (gli hook possono essere lenti).
- Niente commit durante un merge, un rebase o un cherry-pick in corso o con HEAD staccato: vale come commit impossibile.
- Un aggiornamento che non aggiunge Task non fa commit.
- Le regole di qualità che dicevano che Weaver non tocca mai l'indice di git ora fanno eccezione per i commit atomici.
- Le sezioni `history-entry` della riga di comando escono da un frammento vicino ai 12 KB, e le sezioni di qualità di Solo mode e merge vanno in un frammento `git` con quelle dei commit atomici.
- La prima riga del messaggio di commit ha al massimo 72 caratteri; l'agente dichiara i controlli falliti con un campo del comando.
- Lo stato è `.weaver/commits/commits.json`; i file che Weaver ha messo in stage restano in stage se il commit fallisce.
- Un commit fatto dalla riga di comando con l'app chiusa non può mostrare l'avviso sul Board: il risultato è solo nell'output del comando.

## Raffinamento

- `requirements/data/project-files.md` riscritto sul file attuale, che non ha più le sezioni delle card (ora in `card-files.md`): la cartella locale `commits/` si aggiunge accanto a `timings/`, nel layout e nell'elenco delle cartelle locali.
- `requirements/cli/commands.md` riscritto sul file attuale, che ha spostato `question-round`, `job` e `agent` in `agent-commands.md` e aggiunto i comandi di Chat: la sezione `history-entry` passa comunque nel frammento `history`, il resto resta com'è ora.
- `requirements/rules/skills.md` riscritto sul file attuale: i testi History seguono i nuovi testi delle Code notes e di Chat, tutti conservati.
- `model/actors/developer.md` e `requirements/ui/setup.md` riscritti sui file attuali: restano `CLEAR_GENERAL` con lo scarto delle Draft di Chat e il resto delle Settings; si aggiungono solo `CHANGE_ATOMIC_COMMITS` e la riga `Atomic commits` in Settings → Project.
- `requirements/data/settings.md` con la misura dei job e la sezione `atomic-commits` supererebbe i 12 KB: le sezioni `solo-mode`, `solo-merge` e `atomic-commits` passano, con il testo invariato, nel nuovo frammento `working-copy` dell'Area `data`.
