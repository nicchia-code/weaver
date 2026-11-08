---
status: approved
kind: change
base:
  model/agent-work/ask-conversation.md: f82212aa25950dddfd9fa327c3db209610560b48bb72eeaf867ba5923d8d93ce
  model/agent-work/general-chat.md: a8ea453d16de07059361a93eeca54761a58b5fa688a8d8dbf8f43dc2ecdaa4c0
  requirements/rules/chat-instructions.md: 59a1ebe06f7a1afe5d6b895207ae8cc60011e6332e97a694561c4ffa1ca12166
  requirements/rules/instructions.md: 341910827d7a99ae835770294303883dacfaaa37edebbf531dba9e34c8729bba
created: 2026-11-07T22:33:36.553Z
request: 2026-11-07T23-27-19
label: Chat senza dettagli
job: 9lkknqni
---
# Chat senza dettagli Weaver

Il developer non deve conoscere Weaver dall'interno: nella Chat, e anche in Ask more, l'agente gli parla solo con le parole dell'app e non nomina i dettagli tecnici di Weaver che il developer non vedrebbe.

## Decisioni

- Portata: la regola vale per la Chat e per Ask more; le risposte e i resoconti dei job restano come oggi.
- Cosa non si nomina: tutto ciò che l'app non mostra (comandi `weaver`, opzioni, input e output JSON, codici di uscita, file e cartelle sotto `.weaver/`, skill, istruzioni di Weaver, variabili d'ambiente, omp); l'agente parla con le parole dell'app.
- Card: nominate con numero breve e titolo, come sul Board; l'identità permanente solo per una card chiusa, che non ha numero. Le design request continuano a usare solo identità permanenti.
- Domanda esplicita del developer sui dettagli interni: l'agente risponde, solo allora.
- Rifiuti ed errori di Weaver: l'agente ne spiega il motivo a parole sue, con le parole dell'app, senza citare il comando né il suo output.
- Cosa deve fare il developer: indicato con i controlli dell'app (Board, card della Proposal in Chat, Settings), mai con un comando.
- Le righe dei comandi eseguiti dall'agente restano come sono, in Chat e in Ask more.
- I test dell'istruzione della Chat e di quella di Ask more verificano anche la nuova regola.
