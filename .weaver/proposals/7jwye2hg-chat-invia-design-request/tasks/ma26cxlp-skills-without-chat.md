---
technical: true
refs:
  - requirements/rules#project
---
# Skill senza Chat

Le skill installate non hanno più una sezione per Chat. Quando è fatto deve valere quanto segue.

- `weaver-design` e `weaver-resolve-drifts` non hanno più la sezione `Writing from Chat`. La loro descrizione e le loro regole rigide le applicano solo quando l'istruzione di un job di Weaver le nomina e tengono fuori ogni conversazione che non è un job, Chat e Ask more allo stesso modo. Dicono che in Chat una Proposal parte come design request inviata con `weaver request new`.
- Il resto delle due skill resta com'è: sezione `## Language`, "Sub-agents context sharing", `timeout: 0`, riferimento dei comandi.
- Le copie installate in questo repository (`.agents/skills/`) vengono riallineate con Update skills, non a mano.

## Code notes

Commit: 02129bbb382d41a3a2b2b107bf82f809097051bb

- `crates/core/assets/skills/weaver-design/SKILL.md`: descrizione (riga 3: "…or when the instruction of Chat sends you here…", "(from Chat, with the conversation as the interview)"), prima regola rigida (riga 22, rimando a "Writing from Chat"; l'esclusione di Ask more resta), sezione `## Writing from Chat` (righe 50-57) da togliere.
- `crates/core/assets/skills/weaver-resolve-drifts/SKILL.md`: descrizione (riga 3) e `## Writing from Chat` (righe 46-53); le regole rigide (21-26) non citano Chat.
- Embedding: `crates/core/src/agent_files.rs` (`ASSETS = include_dir!`, l.40). Il test `the_design_and_resolve_skills_have_the_chat_section_and_no_longer_exclude_chat` (~592-617) fissa la sezione: va tolto o riscritto come controllo di assenza. `every_skill_points_to_the_reference…` (~620+) richiede `## Language`, "Sub-agents context sharing" e `` `timeout: 0` `` in ogni skill.
- Nessun'altra skill, `omp/agents/*.md` o `omp/WATCHDOG.yml` cita Chat.
