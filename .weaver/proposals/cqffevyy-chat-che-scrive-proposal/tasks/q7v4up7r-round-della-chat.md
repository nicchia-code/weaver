---
technical: false
refs:
  - question-round
  - requirements/cli#question-round
  - requirements/tech#question-round
  - requirements/ui#question-round
  - requirements/rules#ask-conversation
---
# Round di domande della Chat

`weaver question new` oggi è accettato solo dall'agente di un job (`WEAVER_JOB`). Deve essere accettato anche dall'agente della Chat (General) del progetto, mentre un suo turno è in corso. La Chat usa un round solo quando il developer chiede di essere interrogato così: di norma fa le domande nelle risposte.

## Cosa deve valere

**Model `question-round`**, stato `Not asked`, `agent.ASK_QUESTIONS`:
- → `Unavailable`: quando il chiamante non è né l'agente di un job di Weaver al lavoro né l'agente della Chat del progetto durante un turno, oppure Weaver non è in esecuzione.
- → `Queued`: quando un altro round dello stesso progetto è presentato.
- → `Presented`: altrimenti; la finestra del progetto viene in primo piano.

Un round appartiene al suo job, oppure alla Chat, e al suo progetto; non compare mai nella finestra di un altro progetto. Gli altri stati (Queued, Presented, Answered, Abandoned, Interrupted, Unavailable) restano come sono per entrambe le origini.

**Instradamento e trasporto (`requirements/tech#question-round`):**
- La CLI invia `{secret, job, general, request}`, con le identità che ha (`WEAVER_JOB` e/o `WEAVER_GENERAL`).
- Si instrada solo per `WEAVER_JOB` o, senza di essa, per `WEAVER_GENERAL`; mai per cwd, focus o percorso. I round da sotto-agenti e da git worktree arrivano al progetto del job o della Chat.
- L'app accetta un round solo per un job che ha lanciato, Running o Waiting for the developer, oppure per la Chat viva di un progetto aperto mentre un turno è in corso. Un'identità della Chat non riconosciuta conta come nessuna identità.
- `unavailable` immediato quando: mancano sia `WEAVER_JOB` sia `WEAVER_GENERAL`, o manca `WEAVER_ENDPOINT`; il file di runtime manca o indica un altro endpoint; la connessione fallisce; la versione del protocollo differisce; il job è sconosciuto o finito, oppure l'identità della Chat non è quella viva o non ha un turno in corso.
- Un round della Chat porta la Chat in `Waiting for answers` e la riporta indietro quando il round si chiude: dal lato rounds questo vuol dire avvisare la Chat del progetto all'apertura (`OPENED`) e alla chiusura (`CLOSED`) del round. Lo stato `Waiting for answers` della Chat, il numero `1` sulla voce Chat della barra laterale e la fine del round con Stop, Clear, uscita di omp o chiusura della finestra appartengono al Task di `general-chat`; qui serve solo l'aggancio (un `Effect::Opened`/`Effect::Closed` con origine Chat deve raggiungere la Chat del progetto, non il registro dei job).

**CLI (`requirements/cli#question-round`):**
- `unavailable`: "neither in a working Weaver job nor in the project's Chat during a turn, or no app answers before submission". Il reason resta "Weaver cannot show questions for this command. Do not ask in any other way: stop and tell the developer."
- `cancelled` (exit 1): SIGINT/SIGTERM o stdout chiuso; il round è ritirato. Stop, Clear e un'uscita dell'omp della Chat terminano il comando così.
- `--help` dice inoltre: in Chat, solo quando il developer ha chiesto di essere interrogato con un round. Anche il testo dei risultati in `--help` e la nota di accesso (oggi "Only the agent of a job Weaver started can use it") devono nominare la Chat durante un turno.

**UI (`requirements/ui#question-round`):** intestazione `Questions from the agent · Chat` per un round della Chat (invariati `Questions from the agent · <job name>` e `Question about <D3> · <Drift title>`).

**Ask more (`requirements/rules#ask-conversation`, parte **Question**):** dopo le domande, l'istruzione nomina il job che ha chiesto il round (nome e identità), `Chat` per un round della Chat, o l'identità del Drift per una domanda preparata. Dal lato backend (`requirements/tech#ask-conversation`) si legge dal round presentato anche l'origine Chat. Ask more per il resto non cambia.

## Dove nel codice
- `crates/core/src/question/round.rs`: `Origin` riceve una variante per la Chat (con l'identità del progetto/Chat); `Origin::job()` non la tratta come job.
- `crates/cli/src/round.rs` (l.54–75): oggi `unavailable` senza `env.job`; deve accettare anche `env.general` e mettere nella richiesta `general`. `crates/cli/src/client.rs` (campo della richiesta), `crates/cli/src/commands/question.rs` (testi `--help`), `crates/cli/src/help.rs` l.107 e `Access::JobOrUnavailable` in `crates/cli/src/spec.rs`.
- `src-tauri/src/cli_handler.rs` (costruzione della richiesta `question.new`) e `src-tauri/src/rounds/service.rs::handle` (l.266–): senza `request.job`, riconoscere l'identità della Chat tramite il servizio General (`src-tauri/src/general/service.rs`, campo identità) come Chat viva del progetto con turno in corso; `carry_out` instrada `Opened`/`Closed` con origine Chat verso la Chat; `src-tauri/src/rounds/host.rs` riceve gli agganci verso General.
- `src-tauri/src/rounds/view.rs::OriginView` e `ui/src/components/questions/question-model.ts` (l.14): intestazione `Questions from the agent · Chat`.
- `src-tauri/src/ask/build.rs` (l.300, `Origin::Agent`): ramo per la Chat che scrive `Chat`.

## Test
- `src-tauri/src/rounds/service/tests.rs`: round accettato dalla Chat con turno in corso; `unavailable` per identità della Chat non viva o senza turno; aperture/chiusure notificate alla Chat.
- Test della CLI su `round.rs` e `tests/cli-contract/contract.test.mjs` (l.410–473): il caso "senza `WEAVER_JOB` → unavailable" va cambiato in "senza `WEAVER_JOB` né `WEAVER_GENERAL`", e i controlli sul testo di `--help`.
- `ui/src/components/questions/question-overlay.test.tsx`: intestazione per un round della Chat.
- `src-tauri/src/ask/tests.rs`: istruzione Ask more di un round della Chat contiene `Chat`.
