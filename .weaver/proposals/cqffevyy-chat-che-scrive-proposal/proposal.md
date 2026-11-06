---
status: approved
kind: change
base:
  model/actors/agent.md: df2f7ed9c99023162a3f311aa238caa7d6651d5f4e0da9694fea398750061862
  model/actors/developer.md: 0569aa75ccafcf7620e45b120be2a392391397860100ff2714db41435b8e0db3
  model/agent-work/general-chat.md: 34f26396079caaa98d9386059238bb81ee42a4bc1924663593ac760ef8152ec0
  model/agent-work/question-round.md: 6c09ba3eaed3c05b8bb7fadc5a668c7b84095554433931f4073a8f5c3c9840f0
  model/app/notification.md: 5b5194ebd4d4f7396c2a77fe5212d7d4583320e294a804fce0f068a63eb35b57
  model/model-changes/design-request.md: f4d051837cf8f537117ba1fb328876f59b0b0f562e72efb0b2fd06f8603b4879
  model/model-changes/proposal.md: fc29bd07099d51dfec53264b8eb6223c869c35dd72dd70076267de805b6d1cc8
  requirements/cli/agent-commands.md: new
  requirements/cli/commands.md: 2def5e510b307630a4aae783a8db0f5fd2d92600baf5b8f6eae28a7f5cf1016c
  requirements/data/card-files.md: 33ef9ceae9993a26d1e73808f6b0db862c6b6bb6c18b7f392c3251170ede04f3
  requirements/rules/cards.md: 58853e67a75d923dcccc19c445c557024a06fcef0107823f7bc6f8279b4569fb
  requirements/rules/chat-instructions.md: new
  requirements/rules/instructions.md: 23eb3f05547cc5162872d73768fe43a65b87eb33bdec4933b833673f525a94ba
  requirements/rules/skills.md: 263e0098e616c1dbfaaf5cb0c4d575d8145ccc80b4f286c1d016d9c994b8aee4
  requirements/tech/acp.md: 924729b6d0164b70a55016bdbc2c34e6c7d4dcb127e6fa1937f83691d65e5a17
  requirements/tech/architecture.md: 1c0daa8c6781119bba74542a5cf9d51c2076529d5d9348e2441b52e6a9854a30
  requirements/ui/board-cards.md: acaca6de7917060fc2ca8c85bd410e2f5c91bbcc85828423f526f624c6231ff5
  requirements/ui/conversations.md: d38be8f0719a248657c63978107c5eed2c411495586e0964815d3bef1c7e154c
  requirements/ui/look.md: 7766caec4416efb81954927429474d5124703db91a0da4e2ec23158eda773109
  requirements/ui/rounds.md: c3c3930b1de0176289b6e93112f9f8c71701151ec6fd5ddd1ebf9b4ea8ec3156
  requirements/ui/setup.md: 532b3970ca6ccdb9f5cf67b358a3b816c6dd44ff0913778486ae19b97289d550
created: 2026-11-05T18:49:59.906Z
request: 2026-11-05T19-36-38
label: Chat Proposals
job: eizjj3ip
---
# Chat che scrive Proposal

Oggi la Chat (General) ha un'istruzione che le vieta di scrivere Proposal e di caricare `weaver-design`: il ragionamento fatto in Chat si perde e il developer deve scrivere una richiesta e farsi intervistare di nuovo da un job di design. Questa Proposal riporta la Chat a strumento di brainstorming, come era in Trama, che su ordine esplicito del developer scrive ciò che è stato concordato.

## Decisioni

- Comportamento: brainstorming come in Trama; l'agente legge liberamente, non scrive nulla né in Weaver né nei file del progetto senza un ordine esplicito (un desiderio, un problema o un'idea non sono un ordine), quando c'è qualcosa da mettere per iscritto lo dice in una riga e aspetta, non modifica mai `.weaver/` a mano.
- Su ordine esplicito la Chat scrive Proposal di modifica con i loro Task e Proposal di risoluzione di Drift, e inoltre singoli Task, Drift e decisioni del developer; mai setup né import.
- La Chat resta fuori dai job: nessuna coda, nessun rifiuto per altri job, nulla sulla Board finché parla soltanto.
- Una bozza scritta dalla Chat ha una scheda propria sulla Board, con la struttura di una scheda Open, i segni `draft` e `Chat`, `Open Chat` e `Discard`, senza `Approve` né `Reject`.
- Mentre un turno della Chat è in corso, le Proposal che la Chat ha creato o sta scrivendo non si possono approvare, rifiutare né scartare (`The agent is still finishing`).
- Clear scarta le bozze create dalla Chat, e il dialogo di Clear lo dice; Stop, uscita di omp, chiusura della finestra e uscita da Weaver le lasciano, perché la conversazione resta.
- La Chat può riaprire una Proposal non sua: finché è Draft ha la scheda della Chat e la protezione durante il turno, ma Clear non la scarta.
- Regole e formato: la Chat carica `weaver-design` (o `weaver-resolve-drifts` per una risoluzione), che ricevono una sezione `Writing from Chat`; la conversazione vale come intervista; poi file, Task, `Already present`, consegna e scansione leggera del codice come un job di design.
- Modello: quello di analisi, come oggi.
- Domande: nelle risposte; con `weaver question new` solo quando il developer lo chiede. Un round della Chat la mostra in attesa (numero `1` sulla voce Chat della barra laterale) e notifica a finestra in secondo piano, come un job.
- Vincoli: la Chat può legare la Proposal a una richiesta scritta non inviata, che resta bloccata (sola lettura, banner `Chat is writing a Proposal from this request`) finché la bozza esiste e torna modificabile se la bozza viene scartata; può prendere il posto di qualsiasi Proposal di modifica o risoluzione rifiutata, con o senza Rework, che nessuno ha sostituito e su cui non lavora un job di rework.
- Ask more resta com'è.
- L'istruzione della Chat riceve il limite di sotto-agenti e la frase sulla condivisione del contesto, come i job.
- Nuovo testo della Chat vuota: `Think with omp about this project. Ask it to write a Proposal or start a job of Weaver when you want one.`
- Nessuna riga nella conversazione per la Proposal scritta: basta la scheda sulla Board.

## Scelte dell'agente da verificare

- Il segno sui file della Proposal: `chat: true` permanente per quelle create dalla Chat e `chatWriting: true` finché la Chat le scrive, tolto alla consegna.
- La voce Board della barra laterale non conta le schede Draft della Chat.
- Il controllo dei vincoli (`request`, `reworks`) diventa effettivo per tutti i chiamanti: un job lega solo la richiesta della sua istruzione, un chiamante che non è né un job né la Chat non lega nulla; un vincolo non permesso crea la Proposal senza vincolo, con una nota.
- Un round della Chat finisce senza risposte con Stop, Clear, uscita di omp o chiusura della finestra.
- Il dialogo di conferma di `Discard` sulla scheda Draft: `Discard this Draft?` / `Its files and Tasks are thrown away.`
- Per un singolo Task o Drift la Chat segue l'`--help` del comando e le regole di scrittura di `weaver-design` per i Task e di `weaver-compare` per i Drift.
- Per stare sotto i 12 KB, il frammento `cli/commands` è diviso spostando `question-round`, `job` e `agent` nel nuovo `cli/agent-commands`; le regole dell'istruzione della Chat sono nel nuovo frammento `rules/chat-instructions`.

## Rifinitura

- Le sezioni `proposal` e `design-request` dell'Area `data` ora stanno nel frammento `data/card-files`: le righe `chat` e `chatWriting` e la regola della richiesta tenuta dalla Chat sono scritte lì; il frammento `data/requests-history` non viene più creato e `data/project-files` resta com'è.
- La sezione `job` spostata in `cli/agent-commands` riporta il campo `measure` di `job show <id>`, aggiunto nel frattempo.
- In `rules/skills` la voce sui testi della Chat segue la nuova voce sulle Code notes; in `tech/acp` le modifiche della Chat si applicano al testo attuale, che ora include la misura dei job.

## Already present

- `file-lock/Held/developer.CLEAR_GENERAL` — `src-tauri/src/general/service.rs` (`carry_out`, `Effect::Clear` → `locks.family_ended`)
