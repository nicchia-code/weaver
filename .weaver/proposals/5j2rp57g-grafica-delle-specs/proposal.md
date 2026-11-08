---
status: approved
kind: change
base:
  requirements/ui/usage.md: new
  requirements/ui/views.md: aabbf4bbb13a702d24a37a3a8766cab1398ad8f5a2308b8e70b10820629e61db
  specs.yaml: ff9d515809f751028b18b4fbbc43ff6c423f56204821b03b1eabf43b166d1f9d
created: 2026-11-07T22:40:52.569Z
request: 2026-11-07T23-33-39
label: Specs
job: 2unlt3hm
---
# Grafica delle Specs

La vista Specs mostra ogni voce con un riquadro grigio con l'iniziale e i campi senza icone: con molte voci che iniziano per la stessa lettera i riquadri sembrano segnaposto. La richiesta chiede una revisione grafica (icone e altro).

Decisioni:
- Il riquadro di ogni voce mostra il logo monocromo della tecnologia, nel colore del testo; senza logo, l'icona del campo.
- Ogni intestazione di campo ha un'icona lucide prima del titolo: Languages `Code`, Frameworks `Layers`, Data `Database`, Testing `FlaskConical`, CI `Workflow`, Repository `GitBranch`, Deployment `Rocket`, Cloud `Cloud`, Infrastructure `Server`, Notes `NotebookPen`.
- I loghi vengono da simple-icons, l'intera raccolta inclusa nell'app e caricata alla prima apertura della vista Specs in una finestra; mai scaricata dalla rete.
- Una voce ha un logo solo quando il suo nome intero, senza distinguere maiuscole, spazi e punteggiatura, coincide con il nome o l'identificativo di un logo; nessuna corrispondenza parziale.
- Durante il caricamento la vista appare subito con le icone dei campi e ogni logo le sostituisce appena caricato.
- Il contenuto dei campi è allineato al titolo dell'intestazione della vista; il bordo del focus non lo sposta.
- Card più regolari: altezza minima uguale, ombra leggera come le card della Board, riquadro e nome centrati sulla stessa riga, raggi secondo la scala (6 px riquadro e versione, 12 px card).
- Le etichette `added`/`changed`/`removed` e `Agent's choice` stanno insieme in fondo alla card.
- `Show before` e il testo precedente sono a 13 px come la nota.
- simple-icons entra tra i framework delle Specs.

Assunzioni da verificare:
- Se la raccolta di loghi non si carica, le voci restano con l'icona del campo senza alcun avviso.
- L'altezza minima delle card è di circa 92 px, come nel vecchio mock-up della vista.
- Il fragment `ui/views.md` supererebbe i 12 KB: la sezione `usage` passa, invariata, nel nuovo fragment `ui/usage.md`.
