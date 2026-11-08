---
description: "Use when building or changing question rounds or system notifications."
---
# Question rounds and notifications

## question-round
- **Overlay:** a modal Dialog over the whole window, any view, background dimmed; up to ~760 px wide and 85% of the window high, content scrolling inside. `Esc` and outside clicks do nothing. Focus is trapped and moves to the question text on each question; no transitions under reduced motion.
- **Header:** `Questions from the agent · <job name>`, `Questions from the agent · Chat` for a round of Chat (prepared Drift question: `Question about <D3> · <Drift title>`), below `Question <n> of <N>` or `Summary`; with other rounds waiting, `+<k> more round(s) waiting`. Right: `Minimize` (`Minus`) and `Close` (`X`), which opens the Skip-all confirmation.
- **Index:** left column, one entry per question (number, one-line text with full text in the tooltip, status icon + text: `Answered`, `Skipped`, `Missing`), last entry `Summary`. Below 600 px window width: a row of numbered dots with the same accessible status.
- **Question:** text in full, context below in muted text, both Markdown, in the agent's language; every label of the overlay is English. `Ask more` (see `ask-conversation`) sits at the top right of the question text, on its first line; it is not in the Summary, the Skip-all confirmation or the minimized bar.
- **Options:** full-row radio (single) or checkbox (multiple) with key hint `1`–`9`, bold plain-text label, full Markdown description, `Recommended` badge; the recommended option is preselected on the first visit only. `Show details` / `Hide details`, collapsed on every visit. Clicking a selected single option clears it. A digit selects (single) or toggles (multiple); digits are inert while the textarea has focus. Arrows move within the group.
- **Free answer:** textarea `Your answer (optional)`, placeholder `Write a free answer or a note for the agent`, growing to 10 lines; text reaches the backend within 300 ms of the last keystroke and on blur.
- **Footer:** `Back`, `Skip all`, `Skip`, `Next` (`Review` on the last). `Next`/`Review` disabled, tooltip `Choose an option or write an answer.`, until a choice or non-blank text exists. `Ctrl/Cmd+Enter` = `Next`/`Review`/`Send to agent`.
- **Skip all:** AlertDialog `Skip all unanswered questions?` / `The agent will use its recommendation for each of them.`, `Skip all` and `Go back`, focus on `Go back`. Never kept with the round.
- **Summary:** each answer (labels and text), `Skipped`, or `Missing` (warning colour, `TriangleAlert`), with `Edit` (`Pencil`) going to that question. `Send to agent` (primary) disabled while any is missing, with `Answer or skip <n> more question(s) to send.`
- **Minimized:** a bar at the bottom of the work surface (`requirements/ui#window`), in every view, Chat and Ask more conversations included, next to their own `Answer` and `Back to questions`: `The agent is waiting for your answers (question <n> of <N>)` + `Open questions`.
  - It takes its own place in the layout, below the selected view, and is never drawn over it: the view gets shorter by the bar's height, so nothing of it is covered, neither a message box nor the end of a list, a Board column, an editor or a sheet. The sidebar is untouched.
  - Full width of the work surface, inside its inset and rounded corners; Peach tint background, text and icons in Peach ink, a 1 px Peach top border.
  - One line at a fixed height, whatever the window width: the text is cut with `…`, with its full text in the tooltip; `Open questions` stays whole at the right end.
  - While it shows, toasts sit above it (`requirements/ui#General`).

## notification
| Kind | Title | Message |
|---|---|---|
| Finished | `<project> – <job> finished` | `Weaver is waiting for you.` |
| Several finished | `<project> – <n> jobs finished` | `Weaver is waiting for you.` |
| Failed | `<project> – <job> stopped` | `The agent stopped before finishing its work.` |
| Question round | `<project> – <job> asks a question` | `The agent is waiting for your answers.` |
| Permission | `<project> – <job> asks permission` | `The agent is waiting for your approval.` |
| System Certificates turned on | `System Certificates turned on` | the text of `requirements/ui#system-certificates` |

- `<project>` is the folder name, `<job>` the job's fixed name, or `Chat` for a round of Chat, which only ever raises the Question round kind. Never any message, question, command, path, answer or card. Only the System Certificates notification names a product, and it belongs to no project.
- **Grouping:** a job finishing while a finished notification of the same project is shown replaces it with `<n> jobs finished`; at most one finished notification per project. The other kinds are one per event, never grouped.
- Each notification has an ID of project + job (or Chat) + kind to replace or withdraw it. Where the system cannot remove a delivered one, clicking a withdrawn one only brings the window forward.
- **Click target:** finished/failed → that job's conversation (several → the Jobs view); question → the round overlay; permission → the conversation scrolled to the card; System Certificates turned on → the Settings view of the project window used most recently, else the welcome window.
- App name and icon, system default sound; no setting.
