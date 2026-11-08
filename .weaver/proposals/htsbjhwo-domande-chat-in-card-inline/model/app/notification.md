# Notification

A system notification that tells the developer it is his turn: a job of a project finished, stopped abnormally, or waits for his answers or his permission; or Weaver turned on System Certificates by itself. A notification of a job is shown at once, and only when that project's window is not in the foreground; otherwise nothing is shown. It names the project, the job and what happened, never what the agent wrote, asked or did. Jobs the developer stopped himself never notify, and General never notifies: its cards of questions show only in its conversation and in the sidebar. A notification changes nothing in the project, the jobs or the rounds.

## States

### Not shown (initial)
- `job.ENDED` → Shown: when the job finished or failed and the project's window is not in the foreground.
- `job.WAITING` → Shown: when the project's window is not in the foreground.
- `system-certificates.AUTO_ENABLED` → Shown: always, also when a Weaver window is in the foreground; it belongs to no project.

### Shown
- `desktop.NOTIFICATION_CLICKED` → Clicked
- `question-round.CLOSED` → Withdrawn: the round it announced closed.
- `developer.ANSWER_PERMISSION` → Withdrawn: the permission it announced was answered.
- `job.ENDED` → Withdrawn: it announced a wait of a job that has now ended.

### Clicked (final)
The developer is brought to the project, or to Settings for System Certificates.

### Withdrawn (final)
It is no longer true and leaves the system's list.

## Events

### CLICKED
Emitted when the developer clicks it.
