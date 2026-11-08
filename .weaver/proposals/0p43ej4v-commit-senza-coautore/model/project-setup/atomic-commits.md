# Atomic commits

Whether Weaver makes a git commit of the code of each History entry in one working copy, so that the repository's history follows the work done by the agent one feature at a time. It is off by default. The choice lives only in the working copy, like Solo mode: never in the repository and never in Weaver's settings, so another clone of the same repository starts with it off. The developer turns it on and off only in Settings, with no confirmation, and the change applies at once.

While it is on, every time the agent writes a History entry, or adds Tasks to one, Weaver commits the files of the code the agent names for it, together with what the developer already staged (see `history-entry`). Outside Solo mode the commit also holds the entry's own file and the removal of the files of the Tasks it records; with Solo mode on it holds no file of Weaver. The author is the developer, as git is configured, and the message is the agent's alone, with or without Solo mode: it names no co-author. The repository's hooks and signing apply as to any commit. Weaver never pushes, never amends and never rewrites a commit.

Feature: `project-setup`.

## States

### Off (initial)
Weaver makes no commit for this working copy and remembers nothing. Outside a git repository, or when git cannot be found, it cannot be turned on and Settings says why.
- `developer.CHANGE_ATOMIC_COMMITS` → On: when the developer turns it on in a project whose folder is in a git repository; Weaver remembers it for this working copy.
- `developer.CHANGE_ATOMIC_COMMITS` → Off: when the folder is not in a git repository, git cannot be found or the choice cannot be remembered; nothing changes and Settings says why.

### On
Each History entry the agent writes, and each update that adds Tasks to one, is followed by a commit of its code.
- `developer.CHANGE_ATOMIC_COMMITS` → Off: when the developer turns it off; Weaver forgets the choice. Commits already made stay.
- `developer.CHANGE_ATOMIC_COMMITS` → On: when the choice cannot be forgotten; nothing changes and Settings says why.
- `workspace.PROJECT_FOLDER_REMOVED` → Off: the choice went with the `.weaver` folder.
