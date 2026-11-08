---
status: approved
kind: change
base:
  model/actors/agent.md: 7c4b5e3c7da9bf5498daabe27c2ab94282d71e1a9d813093f4a71b85efc34b54
  model/actors/developer.md: b4f313e300bc9a9d7b8f30ee91daec7cd2e6027feeb206e301f613ea46c957b6
  model/agent-work/ask-conversation.md: aa674e7eecb3735d6746fa672b55210e4a1d80ad02ecc90258a2d010fedd335d
  model/agent-work/feature.md: be2357debda9a4f3fb69c3f6b129c7126bffc1bd2a98dba689e85276d2b9af95
  model/agent-work/general-chat.md: f9e6a3116dc6143d60f74464a102230968a41779388be82d4c65d0544ad8db15
  model/agent-work/question-round.md: 5dbdeefc1d15e65579efa1e1a5a27e4c9f9a8644b753472d31101ef5aa5f7acf
  model/app/notification.md: 18e25a330161e1048e7d1206aea2bcef29a7eca9c30bd6acd339bce9f3c52153
  requirements/cli/agent-commands.md: 6a6e0b0ffbc3d1329f1d25c3a271fc07735243b96dcc5f800f375ea3bc72cb4c
  requirements/data/settings.md: 6fcc5f5de847f0207b9ad59d6001cbbb774133f9dc10eabe46c20d366f32f0fe
  requirements/rules/cards.md: 38542c0a1df1456678841f488345663f6275a503fa3f97bc4df9edac0d6c8827
  requirements/rules/chat-instructions.md: 5454795eb3b6a603ea28b9b7e6e31f12991046d7292407b5f733f72cad182a46
  requirements/rules/instructions.md: d46c32c3730a2cfc53380e60ae80d90c0378ccb6e00fa537cbae19599e1c9066
  requirements/tech/architecture.md: 4537094f0f9ec148d0317725fb45bdbc0045ce4d8f9a604e2cb9cf35285ec49d
  requirements/ui/ask-more.md: 308a3e666e6bd5aa54e5ab5518e3e6c25eae1e0b744105eaf329180e9bf22305
  requirements/ui/chat.md: e3e6f0334dab1110c244cd122637dcee6afd90308b8ac5064aa0e7f371de9aaa
  requirements/ui/rounds.md: b16157e391839e5442eb7446d74e13c48bc241bf80918db5579db5008c03c986
  requirements/ui/window.md: d55d17a077ee1531d4b2a3aea13f836507edf39b237ae7588580df4ca9ec3f76
created: 2026-11-07T22:09:23.843Z
request: 2026-11-07T22-30-16
label: Rollback Chat question cards
job: h5yqkhgm
---
# Rollback domande Chat inline

Annulla la funzionalità della History `228bysg9` "Domande della Chat come card nella conversazione" (Proposal `htsbjhwo`): la Chat torna al round di domande modale introdotto da `1mqs1dkr` e adattato da `lly66ztj`. I 17 file di Model e Requirement toccati da `htsbjhwo` tornano al contenuto che avevano subito prima della sua approvazione; l'unica modifica arrivata dopo, l'uscita 2 di `invalid-input` in `requirements/tech/architecture.md`, resta.

## Decisioni

- Portata: rollback completo. L'agente di Chat apre un round solo su richiesta esplicita dello sviluppatore; il round è il modale di prima (oscuramento, focus intrappolato, `Minimize` e barra, `Close`/`Skip all`, `+<k> more rounds waiting`, colonna indice) e blocca il turno; `weaver question new` in Chat attende le risposte come nei job.
- Tornano `general-chat/Waiting for answers`, `general-chat/Working/question-round.OPENED`, `general-chat.WAITING`, la riga `Weaver is waiting for your answers.` con `Answer`, `Queued` sotto i messaggi inviati durante l'attesa, la notifica `notification/Not shown/general-chat.WAITING`, il badge `1` in Peach con tooltip `Chat is waiting for your answers` e il controllo del modello disabilitato mentre Chat attende.
- Spariscono la card nella conversazione, `Back to questions` verso la card, la riga `Your message also sends the answers chosen so far.`, il tooltip `Chat has questions for you`, `published`, `card-already-published`, `not-answered` e il campo `cards` del record di Chat.
- I round dei job non cambiano.
- Codice: revert selettivo. I Task indicano di riprendere da `git show d267932^:<file>` solo il codice della card e dell'attesa; il resto del commit `d267932` (comandi `usage_status`, `project_switcher`, `project_git`, `general_proposals`, `open_folder_from_window`, sidebar ridisegnata, cambio di modello di Chat, orari di arrivo, testi "the agent", stile nuovo del round, `chat-conversation.tsx` e `chat-draft.tsx`) e il lavoro non committato restano. Un `git revert d267932` intero, provato in una worktree separata, rompe `tsc` e la compilazione di `src-tauri`.
- Le richieste di design come card sulla Board (`4dsvoe5p`) e l'invio di richieste dalla Chat (`lly66ztj`) restano come sono.
- CHANGELOG: si tolgono solo le due voci della card nella sezione Unreleased, senza voce nuova; la card non è mai uscita in una release.

## Assunzioni

- `invalid-input` di `weaver question new` esce con 2, come dice oggi `requirements/tech#question-round`: è una correzione arrivata dopo `htsbjhwo` e resta.
- La History `228bysg9` resta: le voci della History non si cancellano.

## Already present

- `job/Running/developer.SEND_MESSAGE` — `src-tauri/src/jobs/registry.rs` `send_message`; i messaggi ai job non cambiano
- `job/Waiting for the developer/developer.SEND_MESSAGE` — `src-tauri/src/jobs/registry.rs` `send_message`; i messaggi ai job non cambiano
