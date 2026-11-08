//! What the jobs need from the rest of the app, behind one trait: the folders, the settings, omp,
//! the project pictures and their mutation queue, and the events to the UI. [`TauriHost`] is the
//! real one; the tests of the jobs bring their own, so that no window and no omp is needed.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};
use weaver_core::omp::machine::ReadyOmp;
use weaver_core::paths::AppDirs;
use weaver_core::project::image::ProjectImage;
use weaver_core::settings::Settings;
use weaver_core::system_certificates::Variables;

use super::wire::{EntriesEvent, JobsSnapshot};
use crate::error::{AppError, ErrorCode};
use crate::omp::OmpSetup;
use crate::state::AppState;
use crate::subagents::wire::{SubAgentEntries, SubAgentsEvent};
use crate::windows::jobs::BoxFuture;

/// The change a mutation of the project makes: the image after its write, to be published, or
/// nothing when no file changed.
pub type MutationResult = Result<Option<ProjectImage>, String>;

pub trait Host: Send + Sync + 'static {
    fn dirs(&self) -> &AppDirs;
    /// The settings in force now.
    fn settings(&self) -> Settings;
    /// The endpoint of the CLI socket, once the server started.
    fn endpoint(&self) -> Option<String>;
    /// Waits until omp is ready and returns it.
    fn omp_ready(&self) -> BoxFuture<Arc<ReadyOmp>>;
    fn omp_is_ready(&self) -> bool;
    /// The `PATH` of a process that runs omp.
    fn child_path(&self, omp: &Path) -> OsString;
    /// `omp.MISSING`: a job could not start the agent.
    fn report_omp_missing(&self, reason: Option<String>);
    /// The agent lock of the File view: the actors tell it what the agent's tools change. Hosts
    /// of tests that do not look at it have none.
    fn file_locks(&self) -> Option<Arc<crate::file_lock::FileLockService>> {
        None
    }
    /// The names of the assets of a project now, as the app holds them: the `@` tags of a message
    /// or a request are recognised against them at the moment it is sent. Hosts of tests that do
    /// not look at assets have none.
    fn asset_names(&self, _project_id: &str) -> Vec<String> {
        Vec::new()
    }
    /// The jobs the Chat of a project started, or whose request it sent, oldest first: identity,
    /// name and the instant each was created (milliseconds since the Unix epoch). Hosts of tests
    /// that do not look at them have none.
    fn chat_jobs(&self, _project_id: &str) -> Vec<(String, String, i64)> {
        Vec::new()
    }
    /// The environment Weaver itself runs in, to be inherited by omp (minus `WEAVER_*`).
    fn inherited_env(&self) -> Vec<(OsString, OsString)> {
        std::env::vars_os().collect()
    }
    /// Whether `System Certificates` is on now: the proxy explanations of a failure say what is
    /// true at the moment it happens.
    fn system_certificates_enabled(&self) -> bool {
        self.settings().system_certificates.enabled
    }
    /// The System Certificates variables of an omp process that starts now: the setting and the
    /// bundle are read at each call, so every start and resume sees the current value. Empty with
    /// the setting off or without a bundle. Never to be kept in the job registry.
    fn certificate_variables(&self) -> Variables {
        weaver_core::system_certificates::variables(
            self.system_certificates_enabled(),
            &self.dirs().system_certificates_file(),
        )
    }
    /// The folder of omp's own session files, where background sub-agents write their records.
    fn sessions_root(&self) -> Option<PathBuf> {
        let agent = std::env::var_os("PI_CODING_AGENT_DIR")
            .map(PathBuf::from)
            .or_else(|| {
                let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
                Some(PathBuf::from(home).join(".omp").join("agent"))
            })?;
        Some(agent.join("sessions"))
    }
    fn emit_jobs(&self, snapshot: &JobsSnapshot);
    fn emit_entries(&self, event: &EntriesEvent);
    /// The list of the sub-agents of a job changed: the whole list.
    fn emit_sub_agents(&self, event: &SubAgentsEvent);
    /// The conversation of a sub-agent on screen changed.
    fn emit_sub_agent_entries(&self, event: &SubAgentEntries);
    /// What the Board shows changed with no file changing (a job started or ended, a ghost came
    /// or went): a new revision, announced for the `board` slice.
    fn board_changed(&self, project_id: &str);
    /// The current picture of an open project.
    fn picture(&self, project_id: &str) -> Option<(PathBuf, Arc<ProjectImage>)>;
    /// Runs `change` in the project's mutation queue, under the project lock, and publishes the
    /// image it returns. Fails when the project is not open.
    fn mutate(
        &self,
        project_id: &str,
        change: Box<dyn FnOnce(&Path) -> MutationResult + Send>,
    ) -> BoxFuture<Result<(), String>>;
}

/// The host of the running app.
pub struct TauriHost {
    app: AppHandle,
    dirs: AppDirs,
}

impl TauriHost {
    pub fn new(app: AppHandle) -> Self {
        let dirs = app.state::<AppState>().dirs.clone();
        Self { app, dirs }
    }

    fn state(&self) -> tauri::State<'_, AppState> {
        self.app.state::<AppState>()
    }
}

impl Host for TauriHost {
    fn dirs(&self) -> &AppDirs {
        &self.dirs
    }

    fn settings(&self) -> Settings {
        self.state().settings.current()
    }

    fn endpoint(&self) -> Option<String> {
        self.state()
            .server
            .lock()
            .expect("server slot poisoned")
            .as_ref()
            .map(|server| server.endpoint().to_owned())
    }

    fn omp_ready(&self) -> BoxFuture<Arc<ReadyOmp>> {
        let omp = self.app.state::<OmpSetup>().inner().clone();
        Box::pin(async move { omp.wait_ready().await })
    }

    fn omp_is_ready(&self) -> bool {
        self.app.state::<OmpSetup>().ready().is_some()
    }

    fn child_path(&self, omp: &Path) -> OsString {
        self.app.state::<OmpSetup>().child_path(omp)
    }

    fn report_omp_missing(&self, reason: Option<String>) {
        self.app.state::<OmpSetup>().report_missing(reason);
    }

    fn file_locks(&self) -> Option<Arc<crate::file_lock::FileLockService>> {
        Some(self.state().file_locks.clone())
    }

    fn asset_names(&self, project_id: &str) -> Vec<String> {
        self.state()
            .assets
            .assets(project_id)
            .iter()
            .map(|asset| asset.name.clone())
            .collect()
    }

    fn chat_jobs(&self, project_id: &str) -> Vec<(String, String, i64)> {
        let Some(jobs) = self.app.try_state::<Arc<super::Jobs>>() else {
            return Vec::new();
        };
        jobs.chat_jobs(project_id)
            .into_iter()
            .map(|record| {
                let at = weaver_core::project::clock::parse_instant(&record.created).unwrap_or(0);
                (record.id, record.name, at)
            })
            .collect()
    }

    fn emit_jobs(&self, snapshot: &JobsSnapshot) {
        let _ = self
            .app
            .emit(crate::events::EventName::JobsChanged.as_str(), snapshot);
        // The project switchers show how many jobs work in each project.
        crate::windows::switcher::schedule(&self.app);
    }

    fn emit_entries(&self, event: &EntriesEvent) {
        let _ = self
            .app
            .emit(crate::events::EventName::JobEntries.as_str(), event);
    }

    fn emit_sub_agents(&self, event: &SubAgentsEvent) {
        let _ = self
            .app
            .emit(crate::events::EventName::SubAgentsChanged.as_str(), event);
    }

    fn emit_sub_agent_entries(&self, event: &SubAgentEntries) {
        let _ = self
            .app
            .emit(crate::events::EventName::SubAgentEntries.as_str(), event);
    }

    fn board_changed(&self, project_id: &str) {
        let state = self.state();
        // A project whose window is closed has no picture: nothing to tell.
        let _ = crate::board::publish_agent_change(&self.app, &state.pictures, project_id);
    }

    fn picture(&self, project_id: &str) -> Option<(PathBuf, Arc<ProjectImage>)> {
        self.state()
            .pictures
            .get(project_id)
            .ok()
            .map(|picture| (picture.root, picture.image))
    }

    fn mutate(
        &self,
        project_id: &str,
        change: Box<dyn FnOnce(&Path) -> MutationResult + Send>,
    ) -> BoxFuture<Result<(), String>> {
        let app = self.app.clone();
        let id = project_id.to_owned();
        Box::pin(async move {
            let state = app.state::<AppState>();
            let root = state.pictures.get(&id).map_err(|error| error.message)?.root;
            let pictures = state.pictures.clone();
            let published_id = id.clone();
            let revision = state
                .mutations
                .run(&id, move || {
                    let image = change(&root)
                        .map_err(|message| AppError::new(ErrorCode::Internal, message))?;
                    match image {
                        Some(image) => pictures.publish(&published_id, image),
                        None => Ok(None),
                    }
                })
                .await
                .map_err(|error| error.message)?;
            if let Some(revision) = revision {
                let _ = crate::events::emit_project_changed(&app, &revision.event());
            }
            Ok(())
        })
    }
}
