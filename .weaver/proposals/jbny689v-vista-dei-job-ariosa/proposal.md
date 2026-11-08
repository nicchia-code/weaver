---
status: approved
kind: change
base:
  requirements/data/settings.md: 6ea9502480e2658b7ccd790d60826a1a366ffc17f25c956d5dab83ef2f51e19d
  requirements/ui/ask-more.md: ef231443d9cb84939dfa4ebff8ad04e29c7ad7875598be268b63ea5c1b1cc89b
  requirements/ui/conversations.md: a16b36acf94f99d644930b752bd6488168e5abec0fd3ca64d2a86582f6c98c41
created: 2026-11-07T22:41:27.500Z
request: 2026-11-07T23-29-38
label: Vista dei job
job: alxik014
---
# Vista dei job ariosa

La vista dei job è percepita come claustrofobica: lista fissa a 300 px che a 1024 px prende il 40% della superficie, due fasce di intestazione impilate (76 px + circa 83 px) sopra la conversazione, colonna di 860 px attaccata a sinistra, un muro di righe di tool a 12–14 px e una casella del messaggio sottile. La Proposal cambia solo i Requirements (nessun cambiamento al Model).

## Decisioni

- Lista ridimensionabile trascinando il divisore tra 240 e 420 px, larghezza iniziale 280 px.
- Lista nascondibile con un pulsante; la conversazione prende tutta la larghezza.
- Larghezza e stato aperto/chiuso ricordati per progetto, anche dopo il riavvio.
- Divisore usabile da tastiera: frecce di 16 px, `Home`/`End` a 240/420 px, `Enter` e doppio clic tornano a 280 px.
- La conversazione mantiene almeno 480 px: con la finestra stretta la lista si restringe da sola (mai sotto 240 px) e torna alla larghezza scelta quando la finestra si allarga.
- Una sola fascia di intestazione da 76 px, divisa al bordo della lista: a sinistra il titolo `Jobs` con il pulsante per nascondere la lista, a destra l'intestazione del job (nome, stato, ora di avvio, `View request`, `Stop job`).
- Con la lista nascosta la parte sinistra mostra il titolo `Jobs` e il pulsante per riaprirla.
- Aprendo un job o una conversazione da fuori (Chat, Ask more, notifica, palette) la lista nascosta resta nascosta.
- Nella parte destra della fascia: per un sotto-agente il suo percorso e il suo stato, per Ask more il nome della conversazione e il suo stato.
- Colonna della conversazione centrata, larga al massimo 860 px.
- Voci della conversazione come in Chat: riga del mittente, riepilogo dei passi raccolto e testo più arioso; istruzione, note e riga di fine del job restano.
- Il riepilogo dei passi in corso, chiuso, mostra anche il titolo dell'ultimo passo.
- Nella conversazione di un sotto-agente la riga del mittente porta il nome del sotto-agente.
- Casella del messaggio come in Chat (due righe, raggio 16 px, sfumatura), centrata nella colonna.
- A job finito, `This job has ended. Continue in Chat.` con `Open Chat` è una riga centrata nella colonna, sopra la sfumatura, senza bordo.
- Le righe della lista non cambiano.
- Le modifiche valgono per tutte le conversazioni della vista: job, Ask more e sotto-agenti.
- Dopo il controllo del codice: il requisito dello stato locale ora dice che il file del plugin window-state sta nella cartella di configurazione dell'app, dove il plugin lo salva già.
- Dopo il controllo del codice: il requisito dell'hash del progetto ora dice che su Windows il percorso è messo in minuscolo prima del calcolo, come fa già il codice.

## Assunzioni da verificare

- Pulsanti per nascondere e mostrare la lista: icone `PanelLeftClose` e `PanelLeftOpen`, nomi accessibili e tooltip `Hide jobs list` e `Show jobs list`; il divisore ha il nome accessibile `Resize jobs list` e un'area di presa di 8 px.
- La riga del mittente di un sotto-agente mostra un riquadro con l'icona `Bot` al posto del logo di Weaver.
- Le conversazioni dei sotto-agenti aperte da Chat seguono lo stesso disegno, perché Chat le mostra già come quelle della vista dei job; la vista Chat per il resto non cambia.
- La larghezza e lo stato della lista sono conservati nello stato locale di Weaver fuori dal progetto (`state.json`), scritti solo alla fine di un trascinamento, a ogni passo da tastiera e a ogni apertura o chiusura; il restringimento automatico non viene salvato.
- L'eccezione alla regola comune dell'intestazione delle viste è scritta nei Requirements della vista dei job, senza toccare il frammento comune dell'aspetto.
