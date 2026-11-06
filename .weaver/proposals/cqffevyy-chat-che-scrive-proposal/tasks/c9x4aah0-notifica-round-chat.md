---
technical: false
refs:
  - notification
  - requirements/ui#notification
---
# Notifica del round della Chat

Quando la Chat (General) di un progetto aspetta le risposte al proprio round di domande, il developer deve essere avvisato come per un job, se la finestra del progetto non è in primo piano.

## Cosa deve valere

**Model `notification`**, stato `Not shown`:
- `general-chat.WAITING` → `Shown`: quando la finestra del progetto non è in primo piano; altrimenti non si mostra nulla. La notifica è mostrata subito.
- Da `Shown`, `question-round.CLOSED` → `Withdrawn`: il round che annunciava si è chiuso (comunque si chiuda: risposte, Skip all, abbandono, interruzione per Stop, Clear, uscita di omp o chiusura della finestra).
- `desktop.NOTIFICATION_CLICKED` → `Clicked`: il developer è portato al progetto.
- La Chat notifica soltanto l'attesa delle risposte del suo round: mai la fine di un turno, mai altro. Una notifica non cambia nulla nel progetto, nei job o nei round.

L'evento `general-chat.WAITING` (Chat che entra in `Waiting for answers` per un round) è prodotto dal Task di `general-chat` e dal Task di `question-round`; qui si consuma.

**`requirements/ui#notification`:**
- Tipo Question round: titolo `<project> – Chat asks a question`, messaggio `The agent is waiting for your answers.`. `<project>` è il nome della cartella; un round della Chat genera solo il tipo Question round.
- Mai alcun messaggio, domanda, comando, percorso, risposta o scheda.
- ID di ogni notifica: progetto + job (o Chat) + tipo, per sostituirla o ritirarla; dove il sistema non può rimuovere una notifica consegnata, il clic su una ritirata porta solo la finestra in primo piano.
- Clic su una notifica di domanda → l'overlay del round (finestra del progetto in primo piano).
- Nessun raggruppamento per il tipo Question round.

## Dove nel codice
- `crates/core/src/notification.rs` (`Book`, l.250): `question`/`round_closed` oggi prendono l'identità del job; servono le stesse operazioni per la Chat del progetto, con chiave progetto + Chat + tipo distinta da qualunque job.
- `src-tauri/src/notifications.rs`: oggi `round_opened`/`round_closed` arrivano solo come `JobObserver` (l.200–209) con un `JobInfo`. Aggiungere i metodi per la Chat (attesa del round e chiusura), usando `in_background(project_id)` e `waiting` come per i job; il nome del progetto come in `project_name`.
- Chi li chiama: il servizio General (`src-tauri/src/general/service.rs`) quando la Chat entra/esce da `Waiting for answers`, oppure `src-tauri/src/rounds/service.rs::carry_out` per gli effetti `Opened`/`Closed` con origine Chat; collegamento in `notifications::install` / setup dell'app.

## Test
- Test in `src-tauri/src/notifications.rs` (modulo tests, vicino a `in_the_background_the_exact_texts_appear_without_anything_the_agent_said`): in secondo piano appare esattamente `<project> – Chat asks a question` / `The agent is waiting for your answers.`; in primo piano nulla; la chiusura del round la ritira; una notifica di un job e una della Chat dello stesso progetto hanno ID diversi.
- Test di `Book` in `crates/core/src/notification.rs` per la chiave della Chat.
