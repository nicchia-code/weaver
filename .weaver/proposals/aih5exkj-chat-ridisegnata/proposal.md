---
status: approved
kind: change
requires:
- e8oe9509
base:
  model/agent-work/general-chat.md: da75068c36795488dc26f108c40d1784427e9b607c4cf36c79755b671a32f280
  requirements/ui/chat.md: 18930859c7afb644cc6f5d7a66fab330a1e056cc7f5a21fe2c83bbe7f2da8bd5
created: 2026-11-07T10:11:56.108Z
label: Redesign
chat: true
---
# Chat ridisegnata

Quarta e ultima Proposal del redesign (richiede «Viste ridisegnate»). Ridisegna la Chat e vi porta le Proposal consegnate dai job che la Chat ha avviato, da decidere sul posto. Riferimento grafico negli Assets: `redesign-chat.png` e `redesign-chat.html`.

## Decisioni prese con lo sviluppatore
- La card di una Proposal compare in Chat quando la consegna un job avviato dalla Chat, o il job della richiesta che la Chat ha inviato.
- Decisioni nella card: Approve, Reject, Rework, Discard; niente View diff, niente scelta del modello.
- Niente Undo: l'approvazione resta definitiva.
- La card mostra solo dati esistenti: titolo, stato, file cambiati, Task che crea, motivi di blocco.
- «Answer» su una domanda aperta riapre il question round della Chat.
- Niente suggerimenti sopra il campo di testo.
- Il selettore del modello nel campo di testo cambia il modello della Chat, lo stesso dei Settings (Analysis).
- «@» cita solo gli asset; niente pulsante per allegare file.
- «New chat» è il Clear di oggi: la Chat resta una sola conversazione persistente.
- Il riepilogo comprimibile «Worked for … · n steps» sostituisce le righe di tool e Thinking, che si vedono espandendolo.
- La pill nell'header mostra ogni stato dell'agente come «Weaver · …», senza nominare omp.

## Assunzioni
- Il selettore del modello è disattivato durante un turno, un caricamento e un round; un cambio con la Chat ferma chiude omp e il messaggio successivo lo riavvia col nuovo modello ricaricando la conversazione, come già accade dopo un'uscita di omp.
- La card di una Proposal sta sotto la riga «Started <job>» del job che l'ha consegnata e ne segue lo stato dal vivo.
- Per mostrare durata e ora dei messaggi il backend registra l'istante di arrivo di ogni voce della conversazione.
