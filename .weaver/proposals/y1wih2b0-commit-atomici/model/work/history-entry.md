# History entry

What a job did, told by its agent for one macro-feature: a title, a one-line summary, a text and the finished Tasks it groups. The agent writes the entries at the end of a Run, one per macro-feature, and may write one for any other job that finished Tasks. Weaver never writes an entry by itself, so a Task finished and never cited stays out of History.

With every entry it writes, and with every update that adds Tasks, the agent also names the files of the code that belong to that feature, gives a commit message in English and says whether the final checks of its Run failed. Weaver uses them only while Atomic commits is on in the working copy (see `atomic-commits`); otherwise they change nothing. While it is on, a named path outside the project, ignored by git, or that neither exists nor is tracked by git refuses the whole write, listing every such path, and nothing is written.

A commit follows the write; the entry is written whatever happens to the commit. Weaver makes no commit when the agent says the checks failed, nor when the folder is not in a git repository, git cannot be found, a merge, a rebase or a cherry-pick is in progress, HEAD is detached, or git refuses it (a hook, the signing, any other error). In each of these cases the command tells the agent that the entry was not committed and why, and the Board shows a notice with the reason. When nothing is left to commit there is no commit and no notice. History never shows the commits.

## States

### Written (initial)
The entry exists, dated when it was written or last updated. When it is written while Atomic commits is on, Weaver commits the named files, what the developer already staged and, outside Solo mode, the entry's file and the removal of the files of the Tasks it records.
- `agent.UPDATE_HISTORY_ENTRY` → Written: the agent rewrites its text or adds finished Tasks, never removes one; the date becomes the moment of the update. When the update adds Tasks while Atomic commits is on, Weaver makes a new commit of the files named with the update, as for a new entry, and never changes the earlier commit; an update that adds no Task makes no commit.

## Events

### TASKS_CITED
Emitted when the entry is written or updated, for the Tasks it cites for the first time.
