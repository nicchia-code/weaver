# Question round

A group of related decisions the agent asks the developer at once: each question with its options, a recommendation, single or multiple choice and an optional longer context, and always room for a free answer. The developer answers one question at a time, can go back to any of them, and sends the answers from a summary of every answer. A round is also how Weaver itself asks the prepared question of a Drift. A round belongs to its job, or to General, and to its project; it never shows in another project's window, and neither Weaver nor the agent sets a time limit. Questions and answers are never written to the project.

The agent of a job asks a round, which comes over the whole window and holds the job until it is answered. The agent of General instead publishes a card of questions inside its conversation: the card never holds the agent's turn, never waits in the project's queue of rounds and never raises a notification, and the developer may answer it or ignore it and go on writing. At most one card per turn of General, and only the card of the latest one is active.

## States

### Not asked (initial)
- `agent.ASK_QUESTIONS` → Unavailable: when the caller is neither the agent of a working job of Weaver nor the agent of the project's General during a turn, or Weaver is not running.
- `agent.ASK_QUESTIONS` → Not asked: only the agent of General, when it already published a card in this turn; nothing is published, and the agent is told to put every question of the turn in one card.
- `agent.ASK_QUESTIONS` → Active in General: only the agent of General during a turn, with its first card of the turn; the card shows in the conversation at once and the agent is told that it was published, that its turn should end and that the answers will come as a message of the developer.
- `agent.ASK_QUESTIONS` → Queued: only the agent of a job, when another round of the same project is presented.
- `agent.ASK_QUESTIONS` → Presented: only the agent of a job, otherwise; the project's window comes to the front.
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

### Active in General
The card of questions shows in General's conversation, under the reply of the agent that published it, after everything of that turn. The developer answers it one question at a time, or ignores it. Its confirmed answers and skips are kept with the conversation, also across closing the window and quitting Weaver, and come back with it; choices and text not yet confirmed are kept only while the window is open. It stays active when the turn ends, is stopped or fails, and when omp exits. While it is active, General's entry in the sidebar shows that General has questions for the developer.
- `developer.SEND_ANSWERS` → Answered: only when every question is answered or skipped; the answers reach General's agent as a new message of the developer.
- `developer.SEND_MESSAGE` → Answered with a message: only a message written in General.
- `developer.ASK_MORE` → Active in General: on the question in view; nothing in the card changes, neither the question in view nor any answer, confirmed or not. Only the questions, never the answers, reach the agent of the new Ask more conversation.
- `developer.CLEAR_GENERAL` → Removed
- `omp.SESSION_NOT_RESUMED` → Removed: only when General's kept conversation could not be reloaded.

### Answered (final)
The agent receives every answer, with the skipped questions marked as skipped: for each one it uses its recommendation and lists it as a decision to verify. A skipped prepared Drift question leaves the Drift open. A card of General stays in the conversation as the read-only summary of the answers sent.

### Answered with a message (final)
The developer wrote in General while the card was active. His message reaches the agent with the confirmed answers and the skips of the card; every other question, a choice or a text not confirmed included, reaches the agent as not answered: it stays open, and the agent uses no recommendation for it. The card stays in the conversation as the read-only summary of what was sent.

### Removed (final)
The card went away with General's conversation, because the developer started a new one or the kept one could not be reloaded; nothing reaches the agent.

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
