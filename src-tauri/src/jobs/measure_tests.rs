//! The measure of an ended job (`requirements/data#job`, `requirements/tech#job`): the durations
//! with an injected clock, the line in `.weaver/timings/jobs.jsonl`, the jobs that are not
//! measured and `weaver job show <id>`. The session files are written by the tests, in the place
//! the test host says omp keeps them.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use weaver_core::jobs::measure;
use weaver_core::jobs::record::RunningTime;
use weaver_core::jobs::{JobState, JobStore};
use weaver_core::project::clock::ManualClock;
use weaver_core::project::image::ProjectImage;

use super::registry::Jobs;
use super::starts::Ctx;
use super::testing::{row, until, Project, TestHost, LIMIT, TASK};

const T0: &str = "2026-11-02T07:00:00.000Z";

fn with_clock() -> (Arc<TestHost>, Arc<ManualClock>, Arc<Jobs>) {
    let host = TestHost::new();
    let clock = Arc::new(ManualClock::at(T0));
    let jobs = Jobs::with_clock(host.clone(), clock.clone());
    (host, clock, jobs)
}

fn run_task(jobs: &Jobs, project: &Project) -> String {
    let image = ProjectImage::load(&project.root);
    jobs.start_run_task(
        &Ctx {
            project_id: &project.id,
            root: &project.root,
            image: &image,
        },
        "aaaaaaaa",
    )
    .map_err(|refusal| refusal.0)
    .unwrap()
}

async fn state_of(host: &TestHost, project: &Project, job: &str, wanted: JobState) {
    until(&format!("job {job} to be {wanted:?}"), LIMIT, || {
        row(host, &project.id, job).is_some_and(|row| row.state == wanted)
    })
    .await;
}

async fn measured(project: &Project, job: &str) -> Value {
    until("the measure of the job", LIMIT, || {
        measure::find(&project.root, job).is_some()
    })
    .await;
    measure::find(&project.root, job).unwrap()
}

fn write_lines(path: &Path, records: &[Value]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let text: String = records.iter().map(|r| format!("{r}\n")).collect();
    std::fs::write(path, text).unwrap();
}

fn assistant(usage: [u64; 4], calls: &[(&str, Value)]) -> Value {
    let content: Vec<Value> = calls
        .iter()
        .enumerate()
        .map(|(n, (name, arguments))| {
            json!({ "type": "toolCall", "id": format!("c{n}"), "name": name, "arguments": arguments })
        })
        .collect();
    json!({ "type": "message", "message": { "role": "assistant", "content": content,
        "usage": { "input": usage[0], "output": usage[1], "cacheRead": usage[2], "cacheWrite": usage[3] } } })
}

/// The main session file of the fake agent's session, and the folder of its transcripts.
fn session(host: &TestHost) -> (PathBuf, PathBuf) {
    let folder = host
        .dirs
        .local_data_dir()
        .join("omp-sessions")
        .join("-project");
    (
        folder.join("2026-11-02T07-00-00-000Z_fake-session.jsonl"),
        folder.join("2026-11-02T07-00-00-000Z_fake-session"),
    )
}

#[tokio::test]
async fn a_run_is_measured_with_the_time_it_worked_and_what_its_session_files_say() {
    use weaver_cli::exec::execute;
    use weaver_cli::spec::Caller;
    use weaver_core::command::CommandRequest;

    let (host, clock, jobs) = with_clock();
    let (_dir, project) = host
        .project(&[("tasks/aaaaaaaa-first-task.md", TASK)])
        .await;
    host.script(json!({ "prompts": [[{ "hold": true }]] }));

    // Queued for 10 s, waiting for omp.
    host.gate.send_replace(false);
    let id = run_task(&jobs, &project);
    clock.advance(10_000);
    host.gate.send_replace(true);
    state_of(&host, &project, &id, JobState::Running).await;

    // Running 20 s, then waiting for the developer 100 s: only the first counts.
    clock.advance(20_000);
    jobs.round_opened(&id);
    state_of(&host, &project, &id, JobState::Waiting).await;
    clock.advance(100_000);
    jobs.round_closed(&id);
    state_of(&host, &project, &id, JobState::Running).await;

    // Running 5 s, then suspended 60 s: only the first counts.
    clock.advance(5_000);
    jobs.window_closed(&project.id).await;
    assert_eq!(
        jobs.record(&id).unwrap().running,
        Some(RunningTime {
            ms: 25_000,
            since: None
        })
    );
    host.script(json!({ "prompts": [[{ "hold": true }]] }));
    clock.advance(60_000);
    jobs.window_opened(&project.id, &project.root);
    state_of(&host, &project, &id, JobState::Running).await;

    // Running 15 s more, then stopped.
    clock.advance(15_000);

    let (main, folder) = session(&host);
    write_lines(
        &main,
        &[assistant(
            [10, 5, 100, 1000],
            &[
                (
                    "read",
                    json!({ "path": project.root.join("a.rs").to_string_lossy() }),
                ),
                ("read", json!({ "path": "a.rs:1-5" })),
                ("grep", json!({})),
                ("bash", json!({ "command": "cat c.rs" })),
            ],
        )],
    );
    write_lines(
        &folder.join("0-Reader.jsonl"),
        &[assistant(
            [1, 2, 3, 4],
            &[("read", json!({ "path": "b.rs" })), ("glob", json!({}))],
        )],
    );
    write_lines(
        &folder.join("__advisor.turn.jsonl"),
        &[assistant(
            [7, 7, 7, 7],
            &[("read", json!({ "path": "a.rs" }))],
        )],
    );
    jobs.stop(&project.id, &id).unwrap();
    state_of(&host, &project, &id, JobState::Stopped).await;

    let line = measured(&project, &id).await;
    assert_eq!(
        line,
        json!({
            "job": id,
            "kind": "run",
            "name": "Run T1",
            "target": "aaaaaaaa",
            "state": "stopped",
            "created": T0,
            "ended": "2026-11-02T07:03:30.000Z",
            "durationMs": 210_000,
            "activeMs": 40_000,
            "main": { "reads": 2, "files": 1, "searches": 1, "inputTokens": 10, "outputTokens": 5, "cacheReadTokens": 100, "cacheWriteTokens": 1000 },
            "subAgents": { "reads": 1, "files": 1, "searches": 1, "inputTokens": 1, "outputTokens": 2, "cacheReadTokens": 3, "cacheWriteTokens": 4, "count": 1 },
            "advisors": { "reads": 1, "files": 1, "searches": 0, "inputTokens": 7, "outputTokens": 7, "cacheReadTokens": 7, "cacheWriteTokens": 7, "count": 1 },
            "files": 2,
        })
    );
    // The folder is local only, with its own `.gitignore`.
    let timings = project.root.join(".weaver").join("timings");
    assert_eq!(std::fs::read(timings.join(".gitignore")).unwrap(), b"*\n");

    // `weaver job show <id>`: the line as it is in JSON, the durations and a line per part in text.
    let authority = super::authority::AppAuthority { jobs: jobs.clone() };
    let show = |card: &str| {
        execute(
            &CommandRequest {
                object: "job".into(),
                verb: "show".into(),
                root: project.root.to_string_lossy().into_owned(),
                card: Some(card.to_owned()),
                state: None,
                input: serde_json::Map::new(),
                general: None,
                ask: None,
            },
            &Caller::default(),
            &authority,
        )
        .unwrap()
    };
    let shown = show(&id);
    assert_eq!(shown.json["job"]["measure"], line);
    assert_eq!(shown.json["job"]["task"], "aaaaaaaa");
    assert!(
        shown.markdown.contains("- duration: 3m 30s\n")
            && shown.markdown.contains("- active duration: 40s\n")
            && shown.markdown.contains(
                "- main agent: 2 reads, 1 files, 1 searches; tokens: input 10, output 5, cache read 100, cache written 1000\n"
            )
            && shown
                .markdown
                .contains("- sub-agents (1): 1 reads, 1 files, 1 searches;")
            && shown
                .markdown
                .contains("- advisors (1): 1 reads, 1 files, 0 searches;"),
        "{}",
        shown.markdown
    );

    // Without its line, the command shows the job as before.
    std::fs::remove_file(measure::timings_file(&project.root)).unwrap();
    let shown = show(&id);
    assert!(shown.json["job"].get("measure").is_none());
    assert!(!shown.markdown.contains("duration"), "{}", shown.markdown);
}

#[tokio::test]
async fn a_job_stopped_in_the_queue_is_measured_without_session_and_without_active_time() {
    let (host, clock, jobs) = with_clock();
    let (_dir, project) = host
        .project(&[("tasks/aaaaaaaa-first-task.md", TASK)])
        .await;
    host.gate.send_replace(false);
    let id = run_task(&jobs, &project);
    clock.advance(42_000);
    jobs.stop(&project.id, &id).unwrap();
    state_of(&host, &project, &id, JobState::Stopped).await;

    let line = measured(&project, &id).await;
    assert_eq!(line["state"], "stopped");
    assert_eq!(
        (line["durationMs"].as_u64(), line["activeMs"].as_u64()),
        (Some(42_000), Some(0))
    );
    assert!(line.get("activeIncomplete").is_none() && line.get("main").is_none());
    assert!(line.get("files").is_none() && line.get("advisors").is_none());
    assert_eq!(
        line["unavailable"],
        "the job has no session: the agent never opened one"
    );
    // The same line in text: the durations and the reason.
    let text = measure::describe(&line);
    assert!(
        text.contains("- duration: 42s") && text.contains("- unavailable: the job has no session")
    );
    host.gate.send_replace(true);
}

#[tokio::test]
async fn a_session_file_that_is_missing_makes_the_whole_measure_unavailable() {
    let (host, _clock, jobs) = with_clock();
    let (_dir, project) = host
        .project(&[("tasks/aaaaaaaa-first-task.md", TASK)])
        .await;
    host.script(json!({ "prompts": [[{ "hold": true }]] }));

    // The session was opened, its file is not there.
    let id = run_task(&jobs, &project);
    state_of(&host, &project, &id, JobState::Running).await;
    jobs.stop(&project.id, &id).unwrap();
    let line = measured(&project, &id).await;
    let reason = line["unavailable"].as_str().unwrap();
    assert!(
        reason.contains("`*_fake-session.jsonl`") && reason.contains("was not found"),
        "{reason}"
    );
    assert!(line.get("main").is_none() && line.get("files").is_none());
}

#[tokio::test]
async fn a_malformed_line_in_a_transcript_names_the_file() {
    let (host, _clock, jobs) = with_clock();
    let (_dir, project) = host
        .project(&[("tasks/aaaaaaaa-first-task.md", TASK)])
        .await;
    host.script(json!({ "prompts": [[{ "hold": true }]] }));
    let (main, folder) = session(&host);
    write_lines(&main, &[assistant([1, 1, 1, 1], &[])]);
    std::fs::create_dir_all(folder.join("0-A")).unwrap();
    std::fs::write(folder.join("0-A").join("1-B.jsonl"), "{}\nnot json\n").unwrap();

    let id = run_task(&jobs, &project);
    state_of(&host, &project, &id, JobState::Running).await;
    jobs.stop(&project.id, &id).unwrap();
    let line = measured(&project, &id).await;
    let reason = line["unavailable"].as_str().unwrap();
    assert!(
        reason.contains("1-B.jsonl") && reason.contains("malformed line (line 2)"),
        "{reason}"
    );
}

#[tokio::test]
async fn a_job_resumed_after_a_crash_counts_from_its_resume_and_says_the_time_is_incomplete() {
    // The first run of the app: a job that works, as the registry holds it.
    let host = TestHost::new();
    let first = Jobs::new(host.clone());
    let (_dir, project) = host
        .project(&[("tasks/aaaaaaaa-first-task.md", TASK)])
        .await;
    host.script(json!({ "prompts": [[{ "hold": true }]] }));
    let id = run_task(&first, &project);
    state_of(&host, &project, &id, JobState::Running).await;
    first.flush();
    let store = JobStore::new(&host.dirs);
    let mut record = store.read(&id).unwrap().unwrap();
    assert_eq!(record.state, JobState::Running);
    assert!(record.running.as_ref().is_some_and(|r| r.since.is_some()));
    // What the crash left: Running, 5 s counted before the last change of state, and no process.
    record.pid = None;
    record.pid_started = None;
    record.id = "crashed1".to_owned();
    record
        .env
        .insert("WEAVER_JOB".to_owned(), "crashed1".to_owned());
    record.running = Some(RunningTime {
        ms: 5_000,
        since: Some("2026-11-02T06:59:00.000Z".to_owned()),
    });
    record.created = "2026-11-02T06:58:00.000Z".to_owned();
    store.write(&record).unwrap();
    first.stop(&project.id, &id).unwrap();
    state_of(&host, &project, &id, JobState::Stopped).await;
    first.flush();

    // The next run: the time since the last change of state is not counted.
    let clock = Arc::new(ManualClock::at(T0));
    let second = Jobs::with_clock(host.clone(), clock.clone());
    host.script(json!({ "prompts": [[{ "hold": true }]] }));
    second.window_opened(&project.id, &project.root);
    until("the crashed job to work again", LIMIT, || {
        second
            .record("crashed1")
            .is_some_and(|r| r.state == JobState::Running)
    })
    .await;
    let record = second.record("crashed1").unwrap();
    assert!(record.running_incomplete);
    assert_eq!(
        record.running,
        Some(RunningTime {
            ms: 5_000,
            since: Some(T0.to_owned())
        })
    );

    clock.advance(7_000);
    second.stop(&project.id, "crashed1").unwrap();
    let line = measured(&project, "crashed1").await;
    assert_eq!(line["activeMs"], 12_000);
    assert_eq!(line["activeIncomplete"], true);
    assert_eq!(line["durationMs"], 7_000 + 2 * 60_000);
}

#[tokio::test]
async fn a_queued_job_found_after_a_crash_is_stopped_and_measured() {
    let host = TestHost::new();
    let first = Jobs::new(host.clone());
    let (_dir, project) = host
        .project(&[("tasks/aaaaaaaa-first-task.md", TASK)])
        .await;
    host.gate.send_replace(false);
    let id = run_task(&first, &project);
    first.flush();
    let store = JobStore::new(&host.dirs);
    let mut record = store.read(&id).unwrap().unwrap();
    assert_eq!(record.state, JobState::Queued);
    record.id = "queued01".to_owned();
    record
        .env
        .insert("WEAVER_JOB".to_owned(), "queued01".to_owned());
    record.created = "2026-11-02T06:59:00.000Z".to_owned();
    store.write(&record).unwrap();
    first.stop(&project.id, &id).unwrap();
    first.flush();

    let clock = Arc::new(ManualClock::at(T0));
    let second = Jobs::with_clock(host.clone(), clock);
    second.window_opened(&project.id, &project.root);
    let line = measured(&project, "queued01").await;
    assert_eq!(line["state"], "stopped");
    assert_eq!(
        (line["durationMs"].as_u64(), line["activeMs"].as_u64()),
        (Some(60_000), Some(0))
    );
    assert!(line["unavailable"].as_str().unwrap().contains("no session"));
}

#[tokio::test]
async fn the_jobs_that_are_not_measured_leave_no_line() {
    let (host, _clock, jobs) = with_clock();
    let (_dir, project) = host
        .project(&[("tasks/aaaaaaaa-first-task.md", TASK)])
        .await;
    host.script(json!({ "prompts": [[]] }));

    // An import is not measured: it ends, and nothing is written.
    let image = ProjectImage::load(&project.root);
    let import = jobs
        .start_import(
            &Ctx {
                project_id: &project.id,
                root: &project.root,
                image: &image,
            },
            Vec::new(),
        )
        .map_err(|refusal| refusal.0)
        .unwrap();
    state_of(&host, &project, &import, JobState::Done).await;
    jobs.flush();
    let record = jobs.record(&import).unwrap();
    assert_eq!(record.running, None, "an import counts no running time");
    assert!(!record.running_incomplete);

    // A Run is: its line is the only one in the file, and the import's never came.
    host.script(json!({ "prompts": [[]] }));
    let id = run_task(&jobs, &project);
    state_of(&host, &project, &id, JobState::Done).await;
    measured(&project, &id).await;
    let text = std::fs::read_to_string(measure::timings_file(&project.root)).unwrap();
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(!text.contains(&import));
}
