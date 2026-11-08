---
description: "Use when building or changing the Chat view: its conversation, message box, header and the cards it shows."
---
# Chat


## general-chat
- Its own view in every project window, always present, under the view header (`requirements/ui#General`, title `Chat`), with the job conversation rendering (messages, `Thinking`, tool rows, sub-agents, permission cards, queued messages), but no instruction, notes or end line.
- **Sub-agents:** each sub-agent line opens that sub-agent's conversation in place of the chat, as in `## job` (**Sub-agent conversation**), with the header `Chat › <sub-agent name>`, every parent sub-agent in between, and a `Back to Chat` button (`ArrowLeft`) that returns to the chat at the position it had; that header takes the place of the view header, in the same row and at the same height. Meanwhile the chat goes on; `Clear` also returns to the chat.
- Every text of the interface names it `Chat`, never `General`: the sidebar entry, its tooltip, the view's title and the texts below. A request refused because it is not open says `Chat is not open for this project`.
- Empty: centred secondary text `Think with omp about this project. Ask it for a Proposal or a job of Weaver when you want one.`
- **Loading:** while the kept conversation loads, centred secondary text `Loading conversation…` in place of the empty text, above any message sent meanwhile, until the history shows. Waiting for omp with omp missing adds the `omp is not installed.` line and `Install omp` at once; while omp is only checking, updating or not connected, the loading text shows alone.
- The message box is always enabled; a message sent while omp starts or reloads shows at once with `Starting omp…` under it until delivered. `Stop` (no dialog) shows during a turn.
- **Clear:** `Eraser` button `Clear` on the right of the header, disabled while empty; a kept conversation that is loading is not empty. AlertDialog `Clear Chat?` / `The conversation is deleted and a new one starts.` (also while loading; during a turn: `The agent's turn stops, the conversation is deleted and a new one starts.`), `Clear` (destructive) and `Cancel`, focus on `Cancel`.
- **Waiting for answers:** while a round of Chat waits, the sidebar's Chat entry shows `1` in the warning colour (amber), placed as the Jobs entry's number, with tooltip `Chat is waiting for your answers`; a message sent meanwhile shows `Clock` + `Queued` under it until the round ends. The Jobs entry's number never counts Chat.
- **Started jobs:** a line `Bot` + `Started <job name>` with `Open conversation`, which selects that job in the Jobs view; the job a request of Chat started shows the same line, with no other line for the request. A refused start: `Weaver did not start <kind>: <reason>.` in the warning colour, the kind of a refused request being `design` or `setup`.
- **Failures:** `omp stopped: <reason>.` in the error colour, then `Your next message starts omp again.` in secondary text. With omp missing: `omp is not installed.` and the `Install omp` button.
