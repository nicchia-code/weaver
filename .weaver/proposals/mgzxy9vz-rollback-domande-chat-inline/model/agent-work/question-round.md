# Question round

A group of related decisions the agent of a job, or of General when the developer asks to be asked that way, asks the developer at once: each question with its options, a recommendation, single or multiple choice and an optional longer context, and always room for a free answer. The developer answers one question at a time, can go back to any of them, and sends the round from a summary of every answer. A round is also how Weaver itself asks the prepared question of a Drift. A round belongs to its job, or to General, and to its project; it never shows in another project's window, and neither Weaver nor the agent sets a time limit. Questions and answers are never written to the project.

## States

### Not asked (initial)
- `agent.ASK_QUESTIONS` → Unavailable: when the caller is neither the agent of a working job of Weaver nor the agent of the project's General during a turn, or Weaver is not running.
- `agent.ASK_QUESTIONS` → Queued: when another round of the same project is presented.
- `agent.ASK_QUESTIONS` → Presented: otherwise; the project's window comes to the front.
- `developer.RESOLVE_DRIFT` → Queued: the prepared question of the Drift, when another round of the project is presented.
- `developer.RESOLVE_DRIFT` → Presented: the prepared question of the Drift, asked by Weaver with no agent.

### Queued
Waiting for the rounds before it, oldest first; it never replaces the presented one.
- `CLOSED` → Presented: when it is the oldest round of the project still waiting.
- `agent.STOP_WAITING` → Abandoned
- `job.ENDED` → Interrupted
- `window.CLOSED` → Interrupted

### Presented
The developer answers. Confirmed answers and answers not yet confirmed are kept by Weaver, so the round stays as it was when he moves around, reduces it or comes back to it.
- `developer.SEND_ANSWERS` → Answered: only when every question is answered or skipped.
- `developer.SKIP_ALL_QUESTIONS` → Answered: every question without a confirmed answer is skipped, confirmed answers stay.
- `agent.STOP_WAITING` → Abandoned
- `job.ENDED` → Interrupted
- `window.CLOSED` → Interrupted
- `developer.ASK_MORE` → Presented: on the question in view; the round is reduced and nothing else in it changes, neither the question in view nor any answer, confirmed or not. Only the questions, never the answers, reach the agent of the new Ask more conversation.

### Answered (final)
The agent receives every answer, with the skipped questions marked as skipped: for each one it uses its recommendation and lists it as a decision to verify. A skipped prepared Drift question leaves the Drift open.

### Abandoned (final)
The agent stopped waiting; no answer is delivered.

### Interrupted (final)
Its job ended or its window closed before it was answered; every answer is discarded, and the agent, if still there, is told so and stops the work that waited, without using any recommendation.

### Unavailable (final)
The agent is told that Weaver cannot ask, must not ask in any other way, and must stop and tell the developer.

## Events

### OPENED
Emitted when the round is presented.

### CLOSED
Emitted when the presented round ends, however it ends.

### ANSWERED
Emitted when the developer has sent the round.
