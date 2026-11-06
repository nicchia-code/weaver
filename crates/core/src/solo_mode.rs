//! The Machine `solo-mode` (feature `project-setup`): whether Weaver keeps its files of one working
//! copy out of git, through a block of its own in the repository's local exclusions
//! (`git rev-parse --git-path info/exclude`), which git never commits.
//!
//! This is the one module that reads the state (`.weaver/solo/solo.json`), finds the exclusions file
//! and the prefix, writes and removes the block and lists the tracked files
//! (`requirements/rules#solo-mode`, `requirements/tech#solo-mode`). The command line and the app both
//! call it; the interface never runs git.
//!
//! Git is started through [`crate::process::command`] and the time-limited runner shared with omp
//! ([`crate::process::timed`]), with `GIT_OPTIONAL_LOCKS=0`, the project root as working folder, and
//! only two kinds of calls of its own: `git rev-parse --show-prefix --git-path info/exclude` (once
//! per operation) and `git ls-files -z --cached --ignored --exclude=<line>` for each line of the
//! block. [`Git`] is also the runner of Merge (`crate::solo_merge`), which has its own calls, the
//! longer limit of `git merge` and no `GIT_OPTIONAL_LOCKS` for the calls that write.

use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io;
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::atomic;
use crate::paths;
use crate::process;
use crate::process::timed::{self, Failure};
use crate::project::diagnostic::Diagnostic;
use crate::project::layout::{ensure_local_folder, weaver_dir, SOLO_DIR};
use crate::project::text;
use crate::project_files::has_weaver;

/// The state file inside `.weaver/solo/`.
pub const STATE_FILE: &str = "solo.json";

/// How long a git call may take before it is killed.
pub const GIT_LIMIT: Duration = Duration::from_secs(10);

/// How long `git merge` may take before it is killed (`requirements/rules#solo-merge`).
pub const MERGE_LIMIT: Duration = Duration::from_secs(120);

/// How long `git commit` may take before it is killed, because of the hooks
/// (`requirements/rules#atomic-commits`).
pub const COMMIT_LIMIT: Duration = Duration::from_secs(120);

/// The modes: the same three values in the app, the command line and `solo.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(rename = "SoloMode")]
pub enum Mode {
    /// Weaver keeps none of its files out of git.
    #[default]
    Off,
    /// The whole folders `.weaver/`, `.agents/skills/` and `.omp/` (the default when turning on).
    Folders,
    /// `.weaver/` and only the files Weaver installs in the folders of the agent.
    Files,
}

impl Mode {
    /// The three values, as written in the command line and in `solo.json`.
    pub const NAMES: [&'static str; 3] = ["off", "folders", "files"];

    pub fn parse(text: &str) -> Option<Mode> {
        match text {
            "off" => Some(Mode::Off),
            "folders" => Some(Mode::Folders),
            "files" => Some(Mode::Files),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::Folders => "folders",
            Mode::Files => "files",
        }
    }

    pub fn is_on(self) -> bool {
        self != Mode::Off
    }

    /// The lines of the block for a project whose prefix is `prefix` (empty, or the project folder
    /// relative to the repository ending with `/`; escaped here).
    pub fn lines(self, prefix: &str) -> Vec<String> {
        let p = escape_prefix(prefix);
        match self {
            Mode::Off => Vec::new(),
            Mode::Folders => vec![
                format!("/{p}.weaver/"),
                format!("/{p}.agents/skills/"),
                format!("/{p}.omp/"),
            ],
            Mode::Files => vec![
                format!("/{p}.weaver/"),
                format!("/{p}.agents/skills/weaver-*/"),
                format!("/{p}.omp/agents/weaver-*.md"),
                format!("/{p}.omp/WATCHDOG.yml"),
            ],
        }
    }
}

/// Escapes with `\` the characters of `prefix` that are special to git patterns: `*`, `?`, `[`, `\`,
/// a leading `#` or `!`, and trailing spaces.
pub fn escape_prefix(prefix: &str) -> String {
    let mut escaped = String::with_capacity(prefix.len());
    let trailing_from = prefix.trim_end_matches(' ').len();
    for (index, c) in prefix.char_indices() {
        let special = matches!(c, '*' | '?' | '[' | '\\')
            || (index == 0 && matches!(c, '#' | '!'))
            || (index >= trailing_from && c == ' ');
        if special {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

/// Why Solo mode cannot be changed, or why a step of it failed: the `<reason>` of the texts the
/// interface and the command line show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reason {
    /// `git` could not be started.
    NoGit,
    /// `git rev-parse` answered with exit code 128.
    NotARepository,
    /// A git call took longer than its limit and was killed.
    TimedOut(Duration),
    /// Git failed: the first line of its error output, or its exit code.
    GitFailed(String),
    /// The exclusions file, or a file of the state, could not be written.
    CouldNotWrite { path: String, error: String },
    /// The project folder cannot be resolved.
    Root(String),
    /// The folder is not a project: Solo mode keeps its state in `.weaver/`.
    NotAProject,
    /// The block is not in the exclusions file as the mode expects it.
    BlockMissing { path: String },
    /// A merge of Weaver is in progress (`merge/merge.json` exists): Solo mode cannot change
    /// (`requirements/tech#solo-merge`, **Solo mode**).
    MergeInProgress,
}

impl Reason {
    /// Whether this reason makes Solo mode unavailable (git cannot do its part), as opposed to a
    /// failed write.
    pub fn is_unavailable(&self) -> bool {
        matches!(
            self,
            Reason::NoGit
                | Reason::NotARepository
                | Reason::TimedOut(_)
                | Reason::GitFailed(_)
                | Reason::Root(_)
        )
    }

    pub(crate) fn write_failed(path: &Path, error: &io::Error) -> Reason {
        Reason::CouldNotWrite {
            path: path.display().to_string(),
            error: error.to_string(),
        }
    }
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reason::NoGit => f.write_str("git was not found"),
            Reason::NotARepository => f.write_str("this folder is not in a git repository"),
            Reason::TimedOut(limit) => write!(f, "git did not answer within {} s", limit.as_secs()),
            Reason::GitFailed(line) => write!(f, "git failed: {line}"),
            Reason::CouldNotWrite { path, error } => write!(f, "could not write {path}: {error}"),
            Reason::Root(error) => write!(f, "the project folder cannot be read: {error}"),
            Reason::NotAProject => {
                f.write_str("the folder is not a project yet: initialise it first")
            }
            Reason::BlockMissing { path } => write!(
                f,
                "the block of this working copy is missing from {path} or differs from the expected one"
            ),
            Reason::MergeInProgress => f.write_str("Finish or abort the merge in progress first."),
        }
    }
}

impl std::error::Error for Reason {}

/// How a git call is started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Call {
    /// Only reads: `GIT_OPTIONAL_LOCKS=0`, the limit of every call.
    Read,
    /// Changes the working copy or the repository (`git merge --abort`): no editor may open.
    Write,
    /// `git merge`: no editor may open, and the longer limit.
    Merge,
    /// `git add` and `git rm` of Atomic commits: no editor, no prompt for a password, the limit of
    /// every call.
    Stage,
    /// `git commit` of Atomic commits: no editor, no prompt, and the longer limit of its hooks.
    Commit,
}

/// How git is started: the time limit of each call, the one of `git merge` and the one of
/// `git commit`, and optionally the `PATH` it is looked up in (the system's, unless given).
#[derive(Debug, Clone)]
pub struct Git {
    limit: Duration,
    merge_limit: Duration,
    commit_limit: Duration,
    search_path: Option<OsString>,
}

impl Git {
    /// Git on the `PATH`, with the limit of 10 s per call, 2 minutes for `git merge` and 2 minutes
    /// for `git commit`.
    pub fn system() -> Self {
        Self {
            limit: GIT_LIMIT,
            merge_limit: MERGE_LIMIT,
            commit_limit: COMMIT_LIMIT,
            search_path: None,
        }
    }

    /// The same with another limit per call (tests).
    pub fn with_limit(mut self, limit: Duration) -> Self {
        self.limit = limit;
        self
    }

    /// The same with another limit for `git merge` (tests).
    pub fn with_merge_limit(mut self, limit: Duration) -> Self {
        self.merge_limit = limit;
        self
    }

    /// The same with another limit for `git commit` (tests).
    pub fn with_commit_limit(mut self, limit: Duration) -> Self {
        self.commit_limit = limit;
        self
    }

    /// The same looking `git` up in the folders of `path` instead of the `PATH` (tests: an empty
    /// one means no git). The program is resolved here, never by the system's own lookup, which on
    /// Windows also tries the folder of the executable, the system folders and the `PATH` of this
    /// process; when no folder holds it, no process is started.
    pub fn with_search_path(mut self, path: impl Into<OsString>) -> Self {
        self.search_path = Some(path.into());
        self
    }

    fn run(&self, folder: &Path, args: &[&str]) -> Result<timed::Output, Failure> {
        self.exec(folder, args, Call::Read)
    }

    pub(crate) fn exec(
        &self,
        folder: &Path,
        args: &[&str],
        call: Call,
    ) -> Result<timed::Output, Failure> {
        self.exec_with_input(folder, args, call, None)
    }

    /// [`Git::exec`] with `input` on the standard input of git, when given.
    pub(crate) fn exec_with_input(
        &self,
        folder: &Path,
        args: &[&str],
        call: Call,
        input: Option<&[u8]>,
    ) -> Result<timed::Output, Failure> {
        let program = match &self.search_path {
            Some(path) => find_program("git", path).ok_or(Failure::NotStarted)?,
            None => PathBuf::from("git"),
        };
        let mut command = process::command(program, args);
        command.current_dir(folder);
        match call {
            Call::Read => {
                command.env("GIT_OPTIONAL_LOCKS", "0");
            }
            Call::Write | Call::Merge => {
                command
                    .env("GIT_EDITOR", "true")
                    .env("GIT_MERGE_AUTOEDIT", "no");
            }
            Call::Stage | Call::Commit => {
                command
                    .env("GIT_EDITOR", "true")
                    .env("GIT_TERMINAL_PROMPT", "0");
            }
        }
        if let Some(path) = &self.search_path {
            command.env("PATH", path);
        }
        let limit = match call {
            Call::Merge => self.merge_limit,
            Call::Commit => self.commit_limit,
            Call::Read | Call::Write | Call::Stage => self.limit,
        };
        match input {
            Some(input) => timed::run_command_with_input(command, limit, input),
            None => timed::run_command(command, limit),
        }
    }
}

/// The first file named `name` in the folders of `search_path` (empty entries are skipped). On
/// Windows the name is tried with each extension of `PATHEXT`, as the shell does.
fn find_program(name: &str, search_path: &OsStr) -> Option<PathBuf> {
    #[cfg(windows)]
    let extensions: Vec<String> = std::env::var("PATHEXT")
        .unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned())
        .split(';')
        .filter(|extension| !extension.is_empty())
        .map(str::to_owned)
        .collect();
    std::env::split_paths(search_path)
        .filter(|folder| !folder.as_os_str().is_empty())
        .find_map(|folder| {
            #[cfg(windows)]
            {
                extensions
                    .iter()
                    .map(|extension| folder.join(format!("{name}{extension}")))
                    .find(|candidate| candidate.is_file())
            }
            #[cfg(not(windows))]
            {
                Some(folder.join(name)).filter(|candidate| candidate.is_file())
            }
        })
}

/// The first line of git's error output (or its exit code).
pub(crate) fn git_failure(code: Option<i32>, stderr: &str) -> Reason {
    let line = stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| match code {
            Some(code) => format!("exit code {code}"),
            None => "it was stopped".to_owned(),
        });
    Reason::GitFailed(line)
}

pub(crate) fn reason_of(failure: Failure, rev_parse: bool) -> Reason {
    match failure {
        Failure::NotStarted => Reason::NoGit,
        Failure::TimedOut { limit } => Reason::TimedOut(limit),
        Failure::Exit {
            code: Some(128), ..
        } if rev_parse => Reason::NotARepository,
        Failure::Exit { code, stderr } => git_failure(code, &stderr),
    }
}

// --- the state ------------------------------------------------------------------------------------

/// What `solo.json` says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub mode: Mode,
    /// A file that cannot be read or holds another value counts as off and is reported here; only
    /// the next change of Solo mode rewrites it.
    pub diagnostic: Option<Diagnostic>,
}

/// `<root>/.weaver/solo/solo.json`.
pub fn state_path(root: &Path) -> PathBuf {
    weaver_dir(root).join(SOLO_DIR).join(STATE_FILE)
}

/// The content of `solo.json` for an on mode.
pub fn state_text(mode: Mode) -> String {
    format!("{{\"mode\": \"{}\"}}", mode.as_str())
}

/// Reads the state of the working copy at `root`: no file means off.
pub fn read_state(root: &Path) -> State {
    let off = |reason: String| State {
        mode: Mode::Off,
        diagnostic: Some(Diagnostic::error(
            format!("{SOLO_DIR}/{STATE_FILE}"),
            reason,
        )),
    };
    let text = match text::read_text(&state_path(root)) {
        Ok(text) => text,
        Err(text::ReadError::Io(e))
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return State {
                mode: Mode::Off,
                diagnostic: None,
            }
        }
        Err(e) => return off(format!("cannot read the file: {e}")),
    };
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(e) => return off(format!("cannot read the file: {e}")),
    };
    match value
        .get("mode")
        .and_then(serde_json::Value::as_str)
        .and_then(Mode::parse)
    {
        Some(mode) if mode.is_on() => State {
            mode,
            diagnostic: None,
        },
        _ => off("`mode` must be `folders` or `files`".to_owned()),
    }
}

fn write_state(root: &Path, mode: Mode) -> Result<(), Reason> {
    let path = state_path(root);
    ensure_local_folder(root, SOLO_DIR)
        .and_then(|_| text::write_text(&path, &state_text(mode)))
        .map_err(|e| Reason::write_failed(&path, &e))
}

/// `path` with its `..` steps taken out (the root is canonical, so no link can make this differ).
fn without_parent_steps(path: &Path) -> PathBuf {
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                clean.pop();
            }
            other => clean.push(other),
        }
    }
    clean
}

// --- the exclusions file --------------------------------------------------------------------------

/// Where the exclusions are for a working copy: the file, the prefix of the project in the
/// repository and the repository's root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    /// The exclusions file git reads for this working copy (shared by every working copy of a
    /// repository).
    pub exclude: PathBuf,
    /// Empty at the repository's root, otherwise the project folder relative to it, ending with `/`.
    pub prefix: String,
    /// The root of the repository's working tree.
    repository: PathBuf,
    /// The canonical project root with `/` separators: what marks the block of this working copy.
    marker: String,
}

/// Runs `git rev-parse --show-prefix --git-path info/exclude` in the project root.
pub fn locate(git: &Git, root: &Path) -> Result<Location, Reason> {
    let canonical = paths::canonical_root(root).map_err(|e| Reason::Root(e.to_string()))?;
    let output = git
        .run(
            &canonical,
            &["rev-parse", "--show-prefix", "--git-path", "info/exclude"],
        )
        .map_err(|failure| reason_of(failure, true))?;
    let answer = output.stdout.strip_suffix('\n').unwrap_or(&output.stdout);
    let (prefix, exclude) = answer
        .split_once('\n')
        .ok_or_else(|| Reason::GitFailed("it did not name the exclusions file".to_owned()))?;
    let exclude = Path::new(exclude.trim_end_matches('\r'));
    let exclude = if exclude.is_absolute() {
        exclude.to_path_buf()
    } else {
        without_parent_steps(&canonical.join(exclude))
    };
    let mut repository = canonical.clone();
    for _ in prefix.split('/').filter(|part| !part.is_empty()) {
        repository.pop();
    }
    Ok(Location {
        exclude,
        prefix: prefix.to_owned(),
        repository,
        marker: marker_of(&canonical),
    })
}

/// What marks the block of the working copy at `canonical_root` in the exclusions file: the
/// canonical project root with `/` separators on every system. The first line of the block is
/// `# >>> weaver solo mode: ` followed by it.
pub fn marker_of(canonical_root: &Path) -> String {
    canonical_root.to_string_lossy().replace('\\', "/")
}

fn begin_line(marker: &str) -> String {
    format!("# >>> weaver solo mode: {marker}")
}

fn end_line(marker: &str) -> String {
    format!("# <<< weaver solo mode: {marker}")
}

/// The block of one working copy for `mode`.
fn block_text(marker: &str, prefix: &str, mode: Mode) -> String {
    let mut block = begin_line(marker);
    block.push('\n');
    for line in mode.lines(prefix) {
        block.push_str(&line);
        block.push('\n');
    }
    block.push_str(&end_line(marker));
    block.push('\n');
    block
}

/// The content of the exclusions file with the block of `marker` replaced by `block`, or removed
/// when `block` is `None` (`requirements/rules#solo-mode`, "Writing" and "Removing"). Every other
/// byte stays as it was.
///
/// The block is every run from its exact begin line to the next end line with the same root: the
/// first one is replaced in place and any further one removed. A begin line with no end line after
/// it is removed as a single line when writing. Without a run, the block is appended, after an LF
/// when the content does not end with one.
fn rewrite(content: &[u8], marker: &str, block: Option<&str>) -> Vec<u8> {
    let begin = begin_line(marker);
    let end = end_line(marker);
    let lines: Vec<&[u8]> = content.split_inclusive(|byte| *byte == b'\n').collect();
    let is_begin =
        |line: &[u8]| -> bool { line.strip_suffix(b"\n").unwrap_or(line) == begin.as_bytes() };
    let is_end =
        |line: &[u8]| -> bool { line.strip_suffix(b"\n").unwrap_or(line) == end.as_bytes() };

    let mut out: Vec<u8> = Vec::with_capacity(content.len());
    let mut placed = false;
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        if is_begin(line) {
            match (index + 1..lines.len()).find(|&j| is_end(lines[j])) {
                Some(last) => {
                    if let (false, Some(block)) = (placed, block) {
                        out.extend_from_slice(block.as_bytes());
                        placed = true;
                    }
                    index = last + 1;
                    continue;
                }
                None if block.is_some() => {
                    // An orphan begin line goes, alone; removing leaves it where it is.
                    index += 1;
                    continue;
                }
                None => {}
            }
        }
        out.extend_from_slice(line);
        index += 1;
    }
    if let (false, Some(block)) = (placed, block) {
        if !out.is_empty() && !out.ends_with(b"\n") {
            out.push(b'\n');
        }
        out.extend_from_slice(block.as_bytes());
    }
    out
}

impl Location {
    fn read(&self) -> Result<Option<Vec<u8>>, Reason> {
        match fs::read(&self.exclude) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Reason::write_failed(&self.exclude, &e)),
        }
    }

    fn store(&self, bytes: &[u8]) -> Result<(), Reason> {
        atomic::write_keeping_permissions(&self.exclude, bytes)
            .map_err(|e| Reason::write_failed(&self.exclude, &e))
    }

    /// Whether the block `mode` expects is in the exclusions file, byte for byte and alone.
    pub fn block_in_place(&self, mode: Mode) -> Result<bool, Reason> {
        let expected = block_text(&self.marker, &self.prefix, mode);
        let current = self.read()?.unwrap_or_default();
        Ok(rewrite(&current, &self.marker, Some(&expected)) == current)
    }

    /// Writes the block of this working copy for `mode`, leaving every other byte of the file as
    /// it was. Returns whether the file changed: the same block again writes nothing.
    pub fn write_block(&self, mode: Mode) -> Result<bool, Reason> {
        let expected = block_text(&self.marker, &self.prefix, mode);
        let current = self.read()?;
        let before = current.as_deref().unwrap_or_default();
        let after = rewrite(before, &self.marker, Some(&expected));
        if current.is_some() && after == before {
            return Ok(false);
        }
        self.store(&after)?;
        Ok(true)
    }

    /// Removes every run of this working copy's block and nothing else; a file left empty stays,
    /// empty, and a missing file stays missing. Returns whether the file changed.
    pub fn remove_block(&self) -> Result<bool, Reason> {
        let Some(current) = self.read()? else {
            return Ok(false);
        };
        let after = rewrite(&current, &self.marker, None);
        if after == current {
            return Ok(false);
        }
        self.store(&after)?;
        Ok(true)
    }

    /// The files git tracks that the lines of `mode`'s block match, relative to the project root,
    /// with `/`, sorted. Never other ignored tracked files. Empty when `mode` is off.
    pub fn tracked_files(&self, git: &Git, mode: Mode) -> Result<Vec<String>, Reason> {
        self.block_files(git, mode, "--cached")
    }

    /// The files of the working tree that the lines of `mode`'s block match and that git does not
    /// track, relative to the project root, with `/`, sorted. Empty when `mode` is off.
    pub(crate) fn kept_out_files(&self, git: &Git, mode: Mode) -> Result<Vec<String>, Reason> {
        self.block_files(git, mode, "--others")
    }

    /// `git ls-files -z <selection> --ignored --exclude=<line>` for each line of the block, run in
    /// the repository's root.
    fn block_files(&self, git: &Git, mode: Mode, selection: &str) -> Result<Vec<String>, Reason> {
        let mut found: BTreeSet<String> = BTreeSet::new();
        for line in mode.lines(&self.prefix) {
            let exclude = format!("--exclude={line}");
            let output = git
                .run(
                    &self.repository,
                    &["ls-files", "-z", selection, "--ignored", &exclude],
                )
                .map_err(|failure| reason_of(failure, false))?;
            for path in output.stdout.split('\0').filter(|path| !path.is_empty()) {
                if let Some(relative) = path.strip_prefix(self.prefix.as_str()) {
                    found.insert(relative.to_owned());
                }
            }
        }
        Ok(found.into_iter().collect())
    }
}

// --- the File view's listing ----------------------------------------------------------------------

/// The 1-based numbers of the lines of the exclusions file at `source`, as `git check-ignore -v`
/// prints it (absolute, or relative to the repository's root, an ancestor of `root`), that lie
/// inside a Solo mode block of any working copy: the lines between a begin line and the next end
/// line with the same root. The begin and end lines themselves are comments and are not in them.
/// Empty when `source` is not a repository's local exclusions file (`<git dir>/info/exclude`), is
/// not found or cannot be read, or has no block. The file is read once, and git is not called: the
/// File view's listing already has the answer of its one `check-ignore`.
pub(crate) fn block_lines_of_source(root: &Path, source: &str) -> Vec<RangeInclusive<usize>> {
    let source = Path::new(source);
    let in_info = source.file_name().is_some_and(|name| name == "exclude")
        && source
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "info");
    if !in_info {
        return Vec::new();
    }
    let file = if source.is_absolute() {
        source.to_path_buf()
    } else {
        let found = root
            .ancestors()
            .map(|folder| folder.join(source))
            .find(|file| file.is_file());
        match found {
            Some(file) => file,
            None => return Vec::new(),
        }
    };
    fs::read(file)
        .map(|content| block_lines(&content))
        .unwrap_or_default()
}

/// The 1-based numbers of the lines inside Solo mode blocks of `content`, as ranges.
fn block_lines(content: &[u8]) -> Vec<RangeInclusive<usize>> {
    const BEGIN: &str = "# >>> weaver solo mode: ";
    let lines: Vec<&[u8]> = content.split(|byte| *byte == b'\n').collect();
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        index += 1;
        let Some(marker) = line.strip_prefix(BEGIN.as_bytes()) else {
            continue;
        };
        let end = [b"# <<< weaver solo mode: ".as_slice(), marker].concat();
        if let Some(last) = (index..lines.len()).find(|&j| lines[j] == end.as_slice()) {
            // Lines `index + 1 ..= last` (1-based) are the ones between the markers.
            if last > index {
                ranges.push(index + 1..=last);
            }
            index = last + 1;
        }
    }
    ranges
}

// --- operations -----------------------------------------------------------------------------------

/// What Solo mode is, for the command line and for the app after a change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspection {
    pub mode: Mode,
    /// Why Solo mode cannot be changed; `None` when it can.
    pub unavailable: Option<Reason>,
    /// While it is on: why the block is not in place as expected.
    pub problem: Option<Reason>,
    /// The tracked files the block excludes; empty when off.
    pub tracked: Vec<String>,
    /// The state file could not be read or held another value.
    pub diagnostic: Option<Diagnostic>,
}

/// Works out the state at the call: the command line's `project show`, and the answer after a change.
/// The block is never repaired here.
pub fn inspect(git: &Git, root: &Path) -> Inspection {
    let state = read_state(root);
    inspection_of(git, state, locate(git, root))
}

fn inspection_of(git: &Git, state: State, location: Result<Location, Reason>) -> Inspection {
    let mode = state.mode;
    let mut inspection = Inspection {
        mode,
        unavailable: None,
        problem: None,
        tracked: Vec::new(),
        diagnostic: state.diagnostic,
    };
    let location = match location {
        Ok(location) => location,
        Err(reason) => {
            if mode.is_on() {
                inspection.problem = Some(reason.clone());
            }
            inspection.unavailable = Some(reason);
            return inspection;
        }
    };
    if !mode.is_on() {
        return inspection;
    }
    match location.block_in_place(mode) {
        Ok(true) => {}
        Ok(false) => {
            inspection.problem = Some(Reason::BlockMissing {
                path: location.exclude.display().to_string(),
            })
        }
        Err(reason) => inspection.problem = Some(reason),
    }
    match location.tracked_files(git, mode) {
        Ok(tracked) => inspection.tracked = tracked,
        Err(reason) => {
            inspection.problem.get_or_insert(reason);
        }
    }
    inspection
}

/// The tracked files of the working copy's mode, on request (Settings shown). Empty when off,
/// without calling git.
pub fn tracked_files(git: &Git, root: &Path) -> Result<Vec<String>, Reason> {
    let mode = read_state(root).mode;
    if !mode.is_on() {
        return Ok(Vec::new());
    }
    locate(git, root)?.tracked_files(git, mode)
}

/// `developer.CHANGE_SOLO_MODE`: turns Solo mode on, off or to its other mode (`requirements/rules#solo-mode`,
/// "Changes"). Turning on writes the block, then `solo.json`; turning off removes the block, then the
/// `solo/` folder; changing mode rewrites the block, then `solo.json`. A change that fails changes
/// nothing and says why; so does any change while a merge of Weaver is in progress (`merge.json`
/// exists), whether it turns Solo mode off or chooses the other mode: that is checked before git is
/// called or anything is written. The caller holds the project's lock.
pub fn set_mode(git: &Git, root: &Path, mode: Mode) -> Result<Inspection, Reason> {
    if !has_weaver(root) {
        return Err(Reason::NotAProject);
    }
    let before = read_state(root);
    if mode != before.mode && crate::solo_merge::in_progress(root) {
        return Err(Reason::MergeInProgress);
    }
    if mode == Mode::Off && before.mode == Mode::Off && before.diagnostic.is_none() {
        return Ok(inspect(git, root));
    }
    let location = locate(git, root)?;
    if mode.is_on() {
        location.write_block(mode)?;
        if let Err(reason) = write_state(root, mode) {
            // The state did not change: the block goes back to what the earlier state expects.
            let _ = match before.mode {
                Mode::Off => location.remove_block(),
                previous => location.write_block(previous),
            };
            return Err(reason);
        }
    } else {
        location.remove_block()?;
        let folder = weaver_dir(root).join(SOLO_DIR);
        match fs::remove_dir_all(&folder) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => {
                let _ = location.write_block(before.mode);
                return Err(Reason::write_failed(&folder, &e));
            }
        }
    }
    let state = State {
        mode,
        diagnostic: None,
    };
    Ok(inspection_of(git, state, Ok(location)))
}

/// What the opening of a project found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Opening {
    /// Why Solo mode cannot be changed; `None` when it can.
    pub unavailable: Option<Reason>,
    /// While it is on: why its block could not be put in place; `None` when it is in place.
    pub problem: Option<Reason>,
    /// Whether the block was written again.
    pub rewritten: bool,
}

/// `window.OPENED`: one `rev-parse` tells whether Solo mode can be changed; with it on, the block is
/// written again, silently, when it is missing or differs from the one the mode expects. Without git or
/// outside the repository Solo mode stays on and the next opening tries again.
pub fn open(git: &Git, root: &Path) -> Opening {
    let mode = read_state(root).mode;
    match locate(git, root) {
        Err(reason) => Opening {
            problem: mode.is_on().then(|| reason.clone()),
            unavailable: Some(reason),
            rewritten: false,
        },
        Ok(location) if mode.is_on() => match location.write_block(mode) {
            Ok(rewritten) => Opening {
                unavailable: None,
                problem: None,
                rewritten,
            },
            Err(reason) => Opening {
                unavailable: None,
                problem: Some(reason),
                rewritten: false,
            },
        },
        Ok(_) => Opening {
            unavailable: None,
            problem: None,
            rewritten: false,
        },
    }
}

/// `workspace.PROJECT_FOLDER_REMOVED`: removes the block of the working copy at `root`, found by its
/// markers alone, because the state went with `.weaver/`.
pub fn remove_block_of(git: &Git, root: &Path) -> Result<bool, Reason> {
    locate(git, root)?.remove_block()
}

/// `project/Not initialised/developer.INITIALISE_PROJECT` with a mode: writes the block of the project
/// at `root`, before any other file is written. The returned location removes it again
/// ([`Location::remove_block`]) when the initialisation fails later.
pub fn place_block(git: &Git, root: &Path, mode: Mode) -> Result<Location, Reason> {
    let location = locate(git, root)?;
    location.write_block(mode)?;
    Ok(location)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod testing;
