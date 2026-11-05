//! The Machine `solo-merge` (feature `project-setup`): a merge of another branch into the current
//! branch of this working copy, which Weaver runs with git while Solo mode is on, so that the work
//! of the others reaches the Model through a scan.
//!
//! This is the one module that runs every git step of Merge and reads and writes
//! `.weaver/merge/merge.json` (`requirements/rules#solo-merge`, `requirements/tech#solo-merge`,
//! `requirements/data#solo-merge`). Only the Tauri backend calls it: the interface never runs git
//! and the command line has no merge command. It never commits a merge with conflicts, never
//! stages a file, never runs `git fetch` and never leaves `merge.json` behind a merge that ended.
//!
//! Git is started through [`Git`] (the runner of Solo mode): no shell, the project root as working
//! folder, `GIT_OPTIONAL_LOCKS=0` for the calls that only read, `GIT_EDITOR=true` and
//! `GIT_MERGE_AUTOEDIT=no` for the others, 10 s per call and 2 minutes for `git merge`. Every
//! function here blocks on git: the caller runs it off the UI and IPC threads, in the project's
//! mutation queue.

use std::collections::BTreeSet;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths;
use crate::process::timed::{Failure, Output};
use crate::project::clock::{now_instant, SystemClock};
use crate::project::diagnostic::Diagnostic;
use crate::project::layout::{ensure_local_folder, weaver_dir, MERGE_DIR};
use crate::project::text;
use crate::solo_mode::{self, git_failure, reason_of, Call, Git, Mode, Reason};

/// The state file inside `.weaver/merge/`.
pub const FILE: &str = "merge.json";

/// What `git merge` leaves in the working copy while it waits for the developer.
const MERGE_HEAD: &str = "MERGE_HEAD";

/// `<root>/.weaver/merge/merge.json`.
pub fn record_path(root: &Path) -> PathBuf {
    weaver_dir(root).join(MERGE_DIR).join(FILE)
}

/// Whether Weaver's merge is in progress: `merge.json` exists. No git is called.
pub fn in_progress(root: &Path) -> bool {
    record_path(root).exists()
}

// --- merge.json ---------------------------------------------------------------------------------

/// The merge in progress as `merge.json` holds it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    /// The branch the developer chose, as shown.
    pub branch: String,
    /// The short name of the branch the merge goes into.
    pub current: String,
    /// The full hash of the commit the current branch started from.
    pub start: String,
    /// The full hash of the incoming commit.
    pub incoming: String,
    /// The instant the file was written.
    pub started: String,
}

/// What `merge.json` says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stored {
    /// No file: no merge in progress.
    Missing,
    Found(Record),
    /// A file that cannot be read or lacks a field: a merge in progress that Weaver cannot follow.
    Unreadable {
        /// The branch, when the file still says it.
        branch: Option<String>,
        diagnostic: Diagnostic,
    },
}

fn diagnostic_path() -> String {
    format!("{MERGE_DIR}/{FILE}")
}

/// Reads `merge.json` of the working copy at `root`.
pub fn read(root: &Path) -> Stored {
    let unreadable = |branch: Option<String>, reason: String| Stored::Unreadable {
        branch,
        diagnostic: Diagnostic::error(diagnostic_path(), reason),
    };
    let text = match text::read_text(&record_path(root)) {
        Ok(text) => text,
        Err(text::ReadError::Io(e))
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Stored::Missing
        }
        Err(e) => return unreadable(None, format!("cannot read the file: {e}")),
    };
    match serde_json::from_str::<Record>(&text) {
        Ok(record) => Stored::Found(record),
        Err(e) => {
            let branch = serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|value| value.get("branch")?.as_str().map(str::to_owned));
            unreadable(branch, format!("cannot read the file: {e}"))
        }
    }
}

fn write(root: &Path, record: &Record) -> Result<(), Reason> {
    let path = record_path(root);
    let content = serde_json::to_string_pretty(record).expect("a record is JSON");
    ensure_local_folder(root, MERGE_DIR)
        .and_then(|_| text::write_text(&path, &content))
        .map_err(|e| Reason::write_failed(&path, &e))
}

/// Deletes `merge.json`; the folder stays with its `.gitignore`. A missing file is fine.
fn delete(root: &Path) -> Result<(), Reason> {
    let path = record_path(root);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Reason::write_failed(&path, &e)),
    }
}

// --- git ----------------------------------------------------------------------------------------

/// The answer of a call whose exit code 1 is an answer, not a failure.
enum Probe {
    Yes(String),
    No,
}

fn probe(git: &Git, folder: &Path, args: &[&str], repository_check: bool) -> Result<Probe, Reason> {
    match git.exec(folder, args, Call::Read) {
        Ok(output) => Ok(Probe::Yes(output.stdout)),
        Err(Failure::Exit { code: Some(1), .. }) => Ok(Probe::No),
        Err(failure) => Err(reason_of(failure, repository_check)),
    }
}

fn read_call(git: &Git, folder: &Path, args: &[&str]) -> Result<Output, Reason> {
    git.exec(folder, args, Call::Read)
        .map_err(|failure| reason_of(failure, false))
}

fn canonical(root: &Path) -> Result<PathBuf, Reason> {
    paths::canonical_root(root).map_err(|e| Reason::Root(e.to_string()))
}

/// `git rev-parse -q --verify <name>`: whether `name` resolves.
fn resolves(git: &Git, folder: &Path, name: &str, repository_check: bool) -> Result<bool, Reason> {
    Ok(matches!(
        probe(
            git,
            folder,
            &["rev-parse", "-q", "--verify", name],
            repository_check
        )?,
        Probe::Yes(_)
    ))
}

/// The short name of the current branch, `None` when HEAD is detached.
fn current_branch(git: &Git, folder: &Path) -> Result<Option<String>, Reason> {
    match probe(git, folder, &["symbolic-ref", "-q", "HEAD"], false)? {
        Probe::Yes(name) => {
            let name = name.trim();
            Ok(Some(
                name.strip_prefix("refs/heads/").unwrap_or(name).to_owned(),
            ))
        }
        Probe::No => Ok(None),
    }
}

fn is_ancestor(git: &Git, folder: &Path, ancestor: &str, of: &str) -> Result<bool, Reason> {
    Ok(matches!(
        probe(
            git,
            folder,
            &["merge-base", "--is-ancestor", ancestor, of],
            false
        )?,
        Probe::Yes(_)
    ))
}

fn rev_parse(git: &Git, folder: &Path, name: &str) -> Result<String, Reason> {
    Ok(read_call(git, folder, &["rev-parse", name])?
        .stdout
        .trim()
        .to_owned())
}

/// The paths with conflicts: `git diff --name-only -z --diff-filter=U`.
pub fn conflicts(git: &Git, root: &Path) -> Result<Vec<String>, Reason> {
    let folder = canonical(root)?;
    conflicts_in(git, &folder)
}

fn conflicts_in(git: &Git, folder: &Path) -> Result<Vec<String>, Reason> {
    let output = read_call(
        git,
        folder,
        &["diff", "--name-only", "-z", "--diff-filter=U"],
    )?;
    Ok(names(&output.stdout).into_iter().collect())
}

/// The names in a NUL-separated list, sorted and without repeats.
fn names(list: &str) -> BTreeSet<String> {
    list.split('\0')
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The files the merge brought: the ones that differ between the commit the current branch started
/// from and the merge's own commit, relative to the project root, with `/`, sorted
/// (`git diff --name-only -z --no-renames --relative <start> <merge commit>`).
pub fn merged_files(
    git: &Git,
    root: &Path,
    start: &str,
    merge_commit: &str,
) -> Result<Vec<String>, Reason> {
    let folder = canonical(root)?;
    merged_files_in(git, &folder, start, merge_commit)
}

fn merged_files_in(
    git: &Git,
    folder: &Path,
    start: &str,
    merge_commit: &str,
) -> Result<Vec<String>, Reason> {
    let output = read_call(
        git,
        folder,
        &[
            "diff",
            "--name-only",
            "-z",
            "--no-renames",
            "--relative",
            start,
            merge_commit,
            "--",
        ],
    )?;
    Ok(names(&output.stdout).into_iter().collect())
}

// --- availability -------------------------------------------------------------------------------

/// Why Merge is not available, in the order of `requirements/rules#solo-merge`. `Display` is the
/// sentence the Board shows (`requirements/ui#solo-merge`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unavailable {
    /// A job of the project is Queued, Running, Waiting for the developer or Suspended.
    AgentWorking,
    /// Weaver's merge is in progress.
    MergeInProgress,
    /// A git call failed or exceeded its limit.
    Git(Reason),
    /// `MERGE_HEAD`, `CHERRY_PICK_HEAD` or `REBASE_HEAD` resolves, or a rebase folder exists.
    GitBusy,
    /// HEAD is detached.
    Detached,
    /// A file git tracks has uncommitted changes, Weaver's files excepted.
    Uncommitted,
}

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Unavailable::AgentWorking => f.write_str("The agent is working"),
            Unavailable::MergeInProgress => f.write_str("A merge is in progress"),
            Unavailable::Git(reason) => write!(f, "Merge needs git: {reason}"),
            Unavailable::GitBusy => {
                f.write_str("Git is in the middle of a merge, rebase or cherry-pick")
            }
            Unavailable::Detached => f.write_str("Not on a branch"),
            Unavailable::Uncommitted => f.write_str("Commit or stash your changes first"),
        }
    }
}

/// Whether Merge is available and, when it is, the branch it would merge into.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Availability {
    /// The first reason that holds; `None` when Merge is available.
    pub unavailable: Option<Unavailable>,
    /// The current branch, known whenever the checks got as far as HEAD.
    pub current: Option<String>,
}

/// Works out whether Merge is available (`requirements/rules#solo-merge`, **Availability**). The
/// first reason that holds is the one returned. `agent_busy` says whether a job of the project is
/// Queued, Running, Waiting for the developer or Suspended.
pub fn availability(git: &Git, root: &Path, agent_busy: bool) -> Availability {
    let mut answer = Availability {
        unavailable: None,
        current: None,
    };
    answer.unavailable = if agent_busy {
        Some(Unavailable::AgentWorking)
    } else if in_progress(root) {
        Some(Unavailable::MergeInProgress)
    } else {
        match git_conditions(git, root, &mut answer.current) {
            Ok(unavailable) => unavailable,
            Err(reason) => Some(Unavailable::Git(reason)),
        }
    };
    answer
}

/// The git conditions 3 to 6 of the availability, in order.
fn git_conditions(
    git: &Git,
    root: &Path,
    current: &mut Option<String>,
) -> Result<Option<Unavailable>, Reason> {
    let folder = canonical(root)?;
    for (index, name) in [MERGE_HEAD, "CHERRY_PICK_HEAD", "REBASE_HEAD"]
        .into_iter()
        .enumerate()
    {
        if resolves(git, &folder, name, index == 0)? {
            return Ok(Some(Unavailable::GitBusy));
        }
    }
    let answer = read_call(
        git,
        &folder,
        &[
            "rev-parse",
            "--git-path",
            "rebase-merge",
            "--git-path",
            "rebase-apply",
        ],
    )?;
    for line in answer.stdout.lines().filter(|line| !line.is_empty()) {
        if folder.join(line.trim_end_matches('\r')).is_dir() {
            return Ok(Some(Unavailable::GitBusy));
        }
    }
    let Some(branch) = current_branch(git, &folder)? else {
        return Ok(Some(Unavailable::Detached));
    };
    *current = Some(branch);
    if uncommitted(git, root, &folder)? {
        return Ok(Some(Unavailable::Uncommitted));
    }
    Ok(None)
}

/// Whether `git status --porcelain=v1 -z --untracked-files=no` lists a path that no line of this
/// working copy's Solo mode block matches.
fn uncommitted(git: &Git, root: &Path, folder: &Path) -> Result<bool, Reason> {
    let output = read_call(
        git,
        folder,
        &["status", "--porcelain=v1", "-z", "--untracked-files=no"],
    )?;
    let dirty = status_paths(&output.stdout);
    if dirty.is_empty() {
        return Ok(false);
    }
    let mode = solo_mode::read_state(root).mode;
    if !mode.is_on() {
        return Ok(true);
    }
    let location = solo_mode::locate(git, root)?;
    let covered: BTreeSet<String> = location.tracked_files(git, mode)?.into_iter().collect();
    Ok(dirty.iter().any(|path| {
        // A path outside the project folder is never one of Weaver's files.
        match path.strip_prefix(location.prefix.as_str()) {
            Some(relative) => !covered.contains(relative),
            None => true,
        }
    }))
}

/// The paths of a `git status --porcelain=v1 -z` listing, relative to the repository's root; a
/// renamed or copied file counts with both its paths.
fn status_paths(listing: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut entries = listing.split('\0');
    while let Some(entry) = entries.next() {
        let (Some(status), Some(path)) = (entry.get(..2), entry.get(3..)) else {
            continue;
        };
        paths.push(path.to_owned());
        if status.contains('R') || status.contains('C') {
            if let Some(origin) = entries.next().filter(|origin| !origin.is_empty()) {
                paths.push(origin.to_owned());
            }
        }
    }
    paths
}

// --- branches -----------------------------------------------------------------------------------

/// What the Dialog of Merge offers (`requirements/rules#solo-merge`, **Branches** and **Reminder**).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Branches {
    /// The current branch, `None` when HEAD is detached.
    pub current: Option<String>,
    /// The local branches but the current one, sorted by name.
    pub local: Vec<String>,
    /// The remote branches git already knows, sorted by name, without any `<remote>/HEAD`.
    pub remote: Vec<String>,
    /// Weaver's files git still tracks: their changes go into the merge commit.
    pub tracked: Vec<String>,
}

/// The branches to merge from, and Weaver's files git still tracks. Never runs `git fetch`.
pub fn branches(git: &Git, root: &Path) -> Result<Branches, Reason> {
    let folder = canonical(root)?;
    let current = current_branch(git, &folder)?;
    let listing = read_call(
        git,
        &folder,
        &[
            "for-each-ref",
            "--format=%(refname)",
            "refs/heads",
            "refs/remotes",
        ],
    )?;
    let mut local = BTreeSet::new();
    let mut remote = BTreeSet::new();
    for name in listing.stdout.lines().map(str::trim) {
        if let Some(short) = name.strip_prefix("refs/heads/") {
            if current.as_deref() != Some(short) {
                local.insert(short.to_owned());
            }
        } else if let Some(short) = name.strip_prefix("refs/remotes/") {
            if !short.ends_with("/HEAD") {
                remote.insert(short.to_owned());
            }
        }
    }
    let tracked = solo_mode::tracked_files(git, root)?;
    Ok(Branches {
        current,
        local: local.into_iter().collect(),
        remote: remote.into_iter().collect(),
        tracked,
    })
}

// --- the merge in progress ----------------------------------------------------------------------

/// A merge with conflicts that Weaver follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InProgress {
    pub branch: String,
    pub current: String,
    /// How many files still have conflicts; `None` when git could not say just now.
    pub conflicts: Option<usize>,
}

impl InProgress {
    fn of(record: &Record, conflicts: Option<usize>) -> Self {
        Self {
            branch: record.branch.clone(),
            current: record.current.clone(),
            conflicts,
        }
    }
}

/// A merge that ended in a commit: `merge.json` is deleted and the scan can start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Concluded {
    pub branch: String,
    /// The files the merge brought (`requirements/rules#solo-merge`, **Merged files**).
    pub files: Vec<String>,
}

/// The outcome of the check of a merge in progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Check {
    /// No `merge.json`: nothing to check.
    NoMerge,
    /// `MERGE_HEAD` resolves: still merging.
    Merging(InProgress),
    /// Git shows the merge committed; `merge.json` is deleted.
    Concluded(Concluded),
    /// Aborted outside Weaver, another branch, a reset or a file Weaver cannot follow: forgotten
    /// without a scan; `merge.json` is deleted.
    Forgotten {
        branch: Option<String>,
        /// Set when the file could not be read: a diagnostic with its path and reason.
        diagnostic: Option<Diagnostic>,
    },
    /// A git call failed: the merge stays in progress.
    Failed {
        progress: InProgress,
        reason: Reason,
    },
}

/// The check of a merge in progress (`requirements/rules#solo-merge`, **Check of a merge in
/// progress**), while `merge.json` exists. `merge.json` is deleted at every end; a git call that
/// fails leaves the merge in progress.
pub fn check(git: &Git, root: &Path) -> Check {
    let record = match read(root) {
        Stored::Missing => return Check::NoMerge,
        Stored::Found(record) => record,
        Stored::Unreadable { branch, diagnostic } => {
            let diagnostic = Some(diagnostic);
            return match delete(root) {
                Ok(()) => Check::Forgotten { branch, diagnostic },
                Err(reason) => Check::Failed {
                    progress: InProgress {
                        branch: branch.unwrap_or_default(),
                        current: String::new(),
                        conflicts: None,
                    },
                    reason,
                },
            };
        }
    };
    match checked(git, root, &record) {
        Ok(outcome) => outcome,
        Err(reason) => Check::Failed {
            progress: InProgress::of(&record, None),
            reason,
        },
    }
}

fn checked(git: &Git, root: &Path, record: &Record) -> Result<Check, Reason> {
    let folder = canonical(root)?;
    if resolves(git, &folder, MERGE_HEAD, false)? {
        let count = conflicts_in(git, &folder)?.len();
        return Ok(Check::Merging(InProgress::of(record, Some(count))));
    }
    let forgotten = || Check::Forgotten {
        branch: Some(record.branch.clone()),
        diagnostic: None,
    };
    if current_branch(git, &folder)?.as_deref() != Some(record.current.as_str()) {
        delete(root)?;
        return Ok(forgotten());
    }
    let head = rev_parse(git, &folder, "HEAD")?;
    let committed = head != record.start
        && is_ancestor(git, &folder, &record.start, "HEAD")?
        && is_ancestor(git, &folder, &record.incoming, "HEAD")?;
    if !committed {
        delete(root)?;
        return Ok(forgotten());
    }
    let range = format!("{}..HEAD", record.start);
    let listing = read_call(
        git,
        &folder,
        &["rev-list", "--first-parent", "--ancestry-path", &range],
    )?;
    let Some(merge_commit) = listing
        .stdout
        .lines()
        .map(str::trim)
        .rfind(|c| !c.is_empty())
    else {
        delete(root)?;
        return Ok(forgotten());
    };
    let concluded = conclude(git, root, &folder, record, merge_commit)?;
    Ok(Check::Concluded(concluded))
}

/// The merge ended in `merge_commit`: its files, then `merge.json` goes. A failure leaves the file
/// (the next check finds the merge committed again).
fn conclude(
    git: &Git,
    root: &Path,
    folder: &Path,
    record: &Record,
    merge_commit: &str,
) -> Result<Concluded, Reason> {
    let files = merged_files_in(git, folder, &record.start, merge_commit)?;
    delete(root)?;
    Ok(Concluded {
        branch: record.branch.clone(),
        files,
    })
}

// --- start --------------------------------------------------------------------------------------

/// Why a merge was refused; nothing changed and `merge.json` is not left behind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// A condition of the availability holds.
    Unavailable(Unavailable),
    /// Git refused or failed: the first line of its error output.
    Git(String),
    /// The merge would overwrite these files, which Solo mode keeps out of git (relative to the
    /// project root, with `/`).
    KeptOutOfGit(Vec<String>),
    /// `git merge` took longer than 2 minutes: git was killed and the merge aborted.
    TimedOut,
}

/// What a start did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Started {
    /// The branch brings nothing new: nothing was started.
    UpToDate {
        current: String,
        branch: String,
    },
    /// Git merged without conflicts: concluded at once, `merge.json` deleted.
    Concluded(Concluded),
    /// Git stopped on conflicts: Weaver remembers the merge.
    Merging(InProgress),
    Refused(Refusal),
}

/// `developer.MERGE_BRANCH` (`requirements/rules#solo-merge`, **Start**). The caller holds the
/// project's mutation queue and says whether a job of the project has not ended.
pub fn start(git: &Git, root: &Path, branch: &str, agent_busy: bool) -> Started {
    let availability = availability(git, root, agent_busy);
    if let Some(unavailable) = availability.unavailable {
        return Started::Refused(Refusal::Unavailable(unavailable));
    }
    let Some(current) = availability.current else {
        return Started::Refused(Refusal::Unavailable(Unavailable::Detached));
    };
    if branch.is_empty() || branch.starts_with('-') {
        return Started::Refused(Refusal::Git(format!("`{branch}` is not a branch")));
    }
    let refused = |reason: Reason| Started::Refused(Refusal::Git(first_line(&reason)));
    let folder = match canonical(root) {
        Ok(folder) => folder,
        Err(reason) => return refused(reason),
    };
    let start = match rev_parse(git, &folder, "HEAD") {
        Ok(start) => start,
        Err(reason) => return refused(reason),
    };
    let incoming = match rev_parse(git, &folder, &format!("{branch}^{{commit}}")) {
        Ok(incoming) => incoming,
        Err(reason) => return refused(reason),
    };
    match is_ancestor(git, &folder, &incoming, &start) {
        Ok(true) => {
            return Started::UpToDate {
                current,
                branch: branch.to_owned(),
            }
        }
        Ok(false) => {}
        Err(reason) => return refused(reason),
    }
    // git's own `--no-overwrite-ignore` protects the ignored files only on a fast-forward: a merge
    // that makes a commit overwrites them silently, so what the incoming branch brings is checked
    // against the files Solo mode keeps out of git first.
    match would_overwrite(git, root, &folder, &start, &incoming) {
        Ok(paths) if !paths.is_empty() => {
            return Started::Refused(Refusal::KeptOutOfGit(paths));
        }
        Ok(_) => {}
        Err(reason) => return refused(reason),
    }
    let started = match now_instant(&SystemClock) {
        Ok(instant) => instant,
        Err(e) => return Started::Refused(Refusal::Git(e.to_string())),
    };
    let record = Record {
        branch: branch.to_owned(),
        current,
        start,
        incoming,
        started,
    };
    // Written before git runs: a merge interrupted by a crash is found at the next opening.
    if let Err(reason) = write(root, &record) {
        return refused(reason);
    }
    merged(git, root, &folder, &record)
}

/// The files the incoming commit adds or changes, against what the two branches share, that exist in
/// the working copy as files Solo mode keeps out of git: the merge would overwrite them. Relative to
/// the project root, with `/`, sorted. Empty without Solo mode, or when nothing comes in that way.
fn would_overwrite(
    git: &Git,
    root: &Path,
    folder: &Path,
    start: &str,
    incoming: &str,
) -> Result<Vec<String>, Reason> {
    let mode = solo_mode::read_state(root).mode;
    if !mode.is_on() {
        return Ok(Vec::new());
    }
    let base = match probe(git, folder, &["merge-base", start, incoming], false)? {
        Probe::Yes(base) => base.trim().to_owned(),
        // Unrelated histories: git refuses them by itself.
        Probe::No => return Ok(Vec::new()),
    };
    let brought = read_call(
        git,
        folder,
        &[
            "diff",
            "--name-only",
            "-z",
            "--no-renames",
            "--relative",
            "--diff-filter=AMT",
            &base,
            incoming,
            "--",
        ],
    )?;
    let brought = names(&brought.stdout);
    if brought.is_empty() {
        return Ok(Vec::new());
    }
    let location = solo_mode::locate(git, root)?;
    let kept = location.kept_out_files(git, mode)?;
    Ok(kept
        .into_iter()
        .filter(|path| brought.contains(path))
        .collect())
}

/// Runs `git merge` and says what came of it; `merge.json` is already written.
fn merged(git: &Git, root: &Path, folder: &Path, record: &Record) -> Started {
    let outcome = git.exec(
        folder,
        &[
            "merge",
            "--no-edit",
            "--no-overwrite-ignore",
            &record.branch,
        ],
        Call::Merge,
    );
    match outcome {
        Ok(_) => match rev_parse(git, folder, "HEAD")
            .and_then(|head| conclude(git, root, folder, record, &head))
        {
            Ok(concluded) => Started::Concluded(concluded),
            // The merge is done; the check that follows every merge action finds it committed.
            Err(_) => Started::Merging(InProgress::of(record, None)),
        },
        Err(Failure::Exit { stderr, .. }) => match resolves(git, folder, MERGE_HEAD, false) {
            Ok(true) => {
                let count = conflicts_in(git, folder).ok().map(|list| list.len());
                Started::Merging(InProgress::of(record, count))
            }
            Ok(false) => refused_by_git(git, root, &stderr),
            // Git does not answer: the merge stays, and the check that follows decides.
            Err(_) => Started::Merging(InProgress::of(record, None)),
        },
        Err(Failure::TimedOut { .. }) => {
            // Known limitation, as the Model words it (`requirements/rules#solo-merge`): the merge is
            // aborted only when `MERGE_HEAD` resolves. A git killed earlier, for instance while a
            // `pre-merge-commit` hook runs, has written no `MERGE_HEAD` but may have left merged files
            // staged; `git merge --abort` has nothing to undo then, so neither this path, the Board's
            // abort nor the "forgotten" check cleans it up, and the developer does it in git.
            if resolves(git, folder, MERGE_HEAD, false).unwrap_or(false) {
                let _ = git.exec(folder, &["merge", "--abort"], Call::Write);
            }
            end_refused(root, Refusal::TimedOut)
        }
        Err(Failure::NotStarted) => end_refused(root, Refusal::Git(first_line(&Reason::NoGit))),
    }
}

/// `merge.json` goes with a refused merge.
fn end_refused(root: &Path, refusal: Refusal) -> Started {
    let _ = delete(root);
    Started::Refused(refusal)
}

/// Git refused the merge before any conflict: the reason is the ignored files with their paths when
/// git's error lists untracked files that would be overwritten and the Solo mode block covers them,
/// otherwise the first line of git's error output.
fn refused_by_git(git: &Git, root: &Path, stderr: &str) -> Started {
    let listed = overwritten_untracked(stderr);
    if !listed.is_empty() {
        if let Some(kept) = kept_out(git, root, &listed) {
            return end_refused(root, Refusal::KeptOutOfGit(kept));
        }
    }
    let Reason::GitFailed(line) = git_failure(None, stderr) else {
        unreachable!("git_failure answers GitFailed")
    };
    end_refused(root, Refusal::Git(line))
}

/// The paths of the untracked files that git's error says the merge would overwrite.
fn overwritten_untracked(stderr: &str) -> Vec<String> {
    let mut paths = Vec::new();
    let mut lines = stderr.lines().peekable();
    while let Some(line) = lines.next() {
        if line.contains("untracked working tree files would be overwritten") {
            while let Some(path) = lines.next_if(|next| next.starts_with('\t')) {
                paths.push(path.trim().to_owned());
            }
        } else if let Some(rest) = line.strip_prefix("error: Untracked working tree file '") {
            if let Some(path) = rest.strip_suffix("' would be overwritten by merge.") {
                paths.push(path.to_owned());
            }
        }
    }
    paths
}

/// Of `listed`, the paths the working copy's Solo mode block keeps out of git, sorted; `None` when
/// there is none (or the block cannot be worked out: the plain refusal says it).
fn kept_out(git: &Git, root: &Path, listed: &[String]) -> Option<Vec<String>> {
    let mode = solo_mode::read_state(root).mode;
    if mode == Mode::Off {
        return None;
    }
    let location = solo_mode::locate(git, root).ok()?;
    let kept: BTreeSet<String> = location
        .kept_out_files(git, mode)
        .ok()?
        .into_iter()
        .collect();
    let matched: BTreeSet<String> = listed
        .iter()
        .map(|path| path.strip_prefix(location.prefix.as_str()).unwrap_or(path))
        .filter(|path| kept.contains(*path))
        .map(str::to_owned)
        .collect();
    (!matched.is_empty()).then(|| matched.into_iter().collect())
}

/// The first line of git's error output for `reason`, or its whole text.
fn first_line(reason: &Reason) -> String {
    match reason {
        Reason::GitFailed(line) => line.clone(),
        other => other.to_string(),
    }
}

// --- abort --------------------------------------------------------------------------------------

/// What `developer.ABORT_MERGE` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Aborted {
    /// `git merge --abort` succeeded: the merge is forgotten and no scan starts.
    Done,
    /// There is no merge in progress to abort.
    NoMerge,
    /// The job `merge` of the project has not ended: refused.
    JobNotEnded,
    /// Git failed: the merge stays in progress. The first line of git's error output.
    Failed(String),
}

/// `developer.ABORT_MERGE` (`requirements/rules#solo-merge`, **Abort**), after the confirmation of
/// the interface. `merge_job_unended` says whether the job `merge` of the project has not ended.
pub fn abort(git: &Git, root: &Path, merge_job_unended: bool) -> Aborted {
    if !in_progress(root) {
        return Aborted::NoMerge;
    }
    if merge_job_unended {
        return Aborted::JobNotEnded;
    }
    let folder = match canonical(root) {
        Ok(folder) => folder,
        Err(reason) => return Aborted::Failed(first_line(&reason)),
    };
    match git.exec(&folder, &["merge", "--abort"], Call::Write) {
        Ok(_) => match delete(root) {
            Ok(()) => Aborted::Done,
            Err(reason) => Aborted::Failed(first_line(&reason)),
        },
        Err(Failure::Exit { code, stderr }) => {
            Aborted::Failed(first_line(&git_failure(code, &stderr)))
        }
        Err(failure) => Aborted::Failed(first_line(&reason_of(failure, false))),
    }
}

// --- the whole state ----------------------------------------------------------------------------

/// How the end of a merge was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ended {
    Concluded(Concluded),
    Forgotten {
        branch: Option<String>,
        diagnostic: Option<Diagnostic>,
    },
}

/// What a recomputation of Merge's state found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refreshed {
    /// The first reason Merge is not available; `None` when it is.
    pub unavailable: Option<Unavailable>,
    /// The current branch, known whenever the checks got as far as HEAD.
    pub current: Option<String>,
    /// The merge with conflicts that Weaver follows.
    pub in_progress: Option<InProgress>,
    /// The merge that ended in this recomputation: the caller starts the scan for a concluded one.
    pub ended: Option<Ended>,
    /// A git call of the check that failed: the merge is still in progress.
    pub failure: Option<Reason>,
}

/// The check of a merge in progress, then the availability: when the Board is shown, when the
/// window comes back to the foreground, when a job ends and after each merge action. `agent_busy`
/// says whether a job of the project has not ended.
pub fn refresh(git: &Git, root: &Path, agent_busy: bool) -> Refreshed {
    let mut refreshed = Refreshed {
        unavailable: None,
        current: None,
        in_progress: None,
        ended: None,
        failure: None,
    };
    match check(git, root) {
        Check::NoMerge => {}
        Check::Merging(progress) => refreshed.in_progress = Some(progress),
        Check::Concluded(concluded) => refreshed.ended = Some(Ended::Concluded(concluded)),
        Check::Forgotten { branch, diagnostic } => {
            refreshed.ended = Some(Ended::Forgotten { branch, diagnostic })
        }
        Check::Failed { progress, reason } => {
            refreshed.in_progress = Some(progress);
            refreshed.failure = Some(reason);
        }
    }
    let availability = availability(git, root, agent_busy);
    refreshed.unavailable = availability.unavailable;
    refreshed.current = availability.current;
    refreshed
}

#[cfg(test)]
mod tests;
