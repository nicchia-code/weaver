---
technical: false
refs:
  - notification
  - requirements/ui#notification
---

# Chat senza notifiche

Chat (General) non solleva mai una notifica di sistema: le sue card di domande si vedono solo nella sua conversazione e nella sidebar (badge `1` in Peach, tooltip `Chat has questions for you`). Le notifiche restano solo per i job: un job finito o fermato in modo anomalo (`job.ENDED`), un job in attesa di risposte o di un permesso (`job.WAITING`), mostrate subito e solo se la finestra del progetto non è in primo piano; e `System Certificates turned on`, sempre.

Quando è fatto:
- Non esiste più un tipo di notifica "Question round" per Chat: il tipo `Question round` (`<project> – <job> asks a question`, `The agent is waiting for your answers.`) riguarda solo i round dei job, che mantengono la loro notifica, il ritiro alla chiusura del round (`question-round.CLOSED` → Withdrawn) e il clic che apre l'overlay del round.
- Nessun titolo contiene `Chat`; `<project>` è il nome della cartella, `<job>` il nome fisso del job. Mai messaggi, domande, comandi, percorsi, risposte o card.
- L'ID di ogni notifica è solo progetto + job + tipo, per sostituirla o ritirarla; nessun ID dedicato a Chat.
- Non resta nulla del percorso di notifica di Chat: né le voci del registro delle notifiche mostrate, né il bersaglio del clic verso una domanda di Chat, né gli osservatori di apertura/chiusura dei round di Chat, né i testi, né i test che li verificano. I test dei round dei job restano e passano.

La rimozione della transizione `notification/Not shown/general-chat.WAITING` (e dell'evento `general-chat.WAITING`) è coperta da Task di rimozione codice che Weaver crea all'approvazione: questa Task non la duplica e copre tutto il resto del percorso.

## Code notes
Commit: 450785e1b59d7321510ae229ca9cc4280a0dc799

- `src-tauri/src/notifications.rs`: rimuovere `chat_waiting` e `chat_round_closed` (righe ~9-11, 161-212) e la registrazione di `round_opened`/`round_closed` come `JobObserver` per Chat (~383), se servono solo a Chat; test ~693-790 con testi e ID di Chat da eliminare.
- `crates/core/src/notification.rs`: rimuovere `Book::chat_question`, `chat_round_closed`, `Target::ChatQuestion` e la variante di `Destination::Round` usata solo da Chat (righe ~19, 139-161, 208, 390-448, 491); test ~854-919.
- I chiamanti in `src-tauri/src/general/service.rs` (`Notice::Waiting/RoundEnded/RoundClosed`, `ChatObserver::*`) sono della Task sulla general chat: coordinarsi, togliendo qui solo le API e lasciando lì gli usi (o viceversa nello stesso commit) perché compili.
- Attenzione a non toccare il percorso dei round dei job (stesso `Book`, stessi testi `asks a question`).
