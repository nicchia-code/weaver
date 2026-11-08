---
refs:
  - general-chat
  - requirements/rules#general-chat
---

# Chat risponde senza dettagli

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
