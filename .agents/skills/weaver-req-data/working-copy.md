# Working copy state

## solo-mode

- Solo mode is not a setting: it is never in `settings.json` and never in the settings schema. Its state is `.weaver/solo/solo.json` in the working copy, `{"mode": "folders"}` or `{"mode": "files"}`, UTF-8, LF, written atomically like every project file, in the local-only folder `solo/` with its `.gitignore` containing `*` (`requirements/data#General`).
- No file means Solo mode is off. Turning it off deletes the whole `solo/` folder. A file that cannot be read or holds another value counts as off and is reported as a diagnostic with its path and reason (`requirements/data#General`); it is rewritten only by the next change of Solo mode.
- At initialisation with Solo mode chosen, `solo/solo.json` is written with the rest of `.weaver/`, after the block (`requirements/rules#solo-mode`); a failure removes both.

## solo-merge

- The merge in progress is `.weaver/merge/merge.json` in the working copy, in the local-only folder `merge/` with its `.gitignore` containing `*` (`requirements/data#General`): `{"branch", "current", "start", "incoming", "started"}`, the chosen branch as shown, the current branch's short name, the full hashes of the starting and incoming commits, and the instant it was written. UTF-8, LF, written atomically like every project file. It is never in `settings.json`, in the job registry or in the repository.
- No file means no merge in progress. The file is deleted at every end of the merge (`requirements/rules#solo-merge`); the `merge/` folder stays with its `.gitignore`.
- A file that cannot be read or lacks a field counts as a merge in progress that Weaver cannot follow: it is reported as a diagnostic with its path and reason, the check of the merge treats it as forgotten, and it is deleted.

## atomic-commits

- Atomic commits is not a setting: it is never in `settings.json` and never in the settings schema. Its state is `.weaver/commits/commits.json` in the working copy, `{"enabled": true}`, UTF-8, LF, written atomically like every project file, in the local-only folder `commits/` with its `.gitignore` containing `*` (`requirements/data#General`).
- No file means Atomic commits is off. Turning it off deletes the whole `commits/` folder. A file that cannot be read or holds another value counts as off and is reported as a diagnostic with its path and reason (`requirements/data#General`); it is rewritten only by the next change of Atomic commits.
- Nothing about the commits Weaver makes is stored: not in the History entry, the job registry or any local state.
