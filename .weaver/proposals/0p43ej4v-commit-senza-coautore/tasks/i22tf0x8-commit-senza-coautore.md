---
technical: false
refs:
  - atomic-commits
  - requirements/rules#atomic-commits
  - requirements/quality#atomic-commits
  - requirements/docs#atomic-commits
---

# Commit senza coautore

I commit di Atomic commits non nominano più Weaver come coautore. Il messaggio di ogni commit è soltanto quello scritto dall'agente, con o senza Solo mode: nessuna riga `Co-authored-by` né altro trailer, né fuori né dentro Solo mode.

Autore e committer restano quelli della configurazione di git: l'autore è lo sviluppatore. Tutto il resto dei commit resta com'è: cosa entra in un commit in ciascuna modalità (i file nominati dall'agente, quanto lo sviluppatore ha già messo in stage e, fuori da Solo mode, la voce di History e i file rimossi dai suoi Task; con Solo mode attivo, nessun file di Weaver), gli hook e la firma del repository, che si applicano come a qualsiasi commit, e le ragioni per cui un commit viene saltato. Weaver non fa mai push, non modifica né riscrive mai un commit.

I test verificano che il messaggio sia esattamente quello dell'agente, sia con Solo mode spento sia acceso, senza riga `Co-authored-by`, e che autore e committer vengano dalla configurazione di git. I nomi dei test non parlano più di un trailer.

Il README, nella sezione sui commit atomici, dice che il messaggio è quello scritto dall'agente in ogni modalità e che l'autore è lo sviluppatore come configurato in git, senza nominare il coautore. Il CHANGELOG aggiunge sotto `## [Unreleased]`, in una nuova sezione `### Changed`, una riga che dice che i commit di Atomic commits non nominano più Weaver come coautore: il loro messaggio è solo quello dell'agente.

I commit già fatti restano come sono e il CHANGELOG non li menziona.

## Code notes

Commit: da55d220016f8cc433a5d942179d798bcf5c74d8

- `crates/core/src/history_commit.rs:40-41`: `pub const TRAILER` non è usata altrove (crates, src-tauri, ui, e2e): eliminarla.
- `try_commit` (~517-522): togliere solo il blocco `if !solo { … TRAILER }`; tenere `trim_end()` e il singolo `'\n'` prima di `git commit --cleanup=strip -F -`. `solo` (riga ~494) resta usata dallo staging (497-509), quindi nessuna variabile inutilizzata.
- Test in `mod tests`: `a_removed_tracked_file_is_committed_as_a_removal_with_the_entry_and_a_trailer` (770-811; l'assert 799-803 diventa `"Add the cart"`, autore/committer `Dev Eloper <dev@example.com>` già verificati a 804-808) e `with_solo_mode_on_no_file_of_weaver_goes_in_and_there_is_no_trailer` (865-889): rinominare senza "trailer". Helper: `available`, `git`, `put`, `repository`, `turn_on`, `input`, `pending`, `committed`, `last_files`; messaggio letto con `git log -1 --format=%B` + `trim_end()`.
- `README.md:58`: riscrivere il bullet "What goes into a commit" e aggiungere quello sul messaggio/autore.
- `CHANGELOG.md`: `## [Unreleased]` è vuoto, creare `### Changed`.
- Non modificare a mano `.agents/skills/weaver-req-*` (rigenerate) né i record in `.weaver/history/` e `.weaver/proposals/`.
