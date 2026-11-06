//! Support for the tests of the jobs: a [`Host`] with no window and no omp, and the scripted fake
//! ACP agent (`crates/fake-acp`) in the place of omp. No test starts a real omp or uses the
//! network.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use weaver_core::omp::machine::ReadyOmp;
use weaver_core::paths::AppDirs;
use weaver_core::project::image::ProjectImage;
use weaver_core::settings::{Permissions, Settings};

use super::host::{Host, MutationResult};
use super::wire::{EntriesEvent, JobRow, JobsSnapshot};
use crate::file_lock::FileLockService;
use crate::file_tabs::events::testing::{recording, Recorder as FileEventsRecorder};
use crate::mutations::MutationQueues;
use crate::pictures::Pictures;
use crate::subagents::wire::{SubAgentEntries, SubAgentsEvent};
use crate::windows::jobs::BoxFuture;

/// The binary of the fake agent, built once by a nested `cargo build` into its own target folder
/// (the workspace's build folder is locked by the test run itself).
pub fn fake_agent() -> PathBuf {
    static BUILT: LazyLock<PathBuf> = LazyLock::new(|| {
        let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("the workspace");
        let target = workspace.join("target").join("fake-agent");
        let status =
            std::process::Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
                .current_dir(workspace)
                .args(["build", "--package", "fake-acp-agent", "--target-dir"])
                .arg(&target)
                .env_remove("CARGO_ENCODED_RUSTFLAGS")
                .status()
                .expect("cargo runs");
        assert!(status.success(), "the fake agent builds");
        target.join("debug").join(if cfg!(windows) {
            "fake-acp-agent.exe"
        } else {
            "fake-acp-agent"
        })
    });
    BUILT.clone()
}

/// A project in a temporary folder, with its picture open.
pub struct Project {
    pub root: PathBuf,
    pub id: String,
}

pub struct TestHost {
    pub dirs: AppDirs,
    pub settings: Mutex<Settings>,
    pub script: Mutex<PathBuf>,
    pub pictures: Arc<Pictures>,
    queues: Arc<MutationQueues>,
    pub snapshots: Mutex<Vec<JobsSnapshot>>,
    pub entries: Mutex<Vec<EntriesEvent>>,
    pub lists: Mutex<Vec<SubAgentsEvent>>,
    pub agent_entries: Mutex<Vec<SubAgentEntries>>,
    pub missing: Mutex<Vec<Option<String>>>,
    pub board_changes: Mutex<usize>,
    pub ready: Mutex<bool>,
    /// While `false`, omp does not become ready: the jobs stay Queued until it opens.
    pub gate: tokio::sync::watch::Sender<bool>,
    /// The agent lock of the File view, fed by the actors of the jobs and of General.
    pub file_locks: Arc<FileLockService>,
    /// What the agent lock told the UI.
    pub lock_events: Arc<FileEventsRecorder>,
    /// The names of the assets of every project, for the `@` tags.
    pub assets: Mutex<Vec<String>>,
    _base: tempfile::TempDir,
}

impl TestHost {
    pub fn new() -> Arc<TestHost> {
        let base = tempfile::tempdir().unwrap();
        let dirs =
            AppDirs::from_bases(&base.path().join("config"), &base.path().join("data"), None);
        std::fs::create_dir_all(dirs.local_data_dir()).unwrap();
        let (file_events, lock_events) = recording();
        Arc::new(TestHost {
            file_locks: FileLockService::new(file_events),
            lock_events,
            queues: Arc::new(MutationQueues::new(dirs.clone())),
            dirs,
            settings: Mutex::new(Settings::default()),
            script: Mutex::new(base.path().join("script.json")),
            pictures: Arc::new(Pictures::new()),
            snapshots: Mutex::default(),
            entries: Mutex::default(),
            lists: Mutex::default(),
            agent_entries: Mutex::default(),
            missing: Mutex::default(),
            board_changes: Mutex::new(0),
            ready: Mutex::new(true),
            gate: tokio::sync::watch::channel(true).0,
            _base: base,
            assets: Mutex::default(),
        })
    }

    /// Writes the script the fake agent follows.
    pub fn script(&self, script: Value) {
        let path = self.script.lock().unwrap().clone();
        std::fs::write(&path, script.to_string()).unwrap();
    }

    /// The path the agent logs what it receives to.
    pub fn received_log(&self) -> PathBuf {
        self.script.lock().unwrap().with_file_name("received.jsonl")
    }

    /// The lines the agent received, parsed (the answers it logged too).
    pub fn received(&self) -> Vec<Value> {
        std::fs::read_to_string(self.received_log())
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect()
    }

    pub fn permissions(&self, permissions: Permissions) {
        self.settings.lock().unwrap().permissions = permissions;
    }

    /// A project with `.weaver/` and the files given (path under `.weaver/`, content).
    pub async fn project(&self, files: &[(&str, &str)]) -> (tempfile::TempDir, Project) {
        let dir = tempfile::tempdir().unwrap();
        for (rel, content) in files {
            let path = dir.path().join(".weaver").join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, content).unwrap();
        }
        std::fs::create_dir_all(dir.path().join(".weaver")).unwrap();
        let root = weaver_core::paths::canonical_root(dir.path()).unwrap();
        let id = self.pictures.open(root.clone()).await.unwrap();
        (dir, Project { root, id })
    }

    /// The last snapshot of the project's list.
    pub fn last_snapshot(&self, project: &str) -> Option<JobsSnapshot> {
        self.snapshots
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|s| s.project_id == project)
            .cloned()
    }
}

impl Host for TestHost {
    fn dirs(&self) -> &AppDirs {
        &self.dirs
    }

    fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    fn endpoint(&self) -> Option<String> {
        Some("/tmp/weaver-test.sock".to_owned())
    }

    fn omp_ready(&self) -> BoxFuture<Arc<ReadyOmp>> {
        let mut gate = self.gate.subscribe();
        Box::pin(async move {
            let _ = gate.wait_for(|open| *open).await;
            Arc::new(ReadyOmp {
                omp: fake_agent(),
                omp_path: String::new(),
                version: "18.15.0".to_owned(),
                models: Vec::new(),
            })
        })
    }

    fn omp_is_ready(&self) -> bool {
        *self.ready.lock().unwrap()
    }

    fn child_path(&self, _omp: &Path) -> OsString {
        std::env::var_os("PATH").unwrap_or_default()
    }

    fn report_omp_missing(&self, reason: Option<String>) {
        self.missing.lock().unwrap().push(reason);
    }

    fn file_locks(&self) -> Option<Arc<FileLockService>> {
        Some(self.file_locks.clone())
    }

    fn asset_names(&self, _project_id: &str) -> Vec<String> {
        self.assets.lock().unwrap().clone()
    }

    fn inherited_env(&self) -> Vec<(OsString, OsString)> {
        let script = self.script.lock().unwrap().clone();
        vec![
            (OsString::from("FAKE_ACP_SCRIPT"), script.into_os_string()),
            // A variable of Weaver's own environment that omp must never see.
            (OsString::from("WEAVER_JOB"), OsString::from("inherited")),
        ]
    }

    fn sessions_root(&self) -> Option<PathBuf> {
        Some(self.dirs.local_data_dir().join("omp-sessions"))
    }

    fn emit_jobs(&self, snapshot: &JobsSnapshot) {
        self.snapshots.lock().unwrap().push(snapshot.clone());
    }

    fn emit_entries(&self, event: &EntriesEvent) {
        self.entries.lock().unwrap().push(event.clone());
    }

    fn emit_sub_agents(&self, event: &SubAgentsEvent) {
        self.lists.lock().unwrap().push(event.clone());
    }

    fn emit_sub_agent_entries(&self, event: &SubAgentEntries) {
        self.agent_entries.lock().unwrap().push(event.clone());
    }

    fn board_changed(&self, _project_id: &str) {
        *self.board_changes.lock().unwrap() += 1;
    }

    fn picture(&self, project_id: &str) -> Option<(PathBuf, Arc<ProjectImage>)> {
        self.pictures
            .get(project_id)
            .ok()
            .map(|p| (p.root, p.image))
    }

    fn mutate(
        &self,
        project_id: &str,
        change: Box<dyn FnOnce(&Path) -> MutationResult + Send>,
    ) -> BoxFuture<Result<(), String>> {
        let pictures = self.pictures.clone();
        let queues = self.queues.clone();
        let id = project_id.to_owned();
        Box::pin(async move {
            let run_id = id.clone();
            queues
                .run(&id, move || {
                    let root = pictures.get(&run_id)?.root;
                    let image = change(&root).map_err(|message| {
                        crate::error::AppError::new(crate::error::ErrorCode::Internal, message)
                    })?;
                    match image {
                        Some(image) => pictures.publish(&run_id, image),
                        None => Ok(None),
                    }
                })
                .await
                .map(|_| ())
                .map_err(|error| error.message)
        })
    }
}

/// Waits until `check` holds (polled every 10 ms), failing the test after `limit`. A bound on a
/// broken test, never a synchronisation of a passing one.
pub async fn until(what: &str, limit: Duration, mut check: impl FnMut() -> bool) {
    let started = Instant::now();
    while !check() {
        assert!(started.elapsed() < limit, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

pub const LIMIT: Duration = Duration::from_secs(30);

/// The row of a job in the last snapshot of its project.
pub fn row(host: &TestHost, project: &str, job: &str) -> Option<JobRow> {
    host.last_snapshot(project)?
        .jobs
        .into_iter()
        .find(|r| r.id == job)
}

pub fn text_update(kind: &str, text: &str) -> Value {
    json!({ "sessionUpdate": kind, "content": { "type": "text", "text": text } })
}

/// A Task file in To do.
pub const TASK: &str = "---\ncolumn: todo\ntechnical: true\nrefs: []\ncreated: 2026-11-02T07:00:01.000Z\n---\n# First task\n\nDone when.\n";
