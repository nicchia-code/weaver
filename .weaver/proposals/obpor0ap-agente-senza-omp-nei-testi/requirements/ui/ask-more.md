---
description: "Use when building or changing Ask more: its buttons, its conversations in the Jobs view and their texts."
---
# Ask more

## ask-conversation

### The buttons
- Every `Ask more` uses the icon `MessageCircleQuestion` and is always enabled: no job, Run, resolution, question round or other work disables it.
- **Details dialogs** of a Proposal, a Task and a Drift: a `Button` in the `outline` variant, icon to the left of the text `Ask more`, placed in the footer as `requirements/ui#proposal`, `#task` and `#drift` say. Tooltip `Ask an agent about this Proposal`, `Ask an agent about this Task` or `Ask an agent about this Drift`; accessible name `Ask more about <short number>` (the permanent identity when the card has no short number).
- **History entry:** in the footer of a History entry's dialog (`requirements/ui#history-entry`), a `Button` in the `outline` variant, icon to the left of the text `Ask more`. Tooltip `Ask an agent about this History entry`; accessible name `Ask more about <entry identity>`. The Tasks of the entry have no `Ask more` of their own.
- **Question round:** on the question in view (`requirements/ui#question-round`), a small icon-only `ghost` `Button`. Tooltip `Ask an agent about this question`; accessible name `Ask more about this question`.
- **Requirements view:** one `Ask more` for the Requirements as a whole, the only control on the right of the view's header (`requirements/ui#app`): a small `ghost` `Button` with the icon and the text `Ask more`, shown also with a focus or `Only these` on and with any overlay. Section, fragment and Area headings have none, and the Specs view has none. Tooltip `Ask an agent about the Requirements`; accessible name `Ask more about the Requirements`.
- **Pressing it:** the window shows the Jobs view with the new conversation selected and the focus in its message box. A details dialog, a History entry's dialog and the Requirements view (overlay, focus, `Only these`, scroll position) stay as they were and show again as they were when the developer goes back to them. A question round is minimized to its bar, with the question in view, choices and written text unchanged.
- When the chosen thing no longer exists, nothing opens and nothing changes.

### Row in the Jobs view
- Name, set at opening: `Ask <short number>` (`Ask T3`, `Ask P2`, `Ask D4`; the permanent identity when the card has no short number), `Ask history <entry identity>` (`Ask history k3m9x2qa`), `Ask Requirements` or `Ask question`. Equal names are told apart by the opening time.
- Row: name (semibold), kind `ask` in secondary text, state badge, opening time as the started time of `requirements/ui#job`; no running time and no note line.
- Badges, always with text: `running` and `waiting` as for a job, `running` during a turn and `waiting` while a permission waits for the developer; `idle` (outline, no icon) before the first message and between turns.
- `X` on the row in every state: tooltip `Close`, accessible name `Close <name>`. Without a turn in progress it closes at once. During a turn it opens an AlertDialog `Close <name>?` / `The agent's turn stops and the conversation ends.`, `Close` (destructive) and `Cancel`, focus on `Cancel`.
- A waiting permission does not count in the sidebar's number of jobs waiting and raises no notification.

### Conversation
- Rendered as a job's conversation (messages, `Thinking`, tool rows, sub-agents and their read-only conversations, permission cards, queued messages, `Jump to latest`), with no instruction entry, notes, end line or finished line. The sub-agent conversation header starts with the conversation's name: `Ask T3 › <sub-agent name>`.
- The first entry is the developer's first message, as for any developer message; Weaver's instruction is never shown.
- Empty, it shows only the message box, with the placeholder `Ask about the Proposal…`, `Ask about the Task…`, `Ask about the Drift…`, `Ask about the History entry…`, `Ask about the Requirements…` or `Ask about the question…`. After the first message the placeholder is `Message the agent…`.
- Message box as for a job; a message sent while the agent starts or reloads shows `Starting Weaver…` under it until delivered, as in Chat. `Stop` (`Square`, `Stop the agent`) beside `Send` during a turn, without a dialog.
- **Back to questions:** a conversation opened from a question shows, at the top of the conversation and while that round is still open, a `Button` `Back to questions` (`ArrowLeft`) that opens the round again where it was. It disappears when the round ends, however it ends.
- Started jobs, refused starts, failures, a missing agent and a session that could not be reloaded show as in Chat (`requirements/ui#general-chat`), with the same texts.
- No usage bar: usage stays in the `Usage` dialog.
- Every text is in English.
