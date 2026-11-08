---
technical: true
refs:
  - requirements/quality#General
---
# Testi senza omp

Fuori dai Settings l'interfaccia non nomina omp: Weaver non nasconde di usarlo, ma non lo ripete ovunque.

Quando la Task è finita:
- Il nome `omp` compare solo nella vista Settings (gruppo `omp`, riquadro di stato, provider, elenco dei modelli) e in ciò che omp o l'agente scrivono da sé (messaggi, titoli dei tool, righe d'errore sotto `Show details`).
- Altrove l'agente si chiama `Weaver` quando parla o agisce (per esempio `Message Weaver…`, `Weaver asks permission`, `Starting Weaver…`) e `the agent` in stati e motivi (per esempio `The agent is not installed.`, `the agent did not answer within 30 s`, `the agent exited with code <N>`).
- I controlli che agiscono su omp fuori dai Settings restano, con un nome generico (`Install the agent`, `Log in`): la finestra di benvenuto, Get Started, la Board con Model vuoto, la Chat, le conversazioni Ask more e la finestra di Usage non mostrano più il nome.
- I motivi calcolati dal backend e mostrati sia nei Settings sia altrove (fallimenti di avvio, uscita o risposta, motivi del proxy, `Not delivered: <reason>.`, lettura di Usage non valida, notifica dei certificati di sistema) sono scritti con `the agent`.
- Un testo dato da un altro Requirement che nomina omp fuori dai Settings si mostra con questa regola applicata.
- I test unitari che verificano questi testi sono aggiornati.

## Code notes

Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- Riquadro di stato condiviso: `ui/src/components/omp-status.tsx` (`boxFace()` 50-116, `aria-label="omp status"` 159, 192, 235, 259, 274); fuori dai Settings lo usano `usage-dialog.tsx:7,137`, `welcome-window.tsx:7` (`OmpSetup`) e `project/empty-model.tsx:8,56`. Serve una variante di testi per i luoghi fuori dai Settings.
- Letterali: `general/general-view.tsx` (EMPTY 20, 100, 113-115, 126-127), `ask/ask-conversation.tsx:76,82-84`, `jobs/jobs-view.tsx:234-235,286-287,428`, `usage-dialog.tsx:23,176`, `system-certificates-banners.tsx:11` (`TURN_ON_TEXT`), `lib/system-certificates.ts:37`.
- Backend (`crates/core`): `notification.rs:33`, `jobs/general.rs:154`, `omp/failure.rs` (66, 69, 100-104, 123-131, 135-136, 280, 285, 312, 315, 316), `omp/lookup.rs:118`, `omp/probe.rs:136,155-157`, `omp/machine.rs:587`, `omp/usage.rs:20`, `jobs/measure.rs:233,236`, `omp/terminal.rs:64,258`. In `src-tauri/src`: `jobs/proc.rs:122,144,280-281`, `jobs/rpc.rs:53,258`, `jobs/session.rs`, `jobs/actor.rs:205-212,492`, `general/actor.rs:317,320,560`, `subagents/reader.rs:83`, `omp.rs:398,871`.
- Test con questi testi: `general-view.test.tsx:86-155`, `ask.test.tsx:209-214`, `jobs-view.test.tsx:101,111`, `usage-dialog.test.tsx:74-82,192,276-277`, `system-certificates-banners.test.tsx:86,195`, `lib/system-certificates.test.ts:57`, `omp-status.test.tsx:96-131`, `welcome-window.test.tsx:241`.
