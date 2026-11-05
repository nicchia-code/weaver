---
technical: false
refs:
  - window
  - developer.OPEN_PROJECT
  - requirements/ui#window
  - requirements/tech#window
  - requirements/data#window
  - requirements/docs#General
---
# Menu File: apri progetto

Quando il Task è finito, il menu File di ogni finestra di Weaver (di progetto e di benvenuto), su macOS, Windows e Linux, apre un altro progetto senza toccare la finestra da cui si parte.

**Comportamento (Modello).**
- `developer.OPEN_PROJECT` parte ora anche dal menu File di qualsiasi finestra, scegliendo una cartella o uno dei progetti recenti. Una cartella già aperta viene solo in primo piano.
- `window/Closed` → Open con `developer.OPEN_PROJECT`: la finestra da cui il developer ha scelto la cartella resta com'era, con i suoi job.
- `window/Closed` → Closed con `developer.OPEN_PROJECT`: quando la cartella non esiste più o il progetto non si può aprire; la finestra da cui il developer ha scelto la cartella dice perché; quando nessuna finestra aveva il focus, si apre la finestra di benvenuto e dice perché.
- `window/Open` → Shown con `developer.OPEN_PROJECT`: la finestra viene in primo piano, anche dal background; la finestra da cui si è scelto resta com'era.
- Il menu File può anche svuotare la lista dei progetti recenti.

**Menu (`requirements/ui#window`, `requirements/tech#window`).**
- In cima al menu File, su tutti i sistemi: `Open Folder…`, `Open Recent`, un separatore, poi le voci esistenti. Su macOS le voci vanno nel menu File predefinito di Tauri, sopra `Close Window`; su Windows e Linux nel menu File di Weaver, sopra `Close Window` e `Quit`. Un solo menu dell'app, condiviso da tutte le finestre.
- `Open Folder…` ha l'acceleratore nativo `CmdOrCtrl+O` (`Cmd+O` su macOS, `Ctrl+O` altrove) e funziona qualunque cosa abbia il focus, anche l'editor del File view, le Notes o un campo di testo; la UI non lega né `Cmd+O` né `Ctrl+O`. Apre lo stesso selettore di cartelle e la stessa apertura del comando `open_folder` della benvenuto, modale alla finestra da cui si è scelto (un foglio su macOS); senza finestra con il focus il selettore è libero. Annullare non fa nulla. Scegliere di nuovo `Open Folder…` mentre un selettore del menu è aperto non fa nulla. Il gestore degli eventi del menu non si blocca: avvia lavoro asincrono.
- La finestra da cui si è scelto è la finestra di Weaver con il focus quando arriva l'evento del menu (come fa già `Close Window`); non c'è quando nessuna finestra ha il focus (su macOS senza finestre, o con tutte le finestre di progetto in background).
- `Open Recent` è un sottomenu con gli stessi progetti della benvenuto, nello stesso ordine, anche quelli già aperti; ogni voce mostra il nome della cartella e il clic apre quel percorso canonico come il comando `open_project`. Una cartella che non esiste più è una voce disabilitata `<name> — Not found`; se sparisce dopo l'ultimo controllo, il clic mostra l'errore. Dopo i progetti, un separatore e `Clear Recent`, che svuota la lista senza conferma. Con la lista vuota il sottomenu contiene solo la voce disabilitata `No Recent Projects`, senza separatore e senza `Clear Recent`.
- Il sottomenu `Open Recent` si crea una volta e si modifica sul posto: all'avvio, dopo ogni cambio di `recent` (un'apertura, una rimozione nella benvenuto, `Clear Recent`) e ogni volta che una finestra di Weaver prende il focus (`WindowEvent::Focused(true)`). Ogni ricostruzione controlla le cartelle con `Path::is_dir` fuori dal thread principale, poi aggiorna il sottomenu su di esso.
- Ogni cambio di `recent` arriva subito anche alla finestra di benvenuto, quando è aperta, con la nuova lista: la sua lista segue i cambi fatti dal menu e non solo il focus.

**Errori.**
- Un'apertura dal menu che fallisce si registra nel log come warning e il motivo va, come evento, solo alla UI della finestra da cui si è scelto.
- In una finestra di progetto: toast di errore `Could not open <name>.` con il motivo come descrizione; `Could not open the folder.` con il motivo quando fallisce il selettore stesso.
- Nella finestra di benvenuto: il suo `Alert` rosso con i testi che usa già (`<name> could not open: <reason>`, `The folder could not be opened: <reason>`).
- Senza finestra da cui si è scelto, la finestra di benvenuto si apre (o viene in primo piano) e mostra l'errore nel suo `Alert`.

**Dati (`requirements/data#window`).**
- Una voce di `recent` esce solo quando viene spinta fuori da quelle più nuove (anche se la sua cartella non esiste più), quando il developer la toglie dalla benvenuto, o quando `Clear Recent` svuota la lista. Svuotare la lista non cambia `lastSession` né le finestre aperte. Le scritture di `state.json` passano dal suo unico proprietario nel backend.

**Documentazione.** Una riga sotto `## [Unreleased]` di `CHANGELOG.md` dice, per l'utente, che il menu File apre una cartella o un progetto recente da ogni finestra.
