---
status: approved
kind: change
base:
  model/actors/developer.md: 1611204f5042dfae16c8b74991db5812e5e9fc2cef4c9a26ef60ea4ff7497451
  model/app/window.md: ca052fbc119588c6763acae4273d9542d13a8d25927e404aaccee76074c9847f
  requirements/data/settings.md: 32c20218a47c47ce995faa4e11680aa9172eb91f212baa186841f1a1fb2ddb06
  requirements/tech/windows.md: 70d2c72618e8af8dbc591e5fd683dc9ae87e71237d382878e56a7dc87caed5ba
  requirements/ui/views.md: a8a3d3d2bba3f115ea03b4bce17d0aafe07d0989aa01699007b00b827eb96431
created: 2026-11-05T11:22:00.848Z
request: 2026-11-05T12-08-22
label: 'Menu File: apri progetto'
job: 8dujmwn3
---
# Apri progetto dal menu File

Oggi, con una finestra di progetto aperta, non esiste un modo per aprire un altro progetto: `Open Folder` e i progetti recenti stanno solo nella finestra di benvenuto. Il Modello dice già che `developer.OPEN_PROJECT` può partire da una finestra già aperta; questa Proposal aggiunge il punto di ingresso nel menu File di ogni finestra, su macOS, Windows e Linux, e descrive cosa succede quando l'apertura fallisce.

## Decisioni

- `Open Folder…` nel menu File apre il selettore di cartelle del sistema, come `Open Folder` della benvenuto: la cartella si apre in una nuova finestra o la sua finestra viene in primo piano, anche dal background; la finestra di partenza resta com'è, con i suoi job.
- Scorciatoia `Cmd+O` su macOS e `Ctrl+O` su Windows e Linux, valida sempre, anche con il focus nell'editor del File view, nelle Notes o in un campo di testo.
- `Open Recent` elenca gli stessi progetti della benvenuto, nello stesso ordine, anche quelli già aperti; il sottomenu e la benvenuto si aggiornano appena la lista cambia.
- Una cartella che non esiste più è una voce disabilitata con `— Not found`; se sparisce dopo l'ultimo controllo, il clic mostra l'errore.
- In fondo al sottomenu, dopo un separatore, `Clear Recent` svuota la lista senza conferma; è disabilitata quando la lista è vuota.
- Con la lista vuota il sottomenu contiene solo `No Recent Projects`, disabilitata.
- In una finestra di progetto un'apertura fallita è un toast di errore `Could not open <name>.` con il motivo come descrizione; nella benvenuto resta l'`Alert` di oggi.
- Senza una finestra con il focus (su macOS senza finestre, o con tutte le finestre di progetto in background) un'apertura fallita apre la benvenuto con l'errore nel suo `Alert`.
- Il selettore è modale della finestra di partenza (su macOS un foglio); senza finestra con il focus è libero.
- `Open Folder…` scelto di nuovo mentre un selettore è già aperto non fa nulla.
- Le voci stanno in cima al menu File, seguite da un separatore e dalle voci esistenti, su tutti i sistemi.

## Assunzioni dell'agente

- L'esistenza delle cartelle si controlla quando il sottomenu viene ricostruito: all'avvio, quando la lista cambia e quando una finestra di Weaver prende il focus, come la benvenuto ricarica la lista al focus.
- Un errore del selettore stesso in una finestra di progetto è il toast `Could not open the folder.` con il motivo; nella benvenuto resta `The folder could not be opened: <reason>`.
- Svuotare la lista non tocca i progetti aperti né la lista di quelli da riaprire al prossimo avvio.
- Con la lista vuota il sottomenu contiene solo `No Recent Projects`, senza separatore né `Clear Recent`, come dice la risposta sulla lista vuota; con almeno un progetto `Clear Recent` è sempre abilitata.

## Affinamento

- `model/actors/developer.md` è riscritto sul file attuale: tiene la nuova descrizione di `developer.INSTALL_OMP` (installer ufficiale o, se fallisce, le release GitHub di omp), approvata da un'altra Proposal, e cambia solo `developer.OPEN_PROJECT` come prima.
