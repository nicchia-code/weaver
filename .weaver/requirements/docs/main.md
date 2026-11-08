---
description: "Use when writing or updating README.md or CHANGELOG.md, including what they say about System Certificates, installing omp behind a proxy, Solo mode and Merge."
---
# README and CHANGELOG

## General
- `CHANGELOG.md` follows Keep a Changelog 1.1.0: a short header naming the format, then `## [Unreleased]` on top and one `## [<version>] - <YYYY-MM-DD>` section per released version, newest first, each with the groups `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security` that have entries.
- Every change a user of the app can see adds at least one line under `## [Unreleased]`, in the same change as the code: what changed for the user, in one sentence, without Task or Proposal ids. Changes visible only to contributors (tests, CI, refactoring) add none, except security alerts on dependencies.
- Every security alert on a dependency that is closed or accepted as a risk adds a line under `## [Unreleased]`, `Security`, even when the dependency is used only by the tests or the CI: which dependency was fixed, or which is a known risk and where.
- When a version is released, the lines under `## [Unreleased]` move to that version's section.
- `README.md` keeps its sections for installing and for contributors; user-facing behaviour that someone needs to get Weaver working goes in its own section after `## Install`.

## system-certificates
- `README.md` has a section `## Behind a corporate proxy`, after `## Install`, saying:
  - why omp fails to install or to reach its provider behind a proxy that inspects HTTPS traffic;
  - what System Certificates does, that it is off by default and where it is (Settings → omp, the omp setup box of Get Started and the welcome window);
  - which certificates it uses: those the system trusts (on macOS the user, admin and system trust settings; on Windows the user's and the machine's root stores; on Linux the system bundle), plus Mozilla's public roots; that it refreshes them at start when older than a day, every day while Weaver is open, and with `Refresh`;
  - the variables it sets, overwriting inherited values, including `PI_REQUEST_COMPRESSION=0`, and that they reach every omp process Weaver starts and its terminals, so the agent's commands inherit them;
  - the one-time turn-on when Zscaler is installed, on macOS and Windows only, never repeated after it is turned off;
  - the explanations shown for certificate errors, HTTP errors and a failed installer, and their suggestion while the setting is off;
  - that Java's tools and every program that reads a truststore instead of a PEM bundle are not covered.
- `README.md` `### Where Weaver keeps files` names the bundle `system-certificates.pem` and the installer markers `omp-install/` in the local data folder.
- `CHANGELOG.md` is created with this change; its first `## [Unreleased]` entries describe System Certificates, the Zscaler turn-on, the new error explanations and the installer that now reports its failure.

## omp-setup
- `README.md` `## Behind a corporate proxy` also has a subsection `### When the proxy blocks omp's download`, saying:
  - that some proxies block the site omp's official installer and `omp update` download from, whatever the certificates, so System Certificates does not help there;
  - that Weaver then downloads the same omp from its GitHub releases, by itself, on macOS, Windows and Linux: in the installer's terminal after the official installer fails, and in the background after `omp update` fails, only for an omp in its installer's folder and not for one installed with a package manager or set in `omp path`;
  - where it puts omp (`PI_INSTALL_DIR`, else `%LOCALAPPDATA%\omp` or `~/.local/bin`), that on Windows it adds that folder to the user `Path` when missing, and that on macOS and Linux it only prints how to add it;
  - that the file is installed only when its size and SHA-256 match those GitHub publishes and it answers `--version`;
  - the causes shown when that download fails too, with their exit codes `90` to `93`.
- `README.md` `### Where Weaver keeps files` says that the GitHub download writes omp in its installer's folder, outside Weaver's folders, with a temporary `.omp.download` file and, on Windows, the `omp.exe.*.old` files of a replaced omp.
- `CHANGELOG.md` adds under `## [Unreleased]`, `Added`, one line describing the GitHub fallback for installing and updating omp, and under `Changed` one line for the installer's failure messages by cause.

## solo-mode
- `README.md` has a section `## Solo mode`, after `## Install` and `## Behind a corporate proxy`, saying:
  - what Solo mode does: Weaver's files stay only in this working copy and never go into a commit, through git's local exclusions;
  - that it belongs to the working copy, not the user, and where it is chosen (Get Started, Settings → Project, `solo` of `weaver project new` and `project edit`);
  - the two modes, whole folders and Weaver's files only, and when to use the second;
  - that files git already tracks are not hidden and Settings lists them;
  - that it needs git and that, with it on, losing the working copy loses Weaver's files.
- `CHANGELOG.md` adds under `## [Unreleased]`, `Added`, one line describing Solo mode.

## solo-merge

- `README.md` has, inside `## Solo mode`, a subsection `### Merge` saying:
  - what it is for: bringing the others' work into the current branch and finding, through a scan, what it changed against the Model;
  - where it is (the Board, only with Solo mode on) and when it is available;
  - that Weaver runs `git merge` itself but never fetches and never commits a merge with conflicts: the developer resolves them by hand or with the agent, then commits;
  - that the scan after the merge starts by itself and that its Drifts from the merge recommend adapting the Model.
- `CHANGELOG.md` adds under `## [Unreleased]`, `Added`, one line describing Merge in Solo mode.

## atomic-commits

- `README.md` has a section `## Atomic commits`, right after `## Solo mode`, saying:
  - what it does: after each History entry the agent writes, or each update that adds Tasks to one, Weaver commits the code of that entry, one commit per entry, with an English message written by the agent;
  - that it is off by default, belongs to the working copy and is turned on in Settings → Project only;
  - what goes into a commit: the files the agent names, what the developer already staged, and, outside Solo mode, the History entry and its Tasks' removed files; with Solo mode on, none of Weaver's files;
  - that the message is the one the agent wrote, with or without Solo mode, and that the author is the developer, as git is configured;
  - that hooks and signing run as configured, that Weaver never amends, pushes or fetches, and that a commit is skipped, with a notice on the Board, when the Run's checks failed or git cannot commit;
  - that it needs git.
- `CHANGELOG.md` adds under `## [Unreleased]`, `Added`, one line describing Atomic commits.
- `CHANGELOG.md` adds under `## [Unreleased]`, `Changed`, one line saying that the commits of Atomic commits no longer name Weaver as co-author: their message is the agent's alone.
