---
description: "Use when building the ACP client of jobs and General: command line, process lifetime, environment, sessions and resume."
---
# ACP client

## job
- **Command line:** `<omp> [--model <m>] [--slow <scan>] [--advisor] --config <overlay> --approval-mode <yolo|always-ask> acp`.
  - The working folder is the project root, with stdio piped.
  - `--model` is passed only for a non-empty role or Proposal model. A thinking suffix `:<level>` is passed whole, in `--model`, `--slow` and the advisor role.
  - `--slow` is passed only for a Scan, whose skill hands each Machine to the `weaver-analysis` sub-agent, and only when the scan model is set. The job's kind decides, not the text of the instruction.
  - `--advisor` is passed only for Runs.
- **Process lifetime:** omp never outlives Weaver. On Unix it runs in a new process group, with `PR_SET_PDEATHSIG(SIGTERM)` on Linux. On Windows it runs in a Job Object with `KILL_ON_JOB_CLOSE`.
- **Environment:**
  - Inherited `WEAVER_*` variables are removed.
  - Weaver adds `WEAVER_JOB=<job id>` and `WEAVER_ENDPOINT`. Runs also get `WEAVER_RUN=1`, and Runs of one Task get `WEAVER_TASK=<task id>`.
  - PATH is set as in `## omp`, with the certificate variables of `## omp`.
- **Overlay:** `<app local data>/overlays/<job id>.yml`, never in the repository, deleted when the job ends.
  - Always: `bash.autoBackground.enabled: false` and `tools.maxTimeout: 0`. The skills tell the agent to run `weaver question new` with bash `timeout: 0`, since omp's default is 300 s.
  - Runs also get `modelRoles.advisor` set to the quoted scan model, only when set, and `task.agentAdvisor` enabled for `weaver-implementer`.
  - Acceptance: at the minimum version, a `weaver question new` left unanswered for 10 minutes still gets its answers.
- **Advisor:** in Runs, omp's advisor reads the installed `.omp/WATCHDOG.yml`. It defines two advisors with tools `read`, `grep`, `glob` and `bash`, and no model:
  - a turn reviewer (`reviewMode: turn`, `syncBacklog: off`);
  - a final reviewer (`reviewMode: agent-end`, `syncBacklog: strict`), so the turn ends only after its review.

  Their instructions run `weaver job show` first in each session. Without the Run marker they stay silent.
- **Opening:**
  - One 30 s limit covers spawn, `initialize` and `session/new` (or `session/load`).
  - `initialize` sends protocol version 1 and `clientCapabilities` without `fs` and `terminal`, with `elicitation.form`. Missing `loadSession` or `_meta.steering.supported` is a start failure.
  - `SESSION_STARTED` is when `session/new` returned and the instruction went out as `session/prompt`. An error reply to that prompt before any `session/update` is `START_FAILED`.
- **Forms:** every elicitation or omp ask request in a job is answered `decline` at once, with the fixed text: "In Weaver jobs, ask the developer only with `weaver question new` (bash timeout 0). Do not ask in any other way." The UI receives nothing.
- **Permissions:**
  - `yolo`: a request that still arrives is answered with the first `allow_always` option, else `allow_once`.
  - `ask`: the request goes to the UI with omp's own options and no time limit. Stop and close answer `cancelled`. The end of the turn voids it.
- **Messages:**
  - While Running, a message goes out as `_session/steering` with no `_meta`.
  - Replies:
    - `injected`: delivered;
    - `promptRequired`: sent as `session/prompt`, then delivered;
    - `failed` or an error: failed, with the reason.
  - While Waiting for the developer, Weaver holds messages as queued. They go out with `_meta: {steering: {queue: "followUp"}}` once the wait ends. Stop discards held messages.
- **Turn end:** the reply to `session/prompt` carries the stop reason.
  - `end_turn` and `cancelled` are `TURN_ENDED`.
  - `refusal`, `max_tokens`, `max_turn_requests` and request errors are `TURN_FAILED`.
  - An exit Weaver did not ask for, or a closed connection, is `EXITED`, once.
- **Shutdown:** in order:
  1. `session/cancel` if a turn runs;
  2. `session/close`, waiting at most 2 s;
  3. SIGTERM to the process group (Windows: `TerminateJobObject`);
  4. SIGKILL to the group after 3 s.

  A shutdown Weaver started produces no `EXITED`.
- **Stderr:** the last 40 lines (at most 4 KB) are kept in memory only. Reasons add them after the exit code or signal.
- **Resume registry:**
  - `<app local data>/jobs/<job id>.json`, written atomically at every state change and deleted when the job ends.
  - It holds the job id, project root, kind, name, ACP session id, arguments, the env Weaver set (never the inherited env), the state, and the pid plus process start time.
- **Resume:**
  1. Weaver kills any leftover group whose pid and start time match.
  2. It spawns omp again with the same arguments and env, plus the certificate variables of that moment, and rewrites the overlay.
  3. It runs `initialize` and `session/load` within 30 s, then sends the prompt "Weaver stopped and reloaded your session. Continue your work from where it stopped."

  The jobs of a project resume together. The replayed history reaches the UI in one batch at the end of the load.

  `session/load` is sent ordered with the incoming `session/update` notifications, as `session/prompt` is: the load ends when its answer is handled, always after every replayed update read before it. No replayed update reaches the live conversation, and the first agent message after the load starts a new entry.
- **Sub-agents:** a job's direct sub-agents are read from its `task` tool calls' `rawOutput.details.progress[]`: for each entry its `id`, its name (the first of `name`, `agent`, `description`, `id`) and its status.
  - Status mapping, for every source below:
    - `pending` and `running`: working;
    - `completed`: succeeded;
    - `failed`: failed;
    - `aborted`: interrupted.
  - A sub-agent is identified by its `task` call and its `id`. Launch order is the order of the `task` calls, then omp's order inside each.
  - **Background sub-agents** (omp `async.enabled`): the `task` call returns while they are still pending, so their status is taken from every later update of omp's tools that names them: the `task` progress and the `wait` tool call's `rawOutput.details.jobs[]` (`id`, `status`). The `session_exit` record of their transcript stays as a fallback: `kind: normal` means succeeded, an error means failed, anything else means interrupted.
  - **Transcript:** `<main session file minus .jsonl>/<id>.jsonl`, under omp's sessions folder (main file: name ending `_<sessionId>.jsonl`); a nested sub-agent's is `<parent transcript minus .jsonl>/<id>.jsonl`.
    - The `id` must match `^[A-Za-z0-9_.-]+$` without a leading `.`, and the real path must stay in that folder.
    - Files whose name starts with `__advisor.` are omp's advisors: never listed or shown.
    - JSON Lines that omp appends while the sub-agent works. Weaver reads each transcript incrementally from the last offset read, never whole again; a last line not yet complete is read again at the next pass. A transcript is read only while its sub-agent's conversation is shown, its job's sub-agent list is shown, or its status still waits on the `session_exit` fallback, every 2 s while new records may come.
    - Records become entries of the same kinds as the job conversation: the first `message` with `role: user` is the assignment; `assistant` `content[]` gives `text` (agent message), `thinking` (`Thinking`) and `toolCall` (`id`, `name`, `arguments`, `intent`: a tool row); a `toolResult` (`toolCallId`, `isError`) ends the matching row. Other records are ignored.
    - Nested sub-agents, and their status, come from the `task` and `wait` tool results of the parent's transcript, with the same rules as the job's.
    - A missing file, an unreadable file or a malformed complete line makes that conversation `Conversation not available: <reason>`; the sub-agent keeps its status from the other sources.
  - The UI gets a sub-agent's conversation as a snapshot plus change events, as for a job, keyed by project, job (or General) and the path of `task` call and `id` from the job down to that sub-agent.
  - After `session/load` (resume or reload), the sub-agents are rebuilt from the replayed history and their transcripts are read again from the start.
  - Any sub-agent still working when omp exits or the job ends becomes interrupted, nested ones included.
  - These omp internals (`progress[]`, the `wait` details, transcript folders and records) are re-verified at each `MIN_OMP_VERSION` bump.
- **Measure:** when a design, rework, refinement or Run ends (`requirements/data#job`), Weaver reads omp's session files once, after the end has been applied and the end shown, off the UI and IPC threads; the line is written when the reading is over. Nothing of it delays the end, the release of the job's holdings or the next job, and its failure changes nothing of the job.
  - Files: the main session file (name ending `_<sessionId>.jsonl`) is `main`; every transcript in its folder and in the nested folders, at any depth, is `subAgents`, except those whose name starts with `__advisor.`, which are `advisors`. Each file is streamed line by line, never held whole in memory.
  - Records: the `usage` of each `message` with `role: assistant` gives the tokens (`input`, `output`, `cacheRead`, `cacheWrite`); each `toolCall` named `read` is a read, its `arguments.path` the file, and each named `grep` or `glob` a search. Other records are ignored.
  - A missing session id, a missing or unreadable main file, or a malformed complete line in any file makes the whole job `unavailable`, with the reason naming the file. A last line not complete is ignored.
  - These records are among the omp internals re-verified at each `MIN_OMP_VERSION` bump.

## general-chat
- **Process:** one omp per project's General, kept until the window closes, which shuts it down as in `## job`.
  - With a kept session, it starts when the window opens, or as soon as omp-setup becomes Ready if it was not; it is never started for a window with no kept session.
  - Otherwise, and after it stopped, it is started by the next message.
- **Command line:** `<omp> [--model <analysis>] --approval-mode <mode> --append-system-prompt <file> acp`.
  - `<file>` is Chat's instruction (`requirements/rules#general-chat`), built at each start with the sub-agent limit and the language of the Settings at that moment and written to `<app local data>/general.md` before omp is spawned.
  - No `--advisor`, `--slow` or overlay.
- **Environment:**
  - `WEAVER_GENERAL=<General identity>` and `WEAVER_ENDPOINT`, with no `WEAVER_JOB`, so `weaver` refuses job-bound commands.
  - The exceptions are `weaver job new`, accepted only from the live General identity of an open project with the same refusals as the developer's request, and `weaver question new`, accepted only from that identity while a turn of Chat is in progress (`requirements/tech#question-round`).
  - The app knows the live General identity of each open project, so that `proposal new` and `proposal edit` record `chat` and `chatWriting` (`requirements/data#proposal`) and the Board knows whether a turn of Chat is in progress (`requirements/rules#proposal`, reason 1).
  - PATH and the certificate variables as in `## omp`.
- **Session:**
  - The session id and arguments are kept in `<app local data>/general/<project hash>.json`.
  - A later start reloads the session with `session/load`, within the 30 s limit of `## job` and sent ordered with the replayed updates as in `## job` (Resume). If the load fails, an empty session starts and a notice says so.
  - The start at window open sends no prompt. The replay is shown in one piece when the load answers; messages sent meanwhile are held, placed below the replay and then prompted in order.
  - Clear closes the session and deletes the entry; during the load at window open it also ends that omp.
- **Forms and permissions:** forms are declined, with a text telling the agent to ask in its reply. Permissions work as in `## job`.
- **Sub-agents:** read, listed and shown as in `## job` (**Sub-agents**), keyed by the project's General instead of a job.
