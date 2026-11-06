# Developer

The person who owns the project, looks at it in Weaver and decides. Every decision on the Model and on the work is his: approving, rejecting, reworking or discarding a Proposal, deciding on a Drift, running or deleting a Task. The agent may take one of these decisions through the command line only when the developer explicitly asked it for that decision in a conversation; it then counts as the developer's Event, with the same conditions and consequences. A request to design, scan, implement or resolve never by itself allows the agent to decide. Every decision either happens whole or changes nothing, and its conditions are checked again at the moment it is applied, not when the developer started it.

## Events

### OPEN_PROJECT
The developer opens a folder in Weaver, from the welcome window or from the File menu of any window, choosing a folder or one of the recent projects. A folder already open only comes to the front.

### CLOSE_WINDOW
The developer closes the window of a project. When agent jobs of that project are working, or files of its File view have unsaved changes, he confirms first; with unsaved changes he chooses to save them all or to discard them all. When only agent jobs are working, he can also choose to keep them working with the window in the background.

### REOPEN_APP
The developer brings Weaver back: he clicks its icon in the Dock on macOS, launches it again without naming a folder, or chooses to show it from its icon in the tray on Windows and Linux. Every project window in the background comes back and the one used most recently gets the focus; with no project window in the background, the window used most recently comes to the front, and with no project window open, the welcome window.

### QUIT_APP
The developer quits Weaver, also from its icon in the tray on Windows and Linux. When agent jobs are working in any project, or files of a File view have unsaved changes, he confirms first; with unsaved changes he chooses, for each window, to save them all or to discard them all.

### INITIALISE_PROJECT
The developer turns a folder without a project into a Weaver project, choosing first whether Solo mode is on and in which mode, and confirming first when agent files Weaver would install already exist with different content.

### UPDATE_SKILLS
The developer asks Weaver to write again every file it installs for the agent.

### IMPORT_CODE
The developer asks the agent to describe the code already in the project, optionally leaving out some folders for this import only.

### INSTALL_OMP
The developer asks Weaver to install omp with its official installer or, when that fails, from omp's GitHub releases.

### LOG_IN_PROVIDER
The developer asks to connect omp to a model provider, or to connect it again.

### LOG_OUT_PROVIDER
The developer asks omp to forget a connected provider, after a confirmation.

### CHANGE_SETTINGS
The developer changes one of Weaver's settings.

### REFRESH_SYSTEM_CERTIFICATES
The developer asks Weaver to export the certificate authorities his system trusts again, while System Certificates is on.

### CHANGE_SOLO_MODE
The developer turns Solo mode of this working copy on, off, or to its other mode, in Settings or from the command line.

### CHANGE_ATOMIC_COMMITS
The developer turns Atomic commits of this working copy on or off in Settings.

### MERGE_BRANCH
The developer chooses, on the Board, a branch to bring into the current branch and confirms the merge, while Solo mode is on and Merge is available.

### RESOLVE_MERGE_CONFLICTS
The developer asks the agent to resolve the conflicts of the merge in progress.

### ABORT_MERGE
The developer aborts the merge in progress, after a confirmation.

### OPEN_USAGE
The developer opens Usage from the window of a project, to see how much is left of the usage limits of the connected providers.

### RETRY_USAGE
The developer asks Weaver to read the usage limits again after reading them failed.

### CLOSE_USAGE
The developer closes Usage, also by choosing to open Settings from it.

### NEW_REQUEST
The developer starts writing a design request: the setup request when the Model is empty and he chose to start from scratch, an ordinary design request otherwise.

### SEND_REQUEST
The developer sends the design request he wrote to the agent. Every asset the request tags at that moment is passed to the agent as a file to read; the request itself stays as he wrote it.

### DISCARD_REQUEST
The developer throws away a design request he never sent.

### APPROVE_PROPOSAL
The developer approves an open Proposal as a whole.

### REJECT_PROPOSAL
The developer rejects an open Proposal, optionally saying why.

### REWORK_PROPOSAL
The developer rejects an open Proposal with feedback and asks the agent to write it again from that feedback. Empty feedback needs a confirmation.

### DISCARD_PROPOSAL
The developer throws away a Draft the agent never finished.

### CHOOSE_PROPOSAL_MODEL
The developer chooses, or clears, the model that agents use for the work born from one Proposal that carries Tasks.

### RUN_TASK
The developer asks the agent to carry out one Task.

### RUN_ALL
The developer asks the agent to carry out every Task in To do that is not blocked.

### DELETE_TASK
The developer deletes a Task in To do for good, after a confirmation.

### SCAN_SYSTEM
The developer asks the agent to compare the whole System with the code, after a confirmation.

### RESOLVE_DRIFT
The developer asks to decide one open Drift.

### FIX_DRIFT_CODE
The developer decides that the Model and the Requirements are right and the code must change, choosing one of the Drift's fix options and optionally adding a note.

### ACCEPT_DRIFT_PROPOSAL
The developer decides that the code is right and accepts the Proposal attached to the Drift.

### REFINE_DRIFT_PROPOSAL
The developer asks the agent to adapt the stale Proposal attached to an open Drift to the current Model and Requirements, or to continue a refinement of it left half-way, without deciding the Drift.

### DISMISS_DRIFT
The developer decides that nothing changes, after a confirmation.

### SEND_MESSAGE
The developer writes to the agent in the conversation of a working job, in General or in an Ask more conversation. Every asset the message tags at that moment is passed to the agent as a file to read; the conversation shows only what he wrote.

### STOP_AGENT
The developer stops the agent of a job, of General or of an Ask more conversation, after a confirmation when it is a job.

### ANSWER_PERMISSION
The developer answers a permission omp asks for: allow once, allow always, or refuse.

### CLEAR_GENERAL
The developer starts General again from an empty conversation; the Proposals its agent created that are still Drafts are discarded with it.

### ASK_MORE
The developer asks an agent about one thing he is reading: a Proposal, a Task or a Drift in its details, a History entry with the Tasks it cites, the question in view of a question round, or the Requirements as a whole, from the Requirements view.

### CLOSE_ASK
The developer closes an Ask more conversation, after a confirmation when its agent is working.

### SEND_ANSWERS
The developer sends the answers of a question round, every question answered or skipped.

### SKIP_ALL_QUESTIONS
The developer skips every question of the round still without an answer, after a confirmation, and sends the round.

### OPEN_FILE_VIEW
The developer shows the File view of a project window.

### LEAVE_FILE_VIEW
Another view of the window takes the place of the File view, chosen by the developer or by a link or a notification he clicked. When tabs have unsaved changes he chooses first to save them all or to discard them all; cancelling keeps the File view and changes nothing.

### EXPAND_FOLDER
The developer expands a folder of the File view's tree.

### COLLAPSE_FOLDER
The developer collapses a folder of the File view's tree.

### CREATE_ENTRY
The developer creates an empty file or a folder, with its name, in a folder of the File view's tree.

### RENAME_ENTRY
The developer gives a new name to a file or a folder of the File view's tree.

### MOVE_ENTRY
The developer moves a file or a folder of the File view's tree into another folder of the project, by dragging it or by choosing the folder.

### DROP_EXTERNAL_ENTRIES
The developer drags files or folders from outside Weaver, such as the Finder or Explorer, and drops them onto a folder of the File view's tree or onto its empty space for the project's root.

### DELETE_ENTRY
The developer deletes a file or a folder of the File view's tree, after a confirmation that says when tabs on it have unsaved changes.

### OPEN_FILE
The developer opens a file of the File view's tree. A file already open in a tab only shows that tab. When the tabs are at their limit, the tab without unsaved changes shown least recently closes first; when every tab has unsaved changes, nothing opens and a warning says why.

### CLOSE_TAB
The developer closes a tab of the File view. With unsaved changes he chooses first to save them, to discard them or to cancel; a save that fails or is refused leaves the tab open, as a save from the tab would.

### EDIT_FILE
The developer changes the text of a file open in a tab of the File view.

### SAVE_FILE
The developer saves a file open in a tab of the File view, or all of them when he chooses to save all on leaving the File view, closing the window or quitting.

### RELOAD_FILE
The developer chooses to take the file as it is on disk and lose the unsaved changes of its tab, after a save found the file changed on disk or after the agent changed it.

### OVERWRITE_FILE
The developer chooses to write the text of a tab over the file on disk, after a save found the file changed on disk or after the agent changed it.

### OPEN_ASSETS_VIEW
The developer shows the Assets view of a project window.

### LEAVE_ASSETS_VIEW
Another view of the window takes the place of the Assets view, chosen by the developer or by a link or a notification he clicked.

### DROP_ASSETS
The developer drags files from outside Weaver, such as the Finder or Explorer, and drops them onto the Assets view.

### RENAME_ASSET
The developer gives a new name to one asset of the Assets view.

### DELETE_ASSETS
The developer deletes the assets selected in the Assets view, after a confirmation.

### DRAG_ASSETS_OUT
The developer drags assets out of the Assets view and drops them in another app, such as the Finder, Explorer, an editor or a browser: the one he drags, or every selected asset when he drags one of them.

### OPEN_NOTES_VIEW
The developer shows the Notes view of a project window.

### LEAVE_NOTES_VIEW
Another view of the window takes the place of the Notes view, chosen by the developer or by a link or a notification he clicked.

### EDIT_NOTES
The developer changes the notes sheet: he writes, formats, pastes text, moves or removes text and images, or undoes and redoes his changes.

### TICK_NOTES_ITEM
The developer ticks or unticks an item of a checklist in the notes sheet.

### ADD_NOTES_IMAGES
The developer pastes images into the notes sheet, or drags files from outside Weaver, such as the Finder or Explorer, and drops them onto it.
