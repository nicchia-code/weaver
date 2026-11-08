---
name: weaver-req-docs
description: "Use when a change must be described in the README or the CHANGELOG of the repository."
---

The project's documentation is `README.md` and `CHANGELOG.md` at the repository root, both in English.

## General

- `CHANGELOG.md` follows Keep a Changelog 1.1.0: a short header naming the format, then `## [Unreleased]` on top and one `## [<version>] - <YYYY-MM-DD>` section per released version, newest first, each with the groups `Added`, `Changed`, `Deprecated`, `Removed`, `Fixed`, `Security` that have entries.
- Every change a user of the app can see adds at least one line under `## [Unreleased]`, in the same change as the code: what changed for the user, in one sentence, without Task or Proposal ids. Changes visible only to contributors (tests, CI, refactoring) add none, except security alerts on dependencies.
- Every security alert on a dependency that is closed or accepted as a risk adds a line under `## [Unreleased]`, `Security`, even when the dependency is used only by the tests or the CI: which dependency was fixed, or which is a known risk and where.
- When a version is released, the lines under `## [Unreleased]` move to that version's section.
- `README.md` keeps its sections for installing and for contributors; user-facing behaviour that someone needs to get Weaver working goes in its own section after `## Install`.

## README and CHANGELOG

Use when writing or updating README.md or CHANGELOG.md, including what they say about System Certificates, installing omp behind a proxy, Solo mode and Merge.

Sections: `General`, `system-certificates`, `omp-setup`, `solo-mode`, `solo-merge`, `atomic-commits`

File: `main.md`
