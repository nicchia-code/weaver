---
status: approved
kind: change
base:
  model/agent-work/job.md: b2f92607c66e928ed8910e8f3a44a12ebcaede01fd0dbd618249fe5959b238b4
  requirements/cli/commands.md: 1d9fd06293e6feb0867cd55164469e80280ded97a248247ccc0ac21834ddb792
  requirements/data/project-files.md: 8792975dfb02f787bf1dacfce8cca5d91ba53d95a8bbc7b9b347fba6dd86e8de
  requirements/data/settings.md: be6eabc705cbf000035ffa4756c104912d4f453c5b0e134966b69ad208e90a1e
  requirements/tech/acp.md: 8a8f3c027a8a56988717ac27068a97438c3a1145cc02e087870d730603c09493
created: 2026-11-05T18:46:57.071Z
request: 2026-11-05T19-32-07
label: Misura dei job
job: bf1h0l1t
---
# Misura dei job

Per confrontare il tempo e il consumo di design e run prima e dopo le note di codice nelle Task, Weaver misura ogni job di design e ogni run quando finisce e lo registra in `.weaver/timings`. Questa Proposal viene prima di quella sulle note di codice, così si raccolgono dati di partenza con lo stesso metodo prima che le skill cambino.

## Decisioni

- Job misurati: design, rework, refinement e run (una Task o Run all); nessun altro tipo di job.
- Fonte: a fine job Weaver somma i file di sessione di omp, agente principale e ogni sub-agente a qualunque profondità; nessun conteggio dal canale ACP e nessun numero dichiarato dall'agente.
- Cosa si registra: durata totale e durata attiva (solo il tempo in Running: senza attese dello sviluppatore, coda e sospensioni); letture dello strumento `read` e file distinti letti; ricerche (`grep`, `glob`); token in ingresso, in uscita, letti e scritti in cache; separati fra agente principale e sub-agenti.
- Letture: conta solo lo strumento `read` di omp; i comandi di shell non contano.
- Esito: si registrano anche i job Failed e Stopped, con lo stato finale.
- File di sessione illeggibili: la riga c'è comunque, con durate ed esito, e letture e token segnati come non disponibili con il motivo.
- Dove si vede: in `.weaver/timings` e in `weaver job show <id>`; nessuna UI nuova.
- `.weaver/timings` è una cartella solo locale con `.gitignore` `*`; Weaver non cancella mai le righe.
- Crash o chiusura forzata di Weaver mentre il job lavora: il tempo dall'ultimo cambio di stato al crash non si conta e la riga porta `activeIncomplete`, così i confronti possono escluderla.

## Scelte dell'agente da verificare

- Un solo file `.weaver/timings/jobs.jsonl`, una riga JSON per job finito; gli altri file della cartella (i vecchi file di Trama) non vengono mai letti né toccati.
- Gli advisor di omp nei run sono una terza parte separata (`advisors`), perché il loro costo non dipende dalle note di codice.
- Un file letto più volte con selettori di righe diversi conta come un solo file distinto.
- Il record del job nel registro tiene il tempo già passato in Running, così un job sospeso e ripreso continua a contarlo.
- La misura non ritarda mai la fine del job e non ne cambia l'esito.
- I testi installati per l'agente non parlano della misura: l'agente non deve fare nulla.
