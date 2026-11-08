use std::path::Path;

use serde_json::{json, Value};

use super::*;
use crate::jobs::record::RunningTime;

/// A session file of omp: one JSON record per line.
fn write_lines(path: &Path, records: &[Value]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut text = String::new();
    for record in records {
        text.push_str(&record.to_string());
        text.push('\n');
    }
    std::fs::write(path, text).unwrap();
}

fn assistant(usage: (u64, u64, u64, u64), calls: &[(&str, Value)]) -> Value {
    let content: Vec<Value> = calls
        .iter()
        .enumerate()
        .map(|(n, (name, arguments))| {
            json!({ "type": "toolCall", "id": format!("call{n}"), "name": name, "arguments": arguments })
        })
        .collect();
    json!({
        "type": "message",
        "message": {
            "role": "assistant",
            "content": content,
            "usage": { "input": usage.0, "output": usage.1, "cacheRead": usage.2, "cacheWrite": usage.3, "totalTokens": 99 },
        }
    })
}

fn user(text: &str) -> Value {
    json!({ "type": "message", "message": { "role": "user", "content": [{ "type": "text", "text": text }] } })
}

struct Sessions {
    dir: tempfile::TempDir,
}

impl Sessions {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    /// `<root>/-project/2026-11-02T07-00-00-000Z_<session>.jsonl`.
    fn main(&self, session: &str) -> PathBuf {
        self.root()
            .join("-project")
            .join(format!("2026-11-02T07-00-00-000Z_{session}.jsonl"))
    }

    /// The folder of the sub-agents of the session.
    fn folder(&self, session: &str) -> PathBuf {
        agent_folder(&self.main(session)).unwrap()
    }
}

fn project() -> PathBuf {
    std::env::temp_dir().join("weaver-measure-project")
}

/// An absolute path of the project, as omp writes it.
fn inside(rel: &str) -> String {
    project().join(rel).to_string_lossy().into_owned()
}

#[test]
fn the_tokens_and_the_counts_are_summed_for_each_part() {
    let sessions = Sessions::new();
    write_lines(
        &sessions.main("s1"),
        &[
            json!({ "type": "session", "timestamp": "2026-11-02T07:00:00.000Z" }),
            user("Go"),
            assistant(
                (10, 5, 100, 1000),
                &[
                    ("read", json!({ "path": inside("src/a.rs") })),
                    ("read", json!({ "path": "src/a.rs:10-20" })),
                    ("read", json!({ "path": "src/b.rs" })),
                    ("grep", json!({ "pattern": "x" })),
                    ("glob", json!({ "pattern": "**/*.rs" })),
                    ("bash", json!({ "command": "cat src/c.rs" })),
                ],
            ),
            // Not assistant messages: their `usage` and their calls never count.
            json!({ "type": "message", "message": { "role": "toolResult", "usage": { "input": 7777 }, "content": [{ "type": "toolCall", "name": "read", "arguments": { "path": "x" } }] } }),
            json!({ "type": "custom", "customType": "session_exit" }),
            assistant((1, 2, 3, 4), &[("read", json!({ "path": "skill://weaver-design" }))]),
        ],
    );
    let folder = sessions.folder("s1");
    write_lines(
        &folder.join("0-Reader.jsonl"),
        &[
            assistant(
                (20, 10, 0, 50),
                &[
                    ("read", json!({ "path": "src/a.rs:raw" })),
                    ("read", json!({ "path": "/elsewhere/z.txt" })),
                ],
            ),
            assistant((1, 1, 1, 1), &[("grep", json!({}))]),
        ],
    );
    // A nested sub-agent, two levels down.
    write_lines(
        &folder.join("0-Reader").join("1-Nested.jsonl"),
        &[assistant(
            (5, 5, 5, 5),
            &[("read", json!({ "path": "src/d.rs" })), ("glob", json!({}))],
        )],
    );
    write_lines(
        &folder.join("0-Reader").join("1-Nested").join("2-Deep.jsonl"),
        &[assistant((100, 0, 0, 0), &[])],
    );
    // The advisors of a Run are apart, whatever the folder.
    write_lines(
        &folder.join("__advisor.turn.jsonl"),
        &[assistant(
            (3, 3, 3, 3),
            &[("read", json!({ "path": "src/b.rs" })), ("grep", json!({}))],
        )],
    );
    write_lines(
        &folder.join("0-Reader").join("__advisor.final.jsonl"),
        &[assistant((4, 4, 4, 4), &[("read", json!({ "path": "src/e.rs" }))])],
    );
    // Something that is not a transcript.
    std::fs::write(folder.join("notes.txt"), "not json").unwrap();

    let parts = read_session(&project(), Some(sessions.root()), Some("s1"), true).unwrap();
    assert_eq!(
        parts.main,
        Counts {
            reads: 4,
            files: 3,
            searches: 2,
            input_tokens: 11,
            output_tokens: 7,
            cache_read_tokens: 103,
            cache_write_tokens: 1004,
        }
    );
    assert_eq!(
        parts.sub_agents,
        Group {
            counts: Counts {
                reads: 3,
                files: 3,
                searches: 2,
                input_tokens: 126,
                output_tokens: 16,
                cache_read_tokens: 6,
                cache_write_tokens: 56,
            },
            count: 3,
        }
    );
    assert_eq!(
        parts.advisors,
        Some(Group {
            counts: Counts {
                reads: 2,
                files: 2,
                searches: 1,
                input_tokens: 7,
                output_tokens: 7,
                cache_read_tokens: 7,
                cache_write_tokens: 7,
            },
            count: 2,
        })
    );
    // src/a.rs, src/b.rs, skill://weaver-design, /elsewhere/z.txt, src/d.rs, src/e.rs.
    assert_eq!(parts.files, 6);
}

#[test]
fn a_job_that_is_not_a_run_has_no_advisors() {
    let sessions = Sessions::new();
    write_lines(&sessions.main("s2"), &[assistant((1, 1, 1, 1), &[])]);
    write_lines(
        &sessions.folder("s2").join("__advisor.turn.jsonl"),
        &[assistant((9, 9, 9, 9), &[])],
    );
    write_lines(
        &sessions.folder("s2").join("0-Agent.jsonl"),
        &[assistant((2, 2, 2, 2), &[])],
    );
    let parts = read_session(&project(), Some(sessions.root()), Some("s2"), false).unwrap();
    assert_eq!(parts.advisors, None);
    assert_eq!(parts.sub_agents.count, 1);
    assert_eq!(parts.sub_agents.counts.input_tokens, 2);
}

#[test]
fn a_job_without_sub_agents_counts_zero_transcripts() {
    let sessions = Sessions::new();
    write_lines(&sessions.main("s3"), &[assistant((1, 2, 3, 4), &[])]);
    let parts = read_session(&project(), Some(sessions.root()), Some("s3"), true).unwrap();
    assert_eq!(parts.sub_agents, Group::default());
    assert_eq!(parts.advisors, Some(Group::default()));
    assert_eq!(parts.main.cache_write_tokens, 4);
    assert_eq!(parts.files, 0);
}

#[test]
fn the_distinct_files_ignore_line_selectors_and_keep_the_other_paths_as_given() {
    let root = project();
    let root = root.as_path();
    let absolute = format!("{}:1-9", inside("src/a.rs"));
    let same = [
        "src/a.rs",
        "src/a.rs:50",
        "src/a.rs:50-80",
        "src/a.rs:-60",
        "src/a.rs:50+150",
        "src/a.rs:5-16,960-973",
        "src/a.rs:raw",
        "src/a.rs:2-4:raw",
        "src/a.rs:conflicts",
        "./src/a.rs",
        absolute.as_str(),
    ];
    for given in same {
        assert_eq!(read_path(root, given), "src/a.rs", "{given}");
    }
    assert_eq!(read_path(root, "/etc/hosts:3-4"), "/etc/hosts");
    assert_eq!(read_path(root, "skill://weaver-design"), "skill://weaver-design");
    assert_eq!(
        read_path(root, "skill://weaver-design/SKILL.md:1-20"),
        "skill://weaver-design/SKILL.md"
    );
    assert_eq!(read_path(root, "artifact://4:raw:11-11"), "artifact://4");
    assert_eq!(read_path(root, "https://example.com:8080"), "https://example.com:8080");
    assert_eq!(
        read_path(root, "https://example.com:8080/a:10-20"),
        "https://example.com:8080/a"
    );
    assert_eq!(read_path(root, "weird:name"), "weird:name");
    assert_eq!(read_path(root, "dir/file:"), "dir/file:");
}

#[test]
fn a_last_line_that_is_not_complete_is_ignored() {
    let sessions = Sessions::new();
    let main = sessions.main("s4");
    write_lines(&main, &[assistant((1, 1, 1, 1), &[])]);
    // omp was writing this one.
    let mut text = std::fs::read_to_string(&main).unwrap();
    text.push_str("{\"type\":\"message\",\"message\":{\"role\":\"assist");
    std::fs::write(&main, text).unwrap();
    let transcript = sessions.folder("s4").join("0-A.jsonl");
    write_lines(&transcript, &[assistant((2, 2, 2, 2), &[])]);
    let mut text = std::fs::read_to_string(&transcript).unwrap();
    text.push_str("{\"type\":");
    std::fs::write(&transcript, text).unwrap();
    let parts = read_session(&project(), Some(sessions.root()), Some("s4"), false).unwrap();
    assert_eq!(parts.main.input_tokens, 1);
    assert_eq!(parts.sub_agents.counts.input_tokens, 2);
}

#[test]
fn a_last_line_without_its_line_break_that_is_valid_still_counts() {
    let sessions = Sessions::new();
    let main = sessions.main("s5");
    std::fs::create_dir_all(main.parent().unwrap()).unwrap();
    std::fs::write(&main, assistant((8, 0, 0, 0), &[]).to_string()).unwrap();
    let parts = read_session(&project(), Some(sessions.root()), Some("s5"), false).unwrap();
    assert_eq!(parts.main.input_tokens, 8);
}

#[test]
fn every_reason_the_session_cannot_be_read_names_what_is_missing() {
    let sessions = Sessions::new();
    let read = |root: Option<&Path>, id: Option<&str>| read_session(&project(), root, id, true);

    let reason = read(Some(sessions.root()), None).unwrap_err();
    assert!(reason.contains("no session"), "{reason}");
    let reason = read(Some(sessions.root()), Some("")).unwrap_err();
    assert!(reason.contains("no session"), "{reason}");

    let reason = read(None, Some("s6")).unwrap_err();
    assert!(reason.contains("session files is not known"), "{reason}");

    let reason = read(Some(sessions.root()), Some("s6")).unwrap_err();
    assert!(
        reason.contains("`*_s6.jsonl`") && reason.contains("was not found"),
        "{reason}"
    );

    // The main file cannot be read.
    let main = sessions.main("s7");
    std::fs::create_dir_all(&main).unwrap();
    let reason = read(Some(sessions.root()), Some("s7")).unwrap_err();
    assert!(
        reason.contains("s7.jsonl") && reason.contains("could not be read"),
        "{reason}"
    );

    // A malformed complete line in the main file.
    let main = sessions.main("s8");
    write_lines(&main, &[assistant((1, 1, 1, 1), &[])]);
    let mut text = std::fs::read_to_string(&main).unwrap();
    text.push_str("this is not json\n");
    std::fs::write(&main, text).unwrap();
    let reason = read(Some(sessions.root()), Some("s8")).unwrap_err();
    assert!(
        reason.contains("s8.jsonl") && reason.contains("malformed line (line 2)"),
        "{reason}"
    );

    // A malformed complete line in a nested transcript, or in an advisor's.
    for name in ["0-A/1-B.jsonl", "__advisor.turn.jsonl"] {
        let sessions = Sessions::new();
        write_lines(&sessions.main("s9"), &[assistant((1, 1, 1, 1), &[])]);
        let file = sessions.folder("s9").join(name);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "{}\n{broken}\n{}\n").unwrap();
        let reason = read_session(&project(), Some(sessions.root()), Some("s9"), true).unwrap_err();
        let leaf = name.rsplit('/').next().unwrap();
        assert!(
            reason.contains(leaf) && reason.contains("malformed line (line 2)"),
            "{reason}"
        );
    }
}

fn run_record(state: JobState) -> JobRecord {
    JobRecord {
        id: "jjjjjjjj".into(),
        kind: JobKind::Run,
        name: "Run T3".into(),
        project: "/work/project".into(),
        target: Some("tttttttt".into()),
        merged: None,
        state,
        reason: None,
        created: "2026-11-02T07:00:00.000Z".into(),
        started: Some("2026-11-02T07:00:05.000Z".into()),
        ended: Some("2026-11-02T07:10:00.500Z".into()),
        session_id: Some("s1".into()),
        omp: String::new(),
        args: Vec::new(),
        env: Default::default(),
        announcements: Vec::new(),
        notes: Vec::new(),
        pid: None,
        pid_started: None,
        empty_model_at_start: false,
        overlay: String::new(),
        running: Some(RunningTime {
            ms: 125_000,
            since: None,
        }),
        running_incomplete: false,
        by_chat: false,
    }
}

fn parts() -> Parts {
    Parts {
        main: Counts {
            reads: 3,
            files: 2,
            searches: 1,
            input_tokens: 10,
            output_tokens: 20,
            cache_read_tokens: 30,
            cache_write_tokens: 40,
        },
        sub_agents: Group {
            counts: Counts::default(),
            count: 2,
        },
        advisors: Some(Group::default()),
        files: 2,
    }
}

#[test]
fn the_line_has_the_fields_of_the_requirement_in_order() {
    let line = line_for(&run_record(JobState::Done), Ok(parts())).unwrap();
    assert_eq!(
        serde_json::to_string(&line).unwrap(),
        concat!(
            r#"{"job":"jjjjjjjj","kind":"run","name":"Run T3","target":"tttttttt","state":"done","#,
            r#""created":"2026-11-02T07:00:00.000Z","ended":"2026-11-02T07:10:00.500Z","#,
            r#""durationMs":600500,"activeMs":125000,"#,
            r#""main":{"reads":3,"files":2,"searches":1,"inputTokens":10,"outputTokens":20,"cacheReadTokens":30,"cacheWriteTokens":40},"#,
            r#""subAgents":{"reads":0,"files":0,"searches":0,"inputTokens":0,"outputTokens":0,"cacheReadTokens":0,"cacheWriteTokens":0,"count":2},"#,
            r#""advisors":{"reads":0,"files":0,"searches":0,"inputTokens":0,"outputTokens":0,"cacheReadTokens":0,"cacheWriteTokens":0,"count":0},"#,
            r#""files":2}"#
        )
    );
}

#[test]
fn an_incomplete_active_time_and_an_unreadable_session_are_said() {
    let mut record = run_record(JobState::Failed);
    record.kind = JobKind::Design;
    record.target = None;
    record.running_incomplete = true;
    let line = line_for(&record, Err("no session".into())).unwrap();
    assert_eq!(
        serde_json::to_string(&line).unwrap(),
        concat!(
            r#"{"job":"jjjjjjjj","kind":"design","name":"Run T3","state":"failed","#,
            r#""created":"2026-11-02T07:00:00.000Z","ended":"2026-11-02T07:10:00.500Z","#,
            r#""durationMs":600500,"activeMs":125000,"activeIncomplete":true,"#,
            r#""unavailable":"no session"}"#
        )
    );
}

#[test]
fn a_job_never_in_running_has_no_active_time_and_the_other_kinds_have_no_line() {
    let mut record = run_record(JobState::Stopped);
    record.running = None;
    let line = line_for(&record, Err("no session".into())).unwrap();
    assert_eq!((line.active_ms, line.state), (0, JobState::Stopped));

    for kind in JobKind::ALL {
        let record = JobRecord {
            kind,
            ..run_record(JobState::Done)
        };
        assert_eq!(
            line_for(&record, Ok(parts())).is_some(),
            kind.is_measured(),
            "{kind:?}"
        );
    }
    // A job that works has no line yet.
    assert!(line_for(&run_record(JobState::Running), Ok(parts())).is_none());
}

#[test]
fn the_lines_are_appended_with_the_gitignore_of_the_local_folder_and_never_rewritten() {
    let project = tempfile::tempdir().unwrap();
    let folder = weaver_dir(project.path()).join("timings");
    assert!(!folder.exists());

    let first = line_for(&run_record(JobState::Done), Ok(parts())).unwrap();
    append(project.path(), &first).unwrap();
    assert_eq!(std::fs::read(folder.join(".gitignore")).unwrap(), b"*\n");
    // An old file of Trama stays as it is.
    std::fs::write(folder.join("old.json"), "old").unwrap();

    let mut second = line_for(&run_record(JobState::Stopped), Err("no session".into())).unwrap();
    second.job = "kkkkkkkk".into();
    std::fs::remove_file(folder.join(".gitignore")).unwrap();
    append(project.path(), &second).unwrap();
    assert_eq!(std::fs::read(folder.join(".gitignore")).unwrap(), b"*\n");
    assert_eq!(std::fs::read_to_string(folder.join("old.json")).unwrap(), "old");

    let text = std::fs::read_to_string(timings_file(project.path())).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], serde_json::to_string(&first).unwrap());
    assert_eq!(lines[1], serde_json::to_string(&second).unwrap());
    assert!(text.ends_with('\n'));

    // `weaver job show` finds the line of each job, as written.
    let found = find(project.path(), "jjjjjjjj").unwrap();
    assert_eq!(found, serde_json::to_value(&first).unwrap());
    assert_eq!(find(project.path(), "kkkkkkkk").unwrap()["unavailable"], "no session");
    assert_eq!(find(project.path(), "zzzzzzzz"), None);
    assert_eq!(find(tempfile::tempdir().unwrap().path(), "jjjjjjjj"), None);
}

#[test]
fn the_text_has_the_two_durations_and_a_line_for_each_part() {
    let line = serde_json::to_value(line_for(&run_record(JobState::Done), Ok(parts())).unwrap()).unwrap();
    assert_eq!(
        describe(&line),
        "- duration: 10m 00s\n\
         - active duration: 2m 05s\n\
         - main agent: 3 reads, 2 files, 1 searches; tokens: input 10, output 20, cache read 30, cache written 40\n\
         - sub-agents (2): 0 reads, 0 files, 0 searches; tokens: input 0, output 0, cache read 0, cache written 0\n\
         - advisors (0): 0 reads, 0 files, 0 searches; tokens: input 0, output 0, cache read 0, cache written 0\n"
    );

    let mut record = run_record(JobState::Failed);
    record.running_incomplete = true;
    let line = serde_json::to_value(line_for(&record, Err("`/x/s1.jsonl` has a malformed line (line 4)".into())).unwrap()).unwrap();
    let text = describe(&line);
    assert!(text.contains("- active duration: 2m 05s (incomplete"), "{text}");
    assert!(
        text.ends_with("- unavailable: `/x/s1.jsonl` has a malformed line (line 4)\n"),
        "{text}"
    );
    assert!(!text.contains("main agent"));
}

#[test]
fn durations_read_like_the_ones_of_the_jobs_list() {
    assert_eq!(format_duration(0), "0s");
    assert_eq!(format_duration(45_999), "45s");
    assert_eq!(format_duration(245_000), "4m 05s");
    assert_eq!(format_duration(4_320_000), "1h 12m");
}

#[test]
fn a_last_line_cut_short_by_a_crash_does_not_swallow_the_next_one() {
    let project = tempfile::tempdir().unwrap();
    let file = timings_file(project.path());
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "{\"job\":\"cut").unwrap();
    let line = line_for(&run_record(JobState::Done), Ok(parts())).unwrap();
    append(project.path(), &line).unwrap();
    let text = std::fs::read_to_string(&file).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert_eq!(lines[1], serde_json::to_string(&line).unwrap());
    assert_eq!(find(project.path(), "jjjjjjjj").unwrap()["job"], "jjjjjjjj");
}
