---
refs:
  - job
  - ask-conversation
  - requirements/ui#job
  - requirements/ui#ask-conversation
  - requirements/data#window
---
# Vista dei job ariosa

La vista dei job deve smettere di sembrare claustrofobica. Quando il Task è finito, nella vista dei job vale quanto segue, come dicono i Requirements `requirements/ui#job`, `requirements/ui#ask-conversation` e `requirements/data#window`.

**Una sola fascia di intestazione.** Sopra lista e conversazione c'è una sola riga, con l'altezza, il bordo inferiore e il titolo dell'intestazione comune delle viste, divisa dove finisce la lista: il bordo della lista prosegue attraverso la fascia. A sinistra il titolo `Jobs` e, alla sua estremità destra, il pulsante a icona `Hide jobs list` (`PanelLeftClose`). A destra, con 28 px di margine ai lati, l'intestazione di ciò che è selezionato:
- per un job: nome (17 px semibold), pill di stato con il tempo di lavoro per un job che lavora, sotto `Started today at HH:MM` (o la data), a destra `View request` quando previsto e `Stop job` finché il job è in coda, in corso, in attesa o sospeso;
- per un sotto-agente: il percorso `<job> › … › <sotto-agente>` con il suo stato;
- per una conversazione di Ask more: il suo nome con il suo stato (`running`, `waiting`, `idle`), senza riga di avvio né pulsanti;
- vuota quando non è selezionato nulla.

La fascia separata dell'intestazione del job dentro il riquadro destro non esiste più. `Back to questions` di Ask more resta in cima alla sua conversazione.

**Lista ridimensionabile.** La lista parte larga 280 px. Trascinando il bordo tra lista e conversazione (area di presa di 8 px, puntatore `col-resize`) la larghezza va da 240 a 420 px, e la divisione della fascia la segue. Il bordo è un separatore verticale che riceve il focus, con nome accessibile `Resize jobs list`, minimo, massimo e valore in px: le frecce lo spostano di 16 px, `Home` e `End` lo portano a 240 e 420 px, `Enter` e il doppio clic tornano a 280 px. La conversazione mantiene almeno 480 px: con la finestra troppo stretta la lista si restringe da sola, mai sotto 240 px, e torna alla larghezza scelta quando la finestra si allarga; questo restringimento non cambia la larghezza scelta.

**Lista nascondibile.** `Hide jobs list` nasconde la lista e il suo bordo e il riquadro destro prende tutta la larghezza; la parte sinistra della fascia resta con il solo titolo `Jobs` seguito da `Show jobs list` (`PanelLeftOpen`), che riporta la lista com'era (larghezza scelta, filtro, selezione). Aprire un job o una conversazione da fuori (Chat, Ask more, una notifica, la palette) lascia nascosta una lista nascosta.

**Memoria per progetto.** La larghezza scelta e lo stato nascosto della lista sono ricordati per progetto, anche dopo il riavvio, nello stato locale di Weaver fuori dal progetto (`jobsList`, per hash del progetto, `{width, hidden}`): un progetto senza valore, o con un valore fuori intervallo o di tipo sbagliato, vale 280 px e lista visibile. Il backend scrive alla fine di un trascinamento, a ogni passo da tastiera e quando la lista viene nascosta o mostrata, mai durante il trascinamento; il restringimento automatico non viene mai scritto. L'interfaccia riceve i valori dal backend e gli manda le modifiche, senza tenere uno stato proprio.

Nella stessa sezione dello stato locale, due frasi descrivono ora quello che il codice fa già e non richiedono lavoro: il file della dimensione e della posizione delle finestre sta nella cartella di configurazione dell'app, dove lo salva il plugin, e su Windows il percorso del progetto è messo in minuscolo prima di calcolarne l'hash.

**Conversazione.** La colonna della conversazione è centrata nel riquadro destro, larga al massimo 860 px, e la casella del messaggio sta nella stessa colonna. Le voci sono come in Chat: messaggi dello sviluppatore, messaggi dell'agente con la riga del mittente (logo, `Weaver`, ora) e il testo di Chat, e il riepilogo dei passi che raccoglie ogni serie di `Thinking`, righe dei tool e righe dei sotto-agenti, chiuso all'inizio; note, permessi, messaggi in coda, righe di stato e di fine restano fuori dal riepilogo. Il riquadro dei passi in corso, quando è l'ultima cosa di un turno che lavora, dice `Working · <n> steps · <ultimo passo>`, dove l'ultimo passo è il titolo della sua ultima riga di tool, o `Thinking`, su una riga tagliata con `…`. L'istruzione di Weaver resta la prima voce, chiusa.

**Casella del messaggio.** Come quella di Chat (aspetto, due righe all'inizio, posata su una sfumatura, posizione del testo), senza il controllo del modello, centrata nella colonna, con segnaposto, invio e regole del job di oggi. A job finito, al suo posto c'è una riga centrata nella colonna, sopra la sfumatura e senza bordo: `This job has ended. Continue in Chat.` con `Open Chat`.

**Ask more e sotto-agenti.** Le conversazioni di Ask more e quelle in sola lettura dei sotto-agenti hanno la stessa colonna, le stesse voci e la stessa casella (disabilitata per i sotto-agenti). Nella conversazione di un sotto-agente la riga del mittente mostra un riquadro di 24 px con l'icona `Bot` e il nome del sotto-agente al posto del logo e di `Weaver`; `Conversation not available: <reason>.` compare in cima alla conversazione. Le conversazioni dei sotto-agenti aperte da Chat seguono lo stesso disegno; il resto della vista Chat non cambia.

Le righe della lista, il filtro, l'ordine, i badge e i tempi restano come sono.

## Code notes

Commit: bcb006abc30b7b93d0223e6a5d5faf43d3e2eb18

- Quasi tutto è in `ui/src/components/jobs/jobs-view.tsx`: `JobsView` (lista `w-[300px]` fissa, nessun ridimensionamento), `ViewHeader view="jobs"` sopra i due pannelli, `JobHeader` (`px-7 py-[18px]` con bordo), `Conversation`, `FollowScroller`, `CONVERSATION_PADDING`/`CONVERSATION_CONTENT` (`max-w-[860px]` senza `mx-auto`), `MessageFrame`/`MessageBox`/`DisabledMessageBox`, `EntryView`, `ToolRow`, `SubAgentConversationView`, `AskListRow`.
- Da riusare: `general/chat-conversation.tsx` (`SenderRow`, `ChatEntry`, `DeveloperMessage`, `WorkSummary`), `general/chat-model.ts` (`chatItems`, `workText`, `clockText`), la colonna centrata e la sfumatura di `general/general-view.tsx` (~131, ~192). `view-header.tsx` (`VIEW_HEADER_CLASS`, 76 px) per l'altezza della fascia.
- Insidia: `EntryView` e `SubAgentConversationView` sono importati da Chat e `FollowScroller`/`EntryView` da `ask/ask-conversation.tsx`; cambiare spaziature lì cambia anche Chat. Ask oggi ha la sua barra `Back to questions` e casella con `border-t p-3`.
- Nessun componente di pannello ridimensionabile in `ui/src/components/ui/`; nessuna dipendenza tipo `react-resizable-panels`.
- Stato locale: `state.json` in `app_local_data_dir` (accanto a `recent`, `lastSession`); `ui/src/lib/backend.ts` per i comandi Tauri.
- Tailwind v4, token in `ui/src/index.css`: `rounded-xl` = 14 px, `rounded-lg` = 12 px. Pill da `TONE_PILL`/`TONE_DOT` in `ui/src/lib/tones.ts`.
- Test che fissano classi e testi: `jobs-view.test.tsx` (~91-117, 293-305, 341-348), `general-view.test.tsx`, `ask.test.tsx` (~134).
