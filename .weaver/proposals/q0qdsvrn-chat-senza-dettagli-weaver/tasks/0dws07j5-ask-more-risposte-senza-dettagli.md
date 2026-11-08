---
refs:
  - ask-conversation
  - requirements/rules#ask-conversation
---

# Ask more senza dettagli Weaver

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
