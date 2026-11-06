//! `weaver history new | edit | show | list`. The rules (checks, copies of the Tasks recorded,
//! dates) live in `weaver_core::project::history`; this file only reads the input and prints.

use std::convert::Infallible;

use serde_json::{json, Value};
use weaver_core::command::CommandReply;
use weaver_core::history_commit::{self, CommitInput, Plan};
use weaver_core::project::cards::CardKind;
use weaver_core::project::changes::{self, Change, MutationError};
use weaver_core::project::clock::SystemClock;
use weaver_core::project::history::{self, Entry, Planned, Problem, TaskCopy};
use weaver_core::project::layout::WEAVER_DIR;
use weaver_core::project::random::OsRandom;
use weaver_core::project::shortnum::{self, ShortNumbers};
use weaver_core::solo_mode::Git;

use super::{card_with, local, reply, written};
use crate::spec::*;

fn entry_fields() -> Vec<Field> {
    card_with(vec![
        req("file", line(), "The file, relative to `.weaver/`."),
        req("recorded", list(line()), "The Tasks now recorded in History: their copy is in the entry and their file is gone."),
        opt("kept", list(line()), "The cited Tasks that stay because their copy cannot be made (the reason is on stderr)."),
        req("commit", object(commit_result_fields()), "How the commit of the entry ended."),
    ])
}

/// The `commit` of the input of `history new` and `history edit`.
fn commit_fields() -> Vec<Field> {
    vec![
        req("files", list(line()), "Paths of the code of this entry's feature, relative to the project root with `/`, without duplicates; may be empty."),
        req("message", text(), "The commit message in English: a first line of at most 72 characters, optionally followed by a blank line and a body."),
        opt("checksFailed", boolean(), "`true` when the final checks of the Run failed; default `false`."),
    ]
}

/// The `commit` of the output.
fn commit_result_fields() -> Vec<Field> {
    vec![
        req("status", one_of(&["off", "none", "committed", "not-committed"]), "`off`: Atomic commits is off; `none`: nothing to commit, or an edit that adds no Task; `committed`; `not-committed`."),
        opt("hash", line(), "The full hash of the commit, when `committed`."),
        opt("reason", line(), "Why no commit was made, when `not-committed`."),
    ]
}

fn task_fields() -> Vec<Field> {
    card_with(vec![
        req("finished", line(), "When the Task was finished."),
        opt(
            "label",
            line(),
            "The label of the Proposal the Task belonged to.",
        ),
        req("refs", list(line()), "What the Task referred to."),
        opt("body", text(), "What the Task said, in Markdown."),
    ])
}

pub fn commands() -> Vec<Command> {
    vec![
        Command::new("history", "new", "Records a History entry for finished Tasks.", new)
            .notes(&[
                "`tasks` needs at least one Task, each Done and not closed without changes. Every problem is listed and nothing is written.",
                "A Task cited for the first time is recorded in the entry at once: its copy is written into every entry that cites it and its file is deleted.",
                "`commit` is always given, whether Atomic commits is on or off. While it is on, Weaver commits with git the files named, what the developer already staged and, outside Solo mode, the entry and the removal of the files of the Tasks it records, with your message; it never amends, pushes or fetches. A path outside the project, ignored by git, or missing and not tracked by git refuses the whole command and nothing is written. The entry is written whatever happens to the commit, and `commit.status` says how it ended. While it is off, `commit` is only checked for its shape.",
            ])
            .input(vec![
                req("title", line(), "The title, on one line."),
                req("summary", line_max(280), "One line of 1 to 280 characters."),
                opt("body", text(), "More detail, in Markdown."),
                req("tasks", list_min(line(), 1), "The Done Tasks the entry records."),
                req("commit", object(commit_fields()), "The commit of the code of this entry's feature."),
            ])
            .output(entry_fields())
            .place(written)
            .example("weaver history new --input -"),
        Command::new("history", "edit", "Changes a History entry: replaces fields and adds Tasks.", edit)
            .notes(&[
                "Replaces the fields given and adds the Tasks given; it never removes a Task. The date of the entry becomes the moment of the edit.",
                "`commit` is required when `tasks` adds at least one Task the entry did not cite, and ignored otherwise. An edit that adds Tasks makes a new commit while Atomic commits is on and never changes the earlier one; an edit that adds none makes no commit.",
            ])
            .arg(Arg::Card(CardKind::History))
            .input(vec![
                opt("title", line(), "The title, on one line."),
                opt("summary", line_max(280), "One line of 1 to 280 characters."),
                opt("body", text(), "More detail, in Markdown."),
                opt("tasks", list(line()), "Tasks to add."),
                opt("commit", object(commit_fields()), "The commit of the code of the feature of the Tasks added; required when the edit adds Tasks."),
            ])
            .output(entry_fields())
            .place(written)
            .example("weaver history edit <id> --input -"),
        Command::new("history", "show", "Prints a History entry with the Tasks it cites.", show)
            .arg(Arg::Card(CardKind::History))
            .output(card_with(vec![
                req("summary", line(), "The summary."),
                req("body", text(), "The body."),
                req("created", line(), "When the entry was first written."),
                req("updated", line(), "When the entry was last written or updated."),
                req("tasks", list(object(task_fields())), "The Tasks it cites, in the order cited."),
            ]))
            .example("weaver history show <id>"),
        Command::new("history", "list", "Lists the History entries, newest first.", list_entries)
            .output(vec![req(
                "entries",
                list(object(card_with(vec![
                    req("summary", line(), "The summary."),
                    req("updated", line(), "When the entry was last written or updated."),
                    req("tasks", int_min(0), "How many Tasks it cites."),
                ]))),
                "The entries, newest first.",
            )])
            .place(local)
            .example("weaver history list"),
    ]
}

fn strings(call: &Call, key: &str) -> Vec<String> {
    call.input
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn text_field<'a>(call: &'a Call, key: &str) -> Option<&'a str> {
    call.input.get(key).and_then(Value::as_str)
}

/// Every problem on a line of its own, for a refusal.
fn refusal(problems: &[Problem]) -> Failure {
    let mut message = String::from("Nothing was written:");
    for problem in problems {
        message.push_str(&format!("\n  - {}: {}", problem.path, problem.message));
    }
    Failure::refused(message)
}

/// The files the write deletes, relative to the project root: the files of the Tasks it records.
fn removed_files(planned: &Planned) -> Vec<String> {
    planned
        .changes
        .iter()
        .filter(|(_, change)| matches!(change, Change::Delete))
        .map(|(path, _)| format!("{WEAVER_DIR}/{path}"))
        .collect()
}

/// The `commit` of the input, and its problems of shape (listed with the others).
fn commit_input(call: &Call) -> (Option<CommitInput>, Vec<Problem>) {
    match history_commit::parse(call.input.get("commit")) {
        Ok(commit) => (commit, Vec::new()),
        Err(problems) => (None, problems),
    }
}

/// Plans what follows the write of `planned`: nothing (Atomic commits off, or an edit that adds no
/// Task) or a commit. Every problem found, those of `shape` included, refuses the command before
/// anything is written.
fn plan_commit(
    call: &Call,
    planned: &Planned,
    commit: Option<&CommitInput>,
    shape: Vec<Problem>,
) -> Result<Plan, Failure> {
    if !shape.is_empty() {
        return Err(refusal(&shape));
    }
    history_commit::plan(
        &Git::system(),
        call.root,
        commit,
        !planned.added.is_empty(),
        &format!("{WEAVER_DIR}/{}", planned.file),
        removed_files(planned),
    )
    .map_err(|problems| refusal(&problems))
}

/// Writes what the command planned and prints it. A commit that follows is only announced
/// (`commit.status` is `pending`): the caller makes it once the project's lock is released
/// (`weaver_cli::exec::settle`), so a slow hook never holds the project.
fn write_planned(call: &Call, planned: Planned, plan: Plan) -> Result<CommandReply, Failure> {
    if !planned.changes.is_empty() {
        changes::apply(call.root, &planned.changes, |_| Ok::<(), Infallible>(())).map_err(
            |error| match error {
                MutationError::Refused(never) => match never {},
                other => Failure::failed(other.to_string()),
            },
        )?;
        history::remove_task_folders(call.root, &planned.recorded);
    }
    let mut markdown = format!(
        "# History entry `{}`\n\n{}\n\nFile: `{}`\n",
        planned.id, planned.title, planned.file
    );
    if planned.recorded.is_empty() {
        markdown.push_str("\nNo Task was recorded.\n");
    } else {
        markdown.push_str("\nRecorded in History:\n\n");
        for id in &planned.recorded {
            markdown.push_str(&format!("- `{id}`\n"));
        }
    }
    let mut note = None;
    if !planned.kept.is_empty() {
        markdown.push_str("\nStill Tasks, with the reason:\n\n");
        let mut lines = Vec::new();
        for kept in &planned.kept {
            markdown.push_str(&format!("- `{}`: {}\n", kept.task, kept.reason));
            lines.push(format!("{} stays a Task: {}.", kept.task, kept.reason));
        }
        note = Some(lines.join("\n"));
    }
    let kept: Vec<&str> = planned.kept.iter().map(|k| k.task.as_str()).collect();
    let commit = match &plan {
        Plan::Off => history_commit::Outcome::Off.to_value(),
        Plan::None => history_commit::Outcome::None.to_value(),
        Plan::Commit(pending) => history_commit::pending_value(pending),
    };
    let mut result = reply(
        markdown,
        json!({ "id": planned.id, "number": null, "title": planned.title, "file": planned.file, "recorded": planned.recorded, "kept": kept, "commit": commit }),
    );
    result.note = note;
    Ok(result)
}

fn new(call: &Call) -> Result<CommandReply, Failure> {
    let title = text_field(call, "title").unwrap_or_default();
    let summary = text_field(call, "summary").unwrap_or_default();
    let tasks = strings(call, "tasks");
    let (commit, shape) = commit_input(call);
    let planned = match history::plan_new(
        call.image,
        title,
        summary,
        text_field(call, "body"),
        &tasks,
        &SystemClock,
        &mut OsRandom,
    ) {
        Ok(planned) => planned,
        Err(mut problems) => {
            problems.extend(shape);
            return Err(refusal(&problems));
        }
    };
    let plan = plan_commit(call, &planned, commit.as_ref(), shape)?;
    write_planned(call, planned, plan)
}

fn edit(call: &Call) -> Result<CommandReply, Failure> {
    let id = call.arg.expect("a History entry is named");
    let tasks = strings(call, "tasks");
    let (commit, shape) = commit_input(call);
    let planned = match history::plan_edit(
        call.image,
        id,
        text_field(call, "title"),
        text_field(call, "summary"),
        text_field(call, "body"),
        &tasks,
        &SystemClock,
    ) {
        Ok(planned) => planned,
        Err(mut problems) => {
            problems.extend(shape);
            return Err(refusal(&problems));
        }
    };
    let plan = plan_commit(call, &planned, commit.as_ref(), shape)?;
    write_planned(call, planned, plan)
}

/// The Task of an entry as shown: from the copy the entry keeps, else from the Task file that is
/// still there, else only its identity.
fn task_json(call: &Call, numbers: &ShortNumbers, entry: &Entry, id: &str) -> (Value, String) {
    let copy: Option<TaskCopy> = history::shown_copy(call.image, entry, id);
    let number = numbers.number_of(CardKind::Task, id);
    match copy {
        Some(copy) => {
            let line = format!(
                "- `{id}` {} (finished {}{})",
                copy.title,
                copy.finished,
                copy.label
                    .as_deref()
                    .map(|l| format!(", {l}"))
                    .unwrap_or_default()
            );
            let mut value = json!({ "id": id, "number": number, "title": copy.title, "finished": copy.finished, "refs": copy.refs, "body": copy.body });
            if let Some(label) = &copy.label {
                value["label"] = json!(label);
            }
            (value, line)
        }
        None => (
            json!({ "id": id, "number": number, "title": "Task not available", "finished": "", "refs": [] }),
            format!("- `{id}` (no copy of the Task is available)"),
        ),
    }
}

fn show(call: &Call) -> Result<CommandReply, Failure> {
    let id = call.arg.expect("a History entry is named");
    let entry = call
        .image
        .card(CardKind::History, id)
        .and_then(history::entry_of)
        .ok_or_else(|| Failure::refused(format!("the History entry {id} cannot be read")))?;
    let numbers = shortnum::peek(call.root, call.image.cards());
    let mut tasks = Vec::new();
    let mut lines = Vec::new();
    for task in &entry.tasks {
        let (value, line) = task_json(call, &numbers, &entry, task);
        tasks.push(value);
        lines.push(line);
    }
    let mut markdown = format!(
        "# {}\n\n`{}` · updated {} · created {}\n\n{}\n",
        entry.title, entry.id, entry.updated, entry.created, entry.summary
    );
    if !entry.text.is_empty() {
        markdown.push_str(&format!("\n{}\n", entry.text));
    }
    markdown.push_str("\n## Tasks\n\n");
    if lines.is_empty() {
        markdown.push_str("No Task.\n");
    }
    for line in lines {
        markdown.push_str(&line);
        markdown.push('\n');
    }
    Ok(reply(
        markdown,
        json!({
            "id": entry.id, "number": null, "title": entry.title, "summary": entry.summary, "body": entry.text,
            "created": entry.created, "updated": entry.updated, "tasks": tasks,
        }),
    ))
}

fn list_entries(call: &Call) -> Result<CommandReply, Failure> {
    let entries = history::entries(call.image);
    let mut markdown = String::from("# History\n\n");
    if entries.is_empty() {
        markdown.push_str("No History entry yet.\n");
    }
    for entry in &entries {
        markdown.push_str(&format!(
            "- `{}` {} ({}, {} Tasks): {}\n",
            entry.id,
            entry.title,
            entry.updated,
            entry.tasks.len(),
            entry.summary
        ));
    }
    let items: Vec<Value> = entries
        .iter()
        .map(|e| json!({ "id": e.id, "number": null, "title": e.title, "summary": e.summary, "updated": e.updated, "tasks": e.tasks.len() }))
        .collect();
    Ok(reply(markdown, json!({ "entries": items })))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use serde_json::Map;
    use weaver_core::command::code;
    use weaver_core::project::image::ProjectImage;

    use super::*;
    use crate::spec::Caller;

    fn run(
        verb: &str,
        root: &Path,
        arg: Option<&str>,
        input: Value,
    ) -> Result<CommandReply, Failure> {
        let all = commands();
        let command = all.iter().find(|c| c.verb == verb).unwrap();
        let image = ProjectImage::load(root);
        let input: Map<String, Value> = input.as_object().unwrap().clone();
        (command.run)(&Call {
            command,
            root,
            image: &image,
            arg,
            state: None,
            input: &input,
            caller: &Caller::default(),
        })
    }

    fn put(root: &Path, rel: &str, text: &str) {
        let path = root.join(".weaver").join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// The `commit` every write of an entry gives, whether Atomic commits is on or off.
    fn commit() -> Value {
        json!({ "files": [], "message": "Add the feature" })
    }

    fn task(root: &Path, id: &str, column: &str, extra: &str) {
        put(
            root,
            &format!("tasks/{id}-job.md"),
            &format!("---\ncolumn: {column}\ntechnical: true\nrefs: []\ncreated: 2026-11-02T07:00:01.000Z\n{extra}---\n# Task {id}\n\nDo it.\n"),
        );
    }

    fn project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        task(
            dir.path(),
            "aaaaaaaa",
            "done",
            "finished: 2026-11-02T08:00:00.000Z\n",
        );
        task(dir.path(), "bbbbbbbb", "todo", "");
        task(dir.path(), "cccccccc", "doing", "holder: jjjjjjjj\n");
        task(
            dir.path(),
            "dddddddd",
            "done",
            "finished: 2026-11-02T08:10:00.000Z\nunchanged: The code already did it.\n",
        );
        dir
    }

    #[test]
    fn new_refuses_tasks_that_are_not_done_or_were_closed_without_changes_listing_every_problem() {
        let dir = project();
        let before = fs::read_dir(dir.path().join(".weaver")).unwrap().count();
        let error = run(
            "new",
            dir.path(),
            None,
            json!({ "title": "Work", "summary": "A line.", "tasks": ["bbbbbbbb", "cccccccc", "dddddddd", "zzzzzzzz", "aaaaaaaa"], "commit": commit() }),
        )
        .unwrap_err();
        assert_eq!((error.code.as_str(), error.exit_code()), (code::REFUSED, 1));
        for expected in [
            "tasks[0]",
            "To do",
            "tasks[1]",
            "in progress",
            "tasks[2]",
            "without changes",
            "tasks[3]",
        ] {
            assert!(
                error.message.contains(expected),
                "{expected} missing in {}",
                error.message
            );
        }
        assert!(!error.message.contains("tasks[4]"));
        assert!(
            !dir.path().join(".weaver/history").exists(),
            "a refusal writes nothing"
        );
        assert_eq!(
            fs::read_dir(dir.path().join(".weaver")).unwrap().count(),
            before
        );
        assert!(dir.path().join(".weaver/tasks/aaaaaaaa-job.md").is_file());
    }

    #[test]
    fn new_records_the_tasks_and_prints_the_entry_and_the_tasks_recorded() {
        let dir = project();
        put(dir.path(), "task-images/aaaaaaaa/a.png", "png");
        let reply = run("new", dir.path(), None, json!({ "title": "Work done", "summary": "A line.", "body": "More.", "tasks": ["aaaaaaaa"], "commit": commit() })).unwrap();
        assert_eq!(reply.json["recorded"], json!(["aaaaaaaa"]));
        assert_eq!(reply.json["title"], "Work done");
        assert!(reply.json["number"].is_null());
        let file = reply.json["file"].as_str().unwrap();
        assert!(file.starts_with("history/") && file.ends_with("-work-done.md"));
        assert!(dir.path().join(".weaver").join(file).is_file());
        assert!(!dir.path().join(".weaver/tasks/aaaaaaaa-job.md").exists());
        assert!(!dir.path().join(".weaver/task-images/aaaaaaaa").exists());
        assert!(reply.markdown.contains("Recorded in History"));

        let id = reply.json["id"].as_str().unwrap();
        let shown = run("show", dir.path(), Some(id), json!({})).unwrap();
        assert_eq!(shown.json["tasks"][0]["id"], "aaaaaaaa");
        assert_eq!(shown.json["tasks"][0]["title"], "Task aaaaaaaa");
        assert_eq!(
            shown.json["tasks"][0]["finished"],
            "2026-11-02T08:00:00.000Z"
        );
        assert_eq!(shown.json["body"], "More.");
        let listed = run("list", dir.path(), None, json!({})).unwrap();
        assert_eq!(listed.json["entries"][0]["id"], id);
        assert_eq!(listed.json["entries"][0]["tasks"], 1);
    }

    #[test]
    fn edit_adds_tasks_and_replaces_fields_and_never_removes_a_task() {
        let dir = project();
        task(
            dir.path(),
            "eeeeeeee",
            "done",
            "finished: 2026-11-02T08:20:00.000Z\n",
        );
        let created = run(
            "new",
            dir.path(),
            None,
            json!({ "title": "Work", "summary": "A line.", "tasks": ["aaaaaaaa"], "commit": commit() }),
        )
        .unwrap();
        let id = created.json["id"].as_str().unwrap().to_owned();
        let edited = run(
            "edit",
            dir.path(),
            Some(&id),
            json!({ "summary": "Another line.", "tasks": ["eeeeeeee"], "commit": commit() }),
        )
        .unwrap();
        assert_eq!(edited.json["recorded"], json!(["eeeeeeee"]));
        let shown = run("show", dir.path(), Some(&id), json!({})).unwrap();
        let ids: Vec<&str> = shown.json["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["id"].as_str().unwrap())
            .collect();
        assert_eq!(ids, ["aaaaaaaa", "eeeeeeee"]);
        assert_eq!(shown.json["summary"], "Another line.");
        let refused = run(
            "edit",
            dir.path(),
            Some(&id),
            json!({ "tasks": ["bbbbbbbb"] }),
        )
        .unwrap_err();
        assert!(refused.message.contains("tasks[0]"), "{}", refused.message);
    }

    #[test]
    fn a_task_that_cannot_be_copied_stays_and_is_said_on_the_note() {
        let dir = project();
        task(dir.path(), "ffffffff", "done", ""); // no `finished`
        let reply = run(
            "new",
            dir.path(),
            None,
            json!({ "title": "Work", "summary": "A line.", "tasks": ["ffffffff", "aaaaaaaa"], "commit": commit() }),
        )
        .unwrap();
        assert_eq!(reply.json["recorded"], json!(["aaaaaaaa"]));
        assert_eq!(reply.json["kept"], json!(["ffffffff"]));
        assert!(reply
            .note
            .as_deref()
            .is_some_and(|n| n.contains("ffffffff") && n.contains("finished")));
        assert!(dir.path().join(".weaver/tasks/ffffffff-job.md").is_file());
    }

    #[test]
    fn list_in_a_project_without_entries_says_so() {
        let dir = project();
        let reply = run("list", dir.path(), None, json!({})).unwrap();
        assert!(reply.markdown.contains("No History entry yet"));
        assert_eq!(reply.json, json!({ "entries": [] }));
    }

    #[test]
    fn with_atomic_commits_off_commit_is_checked_for_its_shape_only_and_the_status_is_off() {
        let dir = project();
        let reply = run(
            "new",
            dir.path(),
            None,
            json!({ "title": "Work", "summary": "A line.", "tasks": ["aaaaaaaa"], "commit": { "files": ["../outside", "nothing.rs"], "message": "Add the feature" } }),
        )
        .unwrap();
        assert_eq!(reply.json["commit"], json!({ "status": "off" }));
        assert!(!reply.markdown.contains("ommitted"), "{}", reply.markdown);
        // Settling an answer that is not pending changes nothing.
        let settled = crate::exec::settle(dir.path(), reply.clone());
        assert_eq!(settled, reply);

        let refused = run(
            "new",
            dir.path(),
            None,
            json!({ "title": "Other", "summary": "A line.", "tasks": ["eeeeeeee"], "commit": { "files": [], "message": "x".repeat(80) } }),
        )
        .unwrap_err();
        assert!(refused.message.contains("commit.message"), "{}", refused.message);
    }

    #[test]
    fn an_edit_needs_commit_only_when_it_adds_a_task_the_entry_did_not_cite() {
        let dir = project();
        task(dir.path(), "eeeeeeee", "done", "finished: 2026-11-02T08:20:00.000Z\n");
        let created = run(
            "new",
            dir.path(),
            None,
            json!({ "title": "Work", "summary": "A line.", "tasks": ["aaaaaaaa"], "commit": commit() }),
        )
        .unwrap();
        let id = created.json["id"].as_str().unwrap().to_owned();
        let missing = run("edit", dir.path(), Some(&id), json!({ "tasks": ["eeeeeeee"] })).unwrap_err();
        assert!(missing.message.contains("commit: is required"), "{}", missing.message);
        assert!(dir.path().join(".weaver/tasks/eeeeeeee-job.md").is_file(), "nothing was written");

        let text = run("edit", dir.path(), Some(&id), json!({ "summary": "Only text." })).unwrap();
        assert_eq!(text.json["commit"], json!({ "status": "off" }));
        // A task the entry already cites adds nothing: no commit is needed.
        let again = run("edit", dir.path(), Some(&id), json!({ "tasks": ["aaaaaaaa"] })).unwrap();
        assert_eq!(again.json["commit"], json!({ "status": "off" }));
    }

    fn git(root: &std::path::Path, args: &[&str]) -> String {
        let output = weaver_core::process::command("git", args.iter().copied())
            .current_dir(root)
            .output()
            .expect("git starts");
        assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    /// The project of `project()` inside a git repository, with Atomic commits on.
    fn repository() -> tempfile::TempDir {
        let dir = project();
        let root = weaver_core::paths::canonical_root(dir.path()).unwrap();
        git(&root, &["init", "-q"]);
        for (key, value) in [("user.name", "Dev"), ("user.email", "dev@example.com"), ("commit.gpgsign", "false"), ("core.hooksPath", ".git/hooks")] {
            git(&root, &["config", key, value]);
        }
        fs::write(root.join("README.md"), "readme\n").unwrap();
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "First"]);
        put(&root, "commits/commits.json", weaver_core::atomic_commits::STATE_TEXT);
        dir
    }

    #[test]
    fn with_atomic_commits_on_an_invalid_path_refuses_everything_and_a_valid_one_is_committed_after_the_write() {
        let dir = repository();
        let root = weaver_core::paths::canonical_root(dir.path()).unwrap();
        fs::write(root.join("a.rs"), "a\n").unwrap();
        let before = git(&root, &["rev-parse", "HEAD"]);

        let refused = run(
            "new",
            &root,
            None,
            json!({ "title": "Work", "summary": "A line.", "tasks": ["aaaaaaaa"], "commit": { "files": ["../outside", "a.rs", "gone.rs"], "message": "Add a" } }),
        )
        .unwrap_err();
        for expected in ["commit.files[0]: ../outside is outside the project", "commit.files[2]: gone.rs does not exist and git does not track it"] {
            assert!(refused.message.contains(expected), "{expected} missing in {}", refused.message);
        }
        assert!(!root.join(".weaver/history").exists(), "a refusal writes nothing");
        assert!(root.join(".weaver/tasks/aaaaaaaa-job.md").is_file());

        let reply = run(
            "new",
            &root,
            None,
            json!({ "title": "Work", "summary": "A line.", "tasks": ["aaaaaaaa"], "commit": { "files": ["a.rs"], "message": "Add a" } }),
        )
        .unwrap();
        // The entry is written; the commit waits for the lock to be released.
        assert_eq!(reply.json["commit"]["status"], "pending");
        assert_eq!(git(&root, &["rev-parse", "HEAD"]), before);
        let reply = crate::exec::settle(&root, reply);
        let hash = git(&root, &["rev-parse", "HEAD"]).trim().to_owned();
        assert_eq!(reply.json["commit"], json!({ "status": "committed", "hash": hash }));
        assert!(reply.markdown.ends_with(&format!("Committed {}.\n", &hash[..7])), "{}", reply.markdown);
        let files = git(&root, &["show", "--name-status", "--format=", "HEAD"]);
        assert!(files.contains("A\ta.rs") && files.contains("A\t.weaver/history/") && files.contains("D\t.weaver/tasks/aaaaaaaa-job.md"), "{files}");
    }

    #[test]
    fn a_commit_that_cannot_be_made_leaves_the_entry_written_and_says_why() {
        let dir = repository();
        let root = weaver_core::paths::canonical_root(dir.path()).unwrap();
        let reply = run(
            "new",
            &root,
            None,
            json!({ "title": "Work", "summary": "A line.", "tasks": ["aaaaaaaa"], "commit": { "files": [], "message": "Add a", "checksFailed": true } }),
        )
        .unwrap();
        let reply = crate::exec::settle(&root, reply);
        assert_eq!(reply.json["commit"], json!({ "status": "not-committed", "reason": "the checks of the Run failed" }));
        assert!(reply.markdown.ends_with("Not committed: the checks of the Run failed.\n"));
        assert!(root.join(".weaver/history").is_dir());
        assert!(!root.join(".weaver/tasks/aaaaaaaa-job.md").exists());
    }

    #[test]
    fn an_edit_that_adds_no_task_makes_no_commit_while_atomic_commits_is_on() {
        let dir = repository();
        let root = weaver_core::paths::canonical_root(dir.path()).unwrap();
        let created = run(
            "new",
            &root,
            None,
            json!({ "title": "Work", "summary": "A line.", "tasks": ["aaaaaaaa"], "commit": { "files": [], "message": "Add a", "checksFailed": true } }),
        )
        .unwrap();
        let id = created.json["id"].as_str().unwrap().to_owned();
        let before = git(&root, &["rev-parse", "HEAD"]);
        let edited = run("edit", &root, Some(&id), json!({ "summary": "Another line." })).unwrap();
        assert_eq!(edited.json["commit"], json!({ "status": "none" }));
        assert_eq!(crate::exec::settle(&root, edited.clone()), edited);
        assert_eq!(git(&root, &["rev-parse", "HEAD"]), before);
    }
}
