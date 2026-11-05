# Solo mode

Whether Weaver keeps its files of one working copy out of git, so that the developer can use Weaver alone in a repository where the others do not. While it is on, Weaver keeps a block of its own in the local exclusions of the git repository, which git never commits: the `.weaver` folder always, and either the whole folders of the agent's skills and of omp (the default mode) or, for a team that already keeps its own files there, only the files Weaver installs in them. The choice lives only in the working copy, never in the repository and never in Weaver's settings: another clone of the same repository starts with it off, and so does a linked worktree.

Each working copy writes and removes only its own block, marked with the place of that working copy, leaving every other line of the exclusions as it was; writing the same block again changes nothing. Its paths start at the root of the repository, so a project in a subfolder excludes only its own files. A linked worktree writes in the exclusions of its repository, which every working copy of that repository shares: a file stays excluded in all of them while any of them keeps a block that excludes it.

An exclusion never hides a file git already tracks. While Solo mode is on, Settings lists the files its block excludes that git still tracks, computed again each time Settings is shown and after every change, and warns that their changes still go into commits; Weaver offers no way to stop tracking them. Settings also warns that Weaver's files are then versioned nowhere: losing the working copy loses them. The File view keeps showing what Solo mode excludes.

Every change needs git and applies at once, with no confirmation. A change that fails changes nothing, and Settings says why.

## States

### Off (initial)
Weaver keeps none of its files out of git for this working copy, and remembers no mode. Outside a git repository, or when git cannot be found, it cannot be changed and Settings says why; in a folder that is not yet a project it is chosen when initialising.
- `developer.INITIALISE_PROJECT` → On: when the developer chose a mode; Weaver writes its block for that mode and remembers the mode before writing any other file of the project. When the folder is not in a git repository, git cannot be found or the block cannot be written, the project is not created and Weaver says why; when the initialisation fails later, Weaver removes the block again.
- `developer.CHANGE_SOLO_MODE` → On: when the developer turns it on in a project whose folder is in a git repository and the block can be written; Weaver writes its block for the chosen mode and remembers the mode. Files of Weaver that git already tracks keep going into commits, and Settings lists them.
- `developer.CHANGE_SOLO_MODE` → Off: when the folder is not in a git repository, git cannot be found or the block cannot be written; nothing changes and Weaver says why.

### On
Weaver's files of this working copy are kept out of git by its block, in the mode the developer chose. Settings lists the files the block excludes that git still tracks and warns that Weaver's files are versioned nowhere. The Board offers Merge (see `solo-merge`).
- `developer.CHANGE_SOLO_MODE` → Off: when the developer turns it off, no merge is in progress and the block can be removed; Weaver removes the block of this working copy, leaving every other line as it was, and forgets the mode. Weaver's files show to git again as untracked files.
- `developer.CHANGE_SOLO_MODE` → Excluded: when the developer chooses the other mode, no merge is in progress and the block can be written; Weaver rewrites its block for that mode and remembers it.
- `developer.CHANGE_SOLO_MODE` → On: when git cannot be found, the folder is no longer in a git repository, or the block cannot be written or removed; nothing changes and Weaver says why.
- `developer.CHANGE_SOLO_MODE` → On: while a merge is in progress (see `solo-merge`), whether turning it off or choosing the other mode; nothing changes and Settings says to finish or abort the merge first.
- `workspace.PROJECT_FOLDER_REMOVED` → Off: the mode went with the `.weaver` folder; Weaver removes the block of this working copy when it can still reach the exclusions. When the folder disappears while Weaver is closed, the block stays.

#### Excluded (initial)
The block of this working copy is in the exclusions as Weaver expects it.
- `window.OPENED` → Excluded: when the block is missing or differs from the one the mode expects; Weaver writes it again silently, in every opened project, also with an empty Model, in the same background work as the check of the files installed for the agent.
- `window.OPENED` → Not excluded: when git cannot be found, the folder is no longer in a git repository, or the block cannot be written; the mode stays.

#### Not excluded
Weaver could not put its block in place at the last opening. The mode stays; Settings says that Weaver's files may not be kept out of git and why, and no other place shows it. Weaver tries again at the next opening.
- `window.OPENED` → Excluded: when the block can be written again; Weaver writes it silently.
- `window.OPENED` → Not excluded: when it still cannot; Settings shows the new reason.
