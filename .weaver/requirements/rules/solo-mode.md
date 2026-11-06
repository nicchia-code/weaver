---
description: Use when implementing Solo mode's block in git's local exclusions, its modes and paths, the list of Weaver's files git still tracks, the git steps of Merge, or the commits of Atomic commits.
---
# Solo mode and Atomic commits

## solo-mode

- **Modes:** `off`, `folders` (default when turning on without a choice of mode), `files`. The same three values in the app, the command line and `solo.json` (`requirements/data#solo-mode`).
- **Exclusions file:** the path git prints for `git rev-parse --git-path info/exclude`, run in the project root and taken relative to it when relative. In a linked worktree it is the main repository's file, as git answers. A missing file, or missing `info/` folder, is created.
- **Prefix:** `git rev-parse --show-prefix` in the project root: empty at the repository's root, otherwise the project folder relative to it, ending with `/`. Characters special to git patterns in it (`*`, `?`, `[`, `\`, a leading `#` or `!`, trailing spaces) are escaped with `\`.
- **Block** of one working copy, `<root>` being the canonical project root with `/` separators and `<p>` the prefix:

  ```
  # >>> weaver solo mode: <root>
  <lines>
  # <<< weaver solo mode: <root>
  ```

  - `folders`: `/<p>.weaver/`, `/<p>.agents/skills/`, `/<p>.omp/`, in this order.
  - `files`: `/<p>.weaver/`, `/<p>.agents/skills/weaver-*/`, `/<p>.omp/agents/weaver-*.md`, `/<p>.omp/WATCHDOG.yml`, in this order.
  - Every line ends with LF.
- **Writing:** the block of this working copy is every run from its exact begin line to the next end line with the same `<root>`. The first such run is replaced in place and any further one removed; without one, the block is appended at the end of the file, after an LF when the file does not end with one. A begin line of this working copy with no end line after it is removed as a single line. Every other byte of the file, blocks of other working copies included, stays as it was. When the result equals the file, nothing is written; otherwise it is written atomically.
- **Removing:** every run of this working copy, begin and end lines included, and nothing else; a file left empty stays, empty.
- **Expected block:** the one the remembered mode and the current prefix give, byte for byte. At the opening, a missing or different block is written again (`solo-mode/Excluded`).
- **Changes:** turning on writes the block, then `solo.json`; turning off removes the block, then the `solo/` folder; changing mode rewrites the block, then `solo.json`. When the block step fails, nothing else is done and the earlier state stays.
- **Tracked files:** the files git tracks that the block's lines match, as relative paths from the project root, with `/`, sorted by path; only these, never other ignored tracked files.
- **Not available:** Solo mode cannot be changed, and an initialisation with a mode is refused, when one of these reasons holds, shown as the `<reason>` of the texts: `git was not found`, `this folder is not in a git repository`, `git did not answer within 10 s`, `git failed: <first line of its error output>`. A failed write of the exclusions file gives `could not write <path>: <system error>`.

## solo-merge

- **Availability,** checked in this order; the first that holds is the reason shown (`requirements/ui#solo-merge`):
  1. a job of this project is Queued, Running, Waiting for the developer or Suspended (General and Ask more are not jobs);
  2. Weaver's merge is in progress (`merge/merge.json` exists, `requirements/data#solo-merge`);
  3. a git call below fails or exceeds its limit: the Solo mode reasons of `requirements/rules#solo-mode`;
  4. git is in the middle of an operation: `MERGE_HEAD`, `CHERRY_PICK_HEAD` or `REBASE_HEAD` resolves with `git rev-parse -q --verify`, or `rebase-merge/` or `rebase-apply/` exists at `git rev-parse --git-path`;
  5. HEAD is detached: `git symbolic-ref -q HEAD` fails;
  6. `git status --porcelain=v1 -z --untracked-files=no` lists a path that no line of this working copy's Solo mode block matches.
- **Branches:** `git for-each-ref --format=%(refname)` on `refs/heads` and `refs/remotes`, shown by short name: local branches first, then remote ones, each sorted by name; the current branch and every `<remote>/HEAD` are left out. Weaver never runs `git fetch`.
- **Reminder:** when the Dialog opens, the tracked files of `requirements/rules#solo-mode` (**Tracked files**); a non-empty list adds the reminder to the confirmation.
- **Start,** in this order, all in the project's mutation queue:
  1. the conditions are checked again; one that holds refuses the merge with its reason;
  2. `start` = `git rev-parse HEAD`, `current` = the short name of the current branch, `incoming` = `git rev-parse <branch>^{commit}`;
  3. when `git merge-base --is-ancestor <incoming> <start>` succeeds, the branch brings nothing new: nothing else is done;
  4. `merge/merge.json` is written, before git runs, so that a merge interrupted by a crash is found at the next opening;
  5. `git merge --no-edit --no-overwrite-ignore <branch>`, with a limit of 2 minutes instead of 10 s.
- **Outcome of `git merge`:**
  - exit 0: concluded at once; the merge's commit is HEAD (the incoming commit after a fast-forward); `merge.json` is deleted;
  - non-zero exit with `MERGE_HEAD` resolving: Merging; conflicts are the paths of `git diff --name-only -z --diff-filter=U`;
  - non-zero exit without `MERGE_HEAD`: refused; `merge.json` is deleted; when git's error lists untracked files that would be overwritten and they are matched by the Solo mode block, the reason is the ignored-files one with those paths, otherwise the first line of git's error output;
  - limit reached: git is killed, then `git merge --abort` runs when `MERGE_HEAD` resolves, and `merge.json` is deleted; reason `git did not finish the merge within 2 minutes`.
- **Check of a merge in progress,** while `merge.json` exists: at the window's opening, when the window comes back to the foreground, when a job of the project ends, and after each merge action of Weaver; never on a timer and never by watching git's folder.
  - `MERGE_HEAD` resolves: still Merging; the count of files with conflicts is computed again.
  - otherwise, when the current branch is `current`, `<start>` and `<incoming>` are both ancestors of HEAD and HEAD differs from `<start>`: committed; the merge's commit is the oldest commit of `git rev-list --first-parent --ancestry-path <start>..HEAD`.
  - anything else (aborted outside Weaver, another branch, a reset): forgotten, with its notice and no scan.
  - `merge.json` is deleted at every end. A git call that fails during the check leaves the merge in progress and is logged.
- **Abort:** `git merge --abort`; when it fails the merge stays in progress and the Board shows `Could not abort the merge: <first line of git's error output>.`
- **Merged files:** `git diff --name-only -z --no-renames --relative <start> <merge commit>`, run in the project root, so paths are relative to it with `/`, sorted by path.
- **Scan after a merge:** a job of kind `scan`, named `Scan`, started in the same step that deletes `merge.json`, with the merged files in its registry record (`requirements/data#job`). It resets the count of Tasks done since the last scan like every scan job (`requirements/ui#work`).
- **Other git calls** have the 10 s limit and the runner of `requirements/tech#solo-mode`.

## atomic-commits

- **Turning on** is refused, nothing written, with the Solo mode reasons of `requirements/rules#solo-mode` (**Not available**), checked with `git rev-parse --show-prefix` in the project root. Turning off needs no git.
- **Paths,** checked only while Atomic commits is on, before anything is written, each listed as `files[<i>]: <path> <problem>`:
  - absolute, empty, or leaving the project root once `.` and `..` are resolved: `is outside the project`;
  - matched by git's ignore rules and not tracked (`git check-ignore -z --stdin`, without `--no-index`): `is ignored by git`;
  - missing from the working tree and not tracked (`git ls-files -z --cached -- <paths>`): `does not exist and git does not track it`. A missing tracked file is valid: its removal is committed.
  - With Solo mode on, a path that a line of this working copy's Solo mode block matches is left out of the commit without a problem.
- **Order:** the entry is written and the recorded Tasks' files removed as without Atomic commits; then, after the project lock is released, the commit runs in a queue of the working copy, one commit at a time, before the command answers.
- **No commit,** checked in this order; the first that holds is the `reason`:
  1. `checksFailed` is `true`: `the checks of the Run failed`;
  2. a reason of **Not available** of `requirements/rules#solo-mode`;
  3. Weaver's merge is in progress, or git is in the middle of an operation as in **Availability** of `requirements/rules#solo-merge`: `a merge, a rebase or a cherry-pick is in progress`;
  4. HEAD is detached: `HEAD is detached`.
- **Staging:** `git add -A -- <paths>` for the valid named paths, so that removed files are staged as removals. Outside Solo mode, also the entry's file in `.weaver/history/` and, as removals, the removed files of the recorded Tasks that git tracks (`git rm -q --cached --ignore-unmatch -- <paths>`); never another file of Weaver. What the developer already staged stays staged and goes into the commit.
- **Nothing to commit:** when `git diff --cached --quiet` exits 0 after staging, no commit is made, `status` is `none` and there is no notice.
- **Commit:** `git commit --cleanup=strip -F -`, the message on standard input. Outside Solo mode the message is the agent's message, a blank line and the trailer `Co-authored-by: Weaver <weaver@users.noreply.github.com>`; with Solo mode on, the agent's message alone. Author and committer come from git's configuration. Never `--no-verify`, `--amend`, `-a`, `--allow-empty`, a push or a fetch: hooks and signing run as configured.
- **Limit:** 2 minutes for the commit, because of hooks; when reached git is killed and the reason is `git did not finish the commit within 2 minutes`. Other calls have the 10 s limit of `requirements/tech#solo-mode`.
- **Failure:** a non-zero exit of `git add`, `git rm` or `git commit` gives `git failed: <first line of its error output>`; what Weaver staged stays staged.
- **Hash:** after a commit, `git rev-parse HEAD`, printed in full in `commit.hash`.
