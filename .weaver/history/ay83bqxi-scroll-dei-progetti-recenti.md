---
created: 2026-11-16T15:28:10.719Z
updated: 2026-11-16T15:28:10.719Z
summary: Nella finestra di benvenuto scorre solo la lista dei progetti recenti, con la propria barra; intestazione, titolo, errore e stato di omp restano sempre visibili.
tasks:
- xwg76q3f
cleared:
  xwg76q3f:
    title: Scroll dei progetti recenti
    refs:
    - requirements/ui#window
    technical: true
    finished: 2026-11-16T15:28:03.871Z
    body: |-
      Nella finestra di benvenuto (640 × 480 px, non ridimensionabile) la lista dei progetti recenti, che può contenere fino a 10 progetti, non deve mai uscire dalla propria area né coprire altri elementi della finestra.

      Quando i progetti recenti non entrano nello spazio disponibile:

      - scorre soltanto la lista dei progetti recenti, con una propria barra di scorrimento verticale;
      - l'intestazione (logo, nome e versione di Weaver, pulsante `Open Folder`), il titolo `Recent projects`, l'eventuale `Alert` di errore e la riga di stato di omp con la sua configurazione restano sempre visibili e non vengono mai coperti né spinti fuori dalla finestra;
      - vale anche quando in cima alla finestra compaiono uno o più banner (avvio, aggiornamento, System Certificates), che riducono lo spazio disponibile;
      - con pochi progetti, che entrano tutti, la finestra appare come oggi, senza barra di scorrimento.

      Le righe della lista restano invariate: nome della cartella, percorso completo in grigio, `Not found` e il pulsante per rimuovere un progetto che non esiste più; ogni riga resta raggiungibile con la tastiera e il focus di una riga fuori vista la porta in vista.

      Alternative scartate dallo sviluppatore: far scorrere l'intera finestra (la riga di stato di omp uscirebbe dalla vista) e rendere la finestra ridimensionabile o più alta (cambierebbe i Requirements).

      ## Code notes
      Commit: 47eb6ea302ec96dd15c28bd7fd77dc5510dd5ce6

      - `ui/src/components/welcome-window.tsx`, `WelcomeWindow`: `main` è `flex min-h-0 flex-1 flex-col gap-4 overflow-auto`; la `section` dei recenti ha `flex min-h-0 flex-col`, ma la `ul` non ha `overflow`. Con `min-h-0` la sezione si restringe sotto il proprio contenuto e la lista trabocca sopra `OmpSetup`, invece di scorrere. Stima: altezza utile circa 416 px, intestazione circa 110 px, ogni riga di `RecentProject` circa 56 px, quindi il problema compare già dal 4°–5° progetto.
      - Probabile correzione: rendere la `ul` (o un suo contenitore) l'unica area che scorre (`min-h-0 overflow-y-auto`), lasciando `shrink-0` a intestazione, `Alert` e `OmpSetup`; controllare che l'anello di focus delle righe non venga tagliato dall'`overflow`.
      - Test esistenti in `ui/src/components/welcome-window.test.tsx` (jsdom, nessun layout): la verifica visiva va fatta nell'app con 10 progetti recenti, anche con un banner visibile.
---
# Scroll dei progetti recenti

La lista dei progetti recenti (fino a 10) non esce più dalla propria area e non copre la riga di stato di omp, anche con banner in cima alla finestra. Con pochi progetti la finestra resta come prima, senza barra di scorrimento. Le righe sono invariate, restano raggiungibili da tastiera e il focus di una riga fuori vista la porta in vista, con l'anello di focus non tagliato. Se la configurazione di omp da sola riempie la finestra (omp senza provider collegato), la lista mantiene un minimo di due righe e scorre il pannello.
