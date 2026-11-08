# Job

One piece of work Weaver gives the agent: a setup, a design on a written request, an import, a scan of the whole System, a Run of one Task or of all runnable Tasks, a resolve of one Drift, the rework of a rejected Proposal, the refinement of a stale Proposal, or the resolution of the conflicts of a merge in progress. It has a fixed name, one omp process and one ACP session, and a conversation in Weaver that shows the agent's messages, its tool calls and its sub-agents. The developer can open the conversation of each of its sub-agents, and of the sub-agents those start, to read what it is doing, while the job works and after it ends, as long as the job is listed; these conversations are read-only. A job owns the Tasks it holds, the cards it announced and the question rounds it asked; when it ends, for any reason, all of them are released or withdrawn in the same step. A card a job handed over can be decided or run only once that job has ended. A job ends only on a definite fact, never because time passed or because the agent seems idle. From the moment a setup, design or import job is created until it ends, the Board always shows at least one ghost of it: whenever none of the cards it announced is still waiting to arrive, Weaver shows a ghost of its own for the job, without waiting for the agent; for a setup or a design it bears the title of the request the job received. While a setup or design job holds its request, the developer can open that request from the job's conversation and read it.

When a design, a rework, a refinement or a Run ends, whatever the way it ends, Weaver measures it on the developer's computer, so that the developer can compare jobs before and after a change of the way the agent works: how long it lasted, how long the agent actually worked (without the waits for the developer, the queue and the suspensions), how many files the agent read and how many distinct files, how many searches it made, and how many tokens it used, for the main agent, its sub-agents and the advisors of a Run separately. When Weaver stopped unexpectedly while the job worked, the time the agent worked before that stop and after its last change of state is unknown: it is not counted, and the measure says the worked time is incomplete. When what omp kept of the session cannot be read, the job is still measured, with its durations and its end, and the rest marked as not available with the reason. The measure never delays the end of the job nor changes it, is never removed by Weaver, and can be read with the command line.

## States

### Not started (initial)
No job exists yet. Only one Run works at a time in a project, Run all counting as one Run even when it starts one per model, and a Run and a resolve exclude each other; scans, designs and imports can always start, except that while a merge is in progress (see `solo-merge`) Runs, Run all, resolves, scans and imports are refused, and only designs, reworks, refinements and the merge conflicts job start.
- `developer.SEND_REQUEST` → Queued: a design job on the request, or a setup job when the Model is empty.
- `agent.SEND_REQUEST` → Queued: a design job on the request the agent of Chat wrote and sent on the developer's explicit request, or a setup job when the Model is empty, with the same refusals as when the developer sends a request.
- `developer.IMPORT_CODE` → Queued: an import job, leaving out the folders the developer excluded; refused while a merge is in progress.
- `developer.SCAN_SYSTEM` → Queued: a scan job on the whole System; refused while a merge is in progress.
- `solo-merge.CONCLUDED` → Queued: a scan job on the whole System, started by Weaver with no confirmation, telling the agent which files the merge brought; it counts as the last Scan System, like one the developer asked for.
- `developer.RUN_TASK` → Queued: a Run of that Task; refused when it is blocked or held, when another Run or a resolve is working, or while a merge is in progress.
- `developer.RUN_ALL` → Queued: a Run of the Tasks in To do that are not blocked, one Run for each model they need, started together; refused when there is none, when another Run or a resolve is working, or while a merge is in progress.
- `proposal.APPROVED` → Queued: a Run of all the Tasks, after a setup Proposal approved when handed over.
- `developer.RESOLVE_DRIFT` → Queued: a resolve job on that Drift, when it has no valid prepared question; refused while a merge is in progress.
- `question-round.ANSWERED` → Queued: a resolve job, when the answer to a prepared Drift question needs the agent.
- `developer.REWORK_PROPOSAL` → Queued: a rework job with the skill of the Proposal's kind and the feedback; for a resolution, only on its Drifts still open, and none when no Drift is open.
- `developer.APPROVE_PROPOSAL` → Queued: a refinement job, when the Proposal approved must be refined first, with the skill of the Proposal's kind as for a rework; for a resolution, only on its Drifts still open.
- `developer.REFINE_DRIFT_PROPOSAL` → Queued: a refinement job on the Proposal attached to that Drift, with the skill of its kind as for a rework.
- `developer.RESOLVE_MERGE_CONFLICTS` → Queued: a merge conflicts job on the merge in progress, with the analysis model of the Settings; the agent resolves the conflicts, asks the developer with a question round when the choice between the two versions is not obvious, marks in git the files it resolved, and never commits nor aborts the merge. Refused when no merge is in progress, when no file still has conflicts, or when another merge conflicts job has not ended.
- `agent.START_JOB` → Queued: the job the agent of General or of an Ask more conversation asked for on the developer's explicit request, with the same refusals as when the developer asks for it.

### Queued
Created, waiting for omp to be ready and for the omp process to open its session. Weaver starts omp in the project, with the `weaver` command line on its path and the job known to it, and sends the instruction.
- `omp.SESSION_STARTED` → Running: the job counts as started only now.
- `omp.START_FAILED` → Failed: with the reason; nothing was announced or held.
- `developer.STOP_AGENT` → Stopped
- `window.CLOSED` → Stopped: it had not started yet.

### Running
The agent works on the instruction.
- `agent.DECLARE_NOTE` → Running: the note shows on the job and on its ghosts, with the earlier ones.
- `agent.ANNOUNCE_CARD` → Running: a ghost of the card appears on the Board until the card arrives or the job ends; for a setup, design or import, Weaver's own ghost of the job leaves in the same picture, and comes back when every card it announced has arrived, while the job has not ended.
- `developer.SEND_MESSAGE` → Running: the agent receives it while it works, without stopping.
- `question-round.OPENED` → Waiting for the developer: only a round of this job.
- `omp.PERMISSION_REQUESTED` → Waiting for the developer
- `omp.TURN_ENDED` → Done: when no message of the developer is still waiting to be delivered.
- `omp.TURN_ENDED` → Running: when messages of the developer still wait; they start a new turn.
- `omp.TURN_FAILED` → Failed: with the reason.
- `omp.EXITED` → Failed: with omp's last lines about the error.
- `developer.STOP_AGENT` → Stopped: omp's turn is cancelled and every process it launched is ended.
- `window.CLOSED` → Suspended

### Waiting for the developer
The agent waits for the answers of a round or for a permission. It still counts as working.
- `question-round.CLOSED` → Running
- `developer.ANSWER_PERMISSION` → Running
- `developer.SEND_MESSAGE` → Waiting for the developer: the message waits and never answers the round or the permission.
- `omp.TURN_ENDED` → Done: a permission still asked is void.
- `omp.TURN_FAILED` → Failed: with the reason.
- `omp.EXITED` → Failed: with omp's last lines about the error.
- `developer.STOP_AGENT` → Stopped: a permission still asked is cancelled.
- `window.CLOSED` → Suspended

### Suspended
The project's window was closed, or Weaver quit or stopped unexpectedly, while the job worked. omp was ended; Weaver kept, outside the project, what it needs to resume the session, including which of the cards it announced have already arrived. The Tasks it holds and its ghosts stay, Weaver's own ghost of a setup, design or import included.
- `window.OPENED` → Suspended: Weaver starts omp again and reloads the session.
- `omp.SESSION_RESUMED` → Running: the agent is told that Weaver stopped and that it must continue from where it stopped.
- `omp.SESSION_NOT_RESUMED` → Failed: the session could not be resumed.
- `developer.STOP_AGENT` → Stopped

### Done (final)
The agent finished its turn. The conversation stays readable but takes no more messages.

### Failed (final)
The job ended abnormally, with a reason the developer can read.

### Stopped (final)
The developer stopped it, or it never started.

## Events

### WAITING
Emitted when the job starts waiting for the developer.

### ENDED
Emitted when the job reaches Done, Failed or Stopped.
