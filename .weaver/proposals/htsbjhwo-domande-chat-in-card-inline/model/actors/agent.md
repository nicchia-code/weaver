# Agent

The coding agent, omp, working inside a job or in General that Weaver started. It reaches Weaver only through the `weaver` command line, which knows the job or the General it belongs to; what it reports there are the facts below. Only an agent of a job Weaver started can hold a Task, announce a card or write a Proposal, only an agent of a job or of General can ask questions (a question round in a job, a card of questions in General), and only the agent of General can send a design request; a command run outside them is refused for those. The agent reads, analyses and writes the developer's code; Weaver only reads and writes the files the developer opens and changes in the File view.

## Events

### ANNOUNCE_CARD
The agent says it is preparing a Proposal, a Task or a Drift, with a short title, so that the Board can show it before it exists.

### DECLARE_NOTE
The agent says in one line what it is doing and why.

### OPEN_PROPOSAL
The agent opens a new Proposal, as a Draft, possibly bound to a written request or taking the place of a rejected Proposal.

### EDIT_PROPOSAL
The agent changes a Proposal: it writes or removes one of its files or Tasks, marks a file for deletion, changes its label, records a decision on a Drift in a resolution, or binds it to a Drift.

### SUBMIT_PROPOSAL
The agent hands a finished Proposal over to the developer.

### TAKE_TASK
The agent starts working on a Task.

### RELEASE_TASK
The agent puts a Task it holds back in To do without finishing it.

### FINISH_TASK
The agent says a Task is done.

### CLOSE_TASK_UNCHANGED
The agent closes the fix Task of a Drift without changing the code, with a note saying what it checked and why nothing had to change.

### PREPARE_DRIFT_QUESTION
The agent prepares, ahead of time, the question with which the developer will decide a Drift it filed.

### ASK_QUESTIONS
The agent asks the developer a group of questions. The agent of a job asks a round and waits for the answers, with no time limit; the agent of General publishes a card of questions in its conversation and does not wait.

### STOP_WAITING
The agent stops waiting for the answers of its round, because its command was interrupted or lost its connection.

### UPDATE_HISTORY_ENTRY
The agent rewrites the text of a History entry or adds Tasks to it.

### START_JOB
The agent in General or in an Ask more conversation asks Weaver to start a job (design, scan, Run or Resolve) because the developer explicitly asked it to.

### SEND_REQUEST
The agent of General writes a design request with what it agreed on with the developer and sends it, because the developer explicitly asked for a Proposal; a design job then writes the Proposal, as if the developer had written and sent the request himself.
