//! The commit that follows a History entry (`requirements/rules#atomic-commits`, feature
//! `project-setup`, Machine `atomic-commits`): while Atomic commits is on in the working copy
//! ([`crate::atomic_commits::is_on`]), writing an entry, or updating one with Tasks it did not cite,
//! commits the code of that feature with git, one commit per entry, and never changes an earlier one.
//!
//! The command that writes the entry goes through three steps:
//!
//! 1. [`parse`] reads the `commit` of its input and checks its shape, whether Atomic commits is on
//!    or not;
//! 2. [`plan`], before anything is written, says what follows the write: nothing (`off`, `none`),
//!    or a [`Pending`] commit whose paths were checked against git ([`check_paths`]); an invalid
//!    path refuses the whole command;
//! 3. [`commit`], once the entry is written and the project's lock is released, stages and commits
//!    in the queue of the working copy, one commit at a time, and answers with an [`Outcome`].
//!
//! A commit that cannot be made never undoes the entry: its [`Outcome`] says why. Git is started
//! through the runner of Solo mode ([`Git`]), never on the thread of the UI, and never with
//! `--no-verify`, `--amend`, `-a`, `--allow-empty`, a push or a fetch: hooks and signing run as the
//! developer configured them.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::atomic_commits;
use crate::paths;
use crate::process::timed::Failure;
use crate::project::history::Problem;
use crate::project_files::folder_tree::git_ignored_by;
use crate::solo_merge::{self, Unavailable};
use crate::solo_mode::{self, block_lines_of_source, git_failure, reason_of, Call, Git};

/// The longest first line of a commit message, in characters.
pub const MESSAGE_LINE_MAX: usize = 72;

/// The reason of a commit not made because the final checks of the Run failed.
pub const REASON_CHECKS_FAILED: &str = "the checks of the Run failed";
/// The reason of a merge, a rebase or a cherry-pick in progress.
pub const REASON_OPERATION: &str = "a merge, a rebase or a cherry-pick is in progress";
/// The reason of a detached HEAD.
pub const REASON_DETACHED: &str = "HEAD is detached";
/// The reason of a `git commit` killed at its limit.
pub const REASON_COMMIT_LIMIT: &str = "git did not finish the commit within 2 minutes";

/// The `commit` of `history new` and `history edit`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInput {
    /// Paths of the code of the entry's feature, relative to the project root, with `/`.
    pub files: Vec<String>,
    /// The commit message in English.
    pub message: String,
    /// The final checks of the Run failed.
    pub checks_failed: bool,
}

fn problem(path: impl Into<String>, message: impl Into<String>) -> Problem {
    Problem {
        path: path.into(),
        message: message.into(),
    }
}

/// Reads and checks the shape of the `commit` of an input; `None` when it is absent. Every problem
/// is listed.
pub fn parse(commit: Option<&Value>) -> Result<Option<CommitInput>, Vec<Problem>> {
    let Some(commit) = commit else {
        return Ok(None);
    };
    let Some(map) = commit.as_object() else {
        return Err(vec![problem("commit", "must be an object")]);
    };
    let mut problems = Vec::new();
    let mut files: Vec<String> = Vec::new();
    match map.get("files").and_then(Value::as_array) {
        Some(items) => {
            let mut seen: BTreeSet<&str> = BTreeSet::new();
            for (index, item) in items.iter().enumerate() {
                match item.as_str() {
                    Some(path) => {
                        if !seen.insert(path) {
                            problems.push(problem(
                                format!("commit.files[{index}]"),
                                format!("{path} is listed twice"),
                            ));
                        }
                        files.push(path.to_owned());
                    }
                    None => problems.push(problem(
                        format!("commit.files[{index}]"),
                        "must be a string",
                    )),
                }
            }
        }
        None => problems.push(problem("commit.files", "must be a list of paths")),
    }
    let message = match map.get("message").and_then(Value::as_str) {
        Some(message) => {
            match message.lines().find(|line| !line.trim().is_empty()) {
                None => problems.push(problem("commit.message", "the message is empty")),
                Some(first) => {
                    let length = first.trim().chars().count();
                    if length > MESSAGE_LINE_MAX {
                        problems.push(problem(
                            "commit.message",
                            format!(
                                "the first line has {length} characters; at most {MESSAGE_LINE_MAX}"
                            ),
                        ));
                    }
                }
            }
            message.to_owned()
        }
        None => {
            problems.push(problem("commit.message", "must be a string"));
            String::new()
        }
    };
    let checks_failed = match map.get("checksFailed") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(flag)) => *flag,
        Some(_) => {
            problems.push(problem("commit.checksFailed", "must be true or false"));
            false
        }
    };
    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(Some(CommitInput {
        files,
        message,
        checks_failed,
    }))
}

// ---------------------------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------------------------

/// The path with `.` and `..` resolved, relative to the project root, with `/`; `None` when it is
/// absolute, empty, names the root itself or leaves it.
fn normalise(path: &str) -> Option<String> {
    if path.is_empty() || path.starts_with('/') || Path::new(path).is_absolute() {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The paths among `paths` that git tracks (`git ls-files -z --cached`), a folder counting when
/// git tracks a file inside it; `None` when git cannot say.
fn tracked_among(git: &Git, root: &Path, paths: &[&str]) -> Option<BTreeSet<String>> {
    if paths.is_empty() {
        return Some(BTreeSet::new());
    }
    let mut args = vec!["ls-files", "-z", "--cached", "--"];
    args.extend_from_slice(paths);
    let output = git.exec(root, &args, Call::Read).ok()?;
    let listed: Vec<&str> = output
        .stdout
        .split('\0')
        .filter(|path| !path.is_empty())
        .collect();
    Some(
        paths
            .iter()
            .filter(|path| {
                listed
                    .iter()
                    .any(|found| found == *path || found.starts_with(&format!("{path}/")))
            })
            .map(|path| (*path).to_owned())
            .collect(),
    )
}

/// Checks the paths of `input` against the project and git, before anything is written
/// (`requirements/rules#atomic-commits`, **Paths**): a path outside the project, ignored by git and
/// untracked, or missing and untracked is a problem `commit.files[<i>]: <path> <problem>`, and
/// every problem is listed. With Solo mode on, a path that a line of its block matches is left out
/// of the answer without a problem. Returns the paths to stage, normalised, in the order given.
///
/// What git cannot say (no git, not a repository, a call over its limit) is no problem here: the
/// commit step reports it as the reason of the commit it cannot make.
pub fn check_paths(
    git: &Git,
    root: &Path,
    input: &CommitInput,
) -> Result<Vec<String>, Vec<Problem>> {
    let mut problems: Vec<Problem> = Vec::new();
    let mut named: Vec<(usize, &str, String)> = Vec::new();
    for (index, path) in input.files.iter().enumerate() {
        match normalise(path) {
            Some(normal) => named.push((index, path.as_str(), normal)),
            None => problems.push(problem(
                format!("commit.files[{index}]"),
                format!("{path} is outside the project"),
            )),
        }
    }
    let mode = solo_mode::read_state(root).mode;
    let mut left_out: BTreeSet<String> = BTreeSet::new();

    // Ignored by git: by a line of the Solo mode block, the path is left out; by any other rule it
    // is a problem, unless git tracks it (check-ignore does not report tracked paths).
    let normals: Vec<&str> = named.iter().map(|(_, _, normal)| normal.as_str()).collect();
    let mut blocks: HashMap<String, Vec<std::ops::RangeInclusive<usize>>> = HashMap::new();
    let mut ignored: BTreeSet<String> = BTreeSet::new();
    for found in git_ignored_by(root, &normals) {
        let lines = blocks
            .entry(found.source.clone())
            .or_insert_with(|| block_lines_of_source(root, &found.source));
        if mode.is_on() && lines.iter().any(|range| range.contains(&found.line)) {
            left_out.insert(found.path);
        } else {
            ignored.insert(found.path);
        }
    }
    for (index, path, normal) in &named {
        if ignored.contains(normal) {
            problems.push(problem(
                format!("commit.files[{index}]"),
                format!("{path} is ignored by git"),
            ));
        }
    }

    // Missing from the working tree: valid only when git tracks it (its removal is committed).
    let missing: Vec<(usize, &str, &str)> = named
        .iter()
        .filter(|(_, _, normal)| !ignored.contains(normal) && !left_out.contains(normal))
        .filter(|(_, _, normal)| fs::symlink_metadata(root.join(normal)).is_err())
        .map(|(index, path, normal)| (*index, *path, normal.as_str()))
        .collect();
    if !missing.is_empty() {
        let names: Vec<&str> = missing.iter().map(|(_, _, normal)| *normal).collect();
        if let Some(tracked) = tracked_among(git, root, &names) {
            for (index, path, normal) in &missing {
                if !tracked.contains(*normal) {
                    problems.push(problem(
                        format!("commit.files[{index}]"),
                        format!("{path} does not exist and git does not track it"),
                    ));
                }
            }
        }
    }
    if !problems.is_empty() {
        problems.sort_by_key(|p| index_of(&p.path));
        return Err(problems);
    }

    // With Solo mode on, a tracked file that a line of its block matches stays out of the commit
    // too: Solo mode keeps it out of git.
    if mode.is_on() {
        if let Ok(location) = solo_mode::locate(git, root) {
            if let Ok(tracked) = location.tracked_files(git, mode) {
                left_out.extend(
                    named
                        .iter()
                        .map(|(_, _, normal)| normal)
                        .filter(|normal| tracked.contains(*normal))
                        .cloned(),
                );
            }
        }
    }
    Ok(named
        .into_iter()
        .map(|(_, _, normal)| normal)
        .filter(|normal| !left_out.contains(normal))
        .collect())
}

/// The `<i>` of `commit.files[<i>]`, to list the problems in the order of the paths.
fn index_of(path: &str) -> usize {
    path.strip_prefix("commit.files[")
        .and_then(|rest| rest.strip_suffix(']'))
        .and_then(|number| number.parse().ok())
        .unwrap_or(usize::MAX)
}

// ---------------------------------------------------------------------------------------------
// The plan and the commit
// ---------------------------------------------------------------------------------------------

/// A commit still to be made, once the entry is written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    /// The valid paths of the feature to stage, relative to the project root, with `/`.
    pub files: Vec<String>,
    /// The agent's message.
    pub message: String,
    /// The final checks of the Run failed: no commit.
    pub checks_failed: bool,
    /// The file of the entry, relative to the project root; outside Solo mode it is staged.
    pub entry: String,
    /// The files the write deleted (the recorded Tasks'), relative to the project root; outside
    /// Solo mode their removal is staged.
    pub removed: Vec<String>,
}

/// What follows the write of an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// Atomic commits is off: nothing runs, `commit` is only checked for its shape.
    Off,
    /// An edit that adds no Task: no commit.
    None,
    /// A commit follows.
    Commit(Pending),
}

/// Plans what follows the write of an entry whose file is `entry` and whose write deletes
/// `removed` (both relative to the project root). `adds_tasks` says whether the write cites a Task
/// the entry did not cite (always for a new entry). Nothing runs when Atomic commits is off.
pub fn plan(
    git: &Git,
    root: &Path,
    commit: Option<&CommitInput>,
    adds_tasks: bool,
    entry: &str,
    removed: Vec<String>,
) -> Result<Plan, Vec<Problem>> {
    if adds_tasks && commit.is_none() {
        return Err(vec![problem(
            "commit",
            "is required when the entry gets Tasks it did not cite",
        )]);
    }
    if !atomic_commits::is_on(root) {
        return Ok(Plan::Off);
    }
    let Some(commit) = commit.filter(|_| adds_tasks) else {
        return Ok(Plan::None);
    };
    let files = check_paths(git, root, commit)?;
    Ok(Plan::Commit(Pending {
        files,
        message: commit.message.clone(),
        checks_failed: commit.checks_failed,
        entry: entry.to_owned(),
        removed,
    }))
}

/// How a commit ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Atomic commits is off.
    Off,
    /// Nothing to commit, or an edit that adds no Task.
    None,
    /// The commit was made: its full hash.
    Committed(String),
    /// No commit was made: why.
    NotCommitted(String),
}

impl Outcome {
    /// `{status, hash?, reason?}`: `off`, `none`, `committed` or `not-committed`.
    pub fn to_value(&self) -> Value {
        match self {
            Outcome::Off => json!({ "status": "off" }),
            Outcome::None => json!({ "status": "none" }),
            Outcome::Committed(hash) => json!({ "status": "committed", "hash": hash }),
            Outcome::NotCommitted(reason) => {
                json!({ "status": "not-committed", "reason": reason })
            }
        }
    }

    /// The last line of the text output: `Committed <short hash>.` or `Not committed: <reason>.`;
    /// nothing for `off` and `none`.
    pub fn line(&self) -> Option<String> {
        match self {
            Outcome::Committed(hash) => {
                Some(format!("Committed {}.", hash.chars().take(7).collect::<String>()))
            }
            Outcome::NotCommitted(reason) => Some(format!("Not committed: {reason}.")),
            Outcome::Off | Outcome::None => None,
        }
    }
}

/// The `commit` of an answer while the commit still waits: `{"status": "pending", ...}`. The
/// caller of the command makes the commit once the project's lock is released ([`settle`]).
pub fn pending_value(pending: &Pending) -> Value {
    let mut value = serde_json::to_value(pending).unwrap_or(Value::Null);
    if let Some(map) = value.as_object_mut() {
        map.insert("status".to_owned(), Value::String("pending".to_owned()));
    }
    value
}

/// Takes the pending commit out of the `commit` of an answer, when there is one.
pub fn take_pending(json: &mut Value) -> Option<Pending> {
    let commit = json.get("commit")?;
    if commit.get("status").and_then(Value::as_str) != Some("pending") {
        return None;
    }
    serde_json::from_value(commit.clone()).ok()
}

/// Makes the pending commit of an answer, if it has one, and puts its outcome in the answer: the
/// `commit` of its JSON and, at the end of its text, `Committed <short hash>.` or
/// `Not committed: <reason>.`. Call it when the project's lock is released.
pub fn settle(git: &Git, root: &Path, json: &mut Value, markdown: &mut String) {
    let Some(pending) = take_pending(json) else {
        return;
    };
    let outcome = commit(git, root, &pending);
    if let Some(map) = json.as_object_mut() {
        map.insert("commit".to_owned(), outcome.to_value());
    }
    if let Some(line) = outcome.line() {
        if !markdown.ends_with('\n') {
            markdown.push('\n');
        }
        markdown.push('\n');
        markdown.push_str(&line);
        markdown.push('\n');
    }
}

fn queue_of(root: &Path) -> Arc<Mutex<()>> {
    static QUEUES: LazyLock<Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> =
        LazyLock::new(Mutex::default);
    let key = paths::canonical_root(root).unwrap_or_else(|_| root.to_path_buf());
    QUEUES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .entry(key)
        .or_default()
        .clone()
}

/// Stages and commits `pending`, in the queue of the working copy: one commit at a time, however
/// many entries are written at once. The reasons of no commit, in order: the checks of the Run
/// failed; git cannot be used (`git was not found`, `this folder is not in a git repository`,
/// `git did not answer within 10 s`, `git failed: ...`); a merge, a rebase or a cherry-pick is in
/// progress; HEAD is detached.
pub fn commit(git: &Git, root: &Path, pending: &Pending) -> Outcome {
    let queue = queue_of(root);
    let _turn = queue
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    match try_commit(git, root, pending) {
        Ok(outcome) => outcome,
        Err(reason) => Outcome::NotCommitted(reason),
    }
}

/// The reason of a failed git call, as the texts of Solo mode say it.
fn failed(failure: Failure) -> String {
    reason_of(failure, false).to_string()
}

fn try_commit(git: &Git, root: &Path, pending: &Pending) -> Result<Outcome, String> {
    if pending.checks_failed {
        return Err(REASON_CHECKS_FAILED.to_owned());
    }
    let folder = paths::canonical_root(root)
        .map_err(|e| solo_mode::Reason::Root(e.to_string()).to_string())?;
    solo_mode::locate(git, &folder).map_err(|reason| reason.to_string())?;
    match solo_merge::availability(git, &folder, false).unavailable {
        Some(Unavailable::MergeInProgress | Unavailable::GitBusy) => {
            return Err(REASON_OPERATION.to_owned())
        }
        Some(Unavailable::Detached) => return Err(REASON_DETACHED.to_owned()),
        Some(Unavailable::Git(reason)) => return Err(reason.to_string()),
        // Merge's own conditions (a job working, uncommitted changes) are no reason here.
        Some(Unavailable::AgentWorking | Unavailable::Uncommitted) | None => {}
    }
    let solo = solo_mode::read_state(&folder).mode.is_on();

    let mut add: Vec<&str> = pending.files.iter().map(String::as_str).collect();
    if !solo {
        add.push(pending.entry.as_str());
    }
    if !add.is_empty() {
        let mut args = vec!["add", "-A", "--"];
        args.extend_from_slice(&add);
        git.exec(&folder, &args, Call::Stage).map_err(failed)?;
    }
    if !solo && !pending.removed.is_empty() {
        let mut args = vec!["rm", "-q", "--cached", "--ignore-unmatch", "--"];
        args.extend(pending.removed.iter().map(String::as_str));
        git.exec(&folder, &args, Call::Stage).map_err(failed)?;
    }

    match git.exec(&folder, &["diff", "--cached", "--quiet"], Call::Read) {
        Ok(_) => return Ok(Outcome::None),
        Err(Failure::Exit { code: Some(1), .. }) => {}
        Err(other) => return Err(failed(other)),
    }

    let mut message = pending.message.trim_end().to_owned();
    message.push('\n');
    git.exec_with_input(
        &folder,
        &["commit", "--cleanup=strip", "-F", "-"],
        Call::Commit,
        Some(message.as_bytes()),
    )
    .map_err(|failure| match failure {
        Failure::TimedOut { .. } => REASON_COMMIT_LIMIT.to_owned(),
        Failure::Exit { code, stderr } => git_failure(code, &stderr).to_string(),
        other => failed(other),
    })?;

    let head = git
        .exec(&folder, &["rev-parse", "HEAD"], Call::Read)
        .map_err(failed)?;
    Ok(Outcome::Committed(head.stdout.trim().to_owned()))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::process;
    use crate::solo_mode::Mode;

    const ENTRY: &str = ".weaver/history/hhhhhhhh-work.md";

    fn available() -> bool {
        process::command("git", ["--version"])
            .output()
            .is_ok_and(|output| output.status.success())
    }

    /// Runs git in `root` and returns what it printed; a failure is a failed test.
    fn git(root: &Path, args: &[&str]) -> String {
        let output = process::command("git", args.iter().copied())
            .current_dir(root)
            .output()
            .expect("git starts");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    fn put(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    /// A repository with one commit, in a temporary folder; `sub` is the project folder inside it
    /// (empty: the repository's root).
    fn repository(sub: &str) -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let top = paths::canonical_root(dir.path()).unwrap();
        git(&top, &["init", "-q"]);
        for (key, value) in [
            ("user.name", "Dev Eloper"),
            ("user.email", "dev@example.com"),
            ("commit.gpgsign", "false"),
            ("core.hooksPath", ".git/hooks"),
        ] {
            git(&top, &["config", key, value]);
        }
        put(&top, "README.md", "readme\n");
        git(&top, &["add", "-A"]);
        git(&top, &["commit", "-q", "-m", "First"]);
        let root = if sub.is_empty() {
            top.clone()
        } else {
            let root = top.join(sub);
            fs::create_dir_all(&root).unwrap();
            root
        };
        put(&root, ".weaver/model/system.md", "# Shop\n");
        (dir, top, root)
    }

    fn turn_on(root: &Path) {
        put(
            root,
            ".weaver/commits/commits.json",
            atomic_commits::STATE_TEXT,
        );
    }

    fn input(files: &[&str], message: &str) -> CommitInput {
        CommitInput {
            files: files.iter().map(|f| (*f).to_owned()).collect(),
            message: message.to_owned(),
            checks_failed: false,
        }
    }

    fn pending(root: &Path, files: &[&str]) -> Pending {
        put(root, ENTRY, "---\ncreated: x\n---\n# Work\n");
        Pending {
            files: files.iter().map(|f| (*f).to_owned()).collect(),
            message: "Add the cart".to_owned(),
            checks_failed: false,
            entry: ENTRY.to_owned(),
            removed: Vec::new(),
        }
    }

    fn committed(outcome: Outcome) -> String {
        match outcome {
            Outcome::Committed(hash) => hash,
            other => panic!("not committed: {other:?}"),
        }
    }

    fn not_committed(outcome: Outcome) -> String {
        match outcome {
            Outcome::NotCommitted(reason) => reason,
            other => panic!("committed or nothing to commit: {other:?}"),
        }
    }

    /// The files of the last commit of `top`, sorted.
    fn last_files(top: &Path) -> Vec<String> {
        let mut files: Vec<String> = git(top, &["show", "--name-status", "--format=", "HEAD"])
            .lines()
            .map(|line| line.replace('\t', " "))
            .collect();
        files.sort();
        files
    }

    #[test]
    fn the_shape_of_commit_is_checked_and_every_problem_is_listed() {
        assert_eq!(parse(None), Ok(None));
        let ok = json!({ "files": ["a.rs", "b.rs"], "message": "Add a thing\n\nLonger.", "checksFailed": true });
        let parsed = parse(Some(&ok)).unwrap().unwrap();
        assert_eq!(parsed.files, ["a.rs", "b.rs"]);
        assert!(parsed.checks_failed);
        assert_eq!(parse(Some(&json!({ "files": [], "message": "x" }))).unwrap().unwrap().files.len(), 0);

        let long = "x".repeat(73);
        let bad = json!({ "files": ["a.rs", "a.rs"], "message": long, "checksFailed": "yes" });
        let problems = parse(Some(&bad)).unwrap_err();
        let texts: Vec<String> = problems
            .iter()
            .map(|p| format!("{}: {}", p.path, p.message))
            .collect();
        assert_eq!(
            texts,
            [
                "commit.files[1]: a.rs is listed twice",
                "commit.message: the first line has 73 characters; at most 72",
                "commit.checksFailed: must be true or false",
            ]
        );
        let blank = parse(Some(&json!({ "files": [], "message": " \n " }))).unwrap_err();
        assert_eq!(blank[0].message, "the message is empty");
        let only_second_line_long = json!({ "files": [], "message": format!("Short\n\n{long}") });
        assert!(parse(Some(&only_second_line_long)).is_ok());
    }

    #[test]
    fn plan_runs_no_git_while_atomic_commits_is_off_and_asks_for_commit_when_tasks_are_added() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let nowhere = Git::system().with_search_path("");
        let given = input(&["../outside", "missing.rs"], "Add");
        assert_eq!(
            plan(&nowhere, root, Some(&given), true, ENTRY, Vec::new()),
            Ok(Plan::Off)
        );
        assert_eq!(plan(&nowhere, root, None, false, ENTRY, Vec::new()), Ok(Plan::Off));
        let missing = plan(&nowhere, root, None, true, ENTRY, Vec::new()).unwrap_err();
        assert_eq!(missing[0].path, "commit");
    }

    #[test]
    fn an_edit_that_adds_no_task_makes_no_commit_even_with_atomic_commits_on() {
        let dir = tempfile::tempdir().unwrap();
        turn_on(dir.path());
        let nowhere = Git::system().with_search_path("");
        assert_eq!(
            plan(&nowhere, dir.path(), None, false, ENTRY, Vec::new()),
            Ok(Plan::None)
        );
        let ignored = input(&["../outside"], "Add");
        assert_eq!(
            plan(&nowhere, dir.path(), Some(&ignored), false, ENTRY, Vec::new()),
            Ok(Plan::None)
        );
    }

    #[test]
    fn paths_outside_the_project_ignored_or_missing_and_untracked_are_refused_together() {
        if !available() {
            return;
        }
        let (_dir, top, root) = repository("");
        put(&top, ".gitignore", "*.log\n");
        put(&top, "src/old.rs", "old\n");
        git(&top, &["add", "-A"]);
        git(&top, &["commit", "-q", "-m", "Second"]);
        fs::remove_file(top.join("src/old.rs")).unwrap();
        put(&root, "src/new.rs", "new\n");
        put(&root, "debug.log", "noise\n");
        turn_on(&root);

        let given = input(
            &[
                "../outside.rs",
                "debug.log",
                "src/missing.rs",
                "src/old.rs",
                "src/new.rs",
                "/etc/passwd",
                "src/../src/new.rs",
            ],
            "Add",
        );
        let problems = check_paths(&Git::system(), &root, &given).unwrap_err();
        let texts: Vec<String> = problems
            .iter()
            .map(|p| format!("{}: {}", p.path, p.message))
            .collect();
        assert_eq!(
            texts,
            [
                "commit.files[0]: ../outside.rs is outside the project",
                "commit.files[1]: debug.log is ignored by git",
                "commit.files[2]: src/missing.rs does not exist and git does not track it",
                "commit.files[5]: /etc/passwd is outside the project",
            ]
        );
        // Through plan, the same problems refuse the command.
        let refused = plan(&Git::system(), &root, Some(&given), true, ENTRY, Vec::new());
        assert_eq!(refused.unwrap_err().len(), 4);

        let fine = input(&["src/old.rs", "src/new.rs", "./src/new2.rs/../new.rs"], "Add");
        put(&root, "src/new.rs", "new\n");
        assert_eq!(
            check_paths(&Git::system(), &root, &fine).unwrap(),
            ["src/old.rs", "src/new.rs", "src/new.rs"]
        );
    }

    #[test]
    fn a_removed_tracked_file_is_committed_as_a_removal_with_the_entry_and_the_agents_message() {
        if !available() {
            return;
        }
        let (_dir, top, root) = repository("");
        put(&top, "src/old.rs", "old\n");
        put(&top, "src/keep.rs", "keep\n");
        git(&top, &["add", "-A"]);
        git(&top, &["commit", "-q", "-m", "Second"]);
        fs::remove_file(top.join("src/old.rs")).unwrap();
        put(&top, "src/keep.rs", "kept and changed\n");
        put(&top, "src/unrelated.rs", "somebody else's work\n");
        put(&top, "src/new.rs", "new\n");

        let work = pending(&root, &["src/old.rs", "src/new.rs"]);
        let hash = committed(commit(&Git::system(), &root, &work));
        assert_eq!(hash, git(&top, &["rev-parse", "HEAD"]).trim());
        assert_eq!(
            last_files(&top),
            [
                "A .weaver/history/hhhhhhhh-work.md",
                "A src/new.rs",
                "D src/old.rs",
            ]
        );
        // Only the paths named went in; the rest of the working copy is untouched.
        let status = git(&top, &["status", "--porcelain"]);
        assert!(status.contains(" M src/keep.rs") && status.contains("?? src/unrelated.rs"), "{status}");
        let message = git(&top, &["log", "-1", "--format=%B"]);
        assert_eq!(message.trim_end(), "Add the cart");
        assert!(!message.contains("Co-authored-by"), "{message}");
        // Author and committer come from git's configuration.
        assert_eq!(
            git(&top, &["log", "-1", "--format=%an <%ae> / %cn <%ce>"]).trim(),
            "Dev Eloper <dev@example.com> / Dev Eloper <dev@example.com>"
        );
        // The earlier commit is as it was.
        assert_eq!(git(&top, &["rev-list", "--count", "HEAD"]).trim(), "3");
    }

    #[test]
    fn the_files_of_the_recorded_tasks_enter_as_removals_and_what_the_developer_staged_goes_in() {
        if !available() {
            return;
        }
        let (_dir, top, root) = repository("");
        put(&root, ".weaver/tasks/aaaaaaaa-job.md", "# Task\n");
        git(&top, &["add", "-A"]);
        git(&top, &["commit", "-q", "-m", "Tasks"]);
        fs::remove_file(root.join(".weaver/tasks/aaaaaaaa-job.md")).unwrap();
        put(&top, "docs/staged.md", "staged by the developer\n");
        git(&top, &["add", "docs/staged.md"]);
        put(&top, "src/a.rs", "a\n");

        let mut work = pending(&root, &["src/a.rs"]);
        work.removed = vec![
            ".weaver/tasks/aaaaaaaa-job.md".to_owned(),
            ".weaver/task-images/aaaaaaaa/a.png".to_owned(),
        ];
        committed(commit(&Git::system(), &root, &work));
        assert_eq!(
            last_files(&top),
            [
                "A .weaver/history/hhhhhhhh-work.md",
                "A docs/staged.md",
                "A src/a.rs",
                "D .weaver/tasks/aaaaaaaa-job.md",
            ]
        );
    }

    #[test]
    fn a_project_in_a_subfolder_commits_its_own_paths() {
        if !available() {
            return;
        }
        let (_dir, top, root) = repository("app");
        put(&root, "src/lib.rs", "lib\n");
        turn_on(&root);
        let given = input(&["src/lib.rs"], "Add the lib");
        let checked = check_paths(&Git::system(), &root, &given).unwrap();
        let work = Pending {
            files: checked,
            ..pending(&root, &[])
        };
        committed(commit(&Git::system(), &root, &work));
        assert_eq!(
            last_files(&top),
            ["A app/.weaver/history/hhhhhhhh-work.md", "A app/src/lib.rs"]
        );
    }

    #[test]
    fn with_solo_mode_on_no_file_of_weaver_goes_in_and_the_message_is_the_agents_alone() {
        if !available() {
            return;
        }
        let (_dir, top, root) = repository("");
        let git_runner = Git::system();
        solo_mode::set_mode(&git_runner, &root, Mode::Folders).unwrap();
        turn_on(&root);
        put(&root, "src/a.rs", "a\n");
        put(&root, ".weaver/notes.txt", "a file of Weaver the agent named\n");

        // A path the block matches is left out without a problem, and the others are checked.
        let given = input(&["src/a.rs", ".weaver/notes.txt"], "Add a");
        let checked = check_paths(&git_runner, &root, &given).unwrap();
        assert_eq!(checked, ["src/a.rs"]);

        let work = Pending {
            files: checked,
            ..pending(&root, &[])
        };
        committed(commit(&git_runner, &root, &work));
        assert_eq!(last_files(&top), ["A src/a.rs"]);
        let message = git(&top, &["log", "-1", "--format=%B"]);
        assert_eq!(message.trim_end(), "Add the cart");
        assert!(!message.contains("Co-authored-by"), "{message}");
        assert_eq!(
            git(&top, &["log", "-1", "--format=%an <%ae> / %cn <%ce>"]).trim(),
            "Dev Eloper <dev@example.com> / Dev Eloper <dev@example.com>"
        );
    }

    #[test]
    fn a_tracked_file_that_solo_mode_keeps_out_is_left_out_unless_the_developer_staged_it() {
        if !available() {
            return;
        }
        let (_dir, top, root) = repository("");
        put(&root, ".agents/skills/weaver-x/SKILL.md", "skill\n");
        git(&top, &["add", "-f", ".agents"]);
        git(&top, &["commit", "-q", "-m", "A tracked skill"]);
        let git_runner = Git::system();
        solo_mode::set_mode(&git_runner, &root, Mode::Folders).unwrap();
        turn_on(&root);
        put(&root, ".agents/skills/weaver-x/SKILL.md", "changed\n");
        put(&root, "src/a.rs", "a\n");

        let given = input(&["src/a.rs", ".agents/skills/weaver-x/SKILL.md"], "Add a");
        assert_eq!(check_paths(&git_runner, &root, &given).unwrap(), ["src/a.rs"]);
        let work = Pending {
            files: vec!["src/a.rs".to_owned()],
            ..pending(&root, &[])
        };
        committed(commit(&git_runner, &root, &work));
        assert_eq!(last_files(&top), ["A src/a.rs"]);

        // What the developer staged stays staged and goes into the next commit.
        put(&root, "src/b.rs", "b\n");
        git(&top, &["add", "-f", ".agents/skills/weaver-x/SKILL.md"]);
        let work = Pending {
            files: vec!["src/b.rs".to_owned()],
            ..pending(&root, &[])
        };
        committed(commit(&git_runner, &root, &work));
        assert_eq!(
            last_files(&top),
            ["A src/b.rs", "M .agents/skills/weaver-x/SKILL.md"]
        );
    }

    #[test]
    fn nothing_to_commit_makes_no_commit() {
        if !available() {
            return;
        }
        let (_dir, top, root) = repository("");
        git(&top, &["add", "-A"]);
        git(&top, &["commit", "-q", "-m", "Weaver files"]);
        let before = git(&top, &["rev-parse", "HEAD"]);
        let mut work = pending(&root, &[]);
        git(&top, &["add", "-A"]);
        git(&top, &["commit", "-q", "-m", "The entry"]);
        let before_entry = git(&top, &["rev-parse", "HEAD"]);
        assert_ne!(before, before_entry);
        work.files = vec!["README.md".to_owned()];
        assert_eq!(commit(&Git::system(), &root, &work), Outcome::None);
        assert_eq!(git(&top, &["rev-parse", "HEAD"]), before_entry);
    }

    #[test]
    fn the_reasons_of_no_commit_come_in_their_order() {
        if !available() {
            return;
        }
        let (_dir, top, root) = repository("");
        put(&root, "src/a.rs", "a\n");
        let mut work = pending(&root, &["src/a.rs"]);
        let nowhere = Git::system().with_search_path("");

        // 1: the checks failed, before anything else is looked at.
        work.checks_failed = true;
        assert_eq!(
            not_committed(commit(&nowhere, &root, &work)),
            "the checks of the Run failed"
        );
        work.checks_failed = false;

        // 2: git cannot be used.
        assert_eq!(
            not_committed(commit(&nowhere, &root, &work)),
            "git was not found"
        );
        let outside = tempfile::tempdir().unwrap();
        let bare = Pending {
            entry: "x.md".to_owned(),
            files: Vec::new(),
            ..work.clone()
        };
        assert_eq!(
            not_committed(commit(&Git::system(), outside.path(), &bare)),
            "this folder is not in a git repository"
        );

        // 3: a merge of Weaver, then a merge, a rebase or a cherry-pick of git.
        put(&root, ".weaver/merge/merge.json", "{}");
        assert_eq!(
            not_committed(commit(&Git::system(), &root, &work)),
            "a merge, a rebase or a cherry-pick is in progress"
        );
        fs::remove_file(root.join(".weaver/merge/merge.json")).unwrap();
        let head = git(&top, &["rev-parse", "HEAD"]);
        fs::write(top.join(".git/CHERRY_PICK_HEAD"), head.trim()).unwrap();
        assert_eq!(
            not_committed(commit(&Git::system(), &root, &work)),
            "a merge, a rebase or a cherry-pick is in progress"
        );
        fs::remove_file(top.join(".git/CHERRY_PICK_HEAD")).unwrap();

        // 4: HEAD is detached.
        git(&top, &["checkout", "-q", "--detach"]);
        assert_eq!(
            not_committed(commit(&Git::system(), &root, &work)),
            "HEAD is detached"
        );
        assert_eq!(git(&top, &["rev-list", "--count", "HEAD"]).trim(), "1");
    }

    #[cfg(unix)]
    fn hook(top: &Path, script: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = top.join(".git/hooks/pre-commit");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_hook_that_refuses_gives_its_first_line_and_leaves_what_was_staged_staged() {
        if !available() {
            return;
        }
        let (_dir, top, root) = repository("");
        put(&root, "src/a.rs", "a\n");
        hook(&top, "echo 'the tests are red' >&2\necho second >&2\nexit 1");
        let work = pending(&root, &["src/a.rs"]);
        assert_eq!(
            not_committed(commit(&Git::system(), &root, &work)),
            "git failed: the tests are red"
        );
        let staged = git(&top, &["diff", "--cached", "--name-only"]);
        assert!(staged.contains("src/a.rs") && staged.contains(".weaver/history/"), "{staged}");
        assert_eq!(git(&top, &["rev-list", "--count", "HEAD"]).trim(), "1");
    }

    #[cfg(unix)]
    #[test]
    fn a_commit_over_its_limit_is_killed_and_says_so() {
        if !available() {
            return;
        }
        let (_dir, top, root) = repository("");
        put(&root, "src/a.rs", "a\n");
        hook(&top, "sleep 5");
        let work = pending(&root, &["src/a.rs"]);
        let quick = Git::system().with_commit_limit(Duration::from_millis(300));
        assert_eq!(
            not_committed(commit(&quick, &root, &work)),
            "git did not finish the commit within 2 minutes"
        );
        assert_eq!(git(&top, &["rev-list", "--count", "HEAD"]).trim(), "1");
    }

    #[test]
    fn two_entries_written_together_are_committed_one_after_the_other() {
        if !available() {
            return;
        }
        let (_dir, top, root) = repository("");
        put(&root, "src/a.rs", "a\n");
        put(&root, "src/b.rs", "b\n");
        put(&root, ".weaver/history/iiiiiiii-other.md", "---\ncreated: x\n---\n# Other\n");
        let first = pending(&root, &["src/a.rs"]);
        let second = Pending {
            files: vec!["src/b.rs".to_owned()],
            entry: ".weaver/history/iiiiiiii-other.md".to_owned(),
            ..first.clone()
        };
        let handles: Vec<_> = [first, second]
            .into_iter()
            .map(|work| {
                let root = root.clone();
                std::thread::spawn(move || commit(&Git::system(), &root, &work))
            })
            .collect();
        let hashes: Vec<String> = handles
            .into_iter()
            .map(|handle| committed(handle.join().unwrap()))
            .collect();
        assert_ne!(hashes[0], hashes[1]);
        assert_eq!(git(&top, &["rev-list", "--count", "HEAD"]).trim(), "3");
        // Each commit holds its own files only.
        let all: Vec<String> = git(&top, &["log", "--name-only", "--format=%s", "-2"])
            .lines()
            .filter(|line| line.starts_with("src/"))
            .map(str::to_owned)
            .collect();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn a_pending_commit_in_an_answer_is_made_by_settle_and_leaves_its_outcome() {
        if !available() {
            return;
        }
        let (_dir, top, root) = repository("");
        put(&root, "src/a.rs", "a\n");
        let work = pending(&root, &["src/a.rs"]);
        let mut answer = json!({ "id": "hhhhhhhh", "commit": pending_value(&work) });
        let mut text = String::from("# History entry\n");
        settle(&Git::system(), &root, &mut answer, &mut text);
        let hash = git(&top, &["rev-parse", "HEAD"]).trim().to_owned();
        assert_eq!(answer["commit"], json!({ "status": "committed", "hash": hash }));
        assert!(text.ends_with(&format!("\nCommitted {}.\n", &hash[..7])), "{text}");

        // A refused one says why in both; an answer with no pending commit is left as it is.
        let mut refused = json!({ "commit": pending_value(&Pending { checks_failed: true, ..work }) });
        let mut text = String::from("# History entry\n");
        settle(&Git::system(), &root, &mut refused, &mut text);
        assert_eq!(
            refused["commit"],
            json!({ "status": "not-committed", "reason": "the checks of the Run failed" })
        );
        assert!(text.ends_with("\nNot committed: the checks of the Run failed.\n"));
        let mut done = json!({ "commit": { "status": "off" } });
        let mut text = String::from("# History entry\n");
        settle(&Git::system(), &root, &mut done, &mut text);
        assert_eq!((done["commit"]["status"].as_str(), text.as_str()), (Some("off"), "# History entry\n"));
    }
}
