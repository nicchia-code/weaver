//! omp in the app: the one Machine `omp-setup`, the processes it asks for, the timers it needs and
//! the commands and event of the UI (`requirements/tech#omp`, `requirements/tech#omp-setup`).
//!
//! The Machine itself is `weaver_core::omp::machine` (pure: events in, effects out). This module
//! is the glue: [`OmpSetup`] holds it, runs each [`Effect`] on the blocking pool, turns what comes
//! back into an [`Event`], keeps the 3 s re-check timer alive only while the Machine wants it,
//! tells every window ([`events::emit_omp_changed`]) and tells the backend whether omp is ready.
//!
//! # For the jobs
//!
//! Nothing that starts omp (a job, a General conversation) may start before the Machine is Ready:
//!
//! ```ignore
//! let omp = app.state::<OmpSetup>();
//! let ready = omp.wait_ready().await;              // `Arc<ReadyOmp>`: omp, version, models
//! let path = omp.child_path(&ready.omp);           // the PATH every omp process gets
//! let settings = app.state::<AppState>().settings.current();
//! // ...start `ready.omp` with PATH = path; when it cannot be started at all:
//! omp.report_missing(Some("could not start omp".into()));
//! ```
//!
//! `omp.ready()` answers at once (`None` while not Ready); `omp.subscribe()` is a watch of the same.
//! A job created while omp is not ready waits on `wait_ready`; one that is already working is not
//! touched by a later state change.

use std::any::Any;
use std::ffi::OsString;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use tauri::{AppHandle, Manager, State};
use tokio::sync::watch;
use weaver_core::omp::github::{self, DownloadOutcome, DownloadRun};
use weaver_core::omp::installer;
use weaver_core::omp::lookup::{LookupEnv, Os};
use weaver_core::omp::machine::{
    Effect, Event, Machine, OmpStatus, ReadyOmp, Ticket, RECHECK_EVERY,
};
use weaver_core::omp::path_env;
use weaver_core::omp::probe::{self, ProbeResult};
use weaver_core::omp::providers::{provider, Provider};
use weaver_core::omp::run::{which, CertificateSource, Certificates, SystemExec};
use weaver_core::omp::terminal::{self, Task};
use weaver_core::omp::usage::{self, connected_providers, Usage};
use weaver_core::process::timed::Failure;
use weaver_core::project::clock::{Clock, SystemClock};
use weaver_core::settings::SettingKey;
use weaver_core::system_certificates::EXPORT_LIMIT;

use crate::error::{AppError, ErrorCode};
use crate::system_certificates::{self, StartupExport, SystemCertificates};
use crate::usage::{Started, UsageState, UsageStatus};
use crate::{blocking, events, log_info, log_warn};

/// The first look for a newer omp, after start, and how often the app asks whether one is due.
/// The Machine lets at most one check through per hour.
const FIRST_UPDATE_CHECK: Duration = Duration::from_secs(60);
const UPDATE_CHECK_TICK: Duration = Duration::from_secs(10 * 60);

/// The terminal could not be opened: why, and the command the developer can copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFailure {
    pub reason: String,
    pub command: String,
}

/// What the Machine asks the system to do. The real one runs processes; the tests use a fake.
pub trait Ops: Send + Sync + 'static {
    /// The lookup, `omp --version` and `omp models --json` for the `ompPath` setting.
    fn probe(&self, setting: &str) -> ProbeResult;
    /// The same questions to an omp already found.
    fn probe_found(&self, omp: &Path) -> ProbeResult;
    /// `omp update` (5 min) for the omp at `omp`, found with the `ompPath` setting `setting`
    /// (trimmed); when it fails and the omp is in a folder of its installer, then the download of
    /// the latest GitHub release in its place. On failure, the reason: the one of `omp update`,
    /// explained for a proxy problem, or the one of the download from GitHub when it ran.
    fn update(&self, omp: &Path, setting: &str) -> Result<(), String>;
    /// `omp update --check` (30 s): the newer version, if any.
    fn update_check(&self, omp: &Path) -> Result<Option<String>, Failure>;
    /// `omp auth-broker logout <provider>` (30 s); on failure, its reason, explained for a proxy
    /// problem.
    fn logout(&self, omp: &Path, provider: &Provider) -> Result<(), String>;
    /// `omp usage --json` (30 s) for the `connected` providers: their limits, or the reason it
    /// failed, explained for a proxy problem. Never an action that writes omp's state.
    fn usage(&self, omp: &Path, connected: &[&Provider]) -> Result<Usage, String>;
    /// Opens the system terminal with the official installer, which leaves its exit code in the
    /// marker of `nonce` after deleting the markers of the earlier installers.
    fn open_installer(&self, nonce: &str) -> Result<(), TerminalFailure>;
    /// Opens the system terminal with `omp login <provider>`.
    fn open_login(&self, omp: &Path, provider: &Provider) -> Result<(), TerminalFailure>;
    /// The exit code of the installer of `nonce` when it is not `0`; `None` for `0`, no marker or
    /// an unreadable one.
    fn installer_exit_code(&self, nonce: &str) -> Option<i32>;
    /// Deletes the markers of the installers: omp was found.
    fn clear_installer_markers(&self);
    /// At the app's start: blocks until the export of System Certificates that started with it
    /// ended, or its limit of 30 s passed.
    fn wait_for_export(&self);
    /// The `PATH` of a process that runs `omp`.
    fn child_path(&self, omp: &Path) -> OsString;
}

/// Where work runs and time passes.
pub trait Scheduler: Send + Sync + 'static {
    /// Runs `job` off the calling thread (the blocking pool).
    fn spawn(&self, job: Box<dyn FnOnce() + Send>);
    /// Calls `tick` after `first`, then every `period`, until the returned guard is dropped.
    fn repeat(
        &self,
        first: Duration,
        period: Duration,
        tick: Arc<dyn Fn() + Send + Sync>,
    ) -> Box<dyn Any + Send>;
}

/// The wiring of a started [`OmpSetup`].
pub struct Runtime {
    pub ops: Arc<dyn Ops>,
    pub scheduler: Arc<dyn Scheduler>,
    pub clock: Arc<dyn Clock>,
    /// Tells the windows that the status changed.
    pub emit: Arc<dyn Fn(&OmpStatus) + Send + Sync>,
    /// Tells the windows that the usage limits, or their reading, changed.
    pub emit_usage: Arc<dyn Fn(&UsageStatus) + Send + Sync>,
}

struct Core {
    machine: Machine,
    /// The timer of `RECHECK_DUE`; alive only while the Machine wants it.
    recheck: Option<Box<dyn Any + Send>>,
    /// The timer of `UPDATE_CHECK_DUE`.
    update_timer: Option<Box<dyn Any + Send>>,
}

struct Shared {
    core: Mutex<Core>,
    runtime: OnceLock<Runtime>,
    ready: watch::Sender<Option<Arc<ReadyOmp>>>,
    usage: UsageState,
}

/// The app-wide readiness of omp. Cheap to clone; managed by Tauri.
#[derive(Clone)]
pub struct OmpSetup {
    shared: Arc<Shared>,
}

impl Default for OmpSetup {
    fn default() -> Self {
        Self::new()
    }
}

impl OmpSetup {
    /// Checking, until [`OmpSetup::start`] tells where omp is.
    pub fn new() -> Self {
        let (ready, _) = watch::channel(None);
        Self {
            shared: Arc::new(Shared {
                core: Mutex::new(Core {
                    machine: Machine::new(),
                    recheck: None,
                    update_timer: None,
                }),
                runtime: OnceLock::new(),
                ready,
                usage: UsageState::new(),
            }),
        }
    }

    /// Starts the Machine: the first check for the `ompPath` setting (`system_certificates` is
    /// the `systemCertificates.enabled` setting; with `exporting` the check waits for the export
    /// of the start), and the hourly look for a newer omp. Does nothing when already started.
    pub fn start(
        &self,
        runtime: Runtime,
        omp_path: &str,
        system_certificates: bool,
        exporting: bool,
    ) {
        if self.shared.runtime.set(runtime).is_err() {
            return;
        }
        let weak = Arc::downgrade(&self.shared);
        let timer = {
            let runtime = self.shared.runtime.get().expect("just set");
            runtime.scheduler.repeat(
                FIRST_UPDATE_CHECK,
                UPDATE_CHECK_TICK,
                Arc::new(move || dispatch_weak(&weak, Event::UpdateCheckDue)),
            )
        };
        self.shared
            .core
            .lock()
            .expect("omp state poisoned")
            .update_timer = Some(timer);
        self.shared.dispatch(Event::Start {
            omp_path: omp_path.to_owned(),
            system_certificates,
            exporting,
        });
    }

    /// The status the UI shows.
    pub fn status(&self) -> OmpStatus {
        self.shared
            .core
            .lock()
            .expect("omp state poisoned")
            .machine
            .status()
            .clone()
    }

    /// omp for a job: `Some` only while the Machine is Ready.
    pub fn ready(&self) -> Option<Arc<ReadyOmp>> {
        self.shared.ready.borrow().clone()
    }

    /// A watch of [`OmpSetup::ready`].
    pub fn subscribe(&self) -> watch::Receiver<Option<Arc<ReadyOmp>>> {
        self.shared.ready.subscribe()
    }

    /// Waits until the Machine is Ready and returns omp.
    pub async fn wait_ready(&self) -> Arc<ReadyOmp> {
        let mut receiver = self.shared.ready.subscribe();
        loop {
            if let Some(ready) = receiver.borrow_and_update().clone() {
                return ready;
            }
            // The sender lives as long as `self`; the error cannot happen while we hold it.
            if receiver.changed().await.is_err() {
                std::future::pending::<()>().await;
            }
        }
    }

    /// The `PATH` of a process that runs `omp`; the base PATH when the Machine has not started.
    pub fn child_path(&self, omp: &Path) -> OsString {
        match self.shared.runtime.get() {
            Some(runtime) => runtime.ops.child_path(omp),
            None => path_env::inherited_path(),
        }
    }

    /// `omp.MISSING` while Ready: a job could not start omp, so it disappeared.
    pub fn report_missing(&self, reason: Option<String>) {
        let omp_path = self.status().omp_path;
        self.shared.dispatch(Event::OmpMissing { omp_path, reason });
    }

    /// `developer.INSTALL_OMP`: a new nonce for the marker of this installer.
    pub fn install(&self) {
        self.shared.dispatch(Event::InstallOmp {
            nonce: installer::new_nonce(),
        });
    }

    /// `developer.LOG_IN_PROVIDER`.
    pub fn log_in(&self, provider: &Provider) {
        self.shared.dispatch(Event::LogInProvider {
            provider: provider.id.to_owned(),
        });
    }

    /// `developer.LOG_OUT_PROVIDER`.
    pub fn log_out(&self, provider: &Provider) {
        self.shared.dispatch(Event::LogOutProvider {
            provider: provider.id.to_owned(),
        });
    }

    /// `developer.CHANGE_SETTINGS`: the `ompPath` setting is now `omp_path` and the
    /// `systemCertificates.enabled` setting is now `system_certificates`.
    pub fn change_settings(&self, omp_path: &str, system_certificates: bool) {
        self.shared.dispatch(Event::ChangeSettings {
            omp_path: omp_path.to_owned(),
            system_certificates,
        });
    }

    /// `system-certificates.EXPORT_ENDED`.
    pub fn export_ended(&self) {
        self.shared.dispatch(Event::ExportEnded);
    }

    /// The usage limits and their reading as the windows last heard of them.
    pub fn usage_status(&self) -> UsageStatus {
        self.shared.usage.status()
    }

    /// `developer.OPEN_USAGE` / `developer.RETRY_USAGE`, without waiting for the end: starts a
    /// reading when omp is Ready and none runs, else joins the one that runs. `None` while omp is
    /// not ready (the dialog shows why and asks again when omp is ready).
    pub fn begin_usage(&self) -> Option<tokio::sync::watch::Receiver<()>> {
        let runtime = self.shared.runtime.get()?;
        let ready = self.ready()?;
        let begin = self.shared.usage.begin(&ready.omp_path);
        if let Some(status) = &begin.status {
            (runtime.emit_usage)(status);
        }
        if let Some(started) = begin.started {
            let shared = Arc::clone(&self.shared);
            runtime
                .scheduler
                .spawn(Box::new(move || shared.run_usage(started, ready)));
        }
        Some(begin.done)
    }

    /// [`OmpSetup::begin_usage`], then waits until the reading ends, and answers the status.
    pub async fn read_usage(&self) -> UsageStatus {
        if let Some(mut done) = self.begin_usage() {
            // The sender is dropped when the reading ends; nothing is ever sent.
            while done.changed().await.is_ok() {}
        }
        self.usage_status()
    }
}

fn dispatch_weak(shared: &Weak<Shared>, event: Event) {
    if let Some(shared) = shared.upgrade() {
        shared.dispatch(event);
    }
}

impl Shared {
    /// Applies `event`, keeps the timer in step, tells the windows and the jobs, and runs the effects.
    fn dispatch(self: &Arc<Self>, event: Event) {
        let Some(runtime) = self.runtime.get() else {
            return;
        };
        let (effects, status, changed, ready) = {
            let mut core = self.core.lock().expect("omp state poisoned");
            let before = core.machine.status().clone();
            let effects = core.machine.handle(event, runtime.clock.now_ms());
            let wants = core.machine.wants_recheck();
            if wants && core.recheck.is_none() {
                let weak = Arc::downgrade(self);
                core.recheck = Some(runtime.scheduler.repeat(
                    RECHECK_EVERY,
                    RECHECK_EVERY,
                    Arc::new(move || dispatch_weak(&weak, Event::RecheckDue)),
                ));
            } else if !wants {
                // Dropping the guard stops the timer: the re-checks end with the state.
                core.recheck = None;
            }
            let status = core.machine.status().clone();
            let changed = status.revision != before.revision;
            if changed && status.state != before.state {
                log_info!("omp setup: {:?} -> {:?}", before.state, status.state);
            }
            (effects, status, changed, core.machine.ready().map(Arc::new))
        };
        self.ready.send_if_modified(|current| {
            if current.as_deref() == ready.as_deref() {
                false
            } else {
                *current = ready;
                true
            }
        });
        if changed {
            (runtime.emit)(&status);
        }
        // What was read for another path is no longer valid.
        if let Some(usage) = self.usage.omp_path_is(&status.omp_path) {
            (runtime.emit_usage)(&usage);
        }
        for effect in effects {
            let shared = Arc::clone(self);
            runtime
                .scheduler
                .spawn(Box::new(move || shared.run(effect)));
        }
    }

    /// Runs one effect (on the blocking pool) and dispatches what came of it. A panic in the
    /// work is a failure of that operation, never a stuck Machine.
    fn run(self: &Arc<Self>, effect: Effect) {
        let runtime = self.runtime.get().expect("effects run after start");
        let ops = &runtime.ops;
        match effect {
            Effect::Probe(ticket) => {
                let Ticket {
                    omp_path,
                    installer,
                    ..
                } = &ticket;
                let mut result =
                    guard(|| ops.probe(omp_path)).unwrap_or_else(|| ProbeResult::Missing {
                        reason: Some("the check of omp failed unexpectedly".to_owned()),
                    });
                if matches!(result, ProbeResult::Missing { .. }) {
                    // While Installing, no omp reads the marker of the current installer.
                    if let Some(nonce) = installer {
                        if let Some(Some(exit_code)) = guard(|| ops.installer_exit_code(nonce)) {
                            result = ProbeResult::InstallFailed { exit_code };
                        }
                    }
                } else {
                    // omp was found: what the installer left is deleted.
                    let _ = guard(|| ops.clear_installer_markers());
                }
                self.dispatch(Event::Probed { ticket, result });
            }
            Effect::RunUpdate { omp_path, omp } => {
                match guard(|| ops.update(&omp, &omp_path))
                    .unwrap_or_else(|| Err(Failure::NotStarted.reason()))
                {
                    Ok(()) => self.dispatch(Event::UpdateEnded { omp_path }),
                    Err(reason) => {
                        log_warn!("omp update failed");
                        let after = guard(|| ops.probe_found(&omp))
                            .unwrap_or(ProbeResult::Missing { reason: None });
                        self.dispatch(Event::UpdateFailed {
                            omp_path,
                            reason,
                            after,
                        });
                    }
                }
            }
            Effect::CheckForUpdate { omp_path, omp } => match guard(|| ops.update_check(&omp)) {
                Some(Ok(available)) => self.dispatch(Event::UpdateCheckDone {
                    omp_path,
                    available,
                }),
                // A check that fails is logged; the UI never hears of it and no job waits for it.
                Some(Err(_)) | None => log_warn!("the check for a newer omp failed"),
            },
            Effect::OpenInstaller { nonce } => {
                if let Some(Err(failure)) = guard(|| ops.open_installer(&nonce)) {
                    self.dispatch(Event::TerminalFailed {
                        reason: failure.reason,
                        command: failure.command,
                    });
                }
            }
            Effect::OpenLogin { omp, provider: id } => {
                let Some(provider) = provider(&id) else {
                    return;
                };
                if let Some(Err(failure)) = guard(|| ops.open_login(&omp, provider)) {
                    self.dispatch(Event::TerminalFailed {
                        reason: failure.reason,
                        command: failure.command,
                    });
                }
            }
            Effect::Logout {
                omp_path,
                omp,
                provider: id,
            } => {
                let Some(provider) = provider(&id) else {
                    return;
                };
                let outcome = match guard(|| ops.logout(&omp, provider)) {
                    Some(Ok(())) => Ok(()),
                    Some(Err(reason)) => Err(reason),
                    None => Err("the log out failed unexpectedly".to_owned()),
                };
                self.dispatch(Event::LoggedOut {
                    omp_path,
                    provider: id,
                    outcome,
                });
            }
            Effect::WaitForExport => {
                let _ = guard(|| ops.wait_for_export());
                self.dispatch(Event::ExportWaitOver);
            }
        }
    }

    /// Runs a reading of usage (on the blocking pool), keeps its result and tells the windows. The
    /// waiting ones wake up when `started` is dropped, after the windows were told.
    fn run_usage(self: &Arc<Self>, started: Started, ready: Arc<ReadyOmp>) {
        let runtime = self.runtime.get().expect("readings run after start");
        let connected = connected_providers(&ready.models);
        let result = guard(|| runtime.ops.usage(&ready.omp, &connected))
            .unwrap_or_else(|| Err("the reading of usage failed unexpectedly".to_owned()));
        if let Err(reason) = &result {
            log_warn!("reading usage failed: {reason}");
        }
        let current = self
            .core
            .lock()
            .expect("omp state poisoned")
            .machine
            .status()
            .omp_path
            .clone();
        if let Some(status) = self.usage.finish(&started, &current, result) {
            (runtime.emit_usage)(&status);
        }
    }
}

/// Logs a background download from GitHub: at `info` its tag, asset, outcome with exit code and
/// duration; a failure also at `warn` with its reason. Never the user `Path`.
fn log_download(download: &DownloadRun) {
    let tag = download.tag.as_deref().unwrap_or("unknown");
    let asset = download.asset.as_deref().unwrap_or("unknown");
    let outcome = match download.outcome {
        DownloadOutcome::Installed => "installed",
        DownloadOutcome::NotNewer => "not newer",
        DownloadOutcome::Failed(_) => "failed",
    };
    let code = download
        .exit_code
        .map_or_else(|| "timed out".to_owned(), |code| code.to_string());
    log_info!(
        "omp update from GitHub: tag {tag}, asset {asset}, {outcome} (exit code {code}), {} ms",
        download.elapsed.as_millis()
    );
    if let (DownloadOutcome::Failed(_), Some(reason)) = (download.outcome, &download.reason) {
        log_warn!("omp update from GitHub failed: {reason}");
    }
}

/// Runs `work`; a panic is logged by the panic hook and comes back as `None`.
fn guard<T>(work: impl FnOnce() -> T) -> Option<T> {
    catch_unwind(AssertUnwindSafe(work)).ok()
}

// --- the real system ----------------------------------------------------------------------------

/// The real processes: omp itself, the lookup and the system terminal.
pub struct SystemOps {
    os: Os,
    local_bin: PathBuf,
    /// macOS and Linux: the login PATH read at start (or the inherited one).
    base_path: OsString,
    /// `<app local data>/omp-install`: where the installer leaves its exit code.
    install_dir: PathBuf,
    /// The setting and the bundle of System Certificates, read at every start of an omp command
    /// and every opening of a terminal, never once.
    certificates: CertificateSource,
    /// The export that started with the app, which the first check waits for, once.
    startup_export: Mutex<Option<StartupExport>>,
}

impl SystemOps {
    pub fn new(
        local_bin: PathBuf,
        base_path: OsString,
        install_dir: PathBuf,
        certificates: CertificateSource,
        startup_export: Option<StartupExport>,
    ) -> Self {
        Self {
            os: Os::current(),
            local_bin,
            base_path,
            install_dir,
            certificates,
            startup_export: Mutex::new(startup_export),
        }
    }

    /// The PATH to search now: Windows reads the registry at every lookup.
    fn search_path(&self) -> OsString {
        if self.os == Os::Windows {
            let root = std::env::var_os("SystemRoot")
                .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
            // `reg.exe` only reads the `Path`: it gets no certificate variable.
            let exec = SystemExec::new(self.local_bin.clone(), None, path_env::inherited_path());
            path_env::read_windows_path(&exec, &root)
        } else {
            self.base_path.clone()
        }
    }

    /// The lookup and the runner of omp's own commands, which get the certificates of the moment.
    fn environment(&self) -> (LookupEnv, SystemExec) {
        let lookup = LookupEnv::from_process(self.search_path());
        let exec = SystemExec::new(
            self.local_bin.clone(),
            lookup.bun_bin(),
            lookup.path.clone(),
        )
        .with_certificates(Arc::clone(&self.certificates));
        (lookup, exec)
    }

    /// Opens the terminal for `task` with the certificate variables of the moment.
    fn open(&self, task: Task, search_path: &OsString) -> Result<(), TerminalFailure> {
        let variables = (self.certificates)().variables;
        let command =
            terminal::command(self.os, task, &variables).map_err(|error| TerminalFailure {
                reason: error.to_string(),
                command: terminal::plain_command(self.os, task),
            })?;
        let fail = |reason: String| TerminalFailure {
            reason,
            command: command.copy.clone(),
        };
        let launch = terminal::launch_plan(self.os, &command, &|name| which(name, search_path))
            .map_err(|error| fail(error.to_string()))?;
        terminal::open(self.os, &launch).map_err(|error| fail(error.to_string()))
    }
}

impl Ops for SystemOps {
    fn probe(&self, setting: &str) -> ProbeResult {
        let (lookup, exec) = self.environment();
        probe::probe(setting, &lookup, &exec)
    }

    fn probe_found(&self, omp: &Path) -> ProbeResult {
        probe::probe_found(omp, &self.environment().1)
    }

    fn update(&self, omp: &Path, setting: &str) -> Result<(), String> {
        let (lookup, exec) = self.environment();
        let run = github::update(omp, setting, &lookup, &exec);
        if let Some(download) = &run.download {
            log_download(download);
        }
        run.result
    }

    fn update_check(&self, omp: &Path) -> Result<Option<String>, Failure> {
        probe::check_for_update(omp, &self.environment().1)
    }

    fn logout(&self, omp: &Path, provider: &Provider) -> Result<(), String> {
        probe::logout(omp, provider, &self.environment().1)
    }

    fn usage(&self, omp: &Path, connected: &[&Provider]) -> Result<Usage, String> {
        usage::read_usage(omp, connected, &self.environment().1)
    }

    fn open_installer(&self, nonce: &str) -> Result<(), TerminalFailure> {
        // A new installer forgets the exit code of the earlier ones.
        if let Err(error) = installer::prepare(&self.install_dir) {
            log_warn!("cannot prepare the folder of the installer's exit code: {error}");
        }
        let Some(marker) = installer::marker_path(&self.install_dir, nonce) else {
            return Err(TerminalFailure {
                reason: "the installer's marker has no valid name".to_owned(),
                command: terminal::plain_command(
                    self.os,
                    Task::Installer {
                        marker: &self.install_dir,
                    },
                ),
            });
        };
        self.open(Task::Installer { marker: &marker }, &self.search_path())
    }

    fn open_login(&self, omp: &Path, provider: &Provider) -> Result<(), TerminalFailure> {
        self.open(Task::Login { omp, provider }, &self.search_path())
    }

    fn installer_exit_code(&self, nonce: &str) -> Option<i32> {
        installer::failed_exit_code(&self.install_dir, nonce)
    }

    fn clear_installer_markers(&self) {
        installer::clear(&self.install_dir);
    }

    fn wait_for_export(&self) {
        let startup = self
            .startup_export
            .lock()
            .expect("startup export poisoned")
            .take();
        if let Some(startup) = startup {
            // Bounded by the 30 s of the start; on the blocking pool, never on the runtime.
            tauri::async_runtime::block_on(startup.ended());
        }
    }

    fn child_path(&self, omp: &Path) -> OsString {
        self.environment().1.child_path(omp)
    }
}

/// The scheduler of the app: Tauri's blocking pool and Tokio's timers.
pub struct TauriScheduler;

struct AbortOnDrop(tauri::async_runtime::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl Scheduler for TauriScheduler {
    fn spawn(&self, job: Box<dyn FnOnce() + Send>) {
        drop(tauri::async_runtime::spawn_blocking(job));
    }

    fn repeat(
        &self,
        first: Duration,
        period: Duration,
        tick: Arc<dyn Fn() + Send + Sync>,
    ) -> Box<dyn Any + Send> {
        Box::new(AbortOnDrop(tauri::async_runtime::spawn(async move {
            tokio::time::sleep(first).await;
            loop {
                // The tick takes a lock and may emit an event: nothing here waits for the disk.
                tick();
                tokio::time::sleep(period).await;
            }
        })))
    }
}

/// The startup step `omp`: reads the login PATH (macOS and Linux, 5 s) and the settings, waits
/// for the export of System Certificates that started with the app (the first check of omp
/// follows it, 30 s at most), starts the Machine and follows later changes of the settings and
/// the end of each export. Runs on the blocking pool.
pub fn start(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<crate::state::AppState>();
    let service = app.state::<SystemCertificates>().inner().clone();
    // Subscribe before reading, so a change between the two is not lost.
    let mut changes = state.settings.subscribe();
    let mut exports = service.subscribe_ended();
    let settings = state.settings.current();

    let base_path = if cfg!(windows) {
        path_env::inherited_path()
    } else {
        login_path()
    };
    // Resolves when the age of the bundle and the Zscaler turn-on are decided (file metadata
    // only, bounded by the limit); `running` says whether an export started with the app.
    let startup = tauri::async_runtime::block_on(service.startup_export(EXPORT_LIMIT));
    let exporting = startup.running;

    let certificates_handle = app.clone();
    let certificates: CertificateSource = Arc::new(move || {
        let state = certificates_handle.state::<crate::state::AppState>();
        Certificates {
            enabled: state.settings.current().system_certificates.enabled,
            variables: system_certificates::variables(&state),
        }
    });
    let ops = SystemOps::new(
        state.dirs.local_data_dir().join("bin"),
        base_path,
        state.dirs.omp_install_dir(),
        certificates,
        Some(startup),
    );
    let emit_handle = app.clone();
    let usage_emit_handle = app.clone();
    let setup = app.state::<OmpSetup>().inner().clone();
    setup.start(
        Runtime {
            ops: Arc::new(ops),
            scheduler: Arc::new(TauriScheduler),
            clock: Arc::new(SystemClock),
            emit: Arc::new(move |status| {
                if let Err(error) = events::emit_omp_changed(&emit_handle, status) {
                    crate::log_error!("cannot tell the windows about omp: {error}");
                }
            }),
            emit_usage: Arc::new(move |status| {
                if let Err(error) = events::emit_usage_changed(&usage_emit_handle, status) {
                    crate::log_error!("cannot tell the windows about usage: {error}");
                }
            }),
        },
        &settings.omp_path,
        settings.system_certificates.enabled,
        exporting,
    );

    let settings_handle = app.clone();
    let settings_setup = setup.clone();
    tauri::async_runtime::spawn(async move {
        use tokio::sync::broadcast::error::RecvError;
        loop {
            match changes.recv().await {
                Ok(change)
                    if change.changed.contains(&SettingKey::OmpPath)
                        || change
                            .changed
                            .contains(&SettingKey::SystemCertificatesEnabled) =>
                {
                    settings_setup.change_settings(
                        &change.settings.omp_path,
                        change.settings.system_certificates.enabled,
                    );
                }
                Ok(_) => {}
                // Fell behind: look at the settings as they are now.
                Err(RecvError::Lagged(_)) => {
                    let handle = settings_handle.clone();
                    if let Ok(current) = blocking::disk(move || {
                        handle.state::<crate::state::AppState>().settings.current()
                    })
                    .await
                    {
                        settings_setup.change_settings(
                            &current.omp_path,
                            current.system_certificates.enabled,
                        );
                    }
                }
                Err(RecvError::Closed) => break,
            }
        }
    });

    // `system-certificates.EXPORT_ENDED`: after every export, successful or not.
    tauri::async_runtime::spawn(async move {
        use tokio::sync::broadcast::error::RecvError;
        // Behind: one look at omp covers the exports it missed.
        while let Ok(_) | Err(RecvError::Lagged(_)) = exports.recv().await {
            setup.export_ended();
        }
    });
    Ok(())
}

/// The login PATH of the user's shell, read once; the inherited PATH when that fails.
fn login_path() -> OsString {
    let inherited = path_env::inherited_path();
    let Some(shell) = std::env::var_os("SHELL")
        .map(PathBuf::from)
        .filter(|s| s.is_absolute())
    else {
        return inherited;
    };
    let exec = SystemExec::new(PathBuf::new(), None, inherited.clone());
    match path_env::read_login_path(&exec, &shell) {
        Some(path) => path,
        None => {
            log_warn!("the login PATH could not be read; the inherited PATH is used");
            inherited
        }
    }
}

// --- commands -------------------------------------------------------------------------------------

/// The status of omp, for a window that just loaded; later changes arrive as `omp-changed`.
#[tauri::command]
pub async fn omp_status(omp: State<'_, OmpSetup>) -> Result<OmpStatus, AppError> {
    Ok(omp.status())
}

/// **Install omp** and **Open installer again**: opens the system terminal with the installer.
#[tauri::command]
pub async fn omp_install(omp: State<'_, OmpSetup>) -> Result<(), AppError> {
    omp.install();
    Ok(())
}

fn known_provider(id: &str) -> Result<&'static Provider, AppError> {
    provider(id).ok_or_else(|| {
        AppError::new(
            ErrorCode::InvalidInput,
            "that is not a provider omp can log in to",
        )
    })
}

/// **Log in** and **Log in again**: opens the system terminal with `omp login <provider>`.
#[tauri::command]
pub async fn omp_log_in(omp: State<'_, OmpSetup>, provider: String) -> Result<(), AppError> {
    omp.log_in(known_provider(&provider)?);
    Ok(())
}

/// **Log out**: omp forgets the credentials it stored for the provider. Returns at once; the
/// progress and the outcome arrive in the status.
#[tauri::command]
pub async fn omp_log_out(omp: State<'_, OmpSetup>, provider: String) -> Result<(), AppError> {
    omp.log_out(known_provider(&provider)?);
    Ok(())
}

/// `developer.OPEN_USAGE` and `developer.RETRY_USAGE`: reads the usage limits of the connected
/// providers (`omp usage --json`), only while omp is Ready, and answers when the reading ends. A
/// reading already running is waited for, never doubled. The windows get the progress and the
/// result through `usage-changed`; the answer is the status at the end, for the one that asked.
#[tauri::command]
pub async fn omp_usage(omp: State<'_, OmpSetup>) -> Result<UsageStatus, AppError> {
    Ok(omp.read_usage().await)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

    use weaver_core::omp::machine::Phase;
    use weaver_core::omp::probe::OmpModel;

    use crate::usage::UsagePhase;

    use super::*;

    fn omp() -> PathBuf {
        PathBuf::from("/opt/omp")
    }

    fn model(provider: &str) -> OmpModel {
        OmpModel {
            provider: provider.into(),
            selector: format!("{provider}/m"),
            name: None,
            thinking_levels: vec![],
        }
    }

    fn ready() -> ProbeResult {
        ProbeResult::Ready {
            omp: omp(),
            version: "18.15.0".into(),
            models: vec![model("anthropic")],
        }
    }

    /// The answers of the fake system, taken in order.
    #[derive(Default)]
    struct FakeOps {
        probes: Mutex<VecDeque<ProbeResult>>,
        updates: Mutex<VecDeque<Result<(), String>>>,
        /// The `ompPath` settings the updates were run for.
        updated_for: Mutex<Vec<String>>,
        update_checks: Mutex<VecDeque<Result<Option<String>, Failure>>>,
        terminal_fails: AtomicBool,
        probed_paths: Mutex<Vec<String>>,
        /// The nonces the installer was opened with.
        installers: Mutex<Vec<String>>,
        /// The exit codes the marker answers, taken in order (`None`: no marker).
        installer_codes: Mutex<VecDeque<Option<i32>>>,
        /// The nonces whose marker was read.
        marker_reads: Mutex<Vec<String>>,
        markers_cleared: Mutex<u32>,
        /// What the system did, in order: `wait` for the export, `probe`.
        log: Mutex<Vec<&'static str>>,
        logins: Mutex<Vec<String>>,
        logouts: Mutex<Vec<String>>,
        /// The answers of the readings of usage, taken in order.
        usages: Mutex<VecDeque<Result<Usage, String>>>,
        /// The omp and the connected providers each reading was made for.
        usage_calls: Mutex<Vec<(PathBuf, Vec<String>)>>,
    }

    impl Ops for FakeOps {
        fn usage(&self, omp: &Path, connected: &[&Provider]) -> Result<Usage, String> {
            self.usage_calls.lock().unwrap().push((
                omp.to_path_buf(),
                connected.iter().map(|p| p.id.to_owned()).collect(),
            ));
            self.usages
                .lock()
                .unwrap()
                .pop_front()
                .expect("a scripted reading of usage")
        }
        fn probe(&self, setting: &str) -> ProbeResult {
            self.log.lock().unwrap().push("probe");
            self.probed_paths.lock().unwrap().push(setting.to_owned());
            self.probes
                .lock()
                .unwrap()
                .pop_front()
                .expect("a scripted probe")
        }
        fn probe_found(&self, _omp: &Path) -> ProbeResult {
            self.probes
                .lock()
                .unwrap()
                .pop_front()
                .expect("a scripted probe")
        }
        fn update(&self, _omp: &Path, setting: &str) -> Result<(), String> {
            self.updated_for.lock().unwrap().push(setting.to_owned());
            self.updates
                .lock()
                .unwrap()
                .pop_front()
                .expect("a scripted update")
        }
        fn update_check(&self, _omp: &Path) -> Result<Option<String>, Failure> {
            self.update_checks
                .lock()
                .unwrap()
                .pop_front()
                .expect("a scripted check")
        }
        fn logout(&self, _omp: &Path, provider: &Provider) -> Result<(), String> {
            self.logouts.lock().unwrap().push(provider.id.to_owned());
            Ok(())
        }
        fn open_installer(&self, nonce: &str) -> Result<(), TerminalFailure> {
            self.installers.lock().unwrap().push(nonce.to_owned());
            if self.terminal_fails.load(Ordering::SeqCst) {
                Err(TerminalFailure {
                    reason: "no terminal emulator was found".into(),
                    command: "curl -fsSL https://omp.sh/install -o install.sh && sh install.sh"
                        .into(),
                })
            } else {
                Ok(())
            }
        }
        fn open_login(&self, _omp: &Path, provider: &Provider) -> Result<(), TerminalFailure> {
            self.logins.lock().unwrap().push(provider.id.to_owned());
            Ok(())
        }
        fn installer_exit_code(&self, nonce: &str) -> Option<i32> {
            self.marker_reads.lock().unwrap().push(nonce.to_owned());
            self.installer_codes
                .lock()
                .unwrap()
                .pop_front()
                .flatten()
                .filter(|code| *code != 0)
        }
        fn clear_installer_markers(&self) {
            *self.markers_cleared.lock().unwrap() += 1;
        }
        fn wait_for_export(&self) {
            self.log.lock().unwrap().push("wait");
        }
        fn child_path(&self, _omp: &Path) -> OsString {
            OsString::from("/child")
        }
    }

    type Tick = Arc<dyn Fn() + Send + Sync>;

    /// Jobs wait in a queue until the test runs them; timers never fire by themselves.
    #[derive(Default)]
    struct ManualScheduler {
        jobs: Mutex<VecDeque<Box<dyn FnOnce() + Send>>>,
        timers: Mutex<Vec<(Duration, Tick, Arc<AtomicBool>)>>,
    }

    struct Guard(Arc<AtomicBool>);

    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.store(false, Ordering::SeqCst);
        }
    }

    impl Scheduler for ManualScheduler {
        fn spawn(&self, job: Box<dyn FnOnce() + Send>) {
            self.jobs.lock().unwrap().push_back(job);
        }
        fn repeat(&self, _first: Duration, period: Duration, tick: Tick) -> Box<dyn Any + Send> {
            let alive = Arc::new(AtomicBool::new(true));
            self.timers
                .lock()
                .unwrap()
                .push((period, tick, alive.clone()));
            Box::new(Guard(alive))
        }
    }

    impl ManualScheduler {
        /// Runs every queued job, including the ones they queue.
        fn drain(&self) {
            loop {
                let job = self.jobs.lock().unwrap().pop_front();
                match job {
                    Some(job) => job(),
                    None => break,
                }
            }
        }

        /// The tick of the timer with this period, when it is alive.
        fn live_timer(&self, period: Duration) -> Option<Tick> {
            self.timers
                .lock()
                .unwrap()
                .iter()
                .filter(|(p, _, alive)| *p == period && alive.load(Ordering::SeqCst))
                .map(|(_, tick, _)| tick.clone())
                .next_back()
        }
    }

    struct Rig {
        setup: OmpSetup,
        ops: Arc<FakeOps>,
        scheduler: Arc<ManualScheduler>,
        clock: Arc<ClockAt>,
        emitted: Arc<Mutex<Vec<OmpStatus>>>,
        usage_emitted: Arc<Mutex<Vec<UsageStatus>>>,
        /// The wiring, taken by `start`.
        runtime: Mutex<Option<Runtime>>,
    }

    struct ClockAt(AtomicI64);

    impl Clock for ClockAt {
        fn now_ms(&self) -> i64 {
            self.0.load(Ordering::SeqCst)
        }
    }

    fn rig() -> Rig {
        let ops = Arc::new(FakeOps::default());
        let scheduler = Arc::new(ManualScheduler::default());
        let clock = Arc::new(ClockAt(AtomicI64::new(1_000_000)));
        let emitted = Arc::new(Mutex::new(Vec::new()));
        let sink = emitted.clone();
        let usage_emitted = Arc::new(Mutex::new(Vec::new()));
        let usage_sink = usage_emitted.clone();
        let runtime = Runtime {
            ops: ops.clone(),
            scheduler: scheduler.clone(),
            clock: clock.clone(),
            emit: Arc::new(move |status| sink.lock().unwrap().push(status.clone())),
            emit_usage: Arc::new(move |status| usage_sink.lock().unwrap().push(status.clone())),
        };
        Rig {
            setup: OmpSetup::new(),
            ops,
            scheduler,
            clock,
            emitted,
            usage_emitted,
            runtime: Mutex::new(Some(runtime)),
        }
    }

    impl Rig {
        fn start(&self, path: &str) {
            self.start_with(path, false, false);
        }
        /// The app starts with System Certificates `certificates` and an export that runs when
        /// `exporting`.
        fn start_with(&self, path: &str, certificates: bool, exporting: bool) {
            let runtime = self.runtime.lock().unwrap().take().expect("started once");
            self.setup.start(runtime, path, certificates, exporting);
        }
        fn script_probe(&self, result: ProbeResult) {
            self.ops.probes.lock().unwrap().push_back(result);
        }
        fn phase(&self) -> Phase {
            self.setup.status().state
        }
    }

    #[test]
    fn the_machine_starts_checking_and_becomes_ready_when_omp_answers() {
        let rig = rig();
        assert_eq!(rig.phase(), Phase::Checking);
        assert!(rig.setup.ready().is_none());
        rig.script_probe(ready());
        rig.start("/custom/omp");
        rig.scheduler.drain();
        assert_eq!(rig.phase(), Phase::Ready);
        assert_eq!(
            rig.ops.probed_paths.lock().unwrap().as_slice(),
            ["/custom/omp"]
        );
        let ready = rig.setup.ready().expect("ready");
        assert_eq!(
            (ready.omp.clone(), ready.version.as_str()),
            (omp(), "18.15.0")
        );
        assert_eq!(rig.setup.child_path(&ready.omp), OsString::from("/child"));
        assert_eq!(
            rig.emitted.lock().unwrap().last().unwrap().state,
            Phase::Ready
        );
    }

    #[test]
    fn a_job_waits_until_the_machine_is_ready() {
        let rig = rig();
        rig.script_probe(ProbeResult::Missing { reason: None });
        rig.start("");
        rig.scheduler.drain();
        assert_eq!(rig.phase(), Phase::Missing);
        let mut watcher = rig.setup.subscribe();
        assert!(watcher.borrow_and_update().is_none());
        rig.setup.install();
        rig.script_probe(ready());
        // Installing: the timer ticks and finds omp.
        let tick = rig
            .scheduler
            .live_timer(RECHECK_EVERY)
            .expect("the re-check timer runs while Installing");
        tick();
        rig.scheduler.drain();
        assert_eq!(rig.phase(), Phase::Ready);
        assert!(watcher.has_changed().unwrap());
        assert!(watcher.borrow_and_update().is_some());
    }

    #[test]
    fn the_recheck_timer_lives_only_while_installing_or_logging_in() {
        let rig = rig();
        rig.script_probe(ProbeResult::Missing { reason: None });
        rig.start("");
        rig.scheduler.drain();
        assert!(rig.scheduler.live_timer(RECHECK_EVERY).is_none());
        rig.setup.install();
        rig.scheduler.drain();
        assert_eq!(rig.ops.installers.lock().unwrap().len(), 1);
        let tick = rig.scheduler.live_timer(RECHECK_EVERY).expect("running");
        // omp still missing: the timer stays; then it is found and the timer stops.
        rig.script_probe(ProbeResult::Missing { reason: None });
        tick();
        rig.scheduler.drain();
        assert!(rig.scheduler.live_timer(RECHECK_EVERY).is_some());
        rig.script_probe(ready());
        tick();
        rig.scheduler.drain();
        assert_eq!(rig.phase(), Phase::Ready);
        assert!(rig.scheduler.live_timer(RECHECK_EVERY).is_none());
        // A tick that was already on its way does nothing.
        tick();
        rig.scheduler.drain();
        assert_eq!(rig.phase(), Phase::Ready);
    }

    #[test]
    fn two_ticks_in_a_row_start_one_check() {
        let rig = rig();
        rig.script_probe(ProbeResult::Missing { reason: None });
        rig.start("");
        rig.scheduler.drain();
        rig.setup.install();
        rig.scheduler.drain();
        let tick = rig.scheduler.live_timer(RECHECK_EVERY).unwrap();
        rig.script_probe(ProbeResult::Missing { reason: None });
        tick();
        tick();
        assert_eq!(rig.scheduler.jobs.lock().unwrap().len(), 1);
        rig.scheduler.drain();
    }

    #[test]
    fn a_terminal_that_cannot_open_is_reported_with_the_command_to_copy() {
        let rig = rig();
        rig.ops.terminal_fails.store(true, Ordering::SeqCst);
        rig.script_probe(ProbeResult::Missing { reason: None });
        rig.start("");
        rig.scheduler.drain();
        rig.setup.install();
        rig.scheduler.drain();
        let problem = rig.setup.status().terminal_problem.expect("a problem");
        assert_eq!(
            (problem.reason.as_str(), problem.command.as_str()),
            (
                "no terminal emulator was found",
                "curl -fsSL https://omp.sh/install -o install.sh && sh install.sh"
            )
        );
    }

    #[test]
    fn an_outdated_omp_is_updated_and_checked_again() {
        let rig = rig();
        rig.script_probe(ProbeResult::Outdated {
            omp: omp(),
            version: Some("18.0.0".into()),
        });
        rig.ops.updates.lock().unwrap().push_back(Ok(()));
        rig.script_probe(ready());
        rig.start("");
        rig.scheduler.drain();
        assert_eq!(rig.phase(), Phase::Ready);
    }

    #[test]
    fn the_update_runs_for_the_omp_path_setting_the_machine_has() {
        let rig = rig();
        rig.script_probe(ProbeResult::Outdated {
            omp: omp(),
            version: Some("18.0.0".into()),
        });
        rig.ops.updates.lock().unwrap().push_back(Ok(()));
        rig.script_probe(ready());
        rig.start("/custom/omp");
        rig.scheduler.drain();
        // The download from GitHub is never tried for an omp the settings chose: the runner
        // decides it from the setting it is given here.
        assert_eq!(*rig.ops.updated_for.lock().unwrap(), ["/custom/omp"]);
    }

    #[test]
    fn a_failed_update_asks_omp_again_and_keeps_working_when_it_answers() {
        let rig = rig();
        rig.script_probe(ProbeResult::Outdated {
            omp: omp(),
            version: Some("18.0.0".into()),
        });
        rig.ops
            .updates
            .lock()
            .unwrap()
            .push_back(Err("disk full".to_owned()));
        rig.script_probe(ready());
        rig.start("");
        rig.scheduler.drain();
        assert_eq!(rig.phase(), Phase::Ready);
        assert_eq!(
            rig.setup.status().update_failure.as_deref(),
            Some("disk full")
        );
    }

    #[test]
    fn the_update_check_runs_in_the_background_without_taking_ready_away() {
        let rig = rig();
        rig.script_probe(ready());
        rig.start("");
        rig.scheduler.drain();
        let tick = rig
            .scheduler
            .live_timer(UPDATE_CHECK_TICK)
            .expect("the update timer runs from start");
        rig.ops
            .update_checks
            .lock()
            .unwrap()
            .push_back(Ok(Some("18.16.0".into())));
        tick();
        // The check is queued: a job still finds omp ready meanwhile.
        assert!(rig.setup.ready().is_some());
        rig.ops.updates.lock().unwrap().push_back(Ok(()));
        rig.script_probe(ready());
        rig.scheduler.drain();
        assert_eq!(rig.phase(), Phase::Ready);
        // A second tick inside the hour asks nothing.
        tick();
        assert!(rig.scheduler.jobs.lock().unwrap().is_empty());
        rig.clock.0.fetch_add(
            weaver_core::omp::machine::UPDATE_CHECK_INTERVAL_MS,
            Ordering::SeqCst,
        );
        rig.ops.update_checks.lock().unwrap().push_back(Ok(None));
        tick();
        assert_eq!(rig.scheduler.jobs.lock().unwrap().len(), 1);
        rig.scheduler.drain();
    }

    #[test]
    fn logging_out_runs_omp_and_then_checks_the_providers() {
        let rig = rig();
        rig.script_probe(ready());
        rig.start("");
        rig.scheduler.drain();
        rig.script_probe(ProbeResult::NoProvider {
            omp: omp(),
            version: "18.15.0".into(),
        });
        rig.setup.log_out(provider("anthropic").unwrap());
        rig.scheduler.drain();
        assert_eq!(rig.ops.logouts.lock().unwrap().as_slice(), ["anthropic"]);
        assert_eq!(rig.phase(), Phase::NotConnected);
    }

    #[test]
    fn a_login_opens_the_terminal_for_the_provider_and_waits() {
        let rig = rig();
        rig.script_probe(ProbeResult::NoProvider {
            omp: omp(),
            version: "18.15.0".into(),
        });
        rig.start("");
        rig.scheduler.drain();
        rig.setup.log_in(provider("cursor").unwrap());
        rig.scheduler.drain();
        assert_eq!(rig.ops.logins.lock().unwrap().as_slice(), ["cursor"]);
        assert_eq!(rig.phase(), Phase::LoggingIn);
        assert!(rig.scheduler.live_timer(RECHECK_EVERY).is_some());
    }

    #[test]
    fn a_change_of_the_path_checks_again_and_a_late_result_for_the_old_path_is_dropped() {
        let rig = rig();
        rig.script_probe(ready());
        rig.start("/old");
        // The first check is queued but has not run when the setting changes.
        rig.setup.change_settings("/new", false);
        rig.script_probe(ProbeResult::Missing {
            reason: Some("omp not found at /new".into()),
        });
        rig.scheduler.drain();
        // The probe of "/old" took the first scripted answer (Ready) but its result was dropped.
        assert_eq!(rig.phase(), Phase::Missing);
        assert_eq!(
            rig.setup.status().reason.as_deref(),
            Some("omp not found at /new")
        );
        assert!(rig.setup.ready().is_none());
    }

    #[test]
    fn an_omp_that_cannot_be_started_by_a_job_is_missing() {
        let rig = rig();
        rig.script_probe(ready());
        rig.start("");
        rig.scheduler.drain();
        rig.setup.report_missing(Some("could not start omp".into()));
        assert_eq!(rig.phase(), Phase::Missing);
        assert!(rig.setup.ready().is_none());
    }

    #[tokio::test]
    async fn wait_ready_returns_when_the_machine_becomes_ready() {
        let rig = rig();
        rig.script_probe(ready());
        let setup = rig.setup.clone();
        let waiting = tokio::spawn(async move { setup.wait_ready().await });
        tokio::task::yield_now().await;
        assert!(!waiting.is_finished());
        rig.start("");
        rig.scheduler.drain();
        let omp = waiting.await.unwrap();
        assert_eq!(omp.version, "18.15.0");
    }

    #[test]
    fn a_provider_outside_the_list_is_refused_by_the_commands() {
        assert_eq!(known_provider("anthropic").unwrap().id, "anthropic");
        assert_eq!(
            known_provider("anthropic && calc").unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }

    // ---- System Certificates and the installer ---------------------------------------------

    #[test]
    fn at_the_start_the_first_check_waits_for_the_export() {
        let rig = rig();
        rig.script_probe(ready());
        rig.start_with("", true, true);
        assert_eq!(rig.phase(), Phase::Checking);
        assert!(rig.setup.status().waiting_for_export);
        assert!(rig.ops.log.lock().unwrap().is_empty(), "nothing ran yet");
        rig.scheduler.drain();
        assert_eq!(rig.ops.log.lock().unwrap().as_slice(), ["wait", "probe"]);
        assert_eq!(rig.phase(), Phase::Ready);
        assert!(!rig.setup.status().waiting_for_export);
    }

    #[test]
    fn without_an_export_the_first_check_does_not_wait() {
        let rig = rig();
        rig.script_probe(ready());
        rig.start_with("", true, false);
        assert!(!rig.setup.status().waiting_for_export);
        rig.scheduler.drain();
        assert_eq!(rig.ops.log.lock().unwrap().as_slice(), ["probe"]);
    }

    #[test]
    fn every_export_that_ends_checks_omp_again_and_so_does_turning_the_setting_off() {
        let rig = rig();
        rig.script_probe(ready());
        rig.start_with("", true, false);
        rig.scheduler.drain();
        rig.script_probe(ready());
        rig.setup.export_ended();
        assert_eq!(rig.phase(), Phase::Checking);
        rig.scheduler.drain();
        assert_eq!(rig.phase(), Phase::Ready);
        assert_eq!(rig.ops.probed_paths.lock().unwrap().len(), 2);
        // Turned on again (no change of value): nothing here; turned off: a check.
        rig.setup.change_settings("", true);
        assert!(rig.scheduler.jobs.lock().unwrap().is_empty());
        rig.script_probe(ready());
        rig.setup.change_settings("", false);
        rig.scheduler.drain();
        assert_eq!(rig.ops.probed_paths.lock().unwrap().len(), 3);
    }

    #[test]
    fn a_failed_installer_reads_the_marker_of_its_nonce_and_reports_missing() {
        let rig = rig();
        rig.script_probe(ProbeResult::Missing { reason: None });
        rig.start("");
        rig.scheduler.drain();
        rig.setup.install();
        rig.scheduler.drain();
        let nonce = rig.ops.installers.lock().unwrap()[0].clone();
        assert_eq!(nonce.len(), 32);
        let tick = rig.scheduler.live_timer(RECHECK_EVERY).expect("running");
        // No marker yet: still installing.
        rig.script_probe(ProbeResult::Missing { reason: None });
        rig.ops.installer_codes.lock().unwrap().push_back(None);
        tick();
        rig.scheduler.drain();
        assert_eq!(rig.phase(), Phase::Installing);
        // The installer ended with 22.
        rig.script_probe(ProbeResult::Missing { reason: None });
        rig.ops.installer_codes.lock().unwrap().push_back(Some(22));
        tick();
        rig.scheduler.drain();
        assert_eq!(rig.phase(), Phase::Missing);
        let status = rig.setup.status();
        assert_eq!(status.reason, None);
        let text = status
            .installer_failure
            .expect("the failure of the installer");
        assert!(
            text.starts_with("The omp installer failed (exit code 22);"),
            "{text}"
        );
        assert!(rig.scheduler.live_timer(RECHECK_EVERY).is_none());
        assert!(rig
            .ops
            .marker_reads
            .lock()
            .unwrap()
            .iter()
            .all(|read| *read == nonce));
        // `Open installer again` has a new nonce.
        rig.setup.install();
        rig.scheduler.drain();
        let installers = rig.ops.installers.lock().unwrap();
        assert_eq!(installers.len(), 2);
        assert_ne!(installers[0], installers[1]);
        assert_eq!(rig.setup.status().installer_failure, None);
    }

    #[test]
    fn a_check_that_finds_omp_deletes_the_markers_and_one_that_does_not_keeps_them() {
        let rig = rig();
        rig.script_probe(ProbeResult::Missing { reason: None });
        rig.start("");
        rig.scheduler.drain();
        assert_eq!(*rig.ops.markers_cleared.lock().unwrap(), 0);
        rig.setup.install();
        rig.scheduler.drain();
        let tick = rig.scheduler.live_timer(RECHECK_EVERY).expect("running");
        rig.script_probe(ready());
        tick();
        rig.scheduler.drain();
        assert_eq!(rig.phase(), Phase::Ready);
        assert_eq!(*rig.ops.markers_cleared.lock().unwrap(), 1);
    }

    #[test]
    fn the_reason_of_a_failed_update_is_the_one_the_runner_gave() {
        let rig = rig();
        rig.script_probe(ProbeResult::Outdated {
            omp: omp(),
            version: Some("18.0.0".into()),
        });
        let explained = "omp could not verify a server's certificate; a corporate proxy that inspects HTTPS traffic is the usual cause, and turning on System Certificates in Weaver's settings may fix it";
        rig.ops
            .updates
            .lock()
            .unwrap()
            .push_back(Err(explained.to_owned()));
        rig.script_probe(ready());
        rig.start("");
        rig.scheduler.drain();
        assert_eq!(
            rig.setup.status().update_failure.as_deref(),
            Some(explained)
        );
    }

    // --- usage ---------------------------------------------------------------------------------

    fn usage_of(fetched_at: f64) -> Usage {
        Usage {
            fetched_at: Some(fetched_at),
            providers: vec![],
        }
    }

    fn ready_rig(path: &str) -> Rig {
        let rig = rig();
        rig.script_probe(ready());
        rig.start(path);
        rig.scheduler.drain();
        assert_eq!(rig.phase(), Phase::Ready);
        rig
    }

    fn script_usage(rig: &Rig, result: Result<Usage, String>) {
        rig.ops.usages.lock().unwrap().push_back(result);
    }

    #[test]
    fn nothing_is_read_while_omp_is_not_ready_or_by_itself() {
        let rig = rig();
        rig.script_probe(ProbeResult::Missing { reason: None });
        rig.start("");
        rig.scheduler.drain();
        assert!(rig.setup.begin_usage().is_none());
        rig.scheduler.drain();
        assert!(rig.ops.usage_calls.lock().unwrap().is_empty());
        assert!(rig.usage_emitted.lock().unwrap().is_empty());
        // Ready on its own reads nothing either: only an opening does.
        let ready = ready_rig("");
        assert!(ready.ops.usage_calls.lock().unwrap().is_empty());
        assert!(ready.usage_emitted.lock().unwrap().is_empty());
    }

    #[test]
    fn a_reading_runs_omp_for_the_connected_providers_and_tells_the_windows() {
        let rig = ready_rig("/omp");
        script_usage(&rig, Ok(usage_of(1.0)));
        let done = rig.setup.begin_usage().expect("omp is ready");
        assert_eq!(rig.setup.usage_status().phase, UsagePhase::Reading);
        rig.scheduler.drain();
        assert!(done.has_changed().is_err(), "the reading ended");
        assert_eq!(
            rig.ops.usage_calls.lock().unwrap().as_slice(),
            [(omp(), vec!["anthropic".to_owned()])]
        );
        let emitted = rig.usage_emitted.lock().unwrap();
        let phases: Vec<UsagePhase> = emitted.iter().map(|s| s.phase).collect();
        assert_eq!(phases, [UsagePhase::Reading, UsagePhase::Idle]);
        assert_eq!(emitted[1].usage, Some(usage_of(1.0)));
        assert!(emitted[1].revision > emitted[0].revision);
    }

    #[test]
    fn an_opening_or_a_retry_during_a_reading_waits_for_it() {
        let rig = ready_rig("");
        script_usage(&rig, Ok(usage_of(1.0)));
        let first = rig.setup.begin_usage().unwrap();
        let second = rig.setup.begin_usage().unwrap();
        assert_eq!(rig.scheduler.jobs.lock().unwrap().len(), 1, "one reading");
        rig.scheduler.drain();
        assert!(first.has_changed().is_err() && second.has_changed().is_err());
        assert_eq!(rig.ops.usage_calls.lock().unwrap().len(), 1);
        // The next opening reads again.
        script_usage(&rig, Ok(usage_of(2.0)));
        rig.setup.begin_usage().unwrap();
        rig.scheduler.drain();
        assert_eq!(rig.ops.usage_calls.lock().unwrap().len(), 2);
        assert_eq!(rig.setup.usage_status().usage, Some(usage_of(2.0)));
    }

    #[test]
    fn a_failed_reading_keeps_the_last_limits_and_a_retry_reads_again() {
        let rig = ready_rig("");
        script_usage(&rig, Ok(usage_of(1.0)));
        rig.setup.begin_usage().unwrap();
        rig.scheduler.drain();
        script_usage(&rig, Err("timed out".into()));
        rig.setup.begin_usage().unwrap();
        rig.scheduler.drain();
        let status = rig.setup.usage_status();
        assert_eq!(status.phase, UsagePhase::Failed);
        assert_eq!(status.failure.as_deref(), Some("timed out"));
        assert_eq!(status.usage, Some(usage_of(1.0)));
        script_usage(&rig, Ok(usage_of(3.0)));
        rig.setup.begin_usage().unwrap();
        rig.scheduler.drain();
        let status = rig.setup.usage_status();
        assert_eq!((status.phase, status.failure), (UsagePhase::Idle, None));
        assert_eq!(status.usage, Some(usage_of(3.0)));
    }

    #[test]
    fn a_reading_that_ends_without_anyone_waiting_still_counts() {
        // The dialog was closed: nobody holds the receiver any more.
        let rig = ready_rig("");
        script_usage(&rig, Ok(usage_of(7.0)));
        drop(rig.setup.begin_usage().unwrap());
        rig.scheduler.drain();
        assert_eq!(rig.setup.usage_status().usage, Some(usage_of(7.0)));
    }

    #[test]
    fn a_change_of_the_omp_path_drops_the_limits_and_ignores_a_reading_in_flight() {
        let rig = ready_rig("/old");
        script_usage(&rig, Ok(usage_of(1.0)));
        rig.setup.begin_usage().unwrap();
        rig.scheduler.drain();
        assert!(rig.setup.usage_status().usage.is_some());

        // A reading is in flight when the path changes.
        script_usage(&rig, Ok(usage_of(2.0)));
        rig.setup.begin_usage().unwrap();
        rig.script_probe(ready());
        rig.setup.change_settings("/new", false);
        assert_eq!(rig.setup.usage_status().usage, None);
        assert_eq!(rig.setup.usage_status().phase, UsagePhase::Idle);
        rig.scheduler.drain();
        // Its result was for "/old": ignored.
        assert_eq!(rig.setup.usage_status().usage, None);
        assert_eq!(rig.phase(), Phase::Ready);
    }
}
