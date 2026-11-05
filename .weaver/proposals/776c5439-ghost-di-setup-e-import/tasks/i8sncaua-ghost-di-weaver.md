---
technical: false
refs:
  - job
  - requirements/ui#work
  - requirements/data#job
---
# Ghost di Weaver per setup e import

Mentre lavora un job di setup (Start from Scratch) o di import (Import from Code), la Board mostra sempre almeno un ghost di quel job. Oggi il ghost compare solo quando l'agente annuncia una card, e per un import questo arriva dopo la lettura di tutto il codice e il giro di domande, mentre le card di Empty Model sono già nascoste.

## Cosa deve valere alla fine

- **Durata:** da quando il job di setup o di import viene creato (in coda, prima che omp apra la sessione) fino alla sua fine, anche mentre è sospeso, quando il job non ha nessun ghost annunciato sulla Board Weaver mostra in Backlog un ghost suo, senza nessun annuncio dell'agente. Gli altri tipi di job non cambiano, compreso il job di merge conflicts.
- **Aspetto:** tipo `Proposal`; titolo `Setup` per un setup e `Import from Code` per un import (testo nuovo: il nome del job import resta `Import`); riga di fase `Waiting for your answers` mentre il job aspetta un giro di domande o un permesso, altrimenti `Preparing`, anche in coda e da sospeso, mai `Writing`; sotto, l'ultima nota del job come sugli altri ghost. Stesso scheletro, stesso pulsante che apre la conversazione del job in Jobs, stesso nome accessibile (`Proposal being prepared by the agent: Setup, Preparing`); non conta nel numero di card della colonna.
- **Alternanza con i ghost annunciati:** nella stessa picture in cui compare il primo ghost annunciato del job, il ghost di Weaver sparisce; ricompare nella stessa picture in cui sparisce l'ultimo ghost annunciato del job, finché il job non è finito (per esempio mentre un import prepara le domande dei Drift, o mentre un setup su Model vuoto chiude il turno dopo l'approvazione automatica). Un ghost annunciato resta finché la sua card non è sulla Board: per una Proposal finché non viene consegnata (la Draft mostra `Writing`), quindi il ghost di Weaver resta assente finché quella Proposal non è consegnata. Mai entrambi assenti, mai il ghost di Weaver insieme a un ghost annunciato dello stesso job.
- **Fine:** sparisce quando il job finisce, per qualsiasi motivo (Done, Failed, Stopped), nella stessa picture degli altri ghost del job.
- **Ordine:** tra i ghost sta nel posto che avrebbe se fosse stato annunciato quando il job è stato creato; con più job, un job creato prima sta sopra.
- **Identità:** derivata dal job, stabile tra una picture e l'altra (la chiave della lista e il focus non cambiano quando cambiano solo fase o nota), e mai uguale all'identità di un annuncio, che è di 8 caratteri casuali (per esempio un prefisso come `job:<id del job>`). Non è un annuncio: non viene salvato nel record del job.
- **Riavvio:** il record del job in `jobs/<job id>.json` conserva per ogni annuncio anche la card creata con esso, `card` `{kind, id}`, scritta quando la card viene creata con quell'annuncio. Oggi questa informazione sta solo in memoria: dopo un riavvio di Weaver (job sospeso e poi ripreso) i ghost di card già arrivate ricompaiono accanto alle loro card fino alla fine del job e il ghost di Weaver non tornerebbe mai. Dopo questo Task, alla ripresa Weaver sa quali card annunciate sono già arrivate, per ogni tipo di job; gli annunci restano ritirati tutti insieme alla fine del job.
- Il resto dei ghost (Writing, Being refined, segnaposto, animazione, ghost che lascia il posto alla card nella stessa picture) non cambia; l'interfaccia disegna già un ghost `proposal` in Backlog senza modifiche.

## Dove nel codice

- `job` — `src-tauri/src/jobs/registry.rs`: `ghosts_of` (circa l. 452-499) costruisce i ghost dei job attivi, dove va aggiunto il ghost di Weaver usando `record.kind` (`JobKind::Setup`/`Import` in `crates/core/src/jobs/kinds.rs`); fase calcolata lì (Waiting prima di tutto).
- `job` — `src-tauri/src/jobs/registry.rs`: `announce` (circa l. 1126), `card_filed` (circa l. 1162, oggi scrive solo `inner.claims` in memoria), `card_is_on_the_board` (circa l. 1480), fine del job che svuota gli annunci (circa l. 785-791), `adopt_records` che ricostruisce l'ordine dopo un riavvio (circa l. 1325-1375), `next_announcement`/`announced` per l'ordine.
- `requirements/data#job` — record del job (`AnnouncementRecord` e il record in `src-tauri/src/jobs/registry.rs`), scritto atomicamente a ogni cambiamento.
- `requirements/ui#work` — `src-tauri/src/agent_work.rs` (`Ghost`, `GhostKind`, `GhostPhase`, contratto "mai entrambi, mai nessuno"), `src-tauri/src/board.rs` (`snapshot`, `publish_agent_change`), `ui/src/components/board/ghost-card.tsx` e `board-view.tsx` (disegnano già il ghost, chiave `ghost.id`).
- Nomi dei job: `crates/core/src/jobs/names.rs` (`SETUP`, `IMPORT`); il titolo `Import from Code` è un testo nuovo.
- Test esistente da estendere: `src-tauri/src/jobs/tests.rs`, `notes_announcements_and_ghosts_follow_the_job_until_the_card_arrives_or_the_job_ends` (circa l. 849-930).
