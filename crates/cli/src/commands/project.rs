//! `weaver project new | edit | show`.

use std::path::Path;

use serde_json::{json, Value};
use weaver_core::command::CommandReply;
use weaver_core::project::board::{self, ActiveEntry, BacklogEntry, ProposalState};
use weaver_core::project::cards::{Card, CardKind};
use weaver_core::project::layout::WEAVER_DIR;
use weaver_core::project::proposal::data;
use weaver_core::project::shortnum::{self, ShortNumbers};
use weaver_core::project_files::{self, InitialiseError, SyncOptions, SyncReport};
use weaver_core::solo_mode::{self, Git, Inspection, Mode, Reason};

use super::{card_with, cards, reply, written};
use crate::spec::*;

/// The `solo` field of the outputs of `project new`, `project edit {solo}` and `project show`.
fn solo_output() -> Field {
    req(
        "solo",
        object(vec![
            req("mode", one_of(&Mode::NAMES), "`off`, `folders` (whole folders) or `files` (only the files Weaver installs in them)."),
            opt("unavailable", line(), "Why Solo mode cannot be changed (git was not found, the folder is not in a git repository, ...); absent when it can."),
            opt("problem", line(), "While Solo mode is on: why Weaver's block is not in git's local exclusions as expected. The command line never repairs it."),
            req("tracked", list(line()), "The files git tracks that the block excludes, relative to the project folder: their changes still go into commits. Empty when Solo mode is off."),
        ]),
        "Solo mode of this working copy.",
    )
}

pub fn commands() -> Vec<Command> {
    vec![
        Command::new("project", "new", "Initialises `.weaver/` in the folder and installs the agent files.", new)
            .notes(&[
                "Creates `.weaver/` with the System and the starting Requirement Areas, and writes the agent files (skills, sub-agent definitions, advisor instructions) in the project. It is refused, creating nothing, when `.weaver/` exists, or when an agent file to install exists with other content: every such file is listed, and `force` overwrites them.",
                "`solo` turns Solo mode on while initialising (`folders`: the whole folders of Weaver, the agent's skills and omp; `files`: only the files Weaver installs in them): Weaver's files then stay out of git in this working copy. It needs git and a repository: otherwise the command is refused with the reason and nothing is created. The default is `off`, and git is not called then.",
            ])
            .input(vec![
                opt("force", boolean(), "Overwrite the agent files that differ from the ones to install."),
                opt("solo", one_of(&Mode::NAMES), "Solo mode of this working copy: `off` (default), `folders` or `files`."),
            ])
            .output(vec![
                req("root", line(), "The project folder."),
                req("files", list(line()), "The files created in `.weaver/`, relative to it."),
                req("areas", list(line()), "The starting Requirement Areas."),
                req("agentFiles", list(line()), "The agent files written, relative to the project folder."),
                solo_output(),
            ])
            .place(written)
            .example("weaver project new --input '{\"solo\":\"folders\"}'"),
        Command::new("project", "edit", "Updates the installed skills, or changes Solo mode.", edit)
            .notes(&[
                "Without input, Update skills: writes again every installed file that differs from the one this version of Weaver carries, creates the starting Requirement Areas when the Requirements are missing, and generates the Requirement skills again. It always reports what it changed, or the error.",
                "With `solo`, only Solo mode changes: `off`, `folders` or `files`. Weaver writes or removes its block in git's local exclusions at once and prints `solo`. A change that fails (no git, not in a repository, exclusions not writable) changes nothing and says why. Files git already tracks are not hidden: they are listed in `solo.tracked`. While a merge of Weaver is in progress (the Board shows it), Solo mode cannot change in any way: the command is refused with `Finish or abort the merge in progress first.` and nothing changes.",
            ])
            .input(vec![opt("solo", one_of(&Mode::NAMES), "Change only Solo mode of this working copy: `off`, `folders` or `files`. Without it, Update skills runs.")])
            .output(vec![
                opt(
                    "changed",
                    list(object(vec![
                        req("path", line(), "The file, relative to the project folder."),
                        req("action", one_of(&["created", "rewritten", "removed"]), "What was done to it."),
                    ])),
                    "Update skills: the files written or removed; empty when everything was up to date.",
                ),
                opt("areas", list(line()), "Update skills: the Requirement Areas that have a generated skill."),
                opt("generated", Ty::Any, "Update skills: the result of the generation: `mode` (`fragment` or `index`), `budget` used out of `limit` (4,000), and `switched` (`from`, `to`) when the mode changed."),
                Field {
                    required: false,
                    doc: "With `solo` in the input: Solo mode after the change.",
                    ..solo_output()
                },
            ])
            .place(written)
            .example("weaver project edit --input '{\"solo\":\"files\"}'"),
        Command::new("project", "show", "Prints the Board and Solo mode.", show)
            .output(vec![
                cards("Open Proposals and open Drifts.", "toDecide"),
                req("inWriting", list(object(card_with(vec![
                    req("job", nullable(line()), "The job writing it."),
                ]))), "Draft Proposals with their job."),
                cards("Tasks in To do.", "todo"),
                req("doing", list(object(card_with(vec![
                    req("holder", nullable(line()), "The job that holds the Task."),
                    req("blockedBy", list(line()), "The Drifts that block it."),
                ]))), "Tasks in progress."),
                solo_output(),
            ])
            .example("weaver project show"),
    ]
}

/// The mode given in the input, `None` when `solo` is absent (the registry already checked the value).
fn solo_input(call: &Call) -> Option<Mode> {
    call.input
        .get("solo")
        .and_then(Value::as_str)
        .and_then(Mode::parse)
}

/// `solo` as the commands print it: `{mode, unavailable?, problem?, tracked}`.
fn solo_json(solo: &Inspection) -> Value {
    let mut value = json!({ "mode": solo.mode.as_str(), "tracked": solo.tracked });
    if let Some(reason) = &solo.unavailable {
        value["unavailable"] = json!(reason.to_string());
    }
    if let Some(reason) = &solo.problem {
        value["problem"] = json!(reason.to_string());
    }
    value
}

/// The lines of the Solo mode section of the Markdown outputs.
fn solo_markdown(solo: &Inspection) -> String {
    let mut text = match solo.mode {
        Mode::Off => "Solo mode is off.\n".to_owned(),
        Mode::Folders => "Solo mode is on: the whole folders of Weaver (`.weaver/`, `.agents/skills/`, `.omp/`) stay out of git in this working copy.\n".to_owned(),
        Mode::Files => "Solo mode is on: only the files Weaver installs (`.weaver/` and its skills, sub-agent definitions and advisor instructions) stay out of git in this working copy.\n".to_owned(),
    };
    if solo.mode.is_on() {
        text.push_str(
            "Weaver's files are not versioned anywhere: losing this working copy loses them.\n",
        );
    }
    if let Some(reason) = &solo.unavailable {
        text.push_str(&format!("Solo mode cannot be changed: {reason}.\n"));
    }
    if solo.problem.is_some() && solo.problem != solo.unavailable {
        if let Some(reason) = &solo.problem {
            text.push_str(&format!(
                "Weaver's block is not in git's local exclusions as expected: {reason}.\n"
            ));
        }
    }
    if let Some(diagnostic) = &solo.diagnostic {
        text.push_str(&format!(
            "`{}` counts as Solo mode off: {}.\n",
            diagnostic.path, diagnostic.message
        ));
    }
    if !solo.tracked.is_empty() {
        text.push_str(&format!(
            "\nGit still tracks {} of Weaver's files: their changes still go into commits.\n\n",
            solo.tracked.len()
        ));
        for path in &solo.tracked {
            text.push_str(&format!("- `{path}`\n"));
        }
    }
    text
}

/// A change of Solo mode that did not happen: the reason, and nothing changed.
fn solo_failure(reason: &Reason) -> Failure {
    // The reason of a merge in progress is a sentence of its own, with its period.
    let reason_text = reason.to_string();
    let message = format!(
        "Could not change Solo mode: {}.",
        reason_text.trim_end_matches('.')
    );
    if reason.is_unavailable() || matches!(reason, Reason::MergeInProgress) {
        Failure::refused(message)
    } else {
        Failure::failed(message)
    }
}

fn new(call: &Call) -> Result<CommandReply, Failure> {
    let root: &Path = call.root;
    let force = call
        .input
        .get("force")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mode = solo_input(call).unwrap_or(Mode::Off);
    let git = Git::system();
    let done = project_files::initialise(root, force, mode, &git).map_err(|error| match error {
        InitialiseError::Exists => {
            Failure::refused(format!("`{WEAVER_DIR}` already exists in {}: the project is initialised", root.display()))
        }
        InitialiseError::Conflicts(conflicts) => {
            let list: Vec<String> = conflicts.iter().map(|path| format!("  - {path}")).collect();
            Failure::refused(format!(
                "These agent files exist with other content:\n{}\nNothing was created. Run again with `{{\"force\":true}}` to overwrite them.",
                list.join("\n")
            ))
        }
        InitialiseError::Solo(reason) => {
            Failure::refused(format!("Solo mode cannot be turned on: {reason}. Nothing was created."))
        }
        InitialiseError::Failed(reason) => Failure::failed(format!("Could not initialise: {reason}.")),
    })?;
    // With Solo mode off git is not called: nothing to say of it but the mode.
    let solo = if mode.is_on() {
        solo_mode::inspect(&git, root)
    } else {
        Inspection {
            mode,
            unavailable: None,
            problem: None,
            tracked: Vec::new(),
            diagnostic: None,
        }
    };
    let mut markdown = format!(
        "# Project initialised\n\nCreated `{WEAVER_DIR}/` in `{}` with the System (`model/system.md`) and the Requirement Areas {}.\n",
        root.display(),
        done.areas.iter().map(|a| format!("`{a}`")).collect::<Vec<_>>().join(", ")
    );
    if done.agent_files.is_empty() {
        markdown.push_str("\nThe agent files were already in place.\n");
    } else {
        markdown.push_str(&format!(
            "\nWrote {} agent files:\n\n",
            done.agent_files.len()
        ));
        for path in &done.agent_files {
            markdown.push_str(&format!("- `{path}`\n"));
        }
    }
    markdown.push_str(&format!("\n## Solo mode\n\n{}", solo_markdown(&solo)));
    Ok(reply(
        markdown,
        json!({ "root": root.display().to_string(), "files": done.files, "areas": done.areas, "agentFiles": done.agent_files, "solo": solo_json(&solo) }),
    ))
}

/// The text the command line and the app show for a report of Update skills.
pub fn skills_markdown(report: &SyncReport) -> String {
    let mut markdown = String::from("# Skills updated\n\n");
    if report.is_unchanged() {
        markdown.push_str("Everything was already up to date.\n");
    } else {
        markdown.push_str(&format!("Updated {} files:\n\n", report.changes.len()));
        for change in &report.changes {
            let action = serde_json::to_value(change.action)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            markdown.push_str(&format!("- `{}` {action}\n", change.path));
        }
    }
    markdown.push_str(&format!("\nRequirement skills: {}.\n", report.mode_line()));
    if let Some(switch) = &report.generated.switched {
        markdown.push_str(&format!(
            "The Requirement skills switched from {} mode to {} mode.\n",
            serde_json::to_value(switch.from)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default(),
            serde_json::to_value(switch.to)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default()
        ));
    }
    markdown
}

fn edit(call: &Call) -> Result<CommandReply, Failure> {
    if let Some(mode) = solo_input(call) {
        // Only Solo mode changes, at once; the block and the state are written or removed.
        let solo = solo_mode::set_mode(&Git::system(), call.root, mode)
            .map_err(|reason| solo_failure(&reason))?;
        let markdown = format!("# Solo mode changed\n\n{}", solo_markdown(&solo));
        return Ok(reply(markdown, json!({ "solo": solo_json(&solo) })));
    }
    let report = project_files::sync(call.root, SyncOptions::update_skills())
        .map_err(|error| Failure::failed(format!("Could not update the agent files: {error}.")))?;
    let json = json!({
        "changed": serde_json::to_value(&report.changes).map_err(|e| Failure::failed(e.to_string()))?,
        "areas": report.areas,
        "generated": serde_json::to_value(&report.generated).map_err(|e| Failure::failed(e.to_string()))?,
    });
    Ok(reply(skills_markdown(&report), json))
}

fn name(numbers: &ShortNumbers, kind: CardKind, id: &str) -> String {
    numbers.label_of(kind, id).unwrap_or_else(|| id.to_owned())
}

fn card_json(numbers: &ShortNumbers, kind: CardKind, card: &Card) -> Value {
    json!({ "id": card.id, "number": numbers.number_of(kind, &card.id), "title": card.title })
}

fn show(call: &Call) -> Result<CommandReply, Failure> {
    let image = call.image;
    let numbers = shortnum::peek(call.root, image.cards());
    let board = board::compute(image, &call.caller.agent);
    let kind_of = |id: &str| {
        if image.card(CardKind::Drift, id).is_some() {
            CardKind::Drift
        } else {
            CardKind::Proposal
        }
    };

    // Open Proposals and open Drifts wait for the developer; a Proposal an agent still writes is in writing.
    let mut to_decide: Vec<(CardKind, &Card)> = Vec::new();
    let mut in_writing: Vec<&Card> = Vec::new();
    for entry in &board.backlog {
        let id = entry.id();
        let kind = kind_of(id);
        let Some(card) = image.card(kind, id) else {
            continue;
        };
        match entry {
            BacklogEntry::Drift { .. } => to_decide.push((kind, card)),
            BacklogEntry::Proposal { state, .. } | BacklogEntry::Resolution { state, .. } => {
                match state {
                    ProposalState::Open | ProposalState::Interrupted => {
                        to_decide.push((kind, card))
                    }
                    ProposalState::Refining => in_writing.push(card),
                }
            }
        }
    }
    for card in image.cards_of(CardKind::Proposal) {
        if data::status_of(card) == Some(data::Status::Draft)
            && card.bool_key("refining") != Some(true)
        {
            in_writing.push(card);
        }
    }
    let task_of = |entry: &ActiveEntry| image.card(CardKind::Task, &entry.id);
    let todo: Vec<&Card> = board
        .active
        .iter()
        .filter(|e| !e.doing)
        .filter_map(task_of)
        .collect();
    let doing: Vec<(&ActiveEntry, &Card)> = board
        .active
        .iter()
        .filter(|e| e.doing)
        .filter_map(|e| task_of(e).map(|t| (e, t)))
        .collect();

    let mut markdown = String::from("# Board\n");
    let mut section = |title: &str, lines: Vec<String>| {
        markdown.push_str(&format!("\n## {title}\n\n"));
        if lines.is_empty() {
            markdown.push_str("_none_\n");
        }
        for line in lines {
            markdown.push_str(&line);
            markdown.push('\n');
        }
    };
    section(
        "To decide",
        to_decide
            .iter()
            .map(|(kind, card)| {
                format!(
                    "- `{}` {} {} ({})",
                    name(&numbers, *kind, &card.id),
                    card.id,
                    card.title,
                    if *kind == CardKind::Drift {
                        "Drift"
                    } else {
                        "Proposal"
                    }
                )
            })
            .collect(),
    );
    section(
        "In writing",
        in_writing
            .iter()
            .map(|card| {
                let job = data::job_of(card)
                    .map(|j| format!(" · job `{j}`"))
                    .unwrap_or_default();
                format!(
                    "- `{}` {} {}{job}",
                    name(&numbers, CardKind::Proposal, &card.id),
                    card.id,
                    card.title
                )
            })
            .collect(),
    );
    section(
        "To do",
        todo.iter()
            .map(|t| {
                format!(
                    "- `{}` {} {}",
                    name(&numbers, CardKind::Task, &t.id),
                    t.id,
                    t.title
                )
            })
            .collect(),
    );
    section(
        "In progress",
        doing
            .iter()
            .map(|(entry, task)| {
                let holder = task
                    .str_key("holder")
                    .map(|h| format!(" · held by job `{h}`"))
                    .unwrap_or_default();
                let blockers: Vec<String> = entry
                    .blocked_by
                    .iter()
                    .map(|d| name(&numbers, CardKind::Drift, d))
                    .collect();
                let blocked = if blockers.is_empty() {
                    String::new()
                } else {
                    format!(" · blocked by Drift {}", blockers.join(", "))
                };
                format!(
                    "- `{}` {} {}{holder}{blocked}",
                    name(&numbers, CardKind::Task, &task.id),
                    task.id,
                    task.title
                )
            })
            .collect(),
    );

    // Solo mode is worked out at the call; the command line never repairs its block.
    let solo = solo_mode::inspect(&Git::system(), call.root);
    markdown.push_str(&format!("\n## Solo mode\n\n{}", solo_markdown(&solo)));

    let json = json!({
        "toDecide": to_decide.iter().map(|(kind, card)| card_json(&numbers, *kind, card)).collect::<Vec<_>>(),
        "inWriting": in_writing.iter().map(|card| {
            let mut value = card_json(&numbers, CardKind::Proposal, card);
            value["job"] = json!(data::job_of(card));
            value
        }).collect::<Vec<_>>(),
        "todo": todo.iter().map(|t| card_json(&numbers, CardKind::Task, t)).collect::<Vec<_>>(),
        "doing": doing.iter().map(|(entry, task)| {
            let mut value = card_json(&numbers, CardKind::Task, task);
            value["holder"] = json!(task.str_key("holder"));
            value["blockedBy"] = json!(entry.blocked_by);
            value
        }).collect::<Vec<_>>(),
        "solo": solo_json(&solo),
    });
    Ok(reply(markdown, json))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::Map;
    use weaver_core::project::image::ProjectImage;

    use super::*;
    use crate::spec::Caller;

    fn run(root: &Path, input: Value) -> Result<CommandReply, Failure> {
        let command = &commands()[0];
        let image = ProjectImage::empty(root);
        let input: Map<String, Value> = input.as_object().unwrap().clone();
        new(&Call {
            command,
            root,
            image: &image,
            arg: None,
            state: None,
            input: &input,
            caller: &Caller::default(),
        })
    }

    #[test]
    fn a_new_project_gets_the_model_the_areas_and_the_agent_files() {
        let dir = tempfile::tempdir().unwrap();
        let reply = run(dir.path(), json!({})).unwrap();
        assert!(reply.markdown.starts_with("# Project initialised"));
        assert!(dir.path().join(".weaver/model/system.md").is_file());
        assert!(dir.path().join(".weaver/requirements/ui/area.md").is_file());
        assert!(dir
            .path()
            .join(".agents/skills/weaver-design/COMMANDS.md")
            .is_file());
        assert!(dir.path().join(".omp/WATCHDOG.yml").is_file());
        let image = ProjectImage::load(dir.path());
        assert!(image.diagnostics().is_empty(), "{:?}", image.diagnostics());
        assert_eq!(reply.json["areas"].as_array().unwrap().len(), 5);
    }

    #[test]
    fn an_existing_weaver_folder_refuses_and_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".weaver")).unwrap();
        let error = run(dir.path(), json!({})).unwrap_err();
        assert_eq!(error.code, "refused");
        assert!(!dir.path().join(".agents").exists());
    }

    #[test]
    fn an_old_trama_folder_is_left_alone_and_weaver_is_created_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join(".trama/model/system.md");
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::write(&old, "# Old\n").unwrap();
        run(dir.path(), json!({})).unwrap();
        assert!(dir.path().join(".weaver/model/system.md").is_file());
        assert_eq!(fs::read_to_string(&old).unwrap(), "# Old\n");
    }

    #[test]
    fn differing_agent_files_are_all_listed_and_nothing_is_created_unless_forced() {
        let dir = tempfile::tempdir().unwrap();
        for file in [".agents/skills/weaver-design/SKILL.md", ".omp/WATCHDOG.yml"] {
            let path = dir.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "mine").unwrap();
        }
        let error = run(dir.path(), json!({})).unwrap_err();
        assert!(
            error
                .message
                .contains("- .agents/skills/weaver-design/SKILL.md"),
            "{}",
            error.message
        );
        assert!(error.message.contains("- .omp/WATCHDOG.yml"));
        assert!(!dir.path().join(".weaver").exists());
        assert!(!dir.path().join(".agents/skills/weaver-setup").exists());
        assert_eq!(
            fs::read_to_string(dir.path().join(".omp/WATCHDOG.yml")).unwrap(),
            "mine"
        );

        run(dir.path(), json!({ "force": true })).unwrap();
        assert!(dir.path().join(".weaver").is_dir());
        assert_ne!(
            fs::read_to_string(dir.path().join(".omp/WATCHDOG.yml")).unwrap(),
            "mine"
        );
    }

    fn run_edit_or_show(index: usize, root: &Path) -> Result<CommandReply, Failure> {
        let command = &commands()[index];
        let image = ProjectImage::load(root);
        let input = Map::new();
        (command.run)(&Call {
            command,
            root,
            image: &image,
            arg: None,
            state: None,
            input: &input,
            caller: &Caller::default(),
        })
    }

    #[test]
    fn project_edit_rewrites_what_differs_and_then_reports_nothing_changed() {
        let dir = tempfile::tempdir().unwrap();
        run(dir.path(), json!({})).unwrap();
        let calm = run_edit_or_show(1, dir.path()).unwrap();
        assert_eq!(calm.json["changed"], json!([]));
        assert!(
            calm.markdown.contains("Everything was already up to date."),
            "{}",
            calm.markdown
        );
        assert_eq!(calm.json["generated"]["mode"], "fragment");
        assert_eq!(calm.json["generated"]["limit"], 4000);

        fs::write(dir.path().join(".omp/WATCHDOG.yml"), "mine").unwrap();
        let busy = run_edit_or_show(1, dir.path()).unwrap();
        assert_eq!(
            busy.json["changed"],
            json!([{ "path": ".omp/WATCHDOG.yml", "action": "rewritten" }])
        );
        assert!(
            busy.markdown.contains("Updated 1 files:")
                && busy.markdown.contains("`.omp/WATCHDOG.yml` rewritten"),
            "{}",
            busy.markdown
        );
        assert_ne!(
            fs::read_to_string(dir.path().join(".omp/WATCHDOG.yml")).unwrap(),
            "mine"
        );
    }

    #[test]
    fn project_show_prints_the_board_of_an_empty_project() {
        let dir = tempfile::tempdir().unwrap();
        run(dir.path(), json!({})).unwrap();
        let reply = run_edit_or_show(2, dir.path()).unwrap();
        let mut json = reply.json.clone();
        let solo = json.as_object_mut().unwrap().remove("solo").unwrap();
        assert_eq!(
            json,
            json!({ "toDecide": [], "inWriting": [], "todo": [], "doing": [] })
        );
        assert_eq!(solo["mode"], "off");
        assert_eq!(solo["tracked"], json!([]));
        assert!(reply.markdown.starts_with("# Board"), "{}", reply.markdown);
        assert!(
            reply.markdown.contains("## Solo mode"),
            "{}",
            reply.markdown
        );
    }

    // --- Solo mode ---------------------------------------------------------------------------

    fn git(dir: &Path, args: &[&str]) {
        let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
        let status = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", null)
            .env("GIT_CONFIG_SYSTEM", null)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .expect("git runs");
        assert!(status.success(), "git {args:?}");
    }

    /// A temporary git repository whose root is canonical, as the command line's root is.
    fn repo() -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = weaver_core::paths::canonical_root(dir.path()).unwrap();
        git(&root, &["init", "-q"]);
        (dir, root)
    }

    fn call_with(index: usize, root: &Path, input: Value) -> Result<CommandReply, Failure> {
        let command = &commands()[index];
        let image = ProjectImage::load(root);
        let input: Map<String, Value> = input.as_object().unwrap().clone();
        (command.run)(&Call {
            command,
            root,
            image: &image,
            arg: None,
            state: None,
            input: &input,
            caller: &Caller::default(),
        })
    }

    fn exclusions(root: &Path) -> String {
        fs::read_to_string(root.join(".git/info/exclude")).unwrap()
    }

    #[test]
    fn project_new_with_solo_writes_the_block_and_prints_the_mode() {
        for (mode, extra) in [("folders", "/.omp/\n"), ("files", "/.omp/WATCHDOG.yml\n")] {
            let (_dir, root) = repo();
            let reply = call_with(0, &root, json!({ "solo": mode })).unwrap();
            assert_eq!(reply.json["solo"], json!({ "mode": mode, "tracked": [] }));
            assert!(
                reply.markdown.contains("## Solo mode"),
                "{}",
                reply.markdown
            );
            assert!(exclusions(&root).contains(extra), "{}", exclusions(&root));
            assert_eq!(
                fs::read_to_string(root.join(".weaver/solo/solo.json")).unwrap(),
                format!("{{\"mode\": \"{mode}\"}}\n")
            );
            let status = std::process::Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(&root)
                .output()
                .unwrap();
            assert!(
                status.stdout.is_empty(),
                "{}",
                String::from_utf8_lossy(&status.stdout)
            );
        }
    }

    #[test]
    fn project_new_without_solo_or_with_off_prints_off_and_leaves_git_alone() {
        let dir = tempfile::tempdir().unwrap();
        for input in [json!({}), json!({ "solo": "off" })] {
            let reply = call_with(0, dir.path(), input).unwrap();
            assert_eq!(reply.json["solo"], json!({ "mode": "off", "tracked": [] }));
            fs::remove_dir_all(dir.path().join(".weaver")).unwrap();
        }
    }

    #[test]
    fn project_new_with_solo_outside_git_is_refused_with_the_reason_and_creates_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let error = call_with(0, dir.path(), json!({ "solo": "folders" })).unwrap_err();
        assert_eq!(error.code, "refused");
        assert!(
            error
                .message
                .contains("this folder is not in a git repository"),
            "{}",
            error.message
        );
        assert_eq!(error.exit_code(), 1);
        assert!(fs::read_dir(dir.path()).unwrap().next().is_none());
    }

    #[test]
    fn project_edit_with_solo_changes_only_solo_mode_and_lists_the_tracked_files() {
        let (_dir, root) = repo();
        call_with(0, &root, json!({})).unwrap();
        fs::write(root.join(".omp/WATCHDOG.yml"), "x").unwrap();
        git(
            &root,
            &["add", "-f", ".omp/WATCHDOG.yml", ".weaver/model/system.md"],
        );
        let before =
            fs::read_to_string(root.join(".agents/skills/weaver-design/SKILL.md")).unwrap();

        let on = call_with(1, &root, json!({ "solo": "files" })).unwrap();
        assert_eq!(on.json.as_object().unwrap().len(), 1, "{}", on.json);
        assert_eq!(
            on.json["solo"],
            json!({ "mode": "files", "tracked": [".omp/WATCHDOG.yml", ".weaver/model/system.md"] })
        );
        assert!(
            on.markdown.contains("Git still tracks 2 of Weaver's files"),
            "{}",
            on.markdown
        );
        assert!(
            on.markdown.contains("- `.weaver/model/system.md`"),
            "{}",
            on.markdown
        );
        assert_eq!(
            fs::read_to_string(root.join(".agents/skills/weaver-design/SKILL.md")).unwrap(),
            before
        );

        // `project show` computes it again at the call.
        let shown = run_edit_or_show(2, &root).unwrap();
        assert_eq!(shown.json["solo"], on.json["solo"]);

        let folders = call_with(1, &root, json!({ "solo": "folders" })).unwrap();
        assert_eq!(folders.json["solo"]["mode"], "folders");
        let off = call_with(1, &root, json!({ "solo": "off" })).unwrap();
        assert_eq!(off.json["solo"], json!({ "mode": "off", "tracked": [] }));
        assert!(!root.join(".weaver/solo").exists());
        assert!(!exclusions(&root).contains("weaver solo mode"));
    }

    #[test]
    fn project_edit_with_solo_that_cannot_change_says_why_and_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        call_with(0, dir.path(), json!({})).unwrap();
        let error = call_with(1, dir.path(), json!({ "solo": "folders" })).unwrap_err();
        assert_eq!(error.code, "refused");
        assert!(
            error
                .message
                .starts_with("Could not change Solo mode: this folder is not in a git repository"),
            "{}",
            error.message
        );
        assert!(!dir.path().join(".weaver/solo").exists());
    }

    #[test]
    fn project_edit_with_solo_is_refused_while_a_merge_is_in_progress_and_changes_nothing() {
        let (_dir, root) = repo();
        call_with(0, &root, json!({ "solo": "folders" })).unwrap();
        let block = exclusions(&root);
        let state = fs::read_to_string(root.join(".weaver/solo/solo.json")).unwrap();
        // A merge of Weaver is in progress: `merge.json` exists.
        fs::create_dir_all(root.join(".weaver/merge")).unwrap();
        fs::write(root.join(".weaver/merge/merge.json"), "{}\n").unwrap();

        for mode in ["off", "files"] {
            let error = call_with(1, &root, json!({ "solo": mode })).unwrap_err();
            assert_eq!(error.code, "refused", "{mode}");
            assert_eq!(
                error.message,
                "Could not change Solo mode: Finish or abort the merge in progress first.",
                "{mode}"
            );
            assert_eq!(exclusions(&root), block, "{mode}");
            assert_eq!(
                fs::read_to_string(root.join(".weaver/solo/solo.json")).unwrap(),
                state,
                "{mode}"
            );
        }
        // Once the merge is over, the change goes through.
        fs::remove_file(root.join(".weaver/merge/merge.json")).unwrap();
        let off = call_with(1, &root, json!({ "solo": "off" })).unwrap();
        assert_eq!(off.json["solo"]["mode"], "off");
    }

    #[test]
    fn project_show_says_why_the_block_is_not_in_place_and_never_repairs_it() {
        let (_dir, root) = repo();
        call_with(0, &root, json!({ "solo": "folders" })).unwrap();
        let exclude = root.join(".git/info/exclude");
        let kept = exclusions(&root);
        fs::write(
            &exclude,
            kept.replace("# >>> weaver solo mode", "# nothing"),
        )
        .unwrap();
        let broken = fs::read_to_string(&exclude).unwrap();

        let shown = run_edit_or_show(2, &root).unwrap();
        assert_eq!(shown.json["solo"]["mode"], "folders");
        assert!(
            shown.json["solo"]["problem"].as_str().is_some(),
            "{}",
            shown.json
        );
        assert!(shown.json["solo"].get("unavailable").is_none());
        assert_eq!(fs::read_to_string(&exclude).unwrap(), broken);
    }
}
