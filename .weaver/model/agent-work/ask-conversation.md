# Ask more conversation

A free conversation with omp about one thing the developer is reading: a Proposal, a Task or a Drift in its details, a History entry together with every Task it cites, the question in view of a question round, or the Requirements as a whole, from the Requirements view. Every request opens a new conversation, also for a thing that already has one, and several can be open together. It is listed in the Jobs view next to the jobs, named after the chosen thing, but it is not a job: it never waits for other work and no work waits for it, it holds no Task, announces no card, asks no question round, shows nothing on the Board and never makes the developer confirm closing the window or quitting. It runs on the analysis model; a Proposal's chosen model never applies to it.

The developer's first message reaches the agent together with an instruction of Weaver that describes the chosen thing as the project's files say it at the moment of the request; the conversation shows only the developer's messages. The agent explains and answers; it changes a Task, a Drift or a Proposal only when the developer asks, and only with the commands of Weaver that allow it, never changes the Model or the Requirements, and starts a job of Weaver only on the developer's explicit request. As in a job, the developer can open and read the conversation of each of its sub-agents, read-only. Nothing of it is kept: it ends with the window.

The agent speaks to the developer as in Chat: only in the words of the app, naming a card as the Board shows it, telling why Weaver refused something or failed in the app's own terms, pointing to the controls of the app when the developer has to act, and going into Weaver's commands, files, skills or instructions only when the developer explicitly asks about them.

## States

### Not opened (initial)
- `developer.ASK_MORE` → Empty: only when the chosen thing still exists, which the Requirements always do; Weaver prepares the instruction from the files at that moment, opens the conversation in the Jobs view, shows it and puts the focus in its message box. For the Requirements, the instruction names no section and copies none of their text: the agent picks by itself the Requirement skills that fit the developer's question, and the Proposal shown over the view is named only while it is still open. Nothing is sent and omp does not start. What the developer was reading stays as it was.
- `developer.ASK_MORE` → Closed: when the chosen thing no longer exists: a card or a History entry that is gone, or a question of a round that has closed. Nothing opens.

### Empty
The conversation is open and has no message yet; omp is not started.
- `developer.SEND_MESSAGE` → Working: Weaver starts omp and sends, in one prompt, the instruction prepared at the opening followed by the message.
- `developer.CLOSE_ASK` → Closed: nothing was sent.
- `window.CLOSED` → Closed: nothing was sent.

### Idle
No turn is in progress.
- `developer.SEND_MESSAGE` → Working: the message goes alone. Weaver starts omp again when it is not running and reloads the conversation's session. When no session was ever opened, the message carries the instruction prepared at the opening; when the session cannot be reloaded, an empty one starts, a notice says so, and the message carries the instruction read again from the files at that moment, or goes alone when the chosen thing no longer exists.
- `developer.CLOSE_ASK` → Closed: omp is ended.
- `window.CLOSED` → Closed: omp is ended.

### Working
The agent answers.
- `omp.TURN_ENDED` → Idle
- `omp.TURN_FAILED` → Idle: the reason is shown.
- `omp.START_FAILED` → Idle: the reason is shown; the next message starts omp again.
- `omp.EXITED` → Idle: the reason is shown; the next message starts omp again.
- `developer.STOP_AGENT` → Idle: without a confirmation, only the turn is cancelled and the conversation stays; when omp is still starting, it is ended.
- `developer.CLOSE_ASK` → Closed: after the developer confirmed; the turn is cancelled and omp is ended.
- `window.CLOSED` → Closed: the turn is cancelled and omp is ended.

#### Answering (initial)
- `developer.SEND_MESSAGE` → Answering: the agent receives it while it works.
- `omp.PERMISSION_REQUESTED` → Waiting for permission: the permission waits in the conversation; no notification is raised and nothing counts it outside the conversation.
- `agent.START_JOB` → Answering: Weaver starts the job the developer explicitly asked for; it gets its own conversation, which a line of this one opens.

#### Waiting for permission
- `developer.ANSWER_PERMISSION` → Answering
- `developer.SEND_MESSAGE` → Waiting for permission: the message waits and reaches the agent after the permission is answered.

### Closed (final)
The conversation leaves the Jobs view; Weaver keeps nothing of it.
