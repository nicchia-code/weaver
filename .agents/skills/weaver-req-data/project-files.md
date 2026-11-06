# Project files

## General

### Layout

```
.weaver/
  model/                       system.md, actors/<actor>.md, <feature>/feature.md, <feature>/<machine>.md
  requirements/<area>/         area.md and <fragment>.md
  specs.yaml                   optional
  proposals/<id>-<slug>/       one folder per Proposal (see `proposal`)
  tasks/<id>-<slug>.md         live Tasks
  drifts/<id>-<slug>.md        every Drift, decided ones included
  history/<id>-<slug>.md       History entries
  assets/<name>                the developer's assets, flat, files only (see `requirements/rules#asset-view`)
  requests/                    local only: design requests (see `design-request`)
  short-numbers/assigned       local only: short-number cache
  task-images/<task id>/       local only: images tied to Tasks
  notes/                       local only: the developer's notes, notes.md and its images (see `notes-view`)
  solo/solo.json               local only: Solo mode of this working copy (see `solo-mode`)
  merge/merge.json             local only: the merge in progress (see `solo-merge`)
  timings/jobs.jsonl           local only: the measures of ended jobs (see `job`)
  commits/commits.json         local only: Atomic commits of this working copy (see `atomic-commits`)
```

- `model/`, `requirements/`, `specs.yaml`, `proposals/`, `tasks/`, `drifts/`, `history/` and `assets/` are versioned with the project. Decided Proposals and Drifts are never deleted.
- `assets/` is created only when the first asset is copied in, never at initialisation, and gets no `.gitignore`. Weaver never reads its files as Model, Requirement, Specs or card files, never puts them in the project picture, and never writes in it except through the Assets view.
- `notes/` is created only when the sheet or an image is first written, never at initialisation. Like `assets/`, its files are never Model, Requirement, Specs or card files nor part of the project picture, and only the Notes view writes in it.
- Local-only folders (`requests/`, `short-numbers/`, `task-images/`, `notes/`, `solo/`, `merge/`, `timings/`, `commits/`) each hold their own `.gitignore` containing `*`, written when the folder is created and written again whenever it is missing. Nothing else in `.weaver/` is ignored.
- The Model, Requirements and Specs formats are exactly those of the `FORMAT.md` reference installed with the `weaver-design` skill. The core's parser and that file are tested together: every example in it parses, and every rule it states is enforced by `model check` or the hand-over.
- Files and folders whose name starts with `.` are never read as Model, Requirement, Specs or card files.
- A `.trama` folder, the project folder of earlier versions of Weaver, is never read, written, moved or deleted: without `.weaver/` the folder is not a project, and with both the project is `.weaver/`.

### Identities and names

- A card identity (Proposal, Task, Drift, History entry) and a job identity match `^[a-z0-9]{8}$`, drawn from a CSPRNG and drawn again when already used by one of the same type. Task identities are unique across `.weaver/tasks/`, the `cleared` copies in `history/` and the `tasks/` folders of every Proposal.
- The identity of a card is read from its file or folder name, never from its content.
- Slug: the title lowercased, diacritics removed, every run of characters outside `[a-z0-9]` turned into one `-`, leading and trailing `-` removed, cut at 40 characters without a trailing `-`; `untitled` when nothing is left. The slug is fixed when the file is created; a later title change never renames it.
- Short numbers never appear in any versioned file. `short-numbers/assigned` holds one line per live card, `<P|T|D> <identity> <number>`, and is only a cache: missing, unreadable or conflicting, it is rebuilt and no command fails because of it.

### Encoding and writes

- Files are UTF-8 without BOM, written with LF line endings and a final newline. On read, a BOM is dropped and CRLF becomes LF before parsing, hashing or comparing, in one shared read function.
- Timestamps are UTC ISO 8601 with milliseconds (`2026-11-02T07:07:19.522Z`), taken from the clock at the moment of the event. An epoch or other placeholder value is never written.
- Front matter is YAML between two `---` lines, keys in the order listed in this Area; absent optional keys are not written. Unknown keys are ignored and kept when the file is rewritten.
- Every write is atomic: a temporary file `.<name>.<random>.tmp` in the same folder, flushed to disk, then renamed over the target. On Windows a rename failing with a sharing or access-denied error is retried every 50 ms for at most 1 s, then fails.
- A file that cannot be read or parsed is reported as a diagnostic with its path and reason (in the project window and in the output of the CLI command that met it). It is never dropped silently, never rewritten automatically, and the other files still load.
