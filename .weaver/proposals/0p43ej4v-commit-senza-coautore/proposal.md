---
status: approved
kind: change
base:
  model/project-setup/atomic-commits.md: 02e006496da16214a368aded8f3c8c94c99ebda090f66f2d2203b044ba07ebb7
  requirements/docs/main.md: 79fb39cf523936312f0e9f87b4cb5df065bf5a9251c173d7217c54e59919c6b7
  requirements/quality/git.md: 01938d19e29cf135db3d2a37bdd5ca505adfd25c3b9b28f85b33e9606151fec4
  requirements/rules/solo-mode.md: f4629af25dc1b63cdc72db7f0889e5dc47dfc608e15576da75f475a807f74fc4
created: 2026-11-08T08:42:21.173Z
request: 2026-11-08T09-39-47
label: Atomic commits
job: 1vmpb8b1
---
# Commit senza coautore

I commit che Weaver crea per le voci della History (Atomic commits) non portano più la riga `Co-authored-by: Weaver <weaver@users.noreply.github.com>`: su GitHub quell'indirizzo appartiene all'account `weaver`, un utente estraneo, che compariva come coautore di ogni commit.

## Decisioni

- Nessuna riga di coautore: il messaggio del commit è solo quello dell'agente, fuori e dentro Solo mode.
- Autore e committer restano quelli della configurazione di git.
- Il resto del comportamento dei commit resta com'è: cosa entra nel commit nelle due modalità, hook, firma, motivi di commit saltato.
- I test verificano che in entrambe le modalità il messaggio sia esattamente quello dell'agente; i loro nomi non parlano più di trailer.
- Il README, nella sezione Atomic commits, dice che il messaggio è quello scritto dall'agente in ogni modalità, senza nominare il coautore.
- Il CHANGELOG aggiunge una riga sotto `## [Unreleased]`, `Changed`.
- I commit già fatti restano come sono; il CHANGELOG non ne parla.
