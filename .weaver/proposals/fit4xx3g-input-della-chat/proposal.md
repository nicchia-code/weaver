---
status: approved
kind: change
base:
  requirements/quality/files.md: 299c00e5e5b69c3216cbb32d667d1b1825ba0c8e3ba50dd1163717a0e34aa230
  requirements/tech/assets.md: f22299c7c2f0cf6bf6fa668ae6f9b2f72800b9f20d48224ec13b9c44090e7f7c
  requirements/ui/assets.md: 6542fe3651cb08c852f9d2779e07938c4569145c4776947fe35d55076cec31d4
  requirements/ui/chat.md: 609d1a01d73f77403569755ecf01ce870ac08e56b6293ff42560ff7f1d73cb33
created: 2026-11-07T18:04:01.603Z
request: 2026-11-07T18-55-26
label: Input della chat
job: yx0mdp4s
---
# Input della chat

Nella casella dei messaggi della Chat il testo sembra scritto a metà della parola. La causa: il testo visibile è disegnato da uno strato specchio dietro l'area di testo trasparente, e la Chat toglie i margini laterali solo all'area di testo. Il testo disegnato finisce così 12 px a destra del cursore e dei caratteri veri, e le righe lunghe vanno a capo in punti diversi. Il Model non cambia: la correzione rende più precisi i Requirements delle caselle con tag.

## Decisioni

- Allineamento garantito dal componente condiviso: margini, bordo e carattere arrivano a entrambi gli strati da un unico punto, e chi usa il componente non può cambiarli su un solo strato.
- Nella Chat il testo resta dove appare oggi, 12 px dentro il margine interno della cornice; cursore, selezione e clic si spostano lì.
- Anche il segnaposto del cursore, usato mentre la lista dei suggerimenti `@` è aperta, non deve mai cambiare dove vanno a capo le righe, in tutte e quattro le caselle.
- In tutte e quattro le caselle, Invio o Tab premuti durante una composizione del metodo di input confermano solo la composizione: non inviano, non scelgono un suggerimento e non spostano il focus.
- Un test Vitest verifica, per ciascuna delle quattro caselle, che i due strati ricevano gli stessi margini, bordo e carattere.
