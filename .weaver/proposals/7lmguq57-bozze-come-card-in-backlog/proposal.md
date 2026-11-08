---
status: approved
kind: change
base:
  model/actors/developer.md: 72443cdd3d908f1039987457c6bd7e93339d727f8c6d0e3eec44a732741da1f5
  model/agent-work/job.md: e8ca3b0626412076f1fcd1e3a9218c38bc47a6432f780f8b5ba0265326902d63
  model/model-changes/design-request.md: d798758bfa2352c625a8bd512bf92692d9722aad1c7c21393f0749f8fa4d0e40
  model/work/feature.md: c2dabd4a621b6a3079b6a7fd6dd5b749c72114ded1c7e39facfb2aadb2e50236
  requirements/tech/architecture.md: f776818baf2217240f470dc857f57bd5c13a359fbf7c2cbfdc428001f5e0de3c
  requirements/ui/board.md: c6f509c0de1f5e2bc43ec49f137540aebdfa16bcd4aeedef7cc4d55e028946a1
  requirements/ui/conversations.md: a41000dfadecea46264c1d9dba715670207f280d4126c9e375be34d94a03062b
  requirements/ui/look.md: ccc80ea3f147fe27e598349838370f21327fdac59b95c8f62bb8d46689ef168a
  requirements/ui/requests.md: new
  requirements/ui/setup.md: 9b716c315bbd3c152cbf9f062239cf76d68aefe7611f493eb2d6e1ab64cd62f6
  requirements/ui/window.md: 47e3bc2aa546cae337fcf9386472d777cc8be1cdb1b87e1531a252ab2c8b38fa
created: 2026-11-07T19:33:50.072Z
request: 2026-11-07T20-18-18
label: Bozze richieste
job: dsdsaxp0
---
# Bozze come card in Backlog

Le richieste di design non ancora inviate diventano card nella colonna Backlog della Board, così che l'intero ciclo (bozza → ghost del job → card della Proposal) stia in un solo posto. Spariscono la striscia `N requests are open` e la lista delle richieste a sinistra dell'editor; la stessa cosa prende un solo nome, `Request`.

## Decisioni

- Nome `Request` ovunque: pulsante `+` della Backlog (`New Request`), stato vuoto della Backlog, azione della palette, tabella delle icone e testi dell'agente che citano `+ Work Item`.
- Le bozze sono card vere: contano nel numero della colonna Backlog e della voce `Board` della sidebar; con sole bozze `Nothing to decide` non compare.
- Posizione: in fondo alla Backlog, dopo Proposal e Drift e prima dei ghost, dalla più vecchia.
- Card della bozza: etichetta `Request` al posto dell'id, titolo (primo titolo o prima riga, altrimenti `Untitled request`), `Not sent · <ora>` con il formato della vista Jobs (`HH:MM` oggi, `D MMM HH:MM` altrimenti), nel piè `Send` (disabilitato a testo vuoto) e `Discard` (con la conferma di oggi). Clic fuori dai pulsanti o Enter sul titolo apre la bozza nell'editor. Stesso aspetto delle altre card.
- Una richiesta vuota viene scartata, senza conferma, quando lo sviluppatore lascia l'editor (`← Board`, cambio di vista) o quando la finestra si chiude con l'editor aperto: nuova Transizione nel Modello della Design request, con il nuovo Evento `developer.LEAVE_REQUEST`.
- Editor: una sola richiesta a tutta vista con `← Board`, senza lista, senza `New request` e senza badge `Draft`/`Sent`. Le nuove richieste nascono solo dal `+` della Backlog e da `Start from Scratch`. Restano testo, `Preview`, salvataggio automatico, `Send`, `Discard`, immagini e avvisi di cambi esterni.
- Dopo `Send` dall'editor si torna alla Board.
- Dopo l'invio Weaver mostra subito un suo ghost `Proposal` anche per i job di design, con il titolo della richiesta, finché l'agente non annuncia una card; anche il ghost di un job di setup prende il titolo della richiesta (quello di import resta `Import from Code`). Cambia la prosa della Macchina Job.
- Richiesta inviata: `View request` nell'intestazione della conversazione del job che l'ha ricevuta, prima di `Stop job`, presente solo finché la richiesta esiste ed è inviata a quel job; apre la vista in sola lettura di oggi con il banner `Sent to the agent · <job name>` e, in alto, `← Jobs`, che torna alla conversazione. Aperta dalla Board l'editor mostra `← Board`.
- Quando il job finisce senza Proposal la richiesta torna bozza: riappare come card normale nella Backlog e, aperta, mostra il banner di oggi.
- `Start from Scratch` apre la bozza più vecchia se ce n'è una, altrimenti ne crea una nuova; il `+` della Backlog crea sempre una richiesta nuova.
- Le bozze compaiono nel gruppo `Cards` della palette come `Request` seguito dal titolo; sceglierne una mostra la Board e la apre nell'editor.
- Il Modello: la descrizione della Feature Work e gli stati della Design request dicono dove si vedono bozze e richieste inviate; nessuno stato nuovo.

## Scelte dell'agente

- Le bozze arrivano nella fotografia della Board calcolata dall'app (le richieste restano fuori dalla fotografia del core); la Board viene ricalcolata quando una richiesta nasce, cambia titolo o vuotezza, viene inviata, scartata, torna bozza o cambia fuori da Weaver, non a ogni salvataggio.
- Un invio o uno scarto fallito dalla card mostra lo stesso toast d'errore dell'editor.
- La sezione `design-request` dei Requirement `ui` passa dal frammento `setup` a un nuovo frammento `requests`: con la card della bozza il frammento `setup` avrebbe superato 12 KB.
- Una richiesta inviata aperta da `View request` che viene consumata o scartata mentre è mostrata chiude la vista e torna alla conversazione del job, senza toast; se il job finisce senza Proposal mentre è mostrata, la stessa vista diventa scrivibile con il banner di ritorno.
- Lo scarto automatico di una richiesta vuota è verificato dal backend sul file, sotto il lock del progetto: non scarta mai una richiesta non vuota o non in scrittura.
- Una richiesta il cui job è finito mentre l'app era chiusa torna bozza all'apertura del progetto, prima della prima fotografia della Board.
