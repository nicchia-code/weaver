---
status: discarded
kind: change
base:
  requirements/data/settings.md: 63adec426fbc6728858ac4712d2e7102ef30ec18aa96f8f7d9954ff0d996298c
  requirements/ui/chat.md: 33a99ea5f9954ceac150698218f426da48fbded532d5f5785b652a7801bc2ebf
created: 2026-11-07T11:21:58.278Z
label: Job di Chat
job: 8ssxwv0i
---
# Job di Chat dopo il riavvio

Chat deve mostrare di nuovo, dopo un riavvio, la riga `Started <job>` e le card delle Proposal dei job che ha avviato, ma i Requirements non dicono dove Weaver ricorda quali job ha avviato la Chat né dove la riga compare, e vietano di salvare il testo della conversazione.

Decisioni (scelte dell'agente, da controllare):
- il record del job ricorda con un flag `byChat` che la Chat lo ha avviato, o ha inviato la richiesta che lo ha avviato;
- dopo un riavvio le righe `Started` tornano in ordine di creazione dopo la cronologia riletta da omp, perché Weaver non conserva la posizione nel testo;
- niente cambia per i job avviati nella sessione in corso, che restano al punto in cui sono comparsi.

## Already present

- `requirements/data#job` — `crates/core/src/jobs/record.rs` (campo `by_chat`) e `src-tauri/src/jobs/registry.rs`
- `requirements/ui#general-chat` — `src-tauri/src/general/service.rs` (righe `Started` dopo la cronologia riletta)
