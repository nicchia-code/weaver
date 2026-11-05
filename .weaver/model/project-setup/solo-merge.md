# Solo merge

A merge of another branch into the current branch of this working copy, which Weaver runs with git while Solo mode is on, so that the work of the others, who do not keep the Model up to date, reaches the Model through a scan. Weaver runs git itself and never reads the code: the agent does. The Board shows `Merge` only while Solo mode is on.

Merge is available only when nothing can be lost or mixed: no uncommitted change to a file git tracks, Weaver's files excepted; no merge, rebase or cherry-pick already in progress in git; the working copy on a branch; and no agent job of this project that has not ended, whether queued, working, waiting for the developer or suspended. Otherwise it is disabled and says why. When git still tracks some of Weaver's files, Merge stays available and its confirmation reminds the developer that their changes go into the merge commit. Weaver works out again whether Merge is available when the Board is shown, when the window comes back to the foreground, when a job ends and after each of its own merge actions, never at intervals.

The developer chooses the branch to bring in among the local branches and the remote branches git already knows, without fetching, and confirms the merge naming both branches. While a merge with conflicts is in progress, Weaver remembers it in a local file of the `.weaver` folder that is never versioned, holding the branch, the commit the current branch started from and the incoming commit, and deletes it when the merge ends, in whatever way. The files the merge brought are those that differ between the commit the current branch started from and the merge's own commit: the work of the others and the resolutions of the conflicts.

Feature: `project-setup`.

## States

### No merge (initial)
Weaver follows no merge in this working copy.
- `developer.MERGE_BRANCH` → No merge: when the chosen branch brings nothing new; nothing is started and Weaver says that the current branch is already up to date.
- `developer.MERGE_BRANCH` → No merge: when git refuses or fails the merge before any conflict, for example because it would overwrite local changes; nothing changes and Weaver shows the first line of git's error.
- `developer.MERGE_BRANCH` → No merge: when the merge would overwrite files that Solo mode keeps out of git; nothing changes and Weaver says which files stop it.
- `developer.MERGE_BRANCH` → No merge: when git takes longer than allowed; Weaver stops git, aborts the merge so that the branch is as it was, and says why.
- `developer.MERGE_BRANCH` → No merge: when git merges without conflicts, moving the branch forward or making its merge commit with git's own message; the merge is concluded at once.
- `developer.MERGE_BRANCH` → Merging: when git stops on conflicts; Weaver remembers the merge in progress.

### Merging
A merge with conflicts is in progress and the Board shows it, with the number of files that still have conflicts, until it ends. Meanwhile Runs, Run all, resolves, scans and imports are refused, and Solo mode cannot be turned off. The developer resolves the conflicts by hand or with the agent, then commits the merge himself in git; Weaver never commits it.
- `developer.RESOLVE_MERGE_CONFLICTS` → Merging: only while some file still has conflicts and no merge conflicts job is unended; a merge conflicts job starts.
- `developer.ABORT_MERGE` → No merge: after a confirmation; refused while the merge conflicts job has not ended. Weaver aborts the merge in git, so that the branch is as it was before, forgets the merge and starts no scan.
- `window.OPENED` → No merge: when git shows the merge committed; Weaver forgets the merge and the merge is concluded.
- `window.OPENED` → No merge: when git shows no merge in progress and no commit of this merge (aborted outside Weaver, a branch switch, a reset); Weaver forgets the merge, starts no scan and says so.
- `window.OPENED` → Merging: when the merge is still in progress; the Board shows how many files still have conflicts.
- `desktop.WINDOW_FOCUSED` → No merge: when git shows the merge committed; as when the window opens.
- `desktop.WINDOW_FOCUSED` → No merge: when git shows no merge in progress and no commit of this merge; as when the window opens.
- `desktop.WINDOW_FOCUSED` → Merging: when the merge is still in progress; the count of files with conflicts is updated.
- `job.ENDED` → No merge: when git shows the merge committed; as when the window opens.
- `job.ENDED` → No merge: when git shows no merge in progress and no commit of this merge; as when the window opens.
- `job.ENDED` → Merging: when the merge is still in progress, also after the merge conflicts job ended with conflicts left; the count of files with conflicts is updated.
- `workspace.PROJECT_FOLDER_REMOVED` → No merge: the memory of the merge went with the `.weaver` folder; git's merge stays as it is and Weaver starts no scan.

## Events

### CONCLUDED
Emitted when a merge is concluded: when git merged without conflicts, or when Weaver finds the merge with conflicts committed. It carries the files the merge brought.
