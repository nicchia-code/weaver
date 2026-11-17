---
status: approved
kind: change
base:
  model/actors/developer.md: 3ef0a596f081f7f6465aba30d21a2d6a388bcc64f9f66d1f0e06f2a6947cb92b
  model/app/feature.md: a9fc5ecc4831cc89180e4f0fe0243aa3d6a0fb737d0ec373f79f47b5f773b3af
  model/app/shell-command.md: new
  model/app/window.md: 28fbf9367aa20fb99dc7e5619506dbc7c39213d5fd422344aa0c97d0379b4108
  requirements/cli/commands.md: ec990c5882258605082ea10816cf9548e90cea153919326d03fe0ac28605daf3
  requirements/docs/main.md: bf340cdb022c1437bf75ccfe69cb65e5aba359c5d48360abcc99a060c21a7a61
  requirements/tech/command-line.md: new
  requirements/tech/windows.md: c778710f8b3a91dbf8abfbe41c3e9eae354590f3df89feaefa2104e9df5983e3
  requirements/ui/setup.md: 4b5bcb44e5f831db02a31c82148614e6a457f85b33d2ab241acd3320c802af5e
created: 2026-11-11T10:45:13.281Z
request: 2026-11-11T11-31-02
label: weaver open
job: dl23gpwv
---
# Comando weaver open

Lo sviluppatore vuole aprire una cartella in Weaver dal proprio terminale. Il comando richiesto diventa `weaver open <cartella>`, e il comando `weaver` si installa nel terminale da un'azione delle Impostazioni.

## Decisioni

- Forma del comando: `weaver open <cartella>` (non `weaver <cartella>`); `weaver` senza argomenti resta un errore d'uso con l'aiuto.
- `weaver open` senza cartella mostra la finestra di benvenuto, anche con finestre di progetto aperte.
- Una sola cartella: con più cartelle è un errore d'uso (codice 2); `--root` e `--input` sono errori d'uso, `--json` è ammesso.
- Weaver non in esecuzione: il comando rifiuta (codice 1) e non avvia Weaver.
- Percorso inesistente o che non è una cartella: errore nel terminale (codice 1), Weaver non viene contattato.
- Cartella senza `.weaver`: si apre con Get Started, come «Open Folder…».
- Esito: la CLI aspetta l'esito fino a 30 secondi; un'apertura fallita compare solo nel terminale (codice 1 col motivo); in caso di successo non stampa nulla (`{}` con `--json`).
- Versione dell'app che non conosce la richiesta: codice 1 con l'invito ad aggiornare Weaver.
- Agenti (job, Chat, Ask more): il comando è rifiutato e il riferimento dei comandi delle skill installate non lo elenca.
- Installazione nel terminale: azione «Install weaver command» / «Remove» nel nuovo gruppo `Command line` delle Impostazioni, dopo `Editor`; Weaver non la fa mai da solo.
- Dove: macOS link in `/usr/local/bin` (password di amministratore quando serve; annullarla non mostra nulla), Linux link in `~/.local/bin` con avviso se la cartella potrebbe non essere nel PATH, Windows cartella di Weaver nel PATH dell'utente.
- Un `weaver` estraneo al suo posto: «Install» rifiuta e dice quale file c'è; «Remove» toglie solo ciò che punta ancora alla copia di Weaver, altrimenti non tocca nulla e lo spiega.
- Linux `.deb`: i pacchetti restano come sono (`/usr/bin/weaver` resta l'app).
- Documentazione: sezione `## Command line` nel README dopo `## Install` e righe nel CHANGELOG.

## Assunzioni da verificare

- Con una finestra di progetto aperta, chiudere la finestra di benvenuto la chiude soltanto, anche su Windows e Linux (oggi chiuderla esce dall'app quando nessuna finestra di progetto è in background).
- Annullare la password di amministratore durante «Remove» su macOS, come durante «Install», non cambia nulla e non mostra nulla.
- Anche l'app rifiuta una richiesta `open` che porta l'identità di un job, di Chat o di Ask more, con lo stesso motivo della CLI.
- L'evento con cui lo sviluppatore chiede la finestra di benvenuto non ha un Machine che lo riceve: la finestra di benvenuto non è modellata come Machine, quindi il controllo del Model lo segnala come avviso.
