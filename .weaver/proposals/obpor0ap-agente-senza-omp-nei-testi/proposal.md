---
status: discarded
kind: change
base:
  requirements/ui/ask-more.md: 2ba15e0f168a6ff7a940ac9a082452d8f27009c67876faaec1e7447d1437fc2f
  requirements/ui/conversations.md: 2e8d535374d312d72f8dcc79bac8e4dcfe8a60620fd8f9fbce25a9feeb51ba40
created: 2026-11-07T11:21:37.423Z
label: Testi agente
job: 8ssxwv0i
---
# Agente senza omp nei testi

Il Requirement di qualità dice che il nome `omp` compare solo nelle Settings e che altrove l'interfaccia dice `Weaver` o `the agent` (esempio: `Weaver asks permission`). Tre frasi di `ui` dicono ancora `omp`: la card di permesso, il job in coda e il box messaggi di Ask more.

Decisioni (scelte dell'agente, da controllare):
- vale la regola di qualità, perché è generale e cita lo stesso testo con `Weaver`;
- le frasi diventano `Weaver asks permission`, `Starting Weaver…`, `Waiting for the agent to be ready.`, con le opzioni del permesso dette `the agent's` invece di `omp's`;
- il resto delle frasi (pulsante `Open Settings`, ordine dei bottoni) non cambia.

## Already present

- `requirements/ui#job` — `ui/src/components/jobs/jobs-view.tsx` (card di permesso e job in coda)
- `requirements/ui#ask-conversation` — `ui/src/components/ask/ask-conversation.tsx` (box messaggi)
