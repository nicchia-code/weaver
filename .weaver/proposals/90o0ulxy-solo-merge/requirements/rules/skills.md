---
description: "Use when changing the skills and files Weaver installs for omp or the Requirement skills."
---
# Agent skills

## project
- Files Weaver installs for the agent, one list in the core, embedded in the binary that the app and the `weaver` command line share:
  - skills in `.agents/skills/<name>/SKILL.md` (plus their reference files): `weaver-setup`, `weaver-design` (with its format reference and command reference), `weaver-import`, `weaver-scan`, `weaver-compare`, `weaver-implement`, `weaver-resolve-drifts`, `weaver-resolve-conflicts`;
  - omp sub-agent definitions: `.omp/agents/weaver-analysis.md`, which compares Machines on the scan model (`model: "@slow"`, omp's slow role being the scan model), and `.omp/agents/weaver-implementer.md`, which names no model and inherits the model of its Run; each `name:` is its file name without `.md`;
  - the instructions of omp's advisor for the Advisor Scanner, `.omp/WATCHDOG.yml`;
  - the generated Requirement skills `weaver-req-*`, in the same skills folder.
- Every installed text is in English, LF line endings; each sub-agent definition is under 6 KB. Installed texts name Weaver's skills and sub-agents by these names. The skills folder is always `.agents/skills` in the project root, with no setting, so the app and the command line find the same one. A file is written, atomically, only when its LF-normalised content differs from what Weaver would write or it is missing; an equal file is left alone. Nothing outside this list is written or deleted, except `weaver-req-*` skills the current mode does not produce.
- Weaver knows only these names. Skills and sub-agent definitions named `trama-*`, installed by earlier versions, are never recognised, renamed, removed or counted, also when Update skills runs; they stay until the developer removes them.
- The installed skills and sub-agent definitions measure the detail of a comparison by the States, Transitions and Events of the Model; they never mention a Diagram, which the app does not have.
- Merge texts: `weaver-resolve-conflicts` tells the agent to read `.weaver/merge/merge.json`, find the files with conflicts with git, resolve the obvious ones, ask the developer with one `weaver question new` round when the choice between the two versions is not obvious (each question naming the file and showing both versions), mark each resolved file with `git add`, and never run `git commit` or `git merge --abort`, nor anything else that ends or loses the merge (`git reset`, `git checkout` of a branch, `git stash`). `weaver-scan`, `weaver-compare` and `weaver-analysis` say that, when the instruction says the scan follows a merge, the agent reads the merged files with `weaver job show`, passes them to every sub-agent, and files with `"merge": true` each Drift whose difference lies in those files and no other. `weaver-resolve-drifts` says that the prepared question of a merge Drift always has the `proposal` option, written with its attached Draft, and recommends it (`requirements/rules#drift`).
- Starting Areas written at initialisation, each with only `area.md`:

  | Area | Title | Description |
  |---|---|---|
  | `ui` | Interface | Use when building or changing screens, layout, styling, components, navigation look or texts shown to users. |
  | `tech` | Technology and constraints | Use when choosing or using languages, frameworks, libraries, platforms, project structure or build and deploy setup. |
  | `data` | Data | Use when defining, storing, validating or showing the information the System keeps and what each piece contains. |
  | `rules` | Business rules | Use when implementing calculations, limits, policies or other business rules behind the Model's conditions and consequences. |
  | `quality` | Quality | Use when the work touches performance, security, privacy, accessibility, offline behaviour, localisation or reliability. |

### Requirement skills
- A fragment has content when at least one of its sections has non-blank text; empty sections are never copied.
- Budget: the sum, over every fragment with content, of the characters (Unicode scalar values) of its skill name `weaver-req-<area>-<fragment>` plus its description. Up to 4,000 inclusive: fragment mode; above: index mode. All Areas switch together.
- Fragment mode: one skill `weaver-req-<area>-<fragment>` per fragment with content; front matter `name` and the fragment's `description`; body: the Area's H1 and prose when present, then each non-empty section with its reference as `##` heading (`General` for the general one).
- Index mode: one skill `weaver-req-<area>` per Area with prose or a fragment with content; front matter `name` and the Area's `description`; body: the Area's prose, the `## General` section in full when non-empty, then for each fragment with content its title, description, the references of its sections and the relative path of `<fragment>.md`, a file in the skill folder holding the fragment's non-empty sections.
- Regeneration happens in the same step as an approval (by the app or the command line), at initialisation, at Update skills, at the opening check, and 1 s after the last change under `.weaver/requirements/` (debounced per project, never across projects).
- Every `weaver-req-*` skill the current mode does not produce is removed: the other mode's after a switch, and those of fragments or Areas deleted or left empty. The previous mode, for reporting a switch, is read only from the existing `weaver-req-*` skills.
- Wherever Weaver names a Requirement skill (command output, instructions), it names the one that exists in the current mode.
- `weaver model check` warns, without refusing anything, about each fragment over 12,288 bytes (whole file, front matter included) with path and size, and for a Proposal about the mode switch its approval would cause.
- The report of a regeneration and of Update skills is one structured result for app and command line: files written and removed, mode, budget used out of 4,000, a switch when it happened. A run that changed nothing reports nothing in the app.
