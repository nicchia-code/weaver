---
technical: true
refs:
  - requirements/ui#general-chat
  - requirements/ui#General
---
# Vista Chat e barra laterale

La Chat ora può scrivere Proposal (bozze Draft con una scheda propria sulla Board, segnate `chat: true` quando create dalla Chat) e aprire un round di domande, durante il quale la Machine `general-chat` è nello State `Waiting for answers`. La vista della Chat e la barra laterale devono mostrarlo.

## Vista Chat (`requirements/ui#general-chat`)

In `ui/src/components/general/general-view.tsx`:

- Testo della Chat vuota (costante `EMPTY`): `Think with omp about this project. Ask it to write a Proposal or start a job of Weaver when you want one.` al posto di quello di oggi.
- Dialogo di Clear: titolo `Clear Chat?`, descrizione `The conversation is deleted and a new one starts.` (anche durante il caricamento) o, durante un turno, `The agent's turn stops, the conversation is deleted and a new one starts.`; quando esistono Draft create in Chat, la descrizione prosegue con ` Its <n> Draft Proposal(s) are discarded.` (n = numero delle Proposal Draft con `chat: true`; non contano quelle soltanto riaperte dalla Chat, che Clear non scarta). Bottoni `Clear` (distruttivo) e `Cancel`, fuoco su `Cancel`. Il numero viene dal backend: esporlo nello snapshot della Chat (`GeneralSnapshot`, `src-tauri/src/general/service.rs:snapshot`) o leggerlo dalle schede della Board, senza contare a mano nei file.
- Waiting for answers: mentre un round della Chat aspetta, un messaggio inviato nel frattempo mostra `Clock` + `Queued` sotto di sé finché il round non finisce (riusare la resa già esistente dei messaggi in coda).

## Barra laterale (`requirements/ui#General` e `requirements/ui#general-chat`)

In `ui/src/components/sidebar.tsx` (`ShellCounts` in `ui/src/views.ts`, valori da `ui/src/components/project-content.tsx` l.~107):

- La voce Chat mostra `1` nel colore di avviso (ambra), posizionato come il numero della voce Jobs, con tooltip `Chat is waiting for your answers`, mentre la Chat è in Waiting for answers; nessun numero quando è 0. Serve un nuovo campo in `ShellCounts` (e in `NO_COUNTS`) alimentato dallo stato della Chat.
- Il numero della voce Jobs non conta mai la Chat.
- La voce Board mostra il numero delle schede reali in Backlog senza le schede Draft della Chat (oggi `snapshot?.backlog.length`): escludere le schede Proposal Draft della Chat come le marca la Board.

## Test

- `ui/src/components/general/general-view.test.tsx`: le asserzioni su `Talk with omp…` (l.~86, 94, 101) vanno cambiate sul nuovo testo; i test del dialogo di Clear (l.~137, 172) restano e se ne aggiunge uno con Draft della Chat che controlla la frase ` Its <n> Draft Proposal(s) are discarded.`; un test per `Queued` mentre la Chat aspetta le risposte.
- `ui/src/components/sidebar.test.tsx`: numero `1` ambra e tooltip sulla voce Chat in attesa, nessun numero altrimenti.
- `ui/src/components/project-content.test.tsx`: il conteggio Board esclude le schede Draft della Chat.
