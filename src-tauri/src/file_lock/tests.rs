//! The agent lock against the scripted fake ACP agent: tools with and without `locations`, several
//! tools on one file, tools of two conversations, the end of a tool by `completed` and `failed`,
//! of a turn, of omp, of a job, of a cleared General and of a closed Ask more conversation,
//! updates replayed while a session loads, a move with its destination, a deletion and paths
//! outside the project. The lock itself is in `weaver-core` (`file_lock`); here the app feeds it.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{json, Value};
use weaver_core::jobs::JobState;
use weaver_core::project::image::ProjectImage;

use crate::file_tabs::wire::FileLockChanged;
use crate::general::service::{AskOpening, General};
use crate::general::wire::{
    AskClosed, AskEntries, AskKind, AskRow, GeneralEntries, GeneralEvents, GeneralState,
};
use crate::jobs::conversation::{Entry, ToolStatus};
use crate::jobs::starts::Ctx;
use crate::jobs::testing::{row, text_update, until, Project, TestHost, LIMIT, TASK};
use crate::jobs::Jobs;

/// General's events are not what these tests look at.
struct Quiet;

impl GeneralEvents for Quiet {
    fn cards(&self, _: &crate::general::cards::ChatCards) {}
    fn state(&self, _: &GeneralState) {}
    fn entries(&self, _: &GeneralEntries) {}
    fn ask_changed(&self, _: &AskRow) {}
    fn ask_entries(&self, _: &AskEntries) {}
    fn ask_closed(&self, _: &AskClosed) {}
}

struct Fx {
    host: Arc<TestHost>,
    jobs: Arc<Jobs>,
    general: Arc<General>,
    _dir: tempfile::TempDir,
    project: Project,
    /// A file of the project that a tab has open.
    file: PathBuf,
    /// Another one.
    other: PathBuf,
    gate: PathBuf,
}

async fn fx() -> Fx {
    let host = TestHost::new();
    let jobs = Jobs::new(host.clone());
    let general = General::new(host.clone(), Arc::new(Quiet));
    let (dir, project) = host
        .project(&[("tasks/aaaaaaaa-first-task.md", TASK)])
        .await;
    general.open(&project.id, &project.root).await;
    let file = project.root.join("src").join("a.rs");
    let other = project.root.join("src").join("b.rs");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "a\n").unwrap();
    fs::write(&other, "b\n").unwrap();
    let gate = project.root.join("gate");
    Fx {
        host,
        jobs,
        general,
        _dir: dir,
        project,
        file,
        other,
        gate,
    }
}

impl Fx {
    fn open_tab(&self, file: &Path) -> bool {
        self.host
            .file_locks
            .file_opened(&self.project.id, &self.project.root, file.to_path_buf())
    }

    fn locked(&self, file: &Path) -> bool {
        self.host.file_locks.is_locked(&self.project.id, file)
    }

    async fn becomes_locked(&self, file: &Path) {
        until("the file to be locked", LIMIT, || self.locked(file)).await;
    }

    async fn becomes_free(&self, file: &Path) {
        until("the file to be free", LIMIT, || !self.locked(file)).await;
    }

    /// Lets the script go on past `{"wait_file": gate}`.
    fn open_gate(&self) {
        fs::write(&self.gate, "go").unwrap();
    }

    fn events(&self) -> Vec<FileLockChanged> {
        self.host.lock_events.locks.lock().unwrap().clone()
    }

    fn run_task(&self) -> String {
        let image = ProjectImage::load(&self.project.root);
        self.jobs
            .start_run_task(
                &Ctx {
                    project_id: &self.project.id,
                    root: &self.project.root,
                    image: &image,
                },
                "aaaaaaaa",
            )
            .map_err(|refusal| refusal.0)
            .unwrap()
    }

    /// Waits until the conversation of the job shows the tool `call` as done.
    async fn tool_over(&self, job: &str, call: &str) {
        until(&format!("tool {call} to be done"), LIMIT, || {
            self.jobs
                .conversation(&self.project.id, job)
                .is_ok_and(|c| {
                    c.entries
                        .iter()
                        .any(|e| matches!(e, Entry::Tool(t) if t.id == call && t.status == ToolStatus::Done))
                })
        })
        .await;
    }

    async fn state(&self, job: &str, wanted: JobState) {
        until(&format!("job {job} to be {wanted:?}"), LIMIT, || {
            row(&self.host, &self.project.id, job).is_some_and(|r| r.state == wanted)
        })
        .await;
    }
}

fn tool_call(call: &str, kind: &str, status: &str, locations: &[&Path]) -> Value {
    let locations: Vec<Value> = locations.iter().map(|p| json!({ "path": p })).collect();
    json!({ "update": {
        "sessionUpdate": "tool_call", "toolCallId": call, "title": "Edit", "kind": kind,
        "status": status, "locations": locations,
    } })
}

fn tool_done(call: &str, status: &str) -> Value {
    json!({ "update": { "sessionUpdate": "tool_call_update", "toolCallId": call, "status": status } })
}

fn gate(fx: &Fx) -> Value {
    json!({ "wait_file": fx.gate })
}

fn script(fx: &Fx, steps: Vec<Value>) {
    fx.host
        .script(json!({ "log": fx.host.received_log(), "prompts": [steps] }));
}

#[tokio::test]
async fn a_tool_that_names_a_file_locks_it_until_it_completes_and_the_job_goes_on() {
    let fx = fx().await;
    assert!(!fx.open_tab(&fx.file));
    assert!(!fx.open_tab(&fx.other));
    script(
        &fx,
        vec![
            tool_call("t1", "edit", "in_progress", &[&fx.file]),
            gate(&fx),
            tool_done("t1", "completed"),
            json!({ "hold": true }),
        ],
    );
    let job = fx.run_task();
    fx.becomes_locked(&fx.file).await;
    assert!(!fx.locked(&fx.other), "the tool names one file only");
    // The event follows the lock: it is sent once the lock is taken, not with it.
    until("the lock event", LIMIT, || !fx.events().is_empty()).await;
    assert_eq!(
        fx.events()
            .iter()
            .map(|e| (e.path.as_str(), e.locked))
            .collect::<Vec<_>>(),
        vec![("src/a.rs", true)]
    );

    fx.open_gate();
    fx.becomes_free(&fx.file).await;
    until("the release event", LIMIT, || fx.events().len() == 2).await;
    assert_eq!(
        fx.events()
            .iter()
            .map(|e| (e.path.as_str(), e.locked))
            .collect::<Vec<_>>(),
        vec![("src/a.rs", true), ("src/a.rs", false)]
    );
    fx.state(&job, JobState::Running).await;
    fx.jobs.stop(&fx.project.id, &job).unwrap();
    fx.state(&job, JobState::Stopped).await;
}

#[tokio::test]
async fn a_tool_that_fails_releases_the_lock_like_one_that_completes() {
    let fx = fx().await;
    fx.open_tab(&fx.file);
    script(
        &fx,
        vec![
            tool_call("t1", "edit", "pending", &[&fx.file]),
            gate(&fx),
            tool_done("t1", "failed"),
            json!({ "hold": true }),
        ],
    );
    let job = fx.run_task();
    fx.becomes_locked(&fx.file).await;
    fx.open_gate();
    fx.becomes_free(&fx.file).await;
    fx.jobs.stop(&fx.project.id, &job).unwrap();
    fx.state(&job, JobState::Stopped).await;
}

#[tokio::test]
async fn a_tool_without_locations_locks_every_open_file_and_later_locations_change_nothing() {
    let fx = fx().await;
    fx.open_tab(&fx.file);
    fx.open_tab(&fx.other);
    script(
        &fx,
        vec![
            tool_call("t1", "edit", "in_progress", &[]),
            json!({ "update": { "sessionUpdate": "tool_call_update", "toolCallId": "t1", "locations": [{ "path": fx.file }] } }),
            gate(&fx),
            tool_done("t1", "completed"),
            json!({ "hold": true }),
        ],
    );
    let job = fx.run_task();
    fx.becomes_locked(&fx.file).await;
    fx.becomes_locked(&fx.other).await;
    fx.open_gate();
    fx.becomes_free(&fx.file).await;
    fx.becomes_free(&fx.other).await;
    fx.jobs.stop(&fx.project.id, &job).unwrap();
    fx.state(&job, JobState::Stopped).await;
}

#[tokio::test]
async fn a_file_opened_while_the_tool_runs_opens_locked() {
    let fx = fx().await;
    script(
        &fx,
        vec![
            tool_call("t1", "edit", "in_progress", &[&fx.file]),
            json!({ "hold": true }),
        ],
    );
    let job = fx.run_task();
    until("the tool to count", LIMIT, || {
        fx.host
            .received()
            .iter()
            .any(|m| m["method"] == "session/prompt")
    })
    .await;
    until("a tab to open locked", LIMIT, || {
        fx.host
            .file_locks
            .file_opened(&fx.project.id, &fx.project.root, fx.file.clone())
    })
    .await;
    assert!(!fx.open_tab(&fx.other));
    fx.jobs.stop(&fx.project.id, &job).unwrap();
    fx.state(&job, JobState::Stopped).await;
    assert!(!fx.locked(&fx.file), "the job ended");
}

#[tokio::test]
async fn only_tools_that_change_files_count() {
    let fx = fx().await;
    fx.open_tab(&fx.file);
    script(
        &fx,
        vec![
            tool_call("t1", "execute", "in_progress", &[&fx.file]),
            tool_call("t2", "other", "in_progress", &[&fx.file]),
            tool_call("t3", "read", "in_progress", &[&fx.file]),
            // The `task` tool of the sub-agents never counts, whatever its kind says.
            json!({ "update": { "sessionUpdate": "tool_call", "toolCallId": "t4", "title": "Task: fix a.rs", "kind": "edit",
                "status": "in_progress", "locations": [{ "path": fx.file }],
                "rawInput": { "tasks": [{ "id": "A", "assignment": "x" }] } } }),
            // A tool that is an edit, to know the others were read: it locks the other file's twin.
            tool_call("t5", "edit", "in_progress", &[&fx.other]),
            json!({ "hold": true }),
        ],
    );
    let job = fx.run_task();
    fx.open_tab(&fx.other);
    until("the edit to be read", LIMIT, || fx.locked(&fx.other)).await;
    assert!(!fx.locked(&fx.file));
    assert!(fx.events().iter().all(|e| e.path != "src/a.rs"));
    fx.jobs.stop(&fx.project.id, &job).unwrap();
    fx.state(&job, JobState::Stopped).await;
}

#[tokio::test]
async fn paths_outside_the_project_never_count() {
    let fx = fx().await;
    fx.open_tab(&fx.file);
    let outside = tempfile::tempdir().unwrap();
    let elsewhere = weaver_core::paths::canonical_root(outside.path())
        .unwrap()
        .join("a.rs");
    fs::write(&elsewhere, "x").unwrap();
    script(
        &fx,
        vec![
            tool_call("t1", "edit", "in_progress", &[&elsewhere]),
            tool_call("t2", "edit", "in_progress", &[&fx.other]),
            json!({ "hold": true }),
        ],
    );
    let job = fx.run_task();
    fx.open_tab(&fx.other);
    fx.becomes_locked(&fx.other).await;
    assert!(
        !fx.locked(&fx.file),
        "a tool that names only a file outside the project counts for no file"
    );
    fx.jobs.stop(&fx.project.id, &job).unwrap();
    fx.state(&job, JobState::Stopped).await;
}

#[tokio::test]
async fn several_tools_on_one_file_hold_it_until_the_last_one_ends() {
    let fx = fx().await;
    fx.open_tab(&fx.file);
    script(
        &fx,
        vec![
            tool_call("t1", "edit", "in_progress", &[&fx.file]),
            tool_call("t2", "edit", "in_progress", &[&fx.file]),
            tool_done("t1", "completed"),
            json!({ "wait_file": fx.gate }),
            tool_done("t2", "completed"),
            json!({ "hold": true }),
        ],
    );
    let job = fx.run_task();
    fx.becomes_locked(&fx.file).await;
    fx.tool_over(&job, "t1").await;
    assert!(fx.locked(&fx.file), "t2 still counts");
    fx.open_gate();
    fx.becomes_free(&fx.file).await;
    assert_eq!(
        fx.events().iter().filter(|e| e.locked).count(),
        1,
        "locked once"
    );
    fx.jobs.stop(&fx.project.id, &job).unwrap();
    fx.state(&job, JobState::Stopped).await;
}

#[tokio::test]
async fn the_end_of_the_turn_the_failure_of_the_turn_and_the_exit_of_omp_release_the_lock() {
    for ending in [
        json!({ "end": "end_turn" }),
        json!({ "error": "the turn failed" }),
        json!({ "exit": 1 }),
    ] {
        let fx = fx().await;
        fx.open_tab(&fx.file);
        script(
            &fx,
            vec![
                tool_call("t1", "edit", "in_progress", &[&fx.file]),
                gate(&fx),
                ending.clone(),
            ],
        );
        let job = fx.run_task();
        fx.becomes_locked(&fx.file).await;
        fx.open_gate();
        fx.becomes_free(&fx.file).await;
        until("the job to end", LIMIT, || {
            row(&fx.host, &fx.project.id, &job).is_some_and(|r| r.state.is_final())
        })
        .await;
        assert!(!fx.locked(&fx.file), "{ending}");
    }
}

#[tokio::test]
async fn stopping_the_job_releases_what_its_agent_held() {
    let fx = fx().await;
    fx.open_tab(&fx.file);
    script(
        &fx,
        vec![
            tool_call("t1", "edit", "in_progress", &[]),
            json!({ "hold": true }),
        ],
    );
    let job = fx.run_task();
    fx.becomes_locked(&fx.file).await;
    fx.jobs.stop(&fx.project.id, &job).unwrap();
    fx.state(&job, JobState::Stopped).await;
    fx.becomes_free(&fx.file).await;
}

#[tokio::test]
async fn the_updates_replayed_while_a_session_loads_never_lock_anything() {
    let fx = fx().await;
    fx.open_tab(&fx.file);
    fx.host.script(json!({
        "log": fx.host.received_log(),
        "prompts": [[json!({ "update": text_update("agent_message_chunk", "Before") }), { "hold": true }]],
    }));
    let job = fx.run_task();
    fx.state(&job, JobState::Running).await;
    fx.jobs.window_closed(&fx.project.id).await;
    // The reloaded session replays an edit that never ended: it is history, not a tool at work.
    fx.host.script(json!({
        "log": fx.host.received_log(),
        "replay": [
            tool_call("old", "edit", "in_progress", &[&fx.file])["update"].clone(),
            tool_call("older", "edit", "in_progress", &[])["update"].clone(),
        ],
        "prompts": [[json!({ "update": text_update("agent_message_chunk", "After") })]],
    }));
    fx.jobs.window_opened(&fx.project.id, &fx.project.root);
    fx.state(&job, JobState::Done).await;
    assert!(!fx.locked(&fx.file));
    assert!(fx.events().is_empty(), "{:?}", fx.events());
}

#[tokio::test]
async fn a_move_carries_its_destination_and_a_deletion_none() {
    let fx = fx().await;
    fx.open_tab(&fx.file);
    fx.open_tab(&fx.other);
    let moved = fx.project.root.join("moved").join("a.rs");
    script(
        &fx,
        vec![
            tool_call("t1", "edit", "in_progress", &[&fx.file, &fx.other]),
            gate(&fx),
            json!({ "update": { "sessionUpdate": "tool_call_update", "toolCallId": "t1", "status": "completed",
                "locations": [{ "path": fx.file }, { "path": fx.other }, { "path": moved }],
                "rawOutput": { "details": { "sourcePath": fx.file } } } }),
            json!({ "hold": true }),
        ],
    );
    let job = fx.run_task();
    fx.becomes_locked(&fx.file).await;
    fx.becomes_locked(&fx.other).await;
    // The agent moves one file and deletes the other, then reports.
    fs::create_dir_all(moved.parent().unwrap()).unwrap();
    fs::rename(&fx.file, &moved).unwrap();
    fs::remove_file(&fx.other).unwrap();
    fx.open_gate();
    fx.becomes_free(&fx.other).await;
    until("both releases", LIMIT, || {
        fx.events().iter().filter(|e| !e.locked).count() == 2
    })
    .await;
    let released: Vec<_> = fx
        .events()
        .into_iter()
        .filter(|e| !e.locked)
        .map(|e| (e.path, e.moved_to))
        .collect();
    assert!(
        released.contains(&("src/a.rs".to_owned(), Some("moved/a.rs".to_owned()))),
        "{released:?}"
    );
    assert!(
        released.contains(&("src/b.rs".to_owned(), None)),
        "{released:?}"
    );
    fx.jobs.stop(&fx.project.id, &job).unwrap();
    fx.state(&job, JobState::Stopped).await;
}

#[tokio::test]
async fn tools_of_two_conversations_keep_the_lock_until_both_are_over() {
    let fx = fx().await;
    fx.open_tab(&fx.file);
    fx.host.script(json!({
        "log": fx.host.received_log(),
        "prompts": [[tool_call("g1", "edit", "in_progress", &[&fx.file]), { "hold": true }]],
    }));
    fx.general.send_message(&fx.project.id, "edit it").unwrap();
    fx.becomes_locked(&fx.file).await;

    // A job's tool on the same file, from another omp with its own script.
    fx.host.script(json!({
        "log": fx.host.received_log(),
        "prompts": [[tool_call("j1", "edit", "in_progress", &[&fx.file]), gate(&fx), tool_done("j1", "completed"), { "hold": true }]],
    }));
    let job = fx.run_task();
    until("the job's tool to run", LIMIT, || {
        fx.host
            .received()
            .iter()
            .filter(|m| m["method"] == "session/prompt")
            .count()
            >= 2
    })
    .await;
    fx.open_gate();
    fx.tool_over(&job, "j1").await;
    assert!(fx.locked(&fx.file), "General's tool still counts");
    assert_eq!(fx.events().iter().filter(|e| e.locked).count(), 1);

    fx.general.clear(&fx.project.id).unwrap();
    fx.jobs.stop(&fx.project.id, &job).unwrap();
    fx.becomes_free(&fx.file).await;
}

#[tokio::test]
async fn clearing_general_releases_what_its_agent_held() {
    let fx = fx().await;
    fx.open_tab(&fx.file);
    fx.host.script(json!({
        "log": fx.host.received_log(),
        "prompts": [[tool_call("g1", "edit", "in_progress", &[]), { "hold": true }]],
    }));
    fx.general.send_message(&fx.project.id, "edit").unwrap();
    fx.becomes_locked(&fx.file).await;
    fx.general.clear(&fx.project.id).unwrap();
    fx.becomes_free(&fx.file).await;
}

#[tokio::test]
async fn closing_an_ask_more_conversation_releases_what_its_agent_held() {
    let fx = fx().await;
    fx.open_tab(&fx.file);
    fx.host.script(json!({
        "log": fx.host.received_log(),
        "prompts": [[tool_call("a1", "edit", "in_progress", &[&fx.file]), { "hold": true }]],
    }));
    let id = fx.general.open_ask(AskOpening {
        project_id: fx.project.id.clone(),
        root: fx.project.root.clone(),
        name: "Ask T1".to_owned(),
        kind: AskKind::Task,
        round: None,
        card: None,
        instruction: "Weaver: Ask more\n".to_owned(),
        rebuild: Arc::new(|| None),
    });
    fx.general.send_message(&id, "change it").unwrap();
    fx.becomes_locked(&fx.file).await;
    fx.general.close_ask(&id).unwrap().await;
    fx.becomes_free(&fx.file).await;
}

/// The events of a job whose conversation nobody listens to are harmless: no host, no lock.
#[test]
fn a_service_with_no_open_file_keeps_nothing_after_its_tools_end() {
    let events = Arc::new(crate::file_tabs::events::FileEvents::default());
    let service = crate::file_lock::FileLockService::new(events);
    let root = tempfile::tempdir().unwrap();
    let root = weaver_core::paths::canonical_root(root.path()).unwrap();
    let update =
        json!({ "sessionUpdate": "tool_call", "toolCallId": "t", "kind": "edit", "locations": [] });
    service.tool_update("p", &root, "job:1", &update);
    assert!(
        service.projects.lock().unwrap().contains_key("p"),
        "a running tool is kept"
    );
    service.conversation_ended("p", "job:1");
    assert!(service.projects.lock().unwrap().is_empty());
}
