//! The measure of an ended job (`requirements/data#job`, `requirements/tech#job`, "Measure").
//!
//! When a design, a rework, a refinement or a Run ends, whatever the way, Weaver reads omp's session
//! files once and appends one line to `.weaver/timings/jobs.jsonl`: how long the job lasted, how long
//! the agent actually worked, and, for the main agent, the sub-agents and the advisors of a Run
//! apart, the reads, the distinct files read, the searches and the tokens. The line is written once
//! and never rewritten, and the file is never deleted or shortened: only the other files of the
//! folder, which Weaver never reads or touches, are not its business.
//!
//! The session files are omp internals, re-verified at each `MIN_OMP_VERSION` bump: the main file
//! (name ending `_<sessionId>.jsonl`), the transcripts of its folder and of the nested folders, and
//! in them the `usage` of each `message` with `role: assistant` and its `toolCall` blocks. Each file
//! is streamed line by line and never held whole in memory.
//!
//! Blocking file I/O: the app runs it on a blocking thread, after the end of the job was applied.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::kinds::{JobKind, JobState};
use super::record::JobRecord;
use super::subagents::{agent_folder, find_main_file, ADVISOR_PREFIX};
use crate::project::clock::parse_instant;
use crate::project::layout::{ensure_local_folder, weaver_dir, TIMINGS_DIR};

/// The file of the measures, in the `timings/` folder.
pub const TIMINGS_FILE: &str = "jobs.jsonl";

/// `<project root>/.weaver/timings/jobs.jsonl`.
pub fn timings_file(project_root: &Path) -> PathBuf {
    weaver_dir(project_root)
        .join(TIMINGS_DIR)
        .join(TIMINGS_FILE)
}

/// What one part of a job (the main agent, its sub-agents together, its advisors together) read,
/// searched and used.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    /// The calls of omp's `read` tool.
    pub reads: u64,
    /// The distinct paths they read, without a line selector.
    pub files: u64,
    /// The calls of `grep` and `glob`.
    pub searches: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
}

/// The sub-agents, or the advisors, taken together.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    #[serde(flatten)]
    pub counts: Counts,
    /// The number of transcripts summed.
    pub count: u64,
}

/// What the session files of a job say.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parts {
    pub main: Counts,
    pub sub_agents: Group,
    /// Only for a Run.
    pub advisors: Option<Group>,
    /// The distinct files read by the whole job.
    pub files: u64,
}

/// One line of `.weaver/timings/jobs.jsonl`, with the fields in the order of `requirements/data#job`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasureLine {
    pub job: String,
    pub kind: JobKind,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// `done`, `failed` or `stopped`.
    pub state: JobState,
    pub created: String,
    pub ended: String,
    /// From `created` to `ended`.
    pub duration_ms: u64,
    /// The time spent in Running only.
    pub active_ms: u64,
    /// Only when the record carried `runningIncomplete`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub active_incomplete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main: Option<Counts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub_agents: Option<Group>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub advisors: Option<Group>,
    /// The distinct files read by the whole job.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<u64>,
    /// Instead of the parts and the top-level `files`: why the session could not be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unavailable: Option<String>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// The line of an ended measured job; `session` is what [`read_session`] answered. `None` for a job
/// that is not measured or has not ended.
pub fn line_for(record: &JobRecord, session: Result<Parts, String>) -> Option<MeasureLine> {
    if !record.kind.is_measured() || !record.state.is_final() {
        return None;
    }
    let ended = record.ended.clone()?;
    let duration_ms = match (parse_instant(&record.created), parse_instant(&ended)) {
        (Some(from), Some(to)) => u64::try_from(to - from).unwrap_or(0),
        _ => 0,
    };
    let mut line = MeasureLine {
        job: record.id.clone(),
        kind: record.kind,
        name: record.name.clone(),
        target: record.target.clone(),
        state: record.state,
        created: record.created.clone(),
        ended,
        duration_ms,
        active_ms: record.running.as_ref().map_or(0, |running| running.ms),
        active_incomplete: record.running_incomplete,
        main: None,
        sub_agents: None,
        advisors: None,
        files: None,
        unavailable: None,
    };
    match session {
        Ok(parts) => {
            line.main = Some(parts.main);
            line.sub_agents = Some(parts.sub_agents);
            line.advisors = parts.advisors;
            line.files = Some(parts.files);
        }
        Err(reason) => line.unavailable = Some(reason),
    }
    Some(line)
}

/// Appends the line to `.weaver/timings/jobs.jsonl`, creating the local-only folder (with its
/// `.gitignore`) and the file with the first line. One write of the whole line, so that two jobs
/// that end together never mix theirs.
pub fn append(project_root: &Path, line: &MeasureLine) -> io::Result<()> {
    let folder = ensure_local_folder(project_root, TIMINGS_DIR)?;
    let mut text = serde_json::to_string(line).map_err(io::Error::other)?;
    text.push('\n');
    let mut file = OpenOptions::new()
        .read(true)
        .create(true)
        .append(true)
        .open(folder.join(TIMINGS_FILE))?;
    // A last line cut short by a crash must not swallow this one: end it first.
    let length = file.metadata()?.len();
    if length > 0 {
        let mut last = [0u8; 1];
        file.seek(SeekFrom::Start(length - 1))?;
        file.read_exact(&mut last)?;
        if last[0] != b'\n' {
            text.insert(0, '\n');
        }
    }
    file.write_all(text.as_bytes())
}

/// The line of `job`, as written, or `None` when there is none (or the file cannot be read). The
/// file is streamed; a line that is not valid is skipped.
pub fn find(project_root: &Path, job: &str) -> Option<Value> {
    let mut reader = BufReader::new(File::open(timings_file(project_root)).ok()?);
    let mut buffer = Vec::new();
    let mut found = None;
    loop {
        buffer.clear();
        if reader.read_until(b'\n', &mut buffer).ok()? == 0 {
            return found;
        }
        let mentions = buffer.windows(job.len()).any(|window| window == job.as_bytes());
        if !mentions {
            continue;
        }
        if let Ok(value) = serde_json::from_slice::<Value>(&buffer) {
            if value.get("job").and_then(Value::as_str) == Some(job) {
                found = Some(value);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Reading the session files
// ---------------------------------------------------------------------------------------------

/// What is counted while the files of one part are read.
#[derive(Default)]
struct Tally {
    counts: Counts,
    files: HashSet<String>,
}

impl Tally {
    fn finish(mut self) -> (Counts, HashSet<String>) {
        self.counts.files = self.files.len() as u64;
        (self.counts, self.files)
    }
}

/// Reads the session files of a job once: the main file of `session_id` under `sessions_root`, and
/// every transcript in its folder and in the nested ones. `project_root` makes the paths read
/// relative. The reason an `Err` carries names the file that made the session unreadable.
pub fn read_session(
    project_root: &Path,
    sessions_root: Option<&Path>,
    session_id: Option<&str>,
    run: bool,
) -> Result<Parts, String> {
    let Some(session_id) = session_id.filter(|id| !id.is_empty()) else {
        return Err("the job has no session: omp never opened one".to_owned());
    };
    let Some(sessions_root) = sessions_root else {
        return Err("the folder of omp's session files is not known".to_owned());
    };
    let Some(main_file) = find_main_file(sessions_root, session_id) else {
        return Err(format!(
            "the main session file `*_{session_id}.jsonl` was not found under {}",
            sessions_root.display()
        ));
    };

    let mut all_files: HashSet<String> = HashSet::new();
    let mut main = Tally::default();
    tally_file(&main_file, project_root, &mut main)?;
    let (main, main_files) = main.finish();
    all_files.extend(main_files);

    let mut transcripts = Vec::new();
    if let Some(folder) = agent_folder(&main_file) {
        collect_transcripts(&folder, &mut transcripts)?;
    }
    let (mut sub_agents, mut advisors) = (Tally::default(), Tally::default());
    let (mut sub_count, mut advisor_count) = (0u64, 0u64);
    for file in &transcripts {
        let is_advisor = file
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(ADVISOR_PREFIX));
        match (is_advisor, run) {
            (true, true) => {
                tally_file(file, project_root, &mut advisors)?;
                advisor_count += 1;
            }
            // Only a Run has advisors.
            (true, false) => {}
            (false, _) => {
                tally_file(file, project_root, &mut sub_agents)?;
                sub_count += 1;
            }
        }
    }
    let (sub_agents, sub_files) = sub_agents.finish();
    let (advisors, advisor_files) = advisors.finish();
    all_files.extend(sub_files);
    all_files.extend(advisor_files);

    Ok(Parts {
        main,
        sub_agents: Group {
            counts: sub_agents,
            count: sub_count,
        },
        advisors: run.then_some(Group {
            counts: advisors,
            count: advisor_count,
        }),
        files: all_files.len() as u64,
    })
}

/// The `.jsonl` files of `folder` and of the folders under it, at any depth, in name order. A
/// folder that does not exist has none.
fn collect_transcripts(folder: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("`{}` could not be read: {error}", folder.display())),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("`{}` could not be read: {error}", folder.display()))?;
        let kind = entry
            .file_type()
            .map_err(|error| format!("`{}` could not be read: {error}", entry.path().display()))?;
        paths.push((entry.path(), kind));
    }
    paths.sort_by(|a, b| a.0.cmp(&b.0));
    for (path, kind) in paths {
        if kind.is_dir() {
            collect_transcripts(&path, out)?;
        } else if kind.is_file() && path.extension().is_some_and(|ext| ext == "jsonl") {
            out.push(path);
        }
    }
    Ok(())
}

/// Streams `file` line by line into `tally`. A complete line that is not valid JSON makes the
/// whole session unreadable; a last line without its line break that is not valid is the line omp
/// was writing, and is ignored.
fn tally_file(file: &Path, project_root: &Path, tally: &mut Tally) -> Result<(), String> {
    let unreadable = |error: io::Error| format!("`{}` could not be read: {error}", file.display());
    let mut reader = BufReader::new(File::open(file).map_err(unreadable)?);
    let mut buffer = Vec::new();
    let mut number = 0usize;
    loop {
        buffer.clear();
        if reader.read_until(b'\n', &mut buffer).map_err(unreadable)? == 0 {
            return Ok(());
        }
        number += 1;
        let complete = buffer.ends_with(b"\n");
        if buffer.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match serde_json::from_slice::<Value>(&buffer) {
            Ok(record) => count_record(&record, project_root, tally),
            Err(_) if !complete => return Ok(()),
            Err(_) => {
                return Err(format!(
                    "`{}` has a malformed line (line {number})",
                    file.display()
                ))
            }
        }
    }
}

fn tokens(usage: &Value, key: &str) -> u64 {
    let Some(value) = usage.get(key) else {
        return 0;
    };
    value
        .as_u64()
        .or_else(|| value.as_f64().filter(|f| *f > 0.0).map(|f| f as u64))
        .unwrap_or(0)
}

fn count_record(record: &Value, project_root: &Path, tally: &mut Tally) {
    if record.get("type").and_then(Value::as_str) != Some("message") {
        return;
    }
    let Some(message) = record.get("message") else {
        return;
    };
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return;
    }
    if let Some(usage) = message.get("usage") {
        let counts = &mut tally.counts;
        counts.input_tokens += tokens(usage, "input");
        counts.output_tokens += tokens(usage, "output");
        counts.cache_read_tokens += tokens(usage, "cacheRead");
        counts.cache_write_tokens += tokens(usage, "cacheWrite");
    }
    let Some(blocks) = message.get("content").and_then(Value::as_array) else {
        return;
    };
    for block in blocks {
        if block.get("type").and_then(Value::as_str) != Some("toolCall") {
            continue;
        }
        match block.get("name").and_then(Value::as_str) {
            Some("read") => {
                tally.counts.reads += 1;
                let path = block
                    .get("arguments")
                    .and_then(|arguments| arguments.get("path"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|path| !path.is_empty());
                if let Some(path) = path {
                    tally.files.insert(read_path(project_root, path));
                }
            }
            Some("grep" | "glob") => tally.counts.searches += 1,
            _ => {}
        }
    }
}

/// Whether `word` is a selector of the `read` tool after a `:`: lines (`50`, `50-200`, `50+150`,
/// `-60`, `5-16,960-973`) or one of the named ones.
fn is_selector(word: &str) -> bool {
    matches!(word, "raw" | "conflicts")
        || (!word.is_empty() && word.bytes().all(|b| b.is_ascii_digit() || matches!(b, b',' | b'-' | b'+')))
}

/// The path a `read` call names, without its line selector, as the distinct files count it: relative
/// to the project with `/` when it lies inside the project, as given otherwise (internal URLs
/// included).
fn read_path(project_root: &Path, given: &str) -> String {
    // The host and port of a web address are not a selector.
    let protected = ["http://", "https://", "ssh://"]
        .iter()
        .find(|scheme| given.starts_with(**scheme))
        .map_or(0, |scheme| {
            let rest = &given[scheme.len()..];
            scheme.len() + rest.find('/').unwrap_or(rest.len())
        });
    let mut path = given;
    while let Some(colon) = path.rfind(':') {
        if colon < protected || !is_selector(&path[colon + 1..]) {
            break;
        }
        path = &path[..colon];
    }
    if path.contains("://") {
        return path.to_owned();
    }
    let path = path.strip_prefix("./").unwrap_or(path);
    let candidate = Path::new(path);
    if candidate.is_absolute() {
        if let Ok(inside) = candidate.strip_prefix(project_root) {
            let parts: Vec<String> = inside
                .components()
                .filter_map(|component| match component {
                    Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
                    _ => None,
                })
                .collect();
            return parts.join("/");
        }
    }
    path.to_owned()
}

// ---------------------------------------------------------------------------------------------
// What `weaver job show <id>` prints
// ---------------------------------------------------------------------------------------------

/// `45s`, `4m 05s`, `1h 12m`.
pub fn format_duration(ms: u64) -> String {
    let seconds = ms / 1000;
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m {:02}s", seconds / 60, seconds % 60)
    } else {
        format!("{}h {:02}m", seconds / 3600, seconds % 3600 / 60)
    }
}

fn number(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn part_line(label: &str, part: &Value) -> String {
    format!(
        "- {label}: {} reads, {} files, {} searches; tokens: input {}, output {}, cache read {}, cache written {}\n",
        number(part, "reads"),
        number(part, "files"),
        number(part, "searches"),
        number(part, "inputTokens"),
        number(part, "outputTokens"),
        number(part, "cacheReadTokens"),
        number(part, "cacheWriteTokens"),
    )
}

/// The measure as text: the two durations, then one line per part, or the reason the session could
/// not be read.
pub fn describe(line: &Value) -> String {
    let mut text = format!(
        "- duration: {}\n",
        format_duration(number(line, "durationMs"))
    );
    let incomplete = line.get("activeIncomplete").and_then(Value::as_bool) == Some(true);
    text.push_str(&format!(
        "- active duration: {}{}\n",
        format_duration(number(line, "activeMs")),
        if incomplete {
            " (incomplete: Weaver stopped unexpectedly while the job worked)"
        } else {
            ""
        }
    ));
    if let Some(reason) = line.get("unavailable").and_then(Value::as_str) {
        text.push_str(&format!("- unavailable: {reason}\n"));
        return text;
    }
    if let Some(main) = line.get("main") {
        text.push_str(&part_line("main agent", main));
    }
    for (key, label) in [("subAgents", "sub-agents"), ("advisors", "advisors")] {
        if let Some(group) = line.get(key) {
            text.push_str(&part_line(
                &format!("{label} ({})", number(group, "count")),
                group,
            ));
        }
    }
    text
}

#[cfg(test)]
mod tests;
