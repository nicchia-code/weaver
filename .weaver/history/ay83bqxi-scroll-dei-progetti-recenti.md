---
created: 2026-11-16T15:28:10.719Z
updated: 2026-11-16T15:46:58.565Z
summary: Nella finestra di benvenuto scorre solo la lista dei progetti recenti, con la propria barra; intestazione, titolo, errore e stato di omp restano sempre visibili.
tasks:
- xwg76q3f
- 38fs730d
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
  38fs730d:
    title: 'Fix the code: Welcome senza provider omp'
    refs:
    - requirements/ui#window
    technical: true
    finished: 2026-11-16T15:44:34.876Z
    body: |-
      ## What happens

      Con omp **non collegato a nessun provider** la finestra di benvenuto mostra, sotto l'avviso, l'elenco dei provider (7 righe, circa 400 px). Insieme a intestazione, titolo e avviso supera i circa 416 px utili della finestra 640 × 480, non ridimensionabile.

      La lista dei progetti recenti si riduce a 0 px, quindi sparisce, e il pannello scorre per intero: la configurazione di omp esce dalla vista.

      ## What should happen

      La finestra misura 640 × 480 px e non è ridimensionabile. Il Task T1 chiede che scorra **solo** la lista dei progetti recenti e che intestazione, titolo, `Alert` e riga di stato di omp con la sua configurazione restano sempre visibili. Con l'elenco dei provider mostrato le due richieste non possono valere insieme.

      ## Chosen option

      ### Provider con scroll proprio

      L'elenco dei provider ha una propria area che scorre nella finestra di benvenuto, con almeno due righe di recenti sempre visibili. Tutto resta in vista, ma il login scorre in un'area molto piccola. Cambia solo il layout, nessun comportamento.

      ## Where in the code

      ui/src/components/welcome-window.tsx, `WelcomeWindow` (`main` e `section` dei recenti); `OmpSetup` con `providers="when-needed"` in ui/src/components/omp-status.tsx
---
# Scroll dei progetti recenti

La lista dei progetti recenti (fino a 10) non esce più dalla propria area e non copre la riga di stato di omp, anche con banner in cima alla finestra. Con pochi progetti la finestra resta come prima, senza barra di scorrimento. Le righe sono invariate, restano raggiungibili da tastiera e il focus di una riga fuori vista la porta in vista, con l'anello di focus non tagliato. Con omp senza provider collegato il logo sta accanto a nome e versione, i recenti mostrano sempre almeno due righe e l'elenco dei provider scorre in una propria area, quindi intestazione, avviso e stato di omp restano in vista e il pannello non scorre.
