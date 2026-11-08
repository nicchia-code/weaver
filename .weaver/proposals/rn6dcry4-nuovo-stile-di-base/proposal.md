---
status: approved
kind: change
base:
  requirements/quality/main.md: edbec8a20bef4c7d6fa3028ce5f5441acdb91f1e62b07c631f7c6c31068efb8c
  requirements/release/pipeline.md: abede03290471e3ea0de587d79920179a64ddb7716f5afa51af1fda6229fea10
  requirements/ui/board-cards.md: 5374303ab76d08a92d4704cf005586a5f9e2188cfe6074ee2132c8cf8eef1dba
  requirements/ui/board.md: 6cd8431f4a5cbf386257541750617ff5b1aea06d2c7665f5505322c9dbc8f55e
  requirements/ui/look.md: 8d9b0fe25c1955500d0b762272aa60afd05b7ccaa358b4d58f858afb0d0c9461
  requirements/ui/views.md: 916f80b2e8924958def36d73dcff0fa99e44274b80c912f6965379ffef23b506
  requirements/ui/window.md: new
  specs.yaml: 8acb99729bff4ea02cd89b01282ecd45819ef12a247267162941276be5fb6a6a
created: 2026-11-07T09:58:58.532Z
label: Redesign
chat: true
---
# Nuovo stile di base

Prima di quattro Proposal del redesign di Weaver (riferimento grafico negli Assets: `redesign-foundations.png`/`.html` e le altre tavole `redesign-*.png`/`.html`). Questa fissa le fondamenta visive valide per tutta l'interfaccia; le successive, che la richiedono, ridisegnano la struttura della finestra con la sidebar, le viste e la Chat.

## Decisioni prese con lo sviluppatore
- Componenti shadcn/Radix mantenuti, ridisegnati con i valori del mockup; le superfici non disegnate le ricava l'agente dalle Foundations, restando vicine a quelle attuali.
- Font Geist e Geist Mono inclusi nell'app (funziona offline), al posto del font di sistema.
- Il nero è l'unico colore delle azioni, al posto del blu.
- Fondo carta, superficie di lavoro bianca e neutri del mockup.
- Cinque colori di stato dal logo: Sky (in coda, in corso, in rifinitura), Peach (in attesa dello sviluppatore, stale, blocked, Drift), Lavender (Proposal, question round), Sage (fatto), Rose (fallito, fermato); neutro per sospeso. Il colore serve solo allo stato.
- Le etichette non hanno più un colore: niente bordo colorato sulle Task, l'etichetta resta come testo.
- Tema scuro mantenuto (Auto/Light/Dark), con una palette scura ricavata dalle Foundations.
- Raggi 6/9/12/14, anelli da 1 px al posto dei bordi sulle card, header delle viste da 76 px invece di 56.
- Icone lucide, le più vicine a quelle del mockup.
- Il logo a fili del mockup diventa anche l'icona dell'app, con un sorgente vettoriale.
- I banner (avvio, aggiornamento, certificati, file dell'agente) stanno in cima alla superficie bianca.
- Etichette dell'interfaccia in inglese; solo i testi dell'agente seguono la lingua scelta.
- Il nome omp compare solo nei Settings: altrove l'agente si chiama Weaver quando parla o agisce, «the agent» in stati e motivi.
- I test end-to-end si aggiornano nella stessa consegna.
