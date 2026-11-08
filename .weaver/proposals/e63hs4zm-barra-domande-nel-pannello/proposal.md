---
status: approved
kind: change
base:
  requirements/ui/look.md: 49fd31ada87a5b965947e562d9a346354cbbf93840a066429ece2c9cd546ac6c
  requirements/ui/rounds.md: 15237c8accedb2db32b5eb06a30d5907c1f2152c68474f9ebc4cd4144dda296d
  requirements/ui/window.md: c3e9223c4ac70eb467ad5963884c784d0e85f7b6a7da3c12ecec610954d0bf0d
created: 2026-11-07T22:45:22.123Z
request: 2026-11-07T23-40-19
label: question-bar
job: wjwdvjcy
---
# Barra domande nel pannello

La barra di un round di domande minimizzato è disegnata sopra la finestra e nessuna vista le lascia spazio: copre il riquadro dei messaggi di Chat, dei job, dei sub-agent e di Ask more, la barra laterale e il fondo di ogni altra vista. La Proposal la sposta nel layout, in fondo al piano di lavoro, così ogni vista si accorcia invece di essere coperta.

Decisioni:
- Posizione: in fondo al piano di lavoro, dentro il pannello, sotto il contenuto della vista; ogni vista si accorcia, la barra laterale non cambia.
- Visibilità: la barra compare in ogni vista, anche in Chat per un round di Chat e nelle conversazioni Ask more, accanto a `Answer` e `Back to questions`.
- Toast: finché la barra è visibile salgono sopra di essa.
- Aspetto: fondo nella tinta Peach, testo e icona nell'inchiostro Peach, bordo superiore Peach.
- Finestra stretta: una sola riga ad altezza fissa, testo troncato con `…` e completo nel tooltip, `Open questions` sempre intero.
