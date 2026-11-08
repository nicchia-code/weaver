---
description: "Use when building or changing question rounds, the cards of questions in Chat, or system notifications."
---
# Question rounds and notifications

## question-round
- **Overlay:** a modal Dialog over the whole window, any view, background dimmed; up to ~760 px wide and 85% of the window high, content scrolling inside. `Esc` and outside clicks do nothing. Focus is trapped and moves to the question text on each question; no transitions under reduced motion. Only rounds of jobs and prepared Drift questions use it; a card of questions of Chat never does.
- **Header:** `Questions from the agent · <job name>` (prepared Drift question: `Question about <D3> · <Drift title>`), below `Question <n> of <N>` or `Summary`; with other rounds waiting, `+<k> more round(s) waiting`. Right: `Minimize` (`Minus`) and `Close` (`X`), which opens the Skip-all confirmation.
- **Index:** left column, one entry per question (number, one-line text with full text in the tooltip, status icon + text: `Answered`, `Skipped`, `Missing`), last entry `Summary`. Below 600 px window width: a row of numbered dots with the same accessible status.
- **Question:** text in full, context below in muted text, both Markdown, in the agent's language; every label of the overlay is English. `Ask more` (see `ask-conversation`) sits at the top right of the question text, on its first line; it is not in the Summary, the Skip-all confirmation or the minimized bar.
- **Options:** full-row radio (single) or checkbox (multiple) with key hint `1`–`9`, bold plain-text label, full Markdown description, `Recommended` badge; the recommended option is preselected on the first visit only. `Show details` / `Hide details`, collapsed on every visit. Clicking a selected single option clears it. A digit selects (single) or toggles (multiple); digits are inert while the textarea has focus. Arrows move within the group.
- **Free answer:** textarea `Your answer (optional)`, placeholder `Write a free answer or a note for the agent`, growing to 10 lines; text reaches the backend within 300 ms of the last keystroke and on blur.
- **Footer:** `Back`, `Skip all`, `Skip`, `Next` (`Review` on the last). `Next`/`Review` disabled, tooltip `Choose an option or write an answer.`, until a choice or non-blank text exists. `Ctrl/Cmd+Enter` = `Next`/`Review`/`Send to agent`.
- **Skip all:** AlertDialog `Skip all unanswered questions?` / `The agent will use its recommendation for each of them.`, `Skip all` and `Go back`, focus on `Go back`. Never kept with the round.
- **Summary:** each answer (labels and text), `Skipped`, or `Missing` (warning colour, `TriangleAlert`), with `Edit` (`Pencil`) going to that question. `Send to agent` (primary) disabled while any is missing, with `Answer or skip <n> more question(s) to send.`
- **Minimized:** a warning-colour bar pinned at the window bottom: `The agent is waiting for your answers (question <n> of <N>)` + `Open questions`.

## question-round/Active in General
- **Place:** a card inside Chat's conversation (`requirements/ui#general-chat`), as wide as the conversation, under everything of the turn that published it: live, it stays below the agent's later text and steps of that turn; after a reload, each closed card shows right before the developer message that closed it and the active one at the end. It is never inside the work summary and never covers or dims anything.
- **Look:** the frame of Chat's Proposal card: Surface, a 1 px `#E2DDF0` ring and a soft Lavender shadow, radius 14 px. Head, on the Lavender tint with a `#ECE7F6` bottom rule: a 32 px Lavender tile with `MessagesSquare`, `Questions from Weaver` (16 px semibold), below it `Question <n> of <N>` or `Summary` in secondary text; then, on a row of its own, the numbered dots of the overlay's narrow index (one per question, then `Summary`, with the same accessible status), which move to that question or the Summary. No index column, no `Minimize`, `Close`, `Skip all` or `+<k> more round(s) waiting`.
- **Body and controls:** as the overlay's **Question**, **Options**, **Free answer** and **Summary**, with `Ask more` on the question in view: footer `Back`, `Skip`, `Next` (`Review` on the last), with the same disabled state and tooltip; the Summary with `Edit` and `Send to agent`, disabled while any question is missing with `Answer or skip <n> more question(s) to send.`
- **Focus and keys:** the card never takes the focus by itself, neither when it appears nor when the conversation changes; the message box keeps it. Once the focus is inside the card, `Back`, `Next`, `Review`, `Edit` and the dots move it to the question text (or to the Summary's heading); `1`–`9` and `Ctrl/Cmd+Enter` work as in the overlay, only while the focus is inside the card and never in its textarea for the digits.
- **Closed:** once sent, or closed by a message, the card shows the read-only Summary of what reached the agent: each question with its answer (labels and text), `Skipped`, or `Not answered` in Muted; no controls, no `Edit`, no `Ask more`, no dots. A pill in the head says how it closed: `Answered` (Sage) when every question was answered or skipped, `Partly answered` (Peach) when some reached the agent as not answered, `Not answered` (neutral) when none was answered or skipped.
- **Delivery:** a card sent from its Summary adds no developer message to the conversation; while its answers wait to be delivered, because the agent starts or reloads, `Starting Weaver…` shows under the card, as under a message.
- Every label of the card is English; questions, contexts and options are in the agent's language.

## notification
| Kind | Title | Message |
|---|---|---|
| Finished | `<project> – <job> finished` | `Weaver is waiting for you.` |
| Several finished | `<project> – <n> jobs finished` | `Weaver is waiting for you.` |
| Failed | `<project> – <job> stopped` | `The agent stopped before finishing its work.` |
| Question round | `<project> – <job> asks a question` | `The agent is waiting for your answers.` |
| Permission | `<project> – <job> asks permission` | `The agent is waiting for your approval.` |
| System Certificates turned on | `System Certificates turned on` | the text of `requirements/ui#system-certificates` |

- `<project>` is the folder name, `<job>` the job's fixed name. Chat raises none of them. Never any message, question, command, path, answer or card. Only the System Certificates notification names a product, and it belongs to no project.
- **Grouping:** a job finishing while a finished notification of the same project is shown replaces it with `<n> jobs finished`; at most one finished notification per project. The other kinds are one per event, never grouped.
- Each notification has an ID of project + job + kind to replace or withdraw it. Where the system cannot remove a delivered one, clicking a withdrawn one only brings the window forward.
- **Click target:** finished/failed → that job's conversation (several → the Jobs view); question → the round overlay; permission → the conversation scrolled to the card; System Certificates turned on → the Settings view of the project window used most recently, else the welcome window.
- App name and icon, system default sound; no setting.
