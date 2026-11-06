# Design request

A Markdown document in which the developer asks for a design: what to add or change, as long and structured as he wants, possibly with images and with tags of assets. He writes it inside Weaver and it is saved as a file of the project while he writes. The setup request is the same document, written when the Model is empty, and it leads to the setup Proposal. The agent receives an instruction that names the request instead of containing it. On the developer's explicit order, the agent of Chat can also write the Proposal of a request that is not sent, binding it to the request.

## States

### Not written (initial)
No request exists yet.
- `developer.NEW_REQUEST` → Writing: an empty request is created and opened for writing.

### Writing
The developer writes it; every change is saved at once.
- `developer.SEND_REQUEST` → Sent: only when it is not empty; Weaver starts a design job on it, or a setup job when the Model is empty, whose instruction also tells the agent to read every asset the request tags at that moment. Sending something empty does nothing.
- `developer.DISCARD_REQUEST` → Discarded: its file and images are deleted.
- `workspace.REQUEST_DELETED` → Discarded: nothing is ever sent afterwards, even if Weaver still showed it.
- `agent.OPEN_PROPOSAL` → Held by Chat: only a Proposal the agent of Chat opens bound to it.

### Sent
The agent works on it. The developer can still read it but not send it again.
- `proposal.SUBMITTED` → Consumed: only the first Proposal bound to it; its file and images are deleted.
- `job.ENDED` → Writing: when the job that received it ended without handing over a Proposal bound to it; the developer can change it and send it again.
- `workspace.REQUEST_DELETED` → Discarded

### Held by Chat
The agent of Chat is writing a Proposal bound to it. The developer can still read it but not change, send or discard it, and can open Chat from it.
- `proposal.SUBMITTED` → Consumed: only the Proposal bound to it; its file and images are deleted.
- `proposal.DISCARDED` → Writing: only the Proposal bound to it, discarded before it was handed over; the developer can change it and send it again.
- `workspace.REQUEST_DELETED` → Discarded

### Consumed (final)
A Proposal was born from it. It never comes back, not even if an editor saves it again.

### Discarded (final)
Thrown away without a Proposal.
