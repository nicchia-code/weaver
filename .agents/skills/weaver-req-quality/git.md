# Git

## solo-mode

- **Reliability:** Weaver never changes a line of the exclusions file outside its own block of this working copy, never touches git's index for Solo mode, and never leaves a block behind a failed initialisation.
- **Performance:** Solo mode starts git only when the project opens, when Solo mode changes, when the Settings view is shown and, for the File view, as part of its one `git check-ignore` per listed folder; never on the UI or IPC thread, never more than 10 s per call.
- **Tests,** against temporary git repositories with real git:
  - the block for both modes at the repository's root and in a subfolder, a prefix with special characters, a linked worktree writing into the main repository's file;
  - other lines kept byte for byte, with and without a final newline; writing the same block twice leaving the file untouched; two working copies' blocks independent; duplicate and broken blocks; removal leaving the rest;
  - git missing (an empty PATH), a folder outside a repository and a git call over the limit (injected limit);
  - initialisation with each mode: the block written before any other file, refused outside git with nothing created, a later failure removing the block;
  - the opening repair with an empty Model and with a Model, silent and never changing the opening check's outcome; `.weaver` vanishing removing the block;
  - the tracked files listed for each mode, team files in `.agents/skills/` left out in `files` mode;
  - the File view listing what a Solo mode block ignores and still leaving out what other rules ignore;
  - the CLI `solo` field of `project new`, `project edit` and `project show`;
  - the control, notes, warnings and list in Settings and Get Started in Vitest, with their keyboard paths and accessible names.

## solo-merge

- **Reliability:** Weaver itself never commits a merge with conflicts, never stages a file for a merge (only the agent of the merge conflicts job marks the files it resolved), never runs `git fetch`, never overwrites a file Solo mode keeps out of git, and never leaves `merge/merge.json` behind an ended merge; a merge stopped by the time limit leaves the branch as it was. A git call that fails while checking a merge in progress leaves it in progress.
- **Performance:** Merge starts git only at the moments of `requirements/rules#solo-merge` (**Check of a merge in progress**) and `requirements/tech#solo-merge` (**State in the app**), never on a timer, never on the UI or IPC thread; one computation of its state runs at a time per project.
- **Tests,** against temporary git repositories with real git, a local branch and a remote-tracking one:
  - every availability reason in its order, Weaver's files excepted from uncommitted changes in both Solo mode modes, and the reminder of tracked Weaver files;
  - nothing to merge; a fast-forward and a merge commit concluded at once; conflicts leading to Merging with their count; git's refusal for local changes; the refusal for files Solo mode keeps out of git; the time limit (injected) aborting the merge;
  - the check of a merge in progress: still in progress, committed outside Weaver (also with later commits on top), aborted outside Weaver, another branch, a reset, an unreadable `merge.json`, a crash after `merge.json` was written;
  - abort, and its refusal while the merge conflicts job has not ended; Solo mode refused while merging;
  - the merged files of a fast-forward, a merge commit and a merge with resolved conflicts, in a project at the repository's root and in a subfolder;
  - the scan after a merge: started without confirmation, `merged` in its record and in `job show`, the count reset; admission refusing Runs, Run all, resolves, scans and imports during a merge;
  - `drift new` with `merge` refused outside a scan after a merge, and a prepared question of a merge Drift refused without `proposal` recommended;
  - the button, Dialog, banner, notices, badge and details in Vitest, with their keyboard paths and accessible names.

## atomic-commits

- **Reliability:** Atomic commits is the only place where Weaver stages files and commits. It stages only the paths of `requirements/rules#atomic-commits` (**Staging**), never unstages what the developer staged, never amends, rewrites, pushes or fetches, and never commits a file Solo mode keeps out of git unless the developer staged it. A failed or skipped commit never undoes the History entry. Two commits of one working copy never run at the same time.
- **Performance:** git runs only when Atomic commits changes, when the Settings view is shown and when a History entry is written while it is on; never on the UI or IPC thread, and never under the project lock while a commit (and its hooks) runs.
- **Tests,** against temporary git repositories with real git:
  - off: `commit` checked for shape only, no git started, `status` `off`;
  - the paths: outside the project, ignored, missing and untracked refused all together with nothing written; a removed tracked file committed as a removal; a project in a subfolder;
  - the commit of a new entry and of an edit adding Tasks (a new commit, the earlier one unchanged); an edit adding no Task making none; the entry file and the removed Task files in the commit outside Solo mode, none of Weaver's files in either Solo mode mode; the developer's staged files included;
  - the message exactly the agent's, with Solo mode off and on, with no `Co-authored-by` line, the author and committer from git's configuration;
  - each reason of **No commit** in order, nothing to commit giving `none`, a failing pre-commit hook, the time limit (injected), the entry written in every case;
  - two entries written at once by two jobs committed one after the other;
  - turning it on refused outside a repository and without git; `commits/commits.json` and its `.gitignore`; an unreadable file counting as off;
  - the switch, its notes and disabled reasons in Settings, and the Board notice, in Vitest, with their keyboard paths and accessible names.
