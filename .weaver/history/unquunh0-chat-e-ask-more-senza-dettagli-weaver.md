---
created: 2026-11-07T23:09:31.310Z
updated: 2026-11-07T23:09:31.310Z
summary: 'Nelle risposte di Chat e di Ask more l''agente usa solo le parole dell''app: niente comandi, file o skill di Weaver, a meno che lo sviluppatore non li chieda.'
tasks:
- zp71avcf
- 0dws07j5
cleared:
  0dws07j5:
    title: Ask more senza dettagli Weaver
    refs:
    - ask-conversation
    - requirements/rules#ask-conversation
    technical: false
    proposal: q0qdsvrn
    label: Chat senza dettagli
    finished: 2026-11-07T22:47:51.953Z
    body: |-
      Nella conversazione di Ask more l'agente parla allo sviluppatore come in Chat: lo sviluppatore conosce Weaver solo attraverso l'app, quindi l'agente usa solo le parole dell'app (Proposal, Task, Drift, la Board, il Model, i Requirements, i job e i controlli dell'app). Le risposte dei job e i report finali restano come oggi; la regola riguarda solo Ask more (e Chat, in un altro Task).

      Quando è fatto, il testo fisso delle istruzioni di Ask more contiene la regola delle risposte allo sviluppatore:

      - di sua iniziativa l'agente non nomina mai ciò che l'app non mostra: i comandi di Weaver, le loro opzioni, il loro input e output JSON e i loro codici di uscita, file e cartelle dentro la cartella di Weaver del progetto, skill, istruzioni di Weaver, variabili d'ambiente e omp;
      - nomina una card con il numero breve e il titolo, come la mostra la Board (per esempio «Proposal <numero breve> – <titolo>»), prendendo il numero breve dalle istruzioni stesse o dai comandi di lettura di Weaver; usa l'identità permanente solo per una card chiusa che non ha numero breve; le richieste di design restano con la sola identità permanente;
      - il motivo di un rifiuto o di un errore di Weaver, come l'avvio rifiutato di un job, lo racconta con parole proprie, nei termini dell'app, senza citare il comando né il suo output; la frase attuale che chiede di spiegare il motivo partendo dall'output va sostituita da questa regola;
      - quando lo sviluppatore deve fare qualcosa, l'agente dice dove e cosa nell'app (la Board, i dettagli di una card, le Settings), mai un comando;
      - solo quando lo sviluppatore chiede esplicitamente dei meccanismi interni di Weaver (un comando, un file della cartella di Weaver, una skill) l'agente risponde con quei dettagli.

      Le altre regole del testo fisso restano invariate (avvio di un job solo su richiesta esplicita, domande solo nelle risposte, ecc.); l'unica frase aggiunta oltre a questa resta quella sulla lingua, e nessuna delle frasi delle istruzioni dei job (limite dei sub-agenti, condivisione del contesto, giri di domande) compare. Le righe degli strumenti per i comandi che l'agente esegue restano come oggi nell'app.

      I test verificano che il testo fisso contenga la regola delle risposte allo sviluppatore.

      ## Code notes

      Commit: bcb006abc30b7b93d0223e6a5d5faf43d3e2eb18

      - `crates/core/src/jobs/ask.rs`: `const INSTRUCTION` (~193-214), elenco puntato che inizia con `Weaver: Ask more`, in seconda persona ("you"). Contiene la stessa lista `weaver job new` e la stessa frase "reason from the output" di Chat, parola per parola: quella frase va sostituita dalla nuova regola. Allinearsi al testo che il Task di Chat mette in `crates/core/src/jobs/general.rs`, se possibile con la stessa formulazione.
      - `instruction(chosen, language)` a :178; `short_line` (~:217) è la convenzione già esistente del "numero breve come lo vede lo sviluppatore".
      - Nessuna prima persona nel testo (regola dei testi di Weaver).
      - Test in `ask.rs` (~:655+): aggiungere la regola alla lista di `every_text_starts_with_the_line_and_holds_the_fixed_rules`; `none_of_the_sentences_of_a_job_instruction_is_there` deve continuare a passare. `src-tauri/src/ask/tests.rs:109` controlla solo le righe iniziali.
      - Nessuna modifica UI: le righe strumento restano invariate.
  zp71avcf:
    title: Chat risponde senza dettagli
    refs:
    - general-chat
    - requirements/rules#general-chat
    technical: false
    proposal: q0qdsvrn
    label: Chat senza dettagli
    finished: 2026-11-07T22:48:04.064Z
    body: |-
      Lo sviluppatore conosce Weaver solo attraverso l'app, quindi nelle risposte della Chat l'agente parla soltanto con le parole dell'app: Proposal, Task, Drift, la Board, il Model, i Requirements, i job e i controlli dell'app. Le risposte dei job e i resoconti finali restano come oggi.

      Quando il lavoro è finito, l'istruzione fissa della Chat contiene una nuova regola, dopo quelle esistenti, che dice all'agente:

      - di propria iniziativa non nomina mai ciò che l'app non mostra: i comandi di Weaver, le loro opzioni, il loro input e output JSON e i loro codici di uscita, i file e le cartelle dentro la cartella di Weaver del progetto, le skill, le istruzioni di Weaver, le variabili d'ambiente e omp;
      - nomina una card come la mostra la Board, con numero breve e titolo (nella forma `Proposal <numero breve> – <titolo>`), prendendo il numero breve dai comandi di lettura di Weaver; usa l'identità permanente solo per una card chiusa che non ha numero breve. Le richieste di design continuano a usare solo identità permanenti, come già dice la regola sulla scrittura della Proposal;
      - quando Weaver rifiuta qualcosa o fallisce, spiega il motivo con parole proprie e nei termini dell'app, senza citare il comando né il suo output;
      - quando lo sviluppatore deve fare qualcosa, indica dove e cosa nell'app (la Board, la card della Proposal in Chat, le Settings), mai un comando;
      - solo quando lo sviluppatore chiede esplicitamente dei meccanismi interni di Weaver (un comando, un file, una skill) risponde con quei dettagli.

      Le regole sulla scrittura della Proposal e sull'avvio dei job, dove oggi dicono di riferire il motivo del rifiuto dall'output del comando, ora rimandano alla nuova regola per come riferire il motivo; il resto resta invariato, compreso il divieto di scrivere la Proposal o avviare il job in un altro modo.

      L'istruzione continua a non contenere numeri brevi reali, testi di card o di richieste, né la prima persona (l'esempio usa il segnaposto del numero breve). L'elenco delle chiamate di avvio dei job condiviso parola per parola con Ask more resta intatto. Le righe dei comandi eseguiti dall'agente nella Chat restano come sono: nessun cambiamento all'interfaccia.

      I test dell'istruzione della Chat verificano anche la nuova regola, nell'ordine, insieme a quelle già controllate.

      ## Code notes

      Commit: bcb006abc30b7b93d0223e6a5d5faf43d3e2eb18

      - Testo fisso: `crates/core/src/jobs/general.rs` ~l.644-664, `const INSTRUCTION: &str` (stringa con continuazioni `\` ed escape `\"`). Modificare l'ultima frase del punto 5 e il punto 10 ("tells the developer the reason as point 11 says"), aggiungere il punto 11 in coda alla lista numerata, prima delle righe aggiunte da `instruction(max_agents, language)` (l.627: `max_agents_sentence`, `CONTEXT_SHARING_SENTENCE`, `NO_SKILL_SENTENCE`, riga lingua).
      - Test: stesso file ~l.1426-1510, `the_instruction_of_chat_holds_the_rules_in_order_and_the_lines_of_the_settings`: estendere l'array `points` con una sottostringa esatta del punto 11 e aggiungere `contains` per le parti chiave (nome della card, rifiuti senza output, controlli dell'app, eccezione su richiesta esplicita).
      - Insidie: il controllo prima persona cerca `" i "`, `" we "`, `" my "`, `"i'm"`… sul testo minuscolo e righe che iniziano con `"I "`; nessun numero breve reale (usare `<short number>`); la Chat non va mai chiamata "General" nel testo.
      - L'elenco `weaver job new` è condiviso parola per parola con `INSTRUCTION` in `crates/core/src/jobs/ask.rs` (~l.193-214): non toccarlo.
      - Test app che devono restare verdi: `src-tauri/src/general/tests.rs` ~l.202-217 (inizia con `# Weaver Chat`, contiene `weaver job new`), `src-tauri/src/general/round_tests.rs` ~l.129-145.
---
# Chat e Ask more senza dettagli Weaver

Il testo fisso delle istruzioni di Chat e quello di Ask more contengono la stessa regola delle risposte allo sviluppatore: l'agente non nomina di sua iniziativa ciò che l'app non mostra (comandi, file della cartella di Weaver, skill, istruzioni, variabili d'ambiente, omp), nomina le card con numero breve e titolo, racconta con parole proprie il motivo di un rifiuto o di un errore, dice dove e cosa fare nell'app e scende nei meccanismi interni solo se lo sviluppatore lo chiede esplicitamente. I test verificano la presenza della regola.
