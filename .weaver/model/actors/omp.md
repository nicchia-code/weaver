# omp

The omp program on the developer's computer: the only agent Weaver works with and the only source of the models it knows. Weaver asks it about itself (where it is, its version, its usable models, a newer version, the usage limits of its connected providers) and starts it as an ACP server for every job and for General; what omp answers and what its ACP sessions do are the facts below.

## Events

### READY
omp answers with at least the minimum version Weaver needs and at least one usable model.

### MISSING
omp cannot be found, or does not answer.

### OUTDATED
omp answers with a version older than the minimum Weaver needs.

### NO_PROVIDER
omp answers but has no usable model, because no model provider is connected.

### UPDATE_AVAILABLE
omp says a newer version of itself exists.

### UPDATE_ENDED
omp's update finished well: omp's own, or, after it failed, the download of omp's newer GitHub release that Weaver put in its place.

### UPDATE_FAILED
omp's update failed or took too long: omp's own and, when Weaver tried it, the download of omp's GitHub release too.

### INSTALL_FAILED
omp's installation, started by Weaver in the system terminal (the official installer and, when it fails, the download from GitHub), ended with an exit code other than 0, and omp is still not found.

### USAGE_READ
omp returned the usage limits of the connected providers, each with how much is used and when it resets, or the reason a provider has none.

### USAGE_FAILED
omp could not return the usage limits, or did not return them within the time allowed, with the reason.

### SESSION_STARTED
omp started, opened the ACP session and accepted the instruction Weaver sent.

### START_FAILED
omp could not start, open the session or accept the instruction within the time allowed, with the reason.

### SESSION_RESUMED
omp started again and reloaded an earlier ACP session.

### SESSION_NOT_RESUMED
omp could not reload an earlier ACP session.

### PERMISSION_REQUESTED
omp asks the developer's permission before doing something, only when its permissions are set to ask.

### TURN_ENDED
omp answered the last prompt normally, or after being cancelled.

### TURN_FAILED
omp ended the prompt abnormally: it refused, ran out of tokens or turns, or refused the prompt itself, with the reason.

### EXITED
The omp process ended, or its ACP connection closed, with the last lines it wrote about the error.

### FILE_TOOL_STARTED
The agent of a conversation omp runs for Weaver started a tool that changes files (an edit, a deletion or a move), naming the files it changes or none.

### FILE_TOOL_ENDED
A tool that changes files ended, completed or failed, naming the files it changed and, when omp says so, where it moved a file.
