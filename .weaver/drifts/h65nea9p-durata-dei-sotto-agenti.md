---
status: dismissed
refs:
- requirements/ui#job
job: 3onlemhi
created: 2026-11-07T23:10:44.276Z
decided: 2026-11-08T07:06:36.819Z
fingerprint: sha256:0c0dd9dd03a46a751aebcef769ad773f6a2d059b1e7d6736dfeb4bed6f54bb1f
---
# Durata dei sotto-agenti

## What happens

Nella conversazione di un sotto-agente, in Chat e nella vista Jobs, il riepilogo del lavoro dice solo **Worked · <n> steps**: la durata manca, perché i passi di un sotto-agente non hanno un istante di arrivo.

## What should happen

Ogni riepilogo del lavoro mostra **Worked for <durata> · <n> steps**, anche per i sotto-agenti, come chiedono i Requirements sul riepilogo del lavoro e sui tempi del job.

## Fix options

### Istanti anche ai sotto-agenti

Il lettore dei sotto-agenti registra l'istante di arrivo di ogni passo, come per i job, e il riepilogo calcola la durata da primo e ultimo passo. Costa una modifica al backend e ai tipi condivisi; il risultato è identico a quello dei job.

### Durata dall'intera esecuzione

La durata del sotto-agente è il tempo tra il suo avvio e la sua fine, ricavato dalle informazioni che il job già conserva su di lui, senza istanti per passo. Più semplice, ma un riepilogo che raggruppa solo alcuni passi mostra la durata dell'intera esecuzione.

## Where in the code

ui/src/components/general/chat-model.ts (testo del riepilogo del lavoro); src-tauri/src/subagents/reader.rs (trascrizione dei sotto-agenti)
