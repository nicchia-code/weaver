//! Updates of Weaver (`requirements/release#General`, "Updates of Weaver").
//!
//! * The Tauri updater plugin looks for `latest.json` at the endpoint of `tauri.conf.json`.
//! * A check runs at start and then every [`CHECK_INTERVAL`] (one hour) while Weaver runs
//!   ([`run_schedule`]). The instant of each check goes to the settings ([`AppSettings`]).
//!   A failed check is logged at `warn` and retried at the next due time; the UI never hears of it.
//! * An update found is offered with its version and notes ([`UpdateStatus`], event
//!   `update-changed`). It is installed only by **Install and restart** ([`update_install`]);
//!   **Later** ([`update_later`]) hides it until the next check that finds it.
//! * **Install and restart** goes through the path of quitting Weaver: the update is downloaded
//!   and its signature verified first, then the app is asked to exit with [`UPDATE_EXIT_CODE`],
//!   which raises `RunEvent::ExitRequested` like any other quit, so the quit path asks its
//!   confirmation when jobs are working. The update is applied at `RunEvent::Exit`
//!   ([`Updates::install_on_exit`]) and the app starts again. The quit path must therefore exit
//!   again with any code once the developer confirms; a quit the developer started
//!   (`ExitRequested` without a code) disarms the update ([`Updates::on_exit_requested`]).
//! * An update whose signature does not verify is refused, logged and not offered again.
//!
//! The plugin is behind [`UpdateSource`] and the clock, the settings, the log and the event
//! are injected, so the tests drive a fake updater.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_updater::UpdaterExt;
use ts_rs::TS;
use weaver_core::project::clock::{now_instant, Clock, SystemClock};

use crate::blocking;
use crate::error::{AppError, ErrorCode};
use crate::logging::{self, Level};
use crate::state::AppState;

/// At most one check in this time while Weaver runs.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// The exit code of the exit that **Install and restart** asks for.
pub const UPDATE_EXIT_CODE: i32 = i32::MAX - 1;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// An update the endpoint announced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    /// The notes of the release, when it has any.
    pub notes: Option<String>,
}

/// What the UI shows: the update on offer (none when there is none or **Later** hid it) and
/// whether it is being downloaded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    pub available: Option<UpdateInfo>,
    pub downloading: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    /// The signature of the update does not verify against the public key of the app.
    #[error("{0}")]
    Signature(String),
    #[error("{0}")]
    Other(String),
}

/// The updater, as the controller needs it.
pub trait UpdateSource: Send + Sync + 'static {
    /// Asks the endpoint whether a newer version exists.
    fn check(&self) -> BoxFuture<'_, Result<Option<UpdateInfo>, UpdateError>>;
    /// Downloads the update the last check found and verifies its signature.
    fn download(&self) -> BoxFuture<'_, Result<(), UpdateError>>;
    /// Applies the downloaded update (on Windows the installer starts and the process ends).
    fn install(&self) -> Result<(), UpdateError>;
}

/// Where the instant of a check is stored.
pub trait LastCheckStore: Send + Sync + 'static {
    fn record(&self, instant: &str) -> Result<(), String>;
}

pub type Emit = Arc<dyn Fn(&UpdateStatus) + Send + Sync>;
pub type Journal = Arc<dyn Fn(Level, String) + Send + Sync>;

#[derive(Default)]
struct Inner {
    available: Option<UpdateInfo>,
    hidden: bool,
    downloading: bool,
    /// The version whose package is downloaded and verified.
    downloaded: Option<String>,
    /// A version whose signature did not verify; it is not offered again.
    refused: Option<String>,
    /// **Install and restart** was chosen and the exit is on its way.
    armed: bool,
}

impl Inner {
    fn status(&self) -> UpdateStatus {
        UpdateStatus {
            available: if self.hidden {
                None
            } else {
                self.available.clone()
            },
            downloading: self.downloading,
        }
    }
}

pub struct Updates {
    source: Arc<dyn UpdateSource>,
    store: Arc<dyn LastCheckStore>,
    clock: Arc<dyn Clock>,
    emit: Emit,
    journal: Journal,
    state: Mutex<Inner>,
}

impl Updates {
    pub fn new(
        source: Arc<dyn UpdateSource>,
        store: Arc<dyn LastCheckStore>,
        clock: Arc<dyn Clock>,
        emit: Emit,
        journal: Journal,
    ) -> Self {
        Self {
            source,
            store,
            clock,
            emit,
            journal,
            state: Mutex::new(Inner::default()),
        }
    }

    fn log(&self, level: Level, message: String) {
        (self.journal)(level, message);
    }

    fn state(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.state.lock().expect("update state poisoned")
    }

    pub fn status(&self) -> UpdateStatus {
        self.state().status()
    }

    /// Runs `change` on the state and tells the UI when what it shows changed.
    fn update(&self, change: impl FnOnce(&mut Inner)) {
        let (before, after) = {
            let mut state = self.state();
            let before = state.status();
            change(&mut state);
            (before, state.status())
        };
        if before != after {
            (self.emit)(&after);
        }
    }

    /// One check. A failure is logged and goes no further; the next one is due at the next
    /// interval. The instant of the attempt is stored either way.
    pub async fn check(&self) {
        if self.state().downloading {
            return;
        }
        let outcome = self.source.check().await;
        self.record_attempt().await;
        match outcome {
            Ok(found) => {
                self.update(|state| {
                    let found = found
                        .filter(|info| state.refused.as_deref() != Some(info.version.as_str()));
                    if state.downloaded.as_deref()
                        != found.as_ref().map(|info| info.version.as_str())
                    {
                        state.downloaded = None;
                        state.armed = false;
                    }
                    state.available = found;
                    // "Later" lasts until the next check.
                    state.hidden = false;
                });
            }
            Err(error) => self.log(Level::Warn, format!("update check failed: {error}")),
        }
    }

    async fn record_attempt(&self) {
        let instant = match now_instant(self.clock.as_ref()) {
            Ok(instant) => instant,
            Err(error) => {
                self.log(
                    Level::Warn,
                    format!("the time of the update check was not stored: {error}"),
                );
                return;
            }
        };
        let store = self.store.clone();
        let stored =
            blocking::disk_named("record_update_check", None, move || store.record(&instant)).await;
        match stored {
            Ok(Ok(())) => {}
            Ok(Err(reason)) => self.log(
                Level::Warn,
                format!("the time of the update check was not stored: {reason}"),
            ),
            Err(error) => self.log(
                Level::Warn,
                format!("the time of the update check was not stored: {error}"),
            ),
        }
    }

    /// **Later**: hides the update until the next check.
    pub fn later(&self) {
        self.update(|state| {
            state.hidden = true;
            state.armed = false;
        });
    }

    /// First half of **Install and restart**: downloads the update and verifies its signature,
    /// then arms it. The caller asks the app to exit with [`UPDATE_EXIT_CODE`] when this is `Ok`.
    /// A signature that does not verify refuses the update: it is logged and not offered again.
    pub async fn prepare_install(&self) -> Result<(), AppError> {
        let version = {
            let mut state = self.state();
            let Some(info) = state.available.clone() else {
                return Err(AppError::new(
                    ErrorCode::InvalidInput,
                    "There is no update to install.",
                ));
            };
            if state.downloading {
                return Err(AppError::new(
                    ErrorCode::InvalidInput,
                    "The update is already downloading.",
                ));
            }
            if state.downloaded.as_deref() == Some(info.version.as_str()) {
                state.armed = true;
                return Ok(());
            }
            info.version
        };
        self.update(|state| state.downloading = true);
        let outcome = self.source.download().await;
        let mut result = Ok(());
        self.update(|state| {
            state.downloading = false;
            match &outcome {
                Ok(()) => {
                    state.downloaded = Some(version.clone());
                    state.armed = true;
                }
                Err(UpdateError::Signature(_)) => {
                    state.refused = Some(version.clone());
                    state.available = None;
                    state.downloaded = None;
                }
                Err(UpdateError::Other(_)) => {}
            }
        });
        match outcome {
            Ok(()) => {}
            Err(UpdateError::Signature(reason)) => {
                self.log(
                    Level::Error,
                    format!("update {version} refused: its signature does not verify ({reason})"),
                );
                result = Err(AppError::internal(
                    "The update was refused: its signature does not verify.",
                ));
            }
            Err(UpdateError::Other(reason)) => {
                self.log(
                    Level::Warn,
                    format!("update {version} was not downloaded: {reason}"),
                );
                result = Err(AppError::internal(format!(
                    "Could not download the update: {reason}"
                )));
            }
        }
        result
    }

    /// `RunEvent::ExitRequested`: an exit the developer started (no code) leaves the update
    /// unapplied; any other, including the quit path's own once confirmed, keeps it armed.
    pub fn on_exit_requested(&self, code: Option<i32>) {
        if code.is_none() {
            self.state().armed = false;
        }
    }

    /// `RunEvent::Exit`: applies the update armed by **Install and restart**. Returns `true` when
    /// it was applied and the app has to start again (on Windows the installer does that and
    /// this never returns).
    pub fn install_on_exit(&self) -> bool {
        let version = {
            let mut state = self.state();
            if !std::mem::take(&mut state.armed) {
                return false;
            }
            state
                .available
                .as_ref()
                .map(|info| info.version.clone())
                .unwrap_or_default()
        };
        self.log(Level::Info, format!("installing update {version}"));
        match self.source.install() {
            Ok(()) => true,
            Err(error) => {
                self.log(
                    Level::Error,
                    format!("update {version} was not installed: {error}"),
                );
                false
            }
        }
    }
}

/// Checks at once and then every `interval`; `sleep` waits for it (Tokio's in the app, the
/// test's own in tests). Never returns.
pub async fn run_schedule<S>(updates: Arc<Updates>, interval: Duration, sleep: S)
where
    S: Fn(Duration) -> BoxFuture<'static, ()>,
{
    loop {
        updates.check().await;
        sleep(interval).await;
    }
}

// --- the Tauri updater plugin ------------------------------------------------------------------

/// [`UpdateSource`] over `tauri-plugin-updater`.
pub struct PluginSource {
    app: AppHandle,
    found: Mutex<Option<tauri_plugin_updater::Update>>,
    package: Mutex<Option<(String, Vec<u8>)>>,
}

impl PluginSource {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            found: Mutex::new(None),
            package: Mutex::new(None),
        }
    }
}

fn classify(error: tauri_plugin_updater::Error) -> UpdateError {
    use tauri_plugin_updater::Error;
    let text = error.to_string();
    match error {
        Error::Minisign(_)
        | Error::Base64(_)
        | Error::SignatureUtf8(_)
        | Error::SignedVersionMismatch { .. } => UpdateError::Signature(text),
        _ => UpdateError::Other(text),
    }
}

impl UpdateSource for PluginSource {
    fn check(&self) -> BoxFuture<'_, Result<Option<UpdateInfo>, UpdateError>> {
        Box::pin(async move {
            let found = self
                .app
                .updater()
                .map_err(classify)?
                .check()
                .await
                .map_err(classify)?;
            let info = found.as_ref().map(|update| UpdateInfo {
                version: update.version.clone(),
                notes: update.body.clone().filter(|notes| !notes.trim().is_empty()),
            });
            let mut package = self.package.lock().expect("update package poisoned");
            if package
                .as_ref()
                .is_some_and(|(version, _)| Some(version) != info.as_ref().map(|i| &i.version))
            {
                *package = None;
            }
            *self.found.lock().expect("found update poisoned") = found;
            Ok(info)
        })
    }

    fn download(&self) -> BoxFuture<'_, Result<(), UpdateError>> {
        Box::pin(async move {
            let update = self
                .found
                .lock()
                .expect("found update poisoned")
                .clone()
                .ok_or_else(|| UpdateError::Other("no update was found".to_owned()))?;
            // `download` verifies the signature before it returns the bytes.
            let bytes = update.download(|_, _| {}, || {}).await.map_err(classify)?;
            *self.package.lock().expect("update package poisoned") =
                Some((update.version.clone(), bytes));
            Ok(())
        })
    }

    fn install(&self) -> Result<(), UpdateError> {
        let update = self
            .found
            .lock()
            .expect("found update poisoned")
            .clone()
            .ok_or_else(|| UpdateError::Other("no update was found".to_owned()))?;
        let (version, bytes) = self
            .package
            .lock()
            .expect("update package poisoned")
            .take()
            .ok_or_else(|| UpdateError::Other("the update was not downloaded".to_owned()))?;
        if version != update.version {
            return Err(UpdateError::Other(
                "the downloaded update is not the one announced".to_owned(),
            ));
        }
        update.install(bytes).map_err(classify)
    }
}

// --- the settings ---------------------------------------------------------------------------------

/// Stores the instant of each check in the settings of the app (`lastUpdateCheck` of
/// `settings.json`, written by [`crate::settings::SettingsService`]).
pub struct AppSettings {
    app: AppHandle,
}

impl AppSettings {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl LastCheckStore for AppSettings {
    fn record(&self, instant: &str) -> Result<(), String> {
        self.app
            .state::<AppState>()
            .settings
            .record_update_check(instant)
    }
}

// --- start-up and commands ----------------------------------------------------------------------

/// Creates the controller over the real updater, manages it and starts the schedule.
pub fn start(app: &AppHandle) {
    let emit_handle = app.clone();
    let updates = Arc::new(Updates::new(
        Arc::new(PluginSource::new(app.clone())),
        Arc::new(AppSettings::new(app.clone())),
        Arc::new(SystemClock),
        Arc::new(move |status| {
            if let Err(error) = crate::events::emit_update_changed(&emit_handle, status) {
                crate::log_error!("cannot tell the UI about the update: {error}");
            }
        }),
        Arc::new(|level, message| {
            logging::log(
                level,
                module_path!(),
                logging::Context::NONE,
                format_args!("{message}"),
            );
        }),
    ));
    app.manage(updates.clone());
    tauri::async_runtime::spawn(run_schedule(updates, CHECK_INTERVAL, |duration| {
        Box::pin(tokio::time::sleep(duration))
    }));
}

#[tauri::command]
pub fn update_status(updates: State<'_, Arc<Updates>>) -> UpdateStatus {
    updates.status()
}

#[tauri::command]
pub fn update_later(updates: State<'_, Arc<Updates>>) {
    updates.later();
}

/// **Install and restart**: downloads, then quits through the app's quit path; the update is
/// applied when the app exits.
#[tauri::command]
pub async fn update_install(
    app: AppHandle,
    updates: State<'_, Arc<Updates>>,
) -> Result<(), AppError> {
    updates.prepare_install().await?;
    app.exit(UPDATE_EXIT_CODE);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use weaver_core::project::clock::ManualClock;

    use super::*;

    fn info(version: &str) -> UpdateInfo {
        UpdateInfo {
            version: version.to_owned(),
            notes: Some(format!("Notes of {version}")),
        }
    }

    #[derive(Default)]
    struct FakeSource {
        checks: Mutex<VecDeque<Result<Option<UpdateInfo>, UpdateError>>>,
        downloads: Mutex<VecDeque<Result<(), UpdateError>>>,
        checked: AtomicUsize,
        downloaded: AtomicUsize,
        installed: AtomicUsize,
    }

    impl FakeSource {
        fn with_checks(checks: Vec<Result<Option<UpdateInfo>, UpdateError>>) -> Arc<Self> {
            Arc::new(Self {
                checks: Mutex::new(checks.into()),
                ..Self::default()
            })
        }
        fn then_download(&self, outcome: Result<(), UpdateError>) {
            self.downloads.lock().unwrap().push_back(outcome);
        }
    }

    impl UpdateSource for FakeSource {
        fn check(&self) -> BoxFuture<'_, Result<Option<UpdateInfo>, UpdateError>> {
            self.checked.fetch_add(1, Ordering::SeqCst);
            let next = self.checks.lock().unwrap().pop_front().unwrap_or(Ok(None));
            Box::pin(async move { next })
        }
        fn download(&self) -> BoxFuture<'_, Result<(), UpdateError>> {
            self.downloaded.fetch_add(1, Ordering::SeqCst);
            let next = self.downloads.lock().unwrap().pop_front().unwrap_or(Ok(()));
            Box::pin(async move { next })
        }
        fn install(&self) -> Result<(), UpdateError> {
            self.installed.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    #[derive(Default)]
    struct MemoryStore(Mutex<Vec<String>>);

    impl LastCheckStore for MemoryStore {
        fn record(&self, instant: &str) -> Result<(), String> {
            self.0.lock().unwrap().push(instant.to_owned());
            Ok(())
        }
    }

    struct Harness {
        updates: Arc<Updates>,
        source: Arc<FakeSource>,
        store: Arc<MemoryStore>,
        clock: Arc<ManualClock>,
        shown: Arc<Mutex<Vec<UpdateStatus>>>,
        log: Arc<Mutex<Vec<(Level, String)>>>,
    }

    fn harness(checks: Vec<Result<Option<UpdateInfo>, UpdateError>>) -> Harness {
        let source = FakeSource::with_checks(checks);
        let store = Arc::new(MemoryStore::default());
        let clock = Arc::new(ManualClock::at("2026-11-02T07:00:00.000Z"));
        let shown = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::new(Mutex::new(Vec::new()));
        let (shown_sink, log_sink) = (shown.clone(), log.clone());
        let updates = Arc::new(Updates::new(
            source.clone(),
            store.clone(),
            clock.clone(),
            Arc::new(move |status| shown_sink.lock().unwrap().push(status.clone())),
            Arc::new(move |level, message| log_sink.lock().unwrap().push((level, message))),
        ));
        Harness {
            updates,
            source,
            store,
            clock,
            shown,
            log,
        }
    }

    impl Harness {
        fn offered(&self) -> Option<String> {
            self.updates.status().available.map(|info| info.version)
        }
        fn logged(&self, level: Level) -> Vec<String> {
            self.log
                .lock()
                .unwrap()
                .iter()
                .filter(|(l, _)| *l == level)
                .map(|(_, m)| m.clone())
                .collect()
        }
    }

    /// The schedule asks for its sleeps one at a time; the test grants each one.
    struct Ticks {
        asked: tokio::sync::mpsc::UnboundedReceiver<Duration>,
        grant: Arc<tokio::sync::Semaphore>,
    }

    fn scheduled(
        updates: Arc<Updates>,
        interval: Duration,
    ) -> (tokio::task::JoinHandle<()>, Ticks) {
        let (asked_tx, asked) = tokio::sync::mpsc::unbounded_channel();
        let grant = Arc::new(tokio::sync::Semaphore::new(0));
        let sleeping = grant.clone();
        let task = tokio::spawn(run_schedule(updates, interval, move |duration| {
            let _ = asked_tx.send(duration);
            let sleeping = sleeping.clone();
            Box::pin(async move {
                sleeping.acquire().await.expect("semaphore open").forget();
            })
        }));
        (task, Ticks { asked, grant })
    }

    #[tokio::test]
    async fn checks_at_start_and_then_once_every_hour() {
        let h = harness(vec![
            Ok(None),
            Ok(Some(info("0.2.0"))),
            Ok(Some(info("0.2.0"))),
        ]);
        let (task, mut ticks) = scheduled(h.updates.clone(), CHECK_INTERVAL);

        // The first check runs at start, then the schedule waits an hour.
        assert_eq!(ticks.asked.recv().await, Some(Duration::from_secs(60 * 60)));
        assert_eq!(h.source.checked.load(Ordering::SeqCst), 1);
        assert_eq!(h.offered(), None);

        ticks.grant.add_permits(1);
        assert_eq!(ticks.asked.recv().await, Some(CHECK_INTERVAL));
        assert_eq!(h.source.checked.load(Ordering::SeqCst), 2);
        assert_eq!(h.offered().as_deref(), Some("0.2.0"));

        // No third check before the second hour is over.
        assert!(ticks.asked.try_recv().is_err());
        assert_eq!(h.source.checked.load(Ordering::SeqCst), 2);
        ticks.grant.add_permits(1);
        assert_eq!(ticks.asked.recv().await, Some(CHECK_INTERVAL));
        assert_eq!(h.source.checked.load(Ordering::SeqCst), 3);
        task.abort();
    }

    #[tokio::test]
    async fn every_check_stores_its_instant_even_a_failed_one() {
        let h = harness(vec![Err(UpdateError::Other("offline".into())), Ok(None)]);
        h.updates.check().await;
        h.clock.advance(CHECK_INTERVAL.as_millis() as i64);
        h.updates.check().await;
        assert_eq!(
            *h.store.0.lock().unwrap(),
            ["2026-11-02T07:00:00.000Z", "2026-11-02T08:00:00.000Z"]
        );
    }

    #[tokio::test]
    async fn a_failed_check_is_logged_never_shown_and_retried_at_the_next_due_time() {
        let h = harness(vec![
            Err(UpdateError::Other("could not reach github.com".into())),
            Ok(Some(info("0.2.0"))),
        ]);
        let (task, mut ticks) = scheduled(h.updates.clone(), CHECK_INTERVAL);

        ticks.asked.recv().await;
        assert_eq!(
            h.logged(Level::Warn),
            ["update check failed: could not reach github.com"]
        );
        assert!(h.logged(Level::Error).is_empty());
        assert!(h.shown.lock().unwrap().is_empty(), "the UI is told nothing");
        assert_eq!(h.updates.status(), UpdateStatus::default());

        ticks.grant.add_permits(1);
        ticks.asked.recv().await;
        assert_eq!(h.offered().as_deref(), Some("0.2.0"));
        task.abort();
    }

    #[tokio::test]
    async fn an_update_found_shows_its_version_and_notes() {
        let h = harness(vec![Ok(Some(info("0.2.0")))]);
        h.updates.check().await;
        let expected = UpdateStatus {
            available: Some(info("0.2.0")),
            downloading: false,
        };
        assert_eq!(h.updates.status(), expected);
        assert_eq!(*h.shown.lock().unwrap(), [expected]);
    }

    #[tokio::test]
    async fn later_hides_the_update_until_the_next_check() {
        let h = harness(vec![
            Ok(Some(info("0.2.0"))),
            Err(UpdateError::Other("offline".into())),
            Ok(Some(info("0.2.0"))),
        ]);
        h.updates.check().await;
        h.updates.later();
        assert_eq!(h.offered(), None);
        assert_eq!(
            h.shown.lock().unwrap().last().unwrap(),
            &UpdateStatus::default()
        );

        // A check that fails shows nothing either.
        h.updates.check().await;
        assert_eq!(h.offered(), None);

        h.updates.check().await;
        assert_eq!(h.offered().as_deref(), Some("0.2.0"));
        assert_eq!(
            h.shown.lock().unwrap().len(),
            3,
            "offered, hidden, offered again"
        );
        assert_eq!(
            h.source.installed.load(Ordering::SeqCst),
            0,
            "Later never installs"
        );
    }

    #[tokio::test]
    async fn an_update_whose_signature_does_not_verify_is_refused_and_logged() {
        let h = harness(vec![
            Ok(Some(info("0.2.0"))),
            Ok(Some(info("0.2.0"))),
            Ok(Some(info("0.3.0"))),
        ]);
        h.source.then_download(Err(UpdateError::Signature(
            "Signature verification failed".into(),
        )));
        h.updates.check().await;

        let error = h.updates.prepare_install().await.unwrap_err();
        assert_eq!(
            error.message,
            "The update was refused: its signature does not verify."
        );
        assert_eq!(h.offered(), None);
        assert_eq!(
            h.logged(Level::Error),
            ["update 0.2.0 refused: its signature does not verify (Signature verification failed)"]
        );
        assert!(!h.updates.install_on_exit(), "nothing was armed");
        assert_eq!(h.source.installed.load(Ordering::SeqCst), 0);

        // The same version is not offered again; another one is.
        h.updates.check().await;
        assert_eq!(h.offered(), None);
        h.updates.check().await;
        assert_eq!(h.offered().as_deref(), Some("0.3.0"));
    }

    #[tokio::test]
    async fn install_and_restart_downloads_then_applies_the_update_when_the_app_exits() {
        let h = harness(vec![Ok(Some(info("0.2.0")))]);
        h.updates.check().await;

        h.updates.prepare_install().await.unwrap();
        assert_eq!(h.source.downloaded.load(Ordering::SeqCst), 1);
        assert_eq!(
            h.source.installed.load(Ordering::SeqCst),
            0,
            "nothing is applied before the app exits"
        );
        let statuses = h.shown.lock().unwrap().clone();
        assert!(
            statuses.iter().any(|s| s.downloading),
            "the UI sees the download"
        );
        assert!(!h.updates.status().downloading);

        // The quit path may ask its confirmation and then exit again with its own code.
        h.updates.on_exit_requested(Some(UPDATE_EXIT_CODE));
        h.updates.on_exit_requested(Some(0));
        assert!(h.updates.install_on_exit());
        assert_eq!(h.source.installed.load(Ordering::SeqCst), 1);
        assert!(!h.updates.install_on_exit(), "it is applied once");
    }

    #[tokio::test]
    async fn a_cancelled_quit_followed_by_the_developers_own_quit_installs_nothing() {
        let h = harness(vec![Ok(Some(info("0.2.0")))]);
        h.updates.check().await;
        h.updates.prepare_install().await.unwrap();
        h.updates.on_exit_requested(Some(UPDATE_EXIT_CODE)); // the confirmation was cancelled here
        h.updates.on_exit_requested(None); // later the developer quits on their own
        assert!(!h.updates.install_on_exit());
        assert_eq!(h.source.installed.load(Ordering::SeqCst), 0);

        // Choosing Install and restart again does not download twice.
        h.updates.prepare_install().await.unwrap();
        assert_eq!(h.source.downloaded.load(Ordering::SeqCst), 1);
        assert!(h.updates.install_on_exit());
    }

    #[tokio::test]
    async fn later_after_the_download_disarms_the_update() {
        let h = harness(vec![Ok(Some(info("0.2.0")))]);
        h.updates.check().await;
        h.updates.prepare_install().await.unwrap();
        h.updates.later();
        assert!(!h.updates.install_on_exit());
    }

    #[tokio::test]
    async fn a_failed_download_is_reported_to_the_caller_and_the_update_stays_on_offer() {
        let h = harness(vec![Ok(Some(info("0.2.0")))]);
        h.source
            .then_download(Err(UpdateError::Other("connection reset".into())));
        h.updates.check().await;
        let error = h.updates.prepare_install().await.unwrap_err();
        assert_eq!(
            error.message,
            "Could not download the update: connection reset"
        );
        assert_eq!(h.offered().as_deref(), Some("0.2.0"));
        assert!(!h.updates.status().downloading);
        assert!(!h.updates.install_on_exit());
    }

    #[tokio::test]
    async fn without_an_update_there_is_nothing_to_install() {
        let h = harness(vec![Ok(None)]);
        h.updates.check().await;
        let error = h.updates.prepare_install().await.unwrap_err();
        assert_eq!(error.code, ErrorCode::InvalidInput);
        assert_eq!(h.source.downloaded.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn the_status_reaches_the_ui_in_camel_case() {
        let json = serde_json::to_value(UpdateStatus {
            available: Some(info("0.2.0")),
            downloading: true,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "available": { "version": "0.2.0", "notes": "Notes of 0.2.0" }, "downloading": true })
        );
    }

    #[test]
    fn the_release_configuration_enables_signed_updates_from_the_releases_of_the_repository() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(config["bundle"]["createUpdaterArtifacts"], true);
        assert_eq!(
            config["plugins"]["updater"]["endpoints"],
            serde_json::json!([
                "https://github.com/nicchia-code/weaver/releases/latest/download/latest.json"
            ])
        );
        assert!(config["plugins"]["updater"]["pubkey"].is_string());
    }

    #[test]
    fn the_update_exit_code_is_not_one_tauri_or_the_shell_uses() {
        assert_ne!(UPDATE_EXIT_CODE, tauri::RESTART_EXIT_CODE);
        const { assert!(UPDATE_EXIT_CODE > 0) };
    }
}
