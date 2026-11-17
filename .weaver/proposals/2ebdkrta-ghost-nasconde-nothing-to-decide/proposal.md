---
status: approved
kind: change
base:
  requirements/ui/board.md: f89527efcc808b12bbef4e7135311cb063c6d01eebba7630d5eb26951fd4f8d2
created: 2026-11-08T22:18:37.396Z
request: 2026-11-08T23-16-23
label: Board empty state
job: fx38rgkd
---
# Ghost nasconde Nothing to Decide

Un ghost in una colonna della Board deve nascondere lo stato vuoto della colonna esattamente come una card vera; oggi lo stato vuoto compare anche sopra i ghost.

Decisioni:
- Backlog con soli ghost: lo stato vuoto (disegno a fili, `Nothing to decide`, testo, `Request`, `Ask in Chat`) sparisce del tutto; la colonna mostra solo i ghost.
- Active: stessa regola, un ghost di Task nasconde `Nothing active`.
- I numeri restano sulle sole card vere: titolo delle colonne, voce `Board` della barra laterale e sottotitolo `<n> active` non contano i ghost.
- Vale per ogni ghost, anche quello che Weaver crea da solo per un job di setup, design o import (assunzione dell'agente: la richiesta parla di ghost senza distinzioni).
- Quando l'ultimo ghost lascia una colonna senza card vere, lo stato vuoto ricompare nella stessa immagine (conseguenza della regola).
