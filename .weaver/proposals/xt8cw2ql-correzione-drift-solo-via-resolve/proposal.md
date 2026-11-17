---
status: approved
kind: change
base:
  model/actors/developer.md: 65d273a0501038e2d464524bce6e01fa29e8da2018aec09e6e5cc714ed9f66e5
  requirements/ui/board-cards.md: f6b888097d2bb4f94c68ecbae7ed619280922747a9a46b29c341b253603d53e0
  requirements/ui/look.md: cb5c3c06cfdef1424ca2dfd2791a54432123dcb03d5f499403949f13dbf554cf
created: 2026-11-11T13:21:30.864Z
request: 2026-11-11T14-06-05
label: Drift Resolve
job: b4gpnm8f
---
# Correzione Drift solo via Resolve

# Correzione Drift solo via Resolve

Nel dettaglio di un Drift la correzione del codice si sceglie solo rispondendo alla domanda di `Resolve`: spariscono la scelta dell'opzione di correzione, il campo `Note` e `Fix the code`, e con loro l'elenco delle opzioni nel dettaglio. Il resto del dettaglio e del flusso resta com'è.

## Decisioni

- Spariscono dal dettaglio la scelta dell'opzione di correzione, `Note` e `Fix the code`; restano `Resolve`, `Dismiss Drift` e `Ask more` nel footer.
- `Accept Proposal` resta, e con esso `Refine` e `Discard` per una Proposal allegata stale o lasciata a metà da un refinement.
- `Accept Proposal`, `Refine` e `Discard` restano dove sono oggi, sotto la sezione della Proposal allegata; il footer non cambia.
- Le opzioni di correzione non compaiono più nel dettaglio, nemmeno in sola lettura, e con loro la frase per un Drift senza opzioni: si vedono solo nella domanda che `Resolve` pone. `What happens`, `What should happen` e `Where in the code` restano.
- Per un Drift nato da un merge, `Accept Proposal` resta marcato `recommended`.
- `weaver drift move` resta com'è, con `code-to-be-fixed`, `proposal-accepted` e `dismissed`: l'agente può ancora correggere il codice su richiesta esplicita del developer in una conversazione. Per questo l'Event del developer per correggere il codice resta nel Model, che ora dice che l'app non offre un controllo per esso.
- La card del Drift sulla Board non cambia: `Resolve` e `Dismiss Drift`.
- Nella tabella delle icone resta solo `Accept Proposal` (`CheckCheck`), senza `Fix the code`.
