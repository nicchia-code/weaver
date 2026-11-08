---
status: approved
kind: change
requires:
- rn6dcry4
base:
  model/actors/developer.md: feaf1a0d194f818a6089562409b5a0fb0a153bda97283db1929b04591d737c06
  model/actors/time.md: a78e6daff9a5b9ec09b5e7e0c237641ff6b87e145a14148654f381ef65d952d7
  model/app/feature.md: 05b5e71c9c131a7a4b50942a17752a77be3aae873cccd6e12e1db3365fefeefd
  model/app/window.md: 88f58d88555a538ee669bcaa5a73c7df780c9ef4dbe8f01b11e267464ab8c2fb
  model/project-setup/feature.md: f4c29ad920701495d795282f7cb39ee39c758589b867fdcafa70fc6d749173e3
  model/project-setup/usage.md: a6e85997050723c0cf3e9c8e851527e48be760da21f154f7d3fecf040b05f534
  requirements/tech/architecture.md: 6cb27bedf51bc93e15c88706de04f4709b3595530c4283972f6e76c4238b92f9
  requirements/tech/usage.md: b27fd861488a97904e86b62837b34981b6f11e84f4cbb4b3c304966c64c99eff
  requirements/tech/windows.md: 393d4598f50c5e2083360eec7de58e760efea69fc1d711d988c2cbaaede59867
  requirements/ui/files.md: f52386203f7b4974f57424e10d195a8aca47df2645bbe06840ccd387102e60bf
  requirements/ui/views.md: 2134e53022440dfd2bba22e1e9bbbf67f5899da19c6f38a9bc1f506d3e4a66fc
  requirements/ui/window.md: 89b46b24784011aacacc76c5982d0e37fc5f2558bcf97faf34910e6d693839fa
created: 2026-11-07T10:04:27.866Z
label: Redesign
chat: true
---
# Sidebar e switch di progetto

Seconda Proposal del redesign (richiede «Nuovo stile di base»). Ridisegna la sidebar della finestra di progetto con lo switch di progetto, la palette ⌘K, il riepilogo di Usage e la riga utente. Riferimento grafico negli Assets: `redesign-sidebar.png`/`.html`.

## Decisioni prese con lo sviluppatore
- Sidebar larga 248 px: in alto lo switch di progetto, poi «Search or run… ⌘K»; voci Board, Chat, Jobs; gruppo «PROJECT» con Requirements, Specs, Files, Assets, Notes, History; in basso il riepilogo di Usage e la riga utente con l'ingranaggio dei Settings.
- Sotto i 1024 px la sidebar resta a sole icone: lo switch diventa il solo logo, la ricerca un'icona, il riepilogo di Usage un'icona; la riga utente sparisce e resta l'ingranaggio.
- Badge: Board conta le card da decidere; Jobs mostra prima i job in attesa (Peach), altrimenti la pill «n running»; Chat mantiene il badge del question round.
- Lo switch apre un altro progetto nella sua finestra o porta davanti quella già aperta: resta una finestra per progetto.
- Menu dello switch: progetti aperti (con indicatore dei job attivi), recenti (al massimo 10, «Not found» rimovibili), `Open Folder…` e un campo di ricerca. Nessuna scorciatoia per lo switch. Il menu File con `Open Recent` resta.
- Titolo dello switch: nome della cartella del progetto; sottotitolo: il branch git corrente, l'hash breve del commit se git non è su un branch, nulla senza git. Il branch si rilegge al focus della finestra e quando git lo cambia.
- Lo stesso logo di Weaver per tutti i progetti. La finestra di benvenuto passa al nuovo stile.
- Palette ⌘K / Ctrl+K attiva ovunque, anche nell'editor della vista Files: viste, azioni (Scan, Run all, New work item, Open Folder, Settings, Usage), progetti, card, sezioni dei Requirements, voci di History. Mai ricerca nei file del progetto.
- Usage: i limiti si leggono anche da soli ogni 5 minuti, una lettura alla volta per tutta l'app, solo con omp pronto e almeno una finestra sullo schermo, appoggiandosi alla cache di omp. Il riepilogo mostra il limite più vicino all'esaurimento con il suo reset; il clic apre Usage.
- Riga utente: nome da `git user.name` (del progetto, altrimenti globale) con le iniziali; senza nome o senza git resta solo l'ingranaggio dei Settings.

## Assunzioni
- La palette usa la stessa lista con ricerca del dialog di Merge, senza nuove librerie.
- «n running» conta i job nello stato Running.
- Il riepilogo di Usage colora la barra solo per lo stato (neutra, Peach quasi esaurito, Rose esaurito).
