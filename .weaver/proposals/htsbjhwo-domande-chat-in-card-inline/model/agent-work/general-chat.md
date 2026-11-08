# General chat

The brainstorming conversation with omp of a project, always there in its window. The developer explores ideas, problems and alternatives about the project and about Weaver with the agent, which reads freely whatever it needs and reasons with him, guided by an instruction of Weaver; no written request is attached to it. The agent writes nothing, neither in Weaver nor in the project's files, unless the developer explicitly orders it, and a wish, a problem or an idea is not an order; when the reasoning reaches something worth writing down, it says so in one line and waits. It never edits Weaver's files by hand.

When the developer explicitly asks for a Proposal, the agent does not write it: it writes a design request with what they agreed on and sends it, as if the developer had written it and sent it himself, so that the design job that starts explores, asks its question rounds and hands the Proposal over. A resolution of open Drifts asked in Chat goes the same way, as a Resolve job the agent starts. The agent never writes or changes a Proposal. On explicit order it may file a Task or a Drift, take one of the developer's decisions, or start a job of Weaver.

The agent asks every question that has distinct options as a card of questions in the conversation, at any moment, and never repeats those questions in the text of its reply; open questions, without options, stay in its reply. A card never holds the turn: the agent publishes it and ends its turn, and the answers reach it as a new message of the developer, as `question-round` says. The developer answers from the card or ignores it and goes on writing; a message he writes while a card is active carries the answers he confirmed so far. Only the card of the agent's latest reply is active.

When a job Chat started, or the job its request started, hands a Proposal over, Chat shows that Proposal in the conversation, under the line of that job, and follows it as it changes. The developer can decide it there exactly as on the Board, with the same conditions and the same reasons for refusing: approve it or have it refined, reject it, reject it with feedback for a rework, or discard a Draft left half-way by a refinement. An approval is final: nothing undoes it.

The model Chat's agent uses is the analysis model of the settings, and the developer can change it from Chat as well; the change is the same change of the settings, for every project. It applies when Chat's agent starts again: changing it while no turn is in progress ends the agent, keeping the conversation, so that the next message starts it again with the new model.

It is not a job: it never waits for other work and no work waits for it, it holds no Task, announces no card, and shows nothing on the Board; the jobs it starts, and the job its request starts, show there as any job does. Its conversation is kept across closing the window and quitting Weaver, and comes back as it was, with its cards of questions in their place. When the window opens, a kept conversation comes back at once, before the developer writes anything and without sending anything to the agent; with no kept conversation, omp starts only with the first message. As in a job, the developer can open and read the conversation of each of its sub-agents, read-only.

## States

### Idle (initial)
No turn is in progress; the conversation may be empty.
- `window.OPENED` → Reloading: only when a conversation is kept and omp is ready; Weaver starts omp and reloads it, sending nothing.
- `window.OPENED` → Waiting for omp: only when a conversation is kept and omp is not ready.
- `developer.SEND_MESSAGE` → Working: Weaver starts omp when it is not running, reloading the kept conversation when it has not come back yet, and sends the message, with the answers of the active card of questions when there is one.
- `developer.SEND_ANSWERS` → Working: only the answers of Chat's active card of questions; Weaver starts omp when it is not running, reloading the kept conversation when it has not come back yet, and sends the answers as a message of the developer.
- `developer.CHANGE_SETTINGS` → Idle: only a change of the analysis model, while omp runs; Weaver ends omp, the conversation is kept, and the next message starts omp again with the new model and reloads the kept conversation.
- `developer.CLEAR_GENERAL` → Idle: the conversation starts again empty, without its cards of questions.

### Loading
The window has just opened and the kept conversation is coming back from omp; nothing is sent to the agent meanwhile.
- `developer.SEND_MESSAGE` → Loading: the message shows at once and waits; it is sent when the conversation has come back, below it.
- `developer.SEND_ANSWERS` → Loading: only the answers of Chat's active card of questions; they wait and are sent when the conversation has come back.
- `developer.CLEAR_GENERAL` → Idle: the loading stops and the conversation starts again empty, without its cards of questions.
- `window.CLOSED` → Idle: omp is ended; the conversation is kept.

#### Reloading (initial)
Weaver has started omp and asks it to reload the kept conversation.
- `omp.SESSION_RESUMED` → Idle: when no message waits; the conversation shows as it was, with its cards of questions.
- `omp.SESSION_RESUMED` → Working: when messages wait; the conversation shows above them, with its cards of questions, and they are sent.
- `omp.SESSION_NOT_RESUMED` → Idle: when no message waits; an empty conversation starts, the cards of questions of the kept one are gone, and a notice says the kept one could not be reloaded.
- `omp.SESSION_NOT_RESUMED` → Working: when messages wait; an empty conversation starts with the notice, without the cards of questions of the kept one, and the messages are sent.
- `omp.START_FAILED` → Idle: the reason is shown; the next message starts omp again and reloads the kept conversation.
- `omp.EXITED` → Idle: the reason is shown; the next message starts omp again and reloads the kept conversation.

#### Waiting for omp
omp is not ready, so it cannot start yet; when omp is missing, the view says so and offers to install it.
- `omp.READY` → Reloading: Weaver starts omp and reloads the kept conversation, without waiting for a message.

### Working
The agent answers: it reasons, reads, publishes cards of questions and, when ordered, writes.
- `developer.SEND_MESSAGE` → Working: the agent receives it while it works, with the answers of the active card of questions when there is one.
- `developer.SEND_ANSWERS` → Working: only the answers of Chat's active card of questions; the agent receives them as a message of the developer while it works.
- `omp.PERMISSION_REQUESTED` → Working: the permission waits for the developer's answer.
- `developer.ANSWER_PERMISSION` → Working
- `agent.START_JOB` → Working: Weaver starts the job the developer explicitly asked for; it gets its own conversation.
- `agent.SEND_REQUEST` → Working: Weaver writes the design request and starts its job, as `design-request` says; the job gets its own conversation.
- `agent.ASK_QUESTIONS` → Working: the card of questions shows in the conversation, as `question-round` says; the turn goes on until the agent ends it.
- `omp.TURN_ENDED` → Idle
- `omp.TURN_FAILED` → Idle: the reason is shown; an active card of questions stays active.
- `omp.EXITED` → Idle: the reason is shown; the next message starts omp again; an active card of questions stays active.
- `developer.STOP_AGENT` → Idle: the turn is cancelled; when omp is still starting, it is ended, and the next message starts it again and reloads the kept conversation. An active card of questions stays active.
- `developer.CLEAR_GENERAL` → Idle: the turn stops and the conversation starts again empty, without its cards of questions.
- `window.CLOSED` → Idle: the turn is cancelled; the conversation is kept, with its cards of questions.
