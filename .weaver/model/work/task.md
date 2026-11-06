# Task

A unit of work for the agent: a title and a body that say, in domain words, what must be true when it is done, with the Model elements and Requirement sections it concerns. A technical Task names no Model element; every other Task names at least one. A Task is born with the Proposal that carries it, when that Proposal is approved; filed by the agent during a scan or a Run; or created by the developer's decision to fix the code of a Drift, as that Drift's fix Task. A Task born with a Proposal, or the fix Task of a Drift whose origin was born with one, belongs to that Proposal's design.

A Task written by a design, a scan or a compare may also carry code notes for whoever implements it, made only of what the agent already read of the code: where to look, the conventions to follow, the pitfalls seen and what can be reused, with the commit on which the code was read. They are hints, never part of what must be true. Whoever implements the Task starts from them and explores again only the files that changed since that commit, or all the files they cite when that cannot be told; it always reads a file before changing it, and when a note contradicts the code, the code wins. A Task without code notes, and every fix Task, is implemented as before, reading the code it needs.

A Task is blocked while an open Drift names it as its origin or concerns the same thing, and while the fix Task of its own Drift is not finished. Blocking is worked out every time, never stored, and never moves a Task.

## States

### To do (initial)
Waiting to be carried out.
- `agent.TAKE_TASK` → In progress: only an agent of a working job, and only when the Task is not blocked; otherwise it is refused, naming what blocks it. The job holds it.
- `developer.DELETE_TASK` → Deleted: refused when an open Drift names it as its origin, when it is the fix Task of a Drift whose code is to be fixed, or when the job that filed it is still working; it leaves no trace.

### In progress
The agent of exactly one job works on it.
- `agent.FINISH_TASK` → Done: only the job that holds it.
- `agent.CLOSE_TASK_UNCHANGED` → Done: only the fix Task of a Drift, held by the job, with a note saying why the code did not change.
- `agent.RELEASE_TASK` → To do: only the job that holds it.
- `job.ENDED` → To do: the job that held it ended without finishing it, whatever the reason.
- `drift.CODE_TO_BE_FIXED` → To do: only the Task named as origin of that Drift; it waits for the fix Task.

### Done
Finished. It leaves the Board at once but stays available, so that a History entry can cite it.
- `history-entry.TASKS_CITED` → Recorded in History: when an entry cites it; a copy of it, text included, stays in every entry that cites it.
- `FINISHED` → Forgotten: when 20 other Tasks of the project have finished after it and no entry cited it, unless an open Drift names it as its origin.

### Deleted (final)
Deleted by the developer before it was carried out.

### Recorded in History (final)
Readable only inside the History entries that cite it.

### Forgotten (final)
Removed without a trace.

## Events

### FINISHED
Emitted when a Task reaches Done because the agent finished it.

### CLOSED_UNCHANGED
Emitted when the fix Task of a Drift is closed without changing the code.
