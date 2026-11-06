# General chat

The brainstorming conversation with omp of a project, always there in its window. The developer explores ideas, problems and alternatives about the project and about Weaver with the agent, which reads freely whatever it needs and reasons with him, guided by an instruction of Weaver; no written request is attached to it. The agent writes nothing, neither in Weaver nor in the project's files, unless the developer explicitly orders it, and a wish, a problem or an idea is not an order; when the reasoning reaches something worth writing down, it says so in one line and waits. It never edits Weaver's files by hand.

When the developer explicitly asks for a Proposal, the agent does not write it: it writes a design request with what they agreed on and sends it, as if the developer had written it and sent it himself, so that the design job that starts explores, asks its question rounds and hands the Proposal over. A resolution of open Drifts asked in Chat goes the same way, as a Resolve job the agent starts. The agent never writes or changes a Proposal. On explicit order it may file a Task or a Drift, take one of the developer's decisions, or start a job of Weaver. It asks the developer in its replies, and with a question round only when the developer asks for one.

It is not a job: it never waits for other work and no work waits for it, it holds no Task, announces no card, and shows nothing on the Board; the jobs it starts, and the job its request starts, show there as any job does. Its conversation is kept across closing the window and quitting Weaver, and comes back as it was. When the window opens, a kept conversation comes back at once, before the developer writes anything and without sending anything to the agent; with no kept conversation, omp starts only with the first message. As in a job, the developer can open and read the conversation of each of its sub-agents, read-only.

## States

### Idle (initial)
No turn is in progress; the conversation may be empty.
- `window.OPENED` → Reloading: only when a conversation is kept and omp is ready; Weaver starts omp and reloads it, sending nothing.
- `window.OPENED` → Waiting for omp: only when a conversation is kept and omp is not ready.
- `developer.SEND_MESSAGE` → Working: Weaver starts omp when it is not running, reloading the kept conversation when it has not come back yet, and sends the message.
- `developer.CLEAR_GENERAL` → Idle: the conversation starts again empty.

### Loading
The window has just opened and the kept conversation is coming back from omp; nothing is sent to the agent meanwhile.
- `developer.SEND_MESSAGE` → Loading: the message shows at once and waits; it is sent when the conversation has come back, below it.
- `developer.CLEAR_GENERAL` → Idle: the loading stops and the conversation starts again empty.
- `window.CLOSED` → Idle: omp is ended; the conversation is kept.

#### Reloading (initial)
Weaver has started omp and asks it to reload the kept conversation.
- `omp.SESSION_RESUMED` → Idle: when no message waits; the conversation shows as it was.
- `omp.SESSION_RESUMED` → Working: when messages wait; the conversation shows above them and they are sent.
- `omp.SESSION_NOT_RESUMED` → Idle: when no message waits; an empty conversation starts and a notice says the kept one could not be reloaded.
- `omp.SESSION_NOT_RESUMED` → Working: when messages wait; an empty conversation starts with the notice and they are sent.
- `omp.START_FAILED` → Idle: the reason is shown; the next message starts omp again and reloads the kept conversation.
- `omp.EXITED` → Idle: the reason is shown; the next message starts omp again and reloads the kept conversation.

#### Waiting for omp
omp is not ready, so it cannot start yet; when omp is missing, the view says so and offers to install it.
- `omp.READY` → Reloading: Weaver starts omp and reloads the kept conversation, without waiting for a message.

### Working
The agent answers: it reasons, reads and, when ordered, writes.
- `developer.SEND_MESSAGE` → Working: the agent receives it while it works.
- `omp.PERMISSION_REQUESTED` → Working: the permission waits for the developer's answer.
- `developer.ANSWER_PERMISSION` → Working
- `agent.START_JOB` → Working: Weaver starts the job the developer explicitly asked for; it gets its own conversation.
- `agent.SEND_REQUEST` → Working: Weaver writes the design request and starts its job, as `design-request` says; the job gets its own conversation.
- `question-round.OPENED` → Waiting for answers: only a round of Chat, which the agent opened because the developer asked to be asked that way.
- `omp.TURN_ENDED` → Idle
- `omp.TURN_FAILED` → Idle: the reason is shown.
- `omp.EXITED` → Idle: the reason is shown; the next message starts omp again.
- `developer.STOP_AGENT` → Idle: the turn is cancelled; when omp is still starting, it is ended, and the next message starts it again and reloads the kept conversation.
- `developer.CLEAR_GENERAL` → Idle: the turn stops and the conversation starts again empty.
- `window.CLOSED` → Idle: the turn is cancelled; the conversation is kept.

### Waiting for answers
The agent waits for the answers of its round, in the same turn; Chat counts as waiting for the developer.
- `developer.SEND_MESSAGE` → Waiting for answers: the message waits and never answers the round; it reaches the agent once the round ends.
- `question-round.CLOSED` → Working
- `omp.TURN_FAILED` → Idle: the reason is shown; the round ends without answers.
- `omp.EXITED` → Idle: the reason is shown; the next message starts omp again; the round ends without answers.
- `developer.STOP_AGENT` → Idle: the turn is cancelled and the round ends without answers.
- `developer.CLEAR_GENERAL` → Idle: the turn stops, the round ends without answers and the conversation starts again empty.
- `window.CLOSED` → Idle: the turn is cancelled and the round ends without answers; the conversation is kept.

## Events

### WAITING
Emitted when the agent of Chat starts waiting for the answers of its round.
