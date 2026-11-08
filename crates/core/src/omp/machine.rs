//! The Machine `omp-setup`: whether omp is ready for Weaver to start it, one for the whole app.
//!
//! It is a pure value: [`Machine::handle`] takes an [`Event`] and the current time in
//! milliseconds and returns the [`Effect`]s the app must run; it reads no clock, starts no process
//! and sleeps never. The app's `omp` module runs the effects and sends their results back as
//! events, so the tests drive every transition with events and a clock they hold.
//!
//! ```text
//! Checking ──READY──▶ Ready          ──MISSING──▶ Missing ──INSTALL_OMP──▶ Installing
//!    │                  │ ▲                          ▲                        │ RECHECK_DUE
//!    ├──OUTDATED──▶ Updating ──UPDATE_ENDED──▶ Checking                      │ (every 3 s)
//!    └──NO_PROVIDER──▶ Not connected ──LOG_IN_PROVIDER──▶ Logging in ◀───────┘
//! ```
//!
//! * Every check carries a [`Ticket`] (the omp path it was computed for and a sequence number); a
//!   result whose ticket is not the current one is ignored, so a result for a place of omp that
//!   is no longer the current one never reaches the state.
//! * Re-checks run only in Installing and Logging in, one at a time: [`Machine::wants_recheck`]
//!   says when the timer must run, and a `RECHECK_DUE` anywhere else changes nothing.
//! * A change of the omp path restarts the check from every state, also from Checking, Updating
//!   and Logging in (the Model names the other states; here no result for the old path is left
//!   to bring those states back).
//! * One `omp update` runs at a time: an update still running for an old path delays the next
//!   one until it ended.

use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use ts_rs::TS;

use super::failure::installer_failed_text;
use super::probe::{too_old_text, OmpModel, ProbeResult};
use super::providers::{provider, ProviderRow, PROVIDERS};
use super::version::MIN_OMP_VERSION;

/// How often Installing and Logging in look at omp again.
pub const RECHECK_EVERY: Duration = Duration::from_secs(3);

/// At most one check for a newer omp in this time, app-wide.
pub const UPDATE_CHECK_INTERVAL_MS: i64 = 60 * 60 * 1000;

/// The states of the Machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename = "OmpPhase")]
pub enum Phase {
    Checking,
    Missing,
    Installing,
    Updating,
    NotConnected,
    LoggingIn,
    Ready,
}

/// The terminal could not be opened: why, and the command the developer can copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TerminalProblem {
    pub reason: String,
    pub command: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum LogoutPhase {
    /// `omp auth-broker logout` runs.
    Running,
    /// It ended; Weaver asks omp whether the provider is still connected.
    Verifying,
    /// It failed; `reason` says why.
    Failed,
    /// It worked, and other credentials of the environment still connect the provider.
    StillConnected,
}

/// What the Providers row of a provider shows after a log out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogoutNote {
    pub provider: String,
    pub phase: LogoutPhase,
    pub reason: Option<String>,
}

/// Everything the UI shows of the Machine. Sent whole at each change.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OmpStatus {
    /// Increases with every change; a window keeps the highest it saw.
    #[ts(type = "number")]
    pub revision: u64,
    pub state: Phase,
    /// The `ompPath` setting this status is for.
    pub omp_path: String,
    /// The version omp answered with.
    pub version: Option<String>,
    pub min_version: String,
    /// The usable models, in omp's order; empty until omp answered.
    pub models: Vec<OmpModel>,
    /// The fixed list of providers; `connected` is `None` until omp answered.
    pub providers: Vec<ProviderRow>,
    /// Missing: why (`None`: omp is not installed).
    pub reason: Option<String>,
    /// Updating: the version being installed, when known.
    pub update_target: Option<String>,
    /// Ready: the update that failed and did not block.
    pub update_failure: Option<String>,
    /// Logging in: the id of the provider.
    pub login_provider: Option<String>,
    /// Installing and Logging in: the terminal could not be opened.
    pub terminal_problem: Option<TerminalProblem>,
    pub logout: Option<LogoutNote>,
    /// Checking at the app's start: the first check waits for the export of System Certificates
    /// (`Exporting system certificates first…`).
    pub waiting_for_export: bool,
    /// Missing after a failed installer: its text, with the exit code, for the setting as it is.
    pub installer_failure: Option<String>,
}

/// The input of a check: the omp path it was computed for, its sequence number, and the installer
/// whose marker it reads when it finds no omp (while Installing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ticket {
    pub omp_path: String,
    pub seq: u64,
    /// The nonce of the installer in flight; `None` in the other states.
    pub installer: Option<String>,
}

/// What happens to the Machine.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The app started: look at omp (`omp_path` is the setting, trimmed; `system_certificates`
    /// the `systemCertificates.enabled` setting). With `exporting` the check waits for the export
    /// of System Certificates that runs.
    Start {
        omp_path: String,
        system_certificates: bool,
        exporting: bool,
    },
    /// A check ended: `omp.READY`, `omp.MISSING`, `omp.OUTDATED` or `omp.NO_PROVIDER`.
    Probed { ticket: Ticket, result: ProbeResult },
    /// `omp.UPDATE_ENDED`.
    UpdateEnded { omp_path: String },
    /// `omp.UPDATE_FAILED`, with what a check found right after.
    UpdateFailed {
        omp_path: String,
        reason: String,
        after: ProbeResult,
    },
    /// The background check ended: `Some(version)` is `omp.UPDATE_AVAILABLE`.
    UpdateCheckDone {
        omp_path: String,
        available: Option<String>,
    },
    /// `omp.MISSING` while Ready: omp disappeared (a job could not start it).
    OmpMissing {
        omp_path: String,
        reason: Option<String>,
    },
    /// `time.RECHECK_DUE`.
    RecheckDue,
    /// `time.UPDATE_CHECK_DUE`.
    UpdateCheckDue,
    /// `developer.INSTALL_OMP`, with the nonce of this installer's marker.
    InstallOmp { nonce: String },
    /// `developer.LOG_IN_PROVIDER`, with an id of the fixed list.
    LogInProvider { provider: String },
    /// `developer.LOG_OUT_PROVIDER`.
    LogOutProvider { provider: String },
    /// `developer.CHANGE_SETTINGS`: the `ompPath` setting is now `omp_path` (trimmed) and the
    /// `systemCertificates.enabled` setting is now `system_certificates`.
    ChangeSettings {
        omp_path: String,
        system_certificates: bool,
    },
    /// `system-certificates.EXPORT_ENDED`: an export ended, with success or not.
    ExportEnded,
    /// The wait for the export at the app's start is over (it ended or 30 s passed).
    ExportWaitOver,
    /// The system terminal could not be opened.
    TerminalFailed { reason: String, command: String },
    /// `omp auth-broker logout` ended.
    LoggedOut {
        omp_path: String,
        provider: String,
        outcome: Result<(), String>,
    },
}

/// What the app must do after an event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Run the lookup, `omp --version` and `omp models --json`; answer with `Event::Probed`.
    /// With `ticket.installer` set, a check that finds no omp reads the marker of that installer
    /// and answers `ProbeResult::InstallFailed` for a code other than `0`.
    Probe(Ticket),
    /// At the app's start, while System Certificates exports its bundle: wait for the end of the
    /// export (30 s at most), then answer with `Event::ExportWaitOver`.
    WaitForExport,
    /// Run `omp update`; answer with `UpdateEnded`, or with `UpdateFailed` after a new check.
    RunUpdate { omp_path: String, omp: PathBuf },
    /// Run `omp update --check` in the background; answer with `UpdateCheckDone`.
    CheckForUpdate { omp_path: String, omp: PathBuf },
    /// Open the system terminal with the official installer, which leaves its exit code in the
    /// marker of `nonce`.
    OpenInstaller { nonce: String },
    /// Open the system terminal with `omp login <provider>`.
    OpenLogin { omp: PathBuf, provider: String },
    /// Run `omp auth-broker logout <provider>`; answer with `LoggedOut`.
    Logout {
        omp_path: String,
        omp: PathBuf,
        provider: String,
    },
}

/// omp, as a job needs it once the Machine is Ready.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadyOmp {
    pub omp: PathBuf,
    pub omp_path: String,
    pub version: String,
    pub models: Vec<OmpModel>,
}

/// The Machine `omp-setup`.
#[derive(Debug, Clone)]
pub struct Machine {
    status: OmpStatus,
    started: bool,
    /// The absolute path of the omp that answered.
    found: Option<PathBuf>,
    /// omp has answered, so `models` says which providers are connected.
    answered: bool,
    /// The check in flight; its result is the only one accepted.
    ticket: Option<Ticket>,
    seq: u64,
    /// The omp path an `omp update` is running for.
    update_running: Option<String>,
    /// Updating, with the update of an old path still running.
    update_waiting: bool,
    last_update_check_ms: Option<i64>,
    /// The `systemCertificates.enabled` setting as the Machine last heard of it.
    system_certificates: bool,
    /// The installer started last, by the nonce of its marker.
    install_nonce: Option<String>,
    /// The exit code of a failed installer, kept until the next `INSTALL_OMP` or until omp is found.
    install_exit_code: Option<i32>,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    /// Checking, before the app told it where omp is: no check runs yet.
    pub fn new() -> Self {
        let mut machine = Self {
            status: OmpStatus {
                revision: 0,
                state: Phase::Checking,
                omp_path: String::new(),
                version: None,
                min_version: MIN_OMP_VERSION.to_owned(),
                models: Vec::new(),
                providers: Vec::new(),
                reason: None,
                update_target: None,
                update_failure: None,
                login_provider: None,
                terminal_problem: None,
                logout: None,
                waiting_for_export: false,
                installer_failure: None,
            },
            started: false,
            found: None,
            answered: false,
            ticket: None,
            seq: 0,
            update_running: None,
            update_waiting: false,
            last_update_check_ms: None,
            system_certificates: false,
            install_nonce: None,
            install_exit_code: None,
        };
        machine.refresh_providers();
        machine
    }

    pub fn status(&self) -> &OmpStatus {
        &self.status
    }

    pub fn phase(&self) -> Phase {
        self.status.state
    }

    /// Whether the timer of `RECHECK_DUE` must run: only in Installing and Logging in.
    pub fn wants_recheck(&self) -> bool {
        matches!(self.status.state, Phase::Installing | Phase::LoggingIn)
    }

    /// omp for a job, only when the Machine is Ready: nothing starts before.
    pub fn ready(&self) -> Option<ReadyOmp> {
        if self.status.state != Phase::Ready {
            return None;
        }
        Some(ReadyOmp {
            omp: self.found.clone()?,
            omp_path: self.status.omp_path.clone(),
            version: self.status.version.clone()?,
            models: self.status.models.clone(),
        })
    }

    /// Applies `event` at `now_ms` (the injected clock) and returns what the app must run.
    pub fn handle(&mut self, event: Event, now_ms: i64) -> Vec<Effect> {
        let before = self.status.clone();
        let effects = self.apply(event, now_ms);
        self.refresh_installer_failure();
        if self.status != before {
            self.status.revision = before.revision + 1;
        }
        effects
    }

    fn apply(&mut self, event: Event, now_ms: i64) -> Vec<Effect> {
        match event {
            Event::Start {
                omp_path,
                system_certificates,
                exporting,
            } => {
                if self.started {
                    return Vec::new();
                }
                self.started = true;
                self.system_certificates = system_certificates;
                self.status.omp_path = omp_path.trim().to_owned();
                if exporting {
                    // At the app's start: the check waits for the export to end, and says so.
                    self.status.waiting_for_export = true;
                    vec![Effect::WaitForExport]
                } else {
                    vec![self.start_probe()]
                }
            }
            Event::ChangeSettings {
                omp_path,
                system_certificates,
            } => {
                if !self.started {
                    return Vec::new();
                }
                let omp_path = omp_path.trim().to_owned();
                let turned_off = self.system_certificates && !system_certificates;
                self.system_certificates = system_certificates;
                if omp_path != self.status.omp_path {
                    self.reset();
                    self.status.omp_path = omp_path;
                    return vec![self.start_probe()];
                }
                // Turned on, it does not look again here: the end of its export does.
                if turned_off
                    && matches!(
                        self.status.state,
                        Phase::Missing | Phase::NotConnected | Phase::Ready
                    )
                {
                    self.set_state(Phase::Checking);
                    return vec![self.start_probe()];
                }
                Vec::new()
            }
            Event::Probed { ticket, result } => {
                if self.ticket.as_ref() != Some(&ticket) {
                    return Vec::new();
                }
                self.ticket = None;
                // An exit code is the one of the installer this check was made for, and only that.
                let result = match result {
                    ProbeResult::InstallFailed { .. }
                        if ticket.installer.is_none() || ticket.installer != self.install_nonce =>
                    {
                        ProbeResult::Missing { reason: None }
                    }
                    other => other,
                };
                match self.status.state {
                    Phase::Checking | Phase::Installing | Phase::LoggingIn => {
                        self.apply_probe(result)
                    }
                    _ => Vec::new(),
                }
            }
            Event::RecheckDue => {
                if self.wants_recheck() && self.ticket.is_none() {
                    vec![self.start_probe()]
                } else {
                    Vec::new()
                }
            }
            Event::InstallOmp { nonce } => match self.status.state {
                Phase::Missing | Phase::Installing => {
                    self.set_state(Phase::Installing);
                    self.status.terminal_problem = None;
                    // The exit code of an earlier installer is forgotten.
                    self.install_nonce = Some(nonce.clone());
                    self.install_exit_code = None;
                    vec![Effect::OpenInstaller { nonce }]
                }
                _ => Vec::new(),
            },
            Event::LogInProvider { provider: id } => {
                let usable = matches!(
                    self.status.state,
                    Phase::NotConnected | Phase::LoggingIn | Phase::Ready
                );
                let (Some(provider), Some(omp)) = (provider(&id), self.found.clone()) else {
                    return Vec::new();
                };
                if !usable {
                    return Vec::new();
                }
                self.status.logout = None;
                self.set_state(Phase::LoggingIn);
                self.status.login_provider = Some(provider.id.to_owned());
                self.status.terminal_problem = None;
                vec![Effect::OpenLogin {
                    omp,
                    provider: provider.id.to_owned(),
                }]
            }
            Event::LogOutProvider { provider: id } => {
                let (Some(provider), Some(omp)) = (provider(&id), self.found.clone()) else {
                    return Vec::new();
                };
                if self.status.state != Phase::Ready {
                    return Vec::new();
                }
                self.set_state(Phase::Checking);
                self.ticket = None;
                self.status.logout = Some(LogoutNote {
                    provider: provider.id.to_owned(),
                    phase: LogoutPhase::Running,
                    reason: None,
                });
                vec![Effect::Logout {
                    omp_path: self.status.omp_path.clone(),
                    omp,
                    provider: provider.id.to_owned(),
                }]
            }
            Event::LoggedOut {
                omp_path,
                provider,
                outcome,
            } => {
                let waiting =
                    self.status.state == Phase::Checking
                        && omp_path == self.status.omp_path
                        && self.status.logout.as_ref().is_some_and(|n| {
                            n.provider == provider && n.phase == LogoutPhase::Running
                        });
                if !waiting {
                    return Vec::new();
                }
                if let Some(note) = self.status.logout.as_mut() {
                    match outcome {
                        Ok(()) => note.phase = LogoutPhase::Verifying,
                        Err(reason) => {
                            note.phase = LogoutPhase::Failed;
                            note.reason = Some(reason);
                        }
                    }
                }
                vec![self.start_probe()]
            }
            Event::TerminalFailed { reason, command } => {
                if self.wants_recheck() {
                    self.status.terminal_problem = Some(TerminalProblem { reason, command });
                }
                Vec::new()
            }
            Event::ExportWaitOver => {
                if !self.status.waiting_for_export {
                    return Vec::new();
                }
                self.status.waiting_for_export = false;
                vec![self.start_probe()]
            }
            Event::ExportEnded => {
                // After every export, whatever the state of omp was: look at it again. The other
                // states have their own way to look again.
                if self.started
                    && matches!(
                        self.status.state,
                        Phase::Missing | Phase::NotConnected | Phase::Ready
                    )
                {
                    self.set_state(Phase::Checking);
                    vec![self.start_probe()]
                } else {
                    Vec::new()
                }
            }
            Event::UpdateCheckDue => {
                if self.status.state != Phase::Ready {
                    return Vec::new();
                }
                let due = self
                    .last_update_check_ms
                    .is_none_or(|last| now_ms.saturating_sub(last) >= UPDATE_CHECK_INTERVAL_MS);
                let Some(omp) = self.found.clone().filter(|_| due) else {
                    return Vec::new();
                };
                self.last_update_check_ms = Some(now_ms);
                vec![Effect::CheckForUpdate {
                    omp_path: self.status.omp_path.clone(),
                    omp,
                }]
            }
            Event::UpdateCheckDone {
                omp_path,
                available,
            } => {
                let Some(version) = available else {
                    return Vec::new();
                };
                if self.status.state != Phase::Ready || omp_path != self.status.omp_path {
                    return Vec::new();
                }
                self.set_state(Phase::Updating);
                self.status.update_target = Some(version);
                self.status.update_failure = None;
                self.start_update()
            }
            Event::OmpMissing { omp_path, reason } => {
                if self.status.state != Phase::Ready || omp_path != self.status.omp_path {
                    return Vec::new();
                }
                self.become_missing(reason);
                Vec::new()
            }
            Event::UpdateEnded { omp_path } => {
                self.update_over(&omp_path);
                if self.status.state == Phase::Updating && omp_path == self.status.omp_path {
                    self.update_waiting = false;
                    self.status.update_failure = None;
                    self.set_state(Phase::Checking);
                    return vec![self.start_probe()];
                }
                self.resume_waiting_update()
            }
            Event::UpdateFailed {
                omp_path,
                reason,
                after,
            } => {
                self.update_over(&omp_path);
                if self.status.state == Phase::Updating && omp_path == self.status.omp_path {
                    self.update_waiting = false;
                    match after {
                        ProbeResult::Ready {
                            omp,
                            version,
                            models,
                        } => {
                            self.answer(omp, version, models);
                            self.set_state(Phase::Ready);
                            self.status.update_failure = Some(reason);
                        }
                        ProbeResult::Outdated { version, .. } => {
                            self.become_missing(Some(too_old_text(version.as_deref(), &reason)));
                        }
                        ProbeResult::Missing { .. }
                        | ProbeResult::InstallFailed { .. }
                        | ProbeResult::NoProvider { .. } => {
                            self.become_missing(Some(format!(
                                "Could not update the agent ({}).",
                                reason.trim_end_matches('.')
                            )));
                        }
                    }
                    return Vec::new();
                }
                self.resume_waiting_update()
            }
        }
    }

    /// A check ended in Checking, Installing or Logging in.
    fn apply_probe(&mut self, result: ProbeResult) -> Vec<Effect> {
        let state = self.status.state;
        match result {
            ProbeResult::Ready {
                omp,
                version,
                models,
            } => {
                self.answer(omp, version, models);
                self.set_state(Phase::Ready);
                self.settle_logout();
                Vec::new()
            }
            ProbeResult::Missing { reason } => {
                // The installer has not finished yet: Installing waits for it.
                if state != Phase::Installing {
                    self.become_missing(reason);
                }
                Vec::new()
            }
            ProbeResult::InstallFailed { exit_code } => {
                if state == Phase::Installing {
                    self.install_exit_code = Some(exit_code);
                    self.become_missing(None);
                }
                Vec::new()
            }
            ProbeResult::Outdated { omp, version } => {
                self.omp_found();
                self.found = Some(omp);
                self.status.version = version;
                self.set_state(Phase::Updating);
                self.status.update_failure = None;
                self.start_update()
            }
            ProbeResult::NoProvider { omp, version } => {
                self.answer(omp, version, Vec::new());
                // The login has not finished yet: Logging in waits for it.
                if state != Phase::LoggingIn {
                    self.set_state(Phase::NotConnected);
                }
                self.settle_logout();
                Vec::new()
            }
        }
    }

    /// omp answered: its place, version and models.
    fn answer(&mut self, omp: PathBuf, version: String, models: Vec<OmpModel>) {
        self.omp_found();
        self.found = Some(omp);
        self.status.version = Some(version);
        self.status.models = models;
        self.answered = true;
        self.refresh_providers();
    }

    fn become_missing(&mut self, reason: Option<String>) {
        self.set_state(Phase::Missing);
        self.status.reason = reason;
        self.found = None;
        self.status.version = None;
        self.status.models.clear();
        self.answered = false;
        self.refresh_providers();
    }

    /// After a log out and a new check: nothing to say when the provider is gone, a note when
    /// other credentials still connect it.
    fn settle_logout(&mut self) {
        let Some(note) = self.status.logout.as_mut() else {
            return;
        };
        if note.phase != LogoutPhase::Verifying {
            return;
        }
        let connected = self
            .status
            .models
            .iter()
            .any(|m| m.provider == note.provider);
        if connected {
            note.phase = LogoutPhase::StillConnected;
        } else {
            self.status.logout = None;
        }
    }

    /// Moves to `state`, dropping what belongs only to the states that are left.
    fn set_state(&mut self, state: Phase) {
        let status = &mut self.status;
        if !matches!(state, Phase::Installing | Phase::LoggingIn) {
            status.terminal_problem = None;
        }
        if state != Phase::LoggingIn {
            status.login_provider = None;
        }
        if state != Phase::Missing {
            status.reason = None;
        }
        if state != Phase::Updating {
            status.update_target = None;
        }
        if matches!(state, Phase::Missing | Phase::Updating) {
            status.logout = None;
        }
        status.state = state;
    }

    /// Forgets everything about the old omp path and goes back to Checking.
    fn reset(&mut self) {
        self.set_state(Phase::Checking);
        self.found = None;
        self.answered = false;
        self.status.version = None;
        self.status.models.clear();
        self.status.update_failure = None;
        self.status.logout = None;
        self.status.waiting_for_export = false;
        self.update_waiting = false;
        self.ticket = None;
        self.refresh_providers();
    }

    fn start_probe(&mut self) -> Effect {
        self.seq += 1;
        // While Installing, a check that finds no omp reads the marker of the current installer.
        let installer = if self.status.state == Phase::Installing {
            self.install_nonce.clone()
        } else {
            None
        };
        let ticket = Ticket {
            omp_path: self.status.omp_path.clone(),
            seq: self.seq,
            installer,
        };
        self.ticket = Some(ticket.clone());
        Effect::Probe(ticket)
    }

    /// Runs `omp update`, unless one is still running for an old path.
    fn start_update(&mut self) -> Vec<Effect> {
        let Some(omp) = self.found.clone() else {
            return Vec::new();
        };
        if self.update_running.is_some() {
            self.update_waiting = true;
            return Vec::new();
        }
        self.update_waiting = false;
        self.update_running = Some(self.status.omp_path.clone());
        vec![Effect::RunUpdate {
            omp_path: self.status.omp_path.clone(),
            omp,
        }]
    }

    fn update_over(&mut self, omp_path: &str) {
        if self.update_running.as_deref() == Some(omp_path) {
            self.update_running = None;
        }
    }

    fn resume_waiting_update(&mut self) -> Vec<Effect> {
        if self.status.state == Phase::Updating && self.update_waiting {
            self.start_update()
        } else {
            Vec::new()
        }
    }

    /// The text of a failed installer shows only in Missing, written for the setting as it is.
    fn refresh_installer_failure(&mut self) {
        self.status.installer_failure = (self.status.state == Phase::Missing)
            .then_some(self.install_exit_code)
            .flatten()
            .map(|code| installer_failed_text(code, self.system_certificates));
    }

    /// omp was found: what the installer left is forgotten.
    fn omp_found(&mut self) {
        self.install_nonce = None;
        self.install_exit_code = None;
    }

    fn refresh_providers(&mut self) {
        self.status.providers = PROVIDERS
            .iter()
            .map(|p| ProviderRow {
                id: p.id.to_owned(),
                name: p.name.to_owned(),
                plan: p.plan.to_owned(),
                connected: self
                    .answered
                    .then(|| self.status.models.iter().any(|m| m.provider == p.id)),
            })
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_000_000;
    const HOUR: i64 = UPDATE_CHECK_INTERVAL_MS;

    fn omp() -> PathBuf {
        PathBuf::from("/opt/omp/bin/omp")
    }

    fn model(provider: &str) -> OmpModel {
        OmpModel {
            provider: provider.into(),
            selector: format!("{provider}/m"),
            name: None,
            thinking_levels: vec![],
        }
    }

    fn ready_result(providers: &[&str]) -> ProbeResult {
        ProbeResult::Ready {
            omp: omp(),
            version: "18.15.0".into(),
            models: providers.iter().map(|p| model(p)).collect(),
        }
    }

    fn no_provider() -> ProbeResult {
        ProbeResult::NoProvider {
            omp: omp(),
            version: "18.15.0".into(),
        }
    }

    fn missing() -> ProbeResult {
        ProbeResult::Missing { reason: None }
    }

    fn outdated() -> ProbeResult {
        ProbeResult::Outdated {
            omp: omp(),
            version: Some("18.0.0".into()),
        }
    }

    /// A Machine and the clock the test holds.
    struct Rig {
        machine: Machine,
        now: i64,
        /// The check in flight, as the app would hold it.
        probe: Option<Ticket>,
    }

    impl Rig {
        fn started(path: &str) -> Self {
            let mut rig = Rig {
                machine: Machine::new(),
                now: T0,
                probe: None,
            };
            let effects = rig.send(start_event(path));
            assert_eq!(effects.len(), 1);
            rig
        }

        fn send(&mut self, event: Event) -> Vec<Effect> {
            let effects = self.machine.handle(event, self.now);
            for effect in &effects {
                if let Effect::Probe(ticket) = effect {
                    self.probe = Some(ticket.clone());
                }
            }
            effects
        }

        /// The check in flight ends with `result`.
        fn probed(&mut self, result: ProbeResult) -> Vec<Effect> {
            let ticket = self.probe.take().expect("a check is in flight");
            self.send(Event::Probed { ticket, result })
        }

        fn state(&self) -> Phase {
            self.machine.phase()
        }

        /// Gets to `state` through the transitions of the Model.
        fn reach(path: &str, state: Phase) -> Self {
            let mut rig = Rig::started(path);
            match state {
                Phase::Checking => {}
                Phase::Ready => {
                    rig.probed(ready_result(&["anthropic"]));
                }
                Phase::Missing => {
                    rig.probed(missing());
                }
                Phase::Installing => {
                    rig.probed(missing());
                    rig.send(install());
                }
                Phase::Updating => {
                    rig.probed(outdated());
                }
                Phase::NotConnected => {
                    rig.probed(no_provider());
                }
                Phase::LoggingIn => {
                    rig.probed(no_provider());
                    rig.send(Event::LogInProvider {
                        provider: "anthropic".into(),
                    });
                }
            }
            assert_eq!(rig.state(), state);
            rig
        }
    }

    fn is_probe(effects: &[Effect]) -> bool {
        matches!(effects, [Effect::Probe(_)])
    }

    fn start_event(path: &str) -> Event {
        Event::Start {
            omp_path: path.into(),
            system_certificates: false,
            exporting: false,
        }
    }

    /// `developer.CHANGE_SETTINGS` for a new `ompPath` with System Certificates off.
    fn change_path(path: &str) -> Event {
        Event::ChangeSettings {
            omp_path: path.into(),
            system_certificates: false,
        }
    }

    /// `developer.CHANGE_SETTINGS` that only moves System Certificates.
    fn change_certificates(path: &str, on: bool) -> Event {
        Event::ChangeSettings {
            omp_path: path.into(),
            system_certificates: on,
        }
    }

    fn install() -> Event {
        Event::InstallOmp { nonce: "aa".into() }
    }

    fn opens_installer() -> Vec<Effect> {
        vec![Effect::OpenInstaller { nonce: "aa".into() }]
    }

    // ---- Checking ------------------------------------------------------------------------

    #[test]
    fn before_the_app_starts_it_it_is_checking_and_runs_nothing() {
        let mut machine = Machine::new();
        assert_eq!(machine.phase(), Phase::Checking);
        assert!(machine.handle(Event::RecheckDue, T0).is_empty());
        assert!(machine.handle(change_path("/x"), T0).is_empty());
        assert!(machine.handle(Event::ExportEnded, T0).is_empty());
        assert_eq!(machine.status().revision, 0);
    }

    #[test]
    fn starting_checks_omp_for_the_trimmed_path_once() {
        let mut rig = Rig::started("  /opt/omp  ");
        assert_eq!(
            rig.probe,
            Some(Ticket {
                omp_path: "/opt/omp".into(),
                seq: 1,
                installer: None
            })
        );
        assert!(rig.send(start_event("/other")).is_empty());
        assert_eq!(rig.machine.status().omp_path, "/opt/omp");
        assert!(!rig.machine.status().waiting_for_export);
    }

    #[test]
    fn checking_goes_where_the_check_says() {
        assert_eq!(Rig::reach("", Phase::Ready).state(), Phase::Ready);
        assert_eq!(Rig::reach("", Phase::Missing).state(), Phase::Missing);
        assert_eq!(
            Rig::reach("", Phase::NotConnected).state(),
            Phase::NotConnected
        );
        let mut rig = Rig::started("");
        let effects = rig.probed(outdated());
        assert_eq!(rig.state(), Phase::Updating);
        assert_eq!(
            effects,
            [Effect::RunUpdate {
                omp_path: "".into(),
                omp: omp()
            }]
        );
    }

    #[test]
    fn ready_shows_version_models_and_connected_providers_for_a_job() {
        let rig = Rig::reach("", Phase::Ready);
        let status = rig.machine.status();
        assert_eq!(status.version.as_deref(), Some("18.15.0"));
        assert_eq!(status.models.len(), 1);
        let connected: Vec<(&str, Option<bool>)> = status
            .providers
            .iter()
            .map(|p| (p.id.as_str(), p.connected))
            .collect();
        assert_eq!(connected[0], ("openai-codex", Some(false)));
        assert_eq!(connected[1], ("anthropic", Some(true)));
        assert_eq!(status.providers.len(), 7);
        let ready = rig.machine.ready().unwrap();
        assert_eq!((ready.omp, ready.version), (omp(), "18.15.0".to_owned()));
    }

    #[test]
    fn no_job_gets_omp_in_any_state_but_ready() {
        for state in [
            Phase::Checking,
            Phase::Missing,
            Phase::Installing,
            Phase::Updating,
            Phase::NotConnected,
            Phase::LoggingIn,
        ] {
            assert_eq!(Rig::reach("", state).machine.ready(), None, "{state:?}");
        }
        assert!(Rig::reach("", Phase::Ready).machine.ready().is_some());
    }

    #[test]
    fn providers_are_unknown_until_omp_answers() {
        let rig = Rig::started("");
        assert!(rig
            .machine
            .status()
            .providers
            .iter()
            .all(|p| p.connected.is_none()));
        let missing = Rig::reach("", Phase::Missing);
        assert!(missing
            .machine
            .status()
            .providers
            .iter()
            .all(|p| p.connected.is_none()));
        let not_connected = Rig::reach("", Phase::NotConnected);
        assert!(not_connected
            .machine
            .status()
            .providers
            .iter()
            .all(|p| p.connected == Some(false)));
    }

    // ---- Missing and Installing ------------------------------------------------------------

    #[test]
    fn missing_keeps_the_reason_and_offers_the_installer() {
        let mut rig = Rig::started("/gone/omp");
        rig.probed(ProbeResult::Missing {
            reason: Some("the agent not found at /gone/omp".into()),
        });
        assert_eq!(
            rig.machine.status().reason.as_deref(),
            Some("the agent not found at /gone/omp")
        );
        assert_eq!(rig.send(install()), opens_installer());
        assert_eq!(rig.state(), Phase::Installing);
        assert_eq!(rig.machine.status().reason, None);
    }

    #[test]
    fn install_omp_means_nothing_outside_missing_and_installing() {
        for state in [
            Phase::Checking,
            Phase::Updating,
            Phase::NotConnected,
            Phase::LoggingIn,
            Phase::Ready,
        ] {
            let mut rig = Rig::reach("", state);
            assert!(rig.send(install()).is_empty(), "{state:?}");
            assert_eq!(rig.state(), state);
        }
    }

    #[test]
    fn installing_rechecks_one_at_a_time_and_waits_while_omp_is_missing() {
        let mut rig = Rig::reach("", Phase::Installing);
        assert!(rig.machine.wants_recheck());
        // The check of the move to Installing is over; the next one starts at RECHECK_DUE.
        assert!(is_probe(&rig.send(Event::RecheckDue)));
        // A second RECHECK_DUE while the check runs starts nothing.
        assert!(rig.send(Event::RecheckDue).is_empty());
        assert!(rig.probed(missing()).is_empty());
        assert_eq!(rig.state(), Phase::Installing);
        assert!(is_probe(&rig.send(Event::RecheckDue)));
    }

    #[test]
    fn installing_leaves_on_ready_outdated_and_no_provider() {
        let mut rig = Rig::reach("", Phase::Installing);
        rig.send(Event::RecheckDue);
        rig.probed(ready_result(&["anthropic"]));
        assert_eq!(rig.state(), Phase::Ready);

        let mut rig = Rig::reach("", Phase::Installing);
        rig.send(Event::RecheckDue);
        let effects = rig.probed(outdated());
        assert_eq!(rig.state(), Phase::Updating);
        assert!(matches!(effects.as_slice(), [Effect::RunUpdate { .. }]));

        let mut rig = Rig::reach("", Phase::Installing);
        rig.send(Event::RecheckDue);
        rig.probed(no_provider());
        assert_eq!(rig.state(), Phase::NotConnected);
    }

    #[test]
    fn the_installer_opens_again_from_installing() {
        let mut rig = Rig::reach("", Phase::Installing);
        rig.send(Event::TerminalFailed {
            reason: "no terminal emulator was found".into(),
            command: "curl x | sh".into(),
        });
        assert_eq!(
            rig.machine
                .status()
                .terminal_problem
                .as_ref()
                .unwrap()
                .command,
            "curl x | sh"
        );
        assert_eq!(rig.send(install()), opens_installer());
        assert_eq!(rig.state(), Phase::Installing);
        assert_eq!(rig.machine.status().terminal_problem, None);
    }

    #[test]
    fn a_terminal_failure_is_remembered_only_while_installing_or_logging_in() {
        let mut rig = Rig::reach("", Phase::Ready);
        rig.send(Event::TerminalFailed {
            reason: "r".into(),
            command: "c".into(),
        });
        assert_eq!(rig.machine.status().terminal_problem, None);
        let mut rig = Rig::reach("", Phase::LoggingIn);
        rig.send(Event::TerminalFailed {
            reason: "r".into(),
            command: "c".into(),
        });
        assert!(rig.machine.status().terminal_problem.is_some());
        rig.send(Event::RecheckDue);
        rig.probed(ready_result(&["anthropic"]));
        assert_eq!(rig.machine.status().terminal_problem, None);
    }

    // ---- Rechecks stop when the state is left --------------------------------------------

    #[test]
    fn rechecks_run_only_in_installing_and_logging_in() {
        for state in [
            Phase::Checking,
            Phase::Missing,
            Phase::Updating,
            Phase::NotConnected,
            Phase::Ready,
        ] {
            let mut rig = Rig::reach("", state);
            assert!(!rig.machine.wants_recheck(), "{state:?}");
            // A tick that arrives late, after the state was left, does nothing.
            let before = rig.machine.status().clone();
            assert!(rig.send(Event::RecheckDue).is_empty(), "{state:?}");
            assert_eq!(rig.machine.status(), &before);
        }
        assert!(Rig::reach("", Phase::Installing).machine.wants_recheck());
        assert!(Rig::reach("", Phase::LoggingIn).machine.wants_recheck());
    }

    #[test]
    fn leaving_installing_or_logging_in_stops_the_rechecks() {
        let mut rig = Rig::reach("", Phase::Installing);
        rig.send(Event::RecheckDue);
        rig.probed(ready_result(&["anthropic"]));
        assert!(!rig.machine.wants_recheck());
        assert!(rig.send(Event::RecheckDue).is_empty());

        let mut rig = Rig::reach("", Phase::LoggingIn);
        rig.send(change_path("/other"));
        assert!(!rig.machine.wants_recheck());
        assert!(rig.send(Event::RecheckDue).is_empty());
    }

    // ---- Updating --------------------------------------------------------------------------

    #[test]
    fn updating_waits_for_the_update_and_ignores_stray_checks() {
        let mut rig = Rig::reach("", Phase::Updating);
        assert_eq!(rig.machine.ready(), None);
        assert!(rig
            .send(Event::Probed {
                ticket: Ticket {
                    omp_path: "".into(),
                    seq: 99,
                    installer: None
                },
                result: ready_result(&["anthropic"])
            })
            .is_empty());
        assert_eq!(rig.state(), Phase::Updating);
    }

    #[test]
    fn an_update_that_ended_leads_back_to_checking() {
        let mut rig = Rig::reach("", Phase::Updating);
        let effects = rig.send(Event::UpdateEnded {
            omp_path: "".into(),
        });
        assert_eq!(rig.state(), Phase::Checking);
        assert!(is_probe(&effects));
        rig.probed(ready_result(&["anthropic"]));
        assert_eq!(rig.state(), Phase::Ready);
        assert_eq!(rig.machine.status().update_failure, None);
    }

    #[test]
    fn a_failed_update_of_a_working_omp_is_shown_and_does_not_block() {
        let mut rig = Rig::reach("", Phase::Updating);
        let effects = rig.send(Event::UpdateFailed {
            omp_path: "".into(),
            reason: "timed out".into(),
            after: ready_result(&["anthropic"]),
        });
        assert!(effects.is_empty());
        assert_eq!(rig.state(), Phase::Ready);
        assert_eq!(
            rig.machine.status().update_failure.as_deref(),
            Some("timed out")
        );
        assert!(rig.machine.ready().is_some());
    }

    #[test]
    fn a_failed_update_of_an_omp_still_too_old_counts_as_missing_with_the_failure() {
        let mut rig = Rig::reach("", Phase::Updating);
        rig.send(Event::UpdateFailed {
            omp_path: "".into(),
            reason: "exit code 1".into(),
            after: ProbeResult::Outdated {
                omp: omp(),
                version: Some("18.0.0".into()),
            },
        });
        assert_eq!(rig.state(), Phase::Missing);
        assert_eq!(
            rig.machine.status().reason.as_deref(),
            Some("the agent 18.0.0 is older than 18.15.0 and could not be updated (exit code 1).")
        );
        // The installer is offered.
        assert_eq!(rig.send(install()), opens_installer());
    }

    #[test]
    fn a_failed_update_after_which_omp_does_not_answer_is_missing_too() {
        let mut rig = Rig::reach("", Phase::Updating);
        rig.send(Event::UpdateFailed {
            omp_path: "".into(),
            reason: "timed out".into(),
            after: missing(),
        });
        assert_eq!(rig.state(), Phase::Missing);
        assert_eq!(
            rig.machine.status().reason.as_deref(),
            Some("Could not update the agent (timed out).")
        );
    }

    #[test]
    fn the_failure_of_an_update_is_forgotten_by_the_next_update() {
        let mut rig = Rig::reach("", Phase::Updating);
        rig.send(Event::UpdateFailed {
            omp_path: "".into(),
            reason: "boom".into(),
            after: ready_result(&["anthropic"]),
        });
        rig.now += HOUR;
        rig.send(Event::UpdateCheckDue);
        rig.send(Event::UpdateCheckDone {
            omp_path: "".into(),
            available: Some("18.16.0".into()),
        });
        assert_eq!(rig.machine.status().update_failure, None);
    }

    #[test]
    fn the_reason_of_a_failed_download_shows_in_ready_and_in_missing_without_a_final_full_stop() {
        use super::super::failure::{download_failed_reason, DownloadFailure};
        for (failure, on) in [
            (DownloadFailure::Exit(90), false),
            (DownloadFailure::Exit(91), false),
            (DownloadFailure::Exit(91), true),
            (DownloadFailure::Exit(92), true),
            (DownloadFailure::Exit(93), false),
            (DownloadFailure::Exit(7), true),
            (DownloadFailure::TimedOut, false),
        ] {
            let reason = download_failed_reason(failure, on);
            let mut rig = Rig::reach("", Phase::Updating);
            rig.send(Event::UpdateFailed {
                omp_path: "".into(),
                reason: reason.clone(),
                after: ready_result(&["anthropic"]),
            });
            assert_eq!(rig.state(), Phase::Ready, "{failure:?}");
            assert_eq!(
                rig.machine.status().update_failure.as_deref(),
                Some(reason.as_str())
            );

            let mut rig = Rig::reach("", Phase::Updating);
            rig.send(Event::UpdateFailed {
                omp_path: "".into(),
                reason: reason.clone(),
                after: missing(),
            });
            assert_eq!(rig.state(), Phase::Missing, "{failure:?}");
            assert_eq!(
                rig.machine.status().reason,
                Some(format!("Could not update the agent ({reason})."))
            );
        }
    }

    // ---- Not connected and Logging in -----------------------------------------------------

    #[test]
    fn not_connected_opens_the_login_of_a_provider_of_the_list() {
        let mut rig = Rig::reach("", Phase::NotConnected);
        assert!(rig
            .send(Event::LogInProvider {
                provider: "evil; rm -rf /".into()
            })
            .is_empty());
        assert_eq!(rig.state(), Phase::NotConnected);
        let effects = rig.send(Event::LogInProvider {
            provider: "cursor".into(),
        });
        assert_eq!(
            effects,
            [Effect::OpenLogin {
                omp: omp(),
                provider: "cursor".into()
            }]
        );
        assert_eq!(rig.state(), Phase::LoggingIn);
        assert_eq!(
            rig.machine.status().login_provider.as_deref(),
            Some("cursor")
        );
    }

    #[test]
    fn logging_in_waits_for_a_model_and_rechecks() {
        let mut rig = Rig::reach("", Phase::LoggingIn);
        assert!(is_probe(&rig.send(Event::RecheckDue)));
        assert!(rig.probed(no_provider()).is_empty());
        assert_eq!(rig.state(), Phase::LoggingIn);
        rig.send(Event::RecheckDue);
        rig.probed(ready_result(&["anthropic"]));
        assert_eq!(rig.state(), Phase::Ready);
        assert_eq!(rig.machine.status().login_provider, None);
    }

    #[test]
    fn logging_in_leaves_on_missing_and_the_login_opens_again() {
        let mut rig = Rig::reach("", Phase::LoggingIn);
        rig.send(Event::RecheckDue);
        rig.probed(ProbeResult::Missing {
            reason: Some("the agent did not answer within 15 s".into()),
        });
        assert_eq!(rig.state(), Phase::Missing);

        let mut rig = Rig::reach("", Phase::LoggingIn);
        let effects = rig.send(Event::LogInProvider {
            provider: "xai-oauth".into(),
        });
        assert_eq!(
            effects,
            [Effect::OpenLogin {
                omp: omp(),
                provider: "xai-oauth".into()
            }]
        );
        assert_eq!(
            rig.machine.status().login_provider.as_deref(),
            Some("xai-oauth")
        );
    }

    #[test]
    fn logging_in_with_an_old_omp_goes_to_updating() {
        let mut rig = Rig::reach("", Phase::LoggingIn);
        rig.send(Event::RecheckDue);
        let effects = rig.probed(outdated());
        assert_eq!(rig.state(), Phase::Updating);
        assert!(matches!(effects.as_slice(), [Effect::RunUpdate { .. }]));
    }

    // ---- Ready -------------------------------------------------------------------------------

    #[test]
    fn ready_can_log_in_again() {
        let mut rig = Rig::reach("", Phase::Ready);
        let effects = rig.send(Event::LogInProvider {
            provider: "anthropic".into(),
        });
        assert_eq!(
            effects,
            [Effect::OpenLogin {
                omp: omp(),
                provider: "anthropic".into()
            }]
        );
        assert_eq!(rig.state(), Phase::LoggingIn);
    }

    #[test]
    fn logging_out_goes_through_checking_and_says_nothing_when_the_provider_is_gone() {
        let mut rig = Rig::reach("", Phase::Ready);
        let effects = rig.send(Event::LogOutProvider {
            provider: "anthropic".into(),
        });
        assert_eq!(
            effects,
            [Effect::Logout {
                omp_path: "".into(),
                omp: omp(),
                provider: "anthropic".into()
            }]
        );
        assert_eq!(rig.state(), Phase::Checking);
        assert_eq!(
            rig.machine.status().logout.as_ref().unwrap().phase,
            LogoutPhase::Running
        );
        assert_eq!(rig.machine.ready(), None);

        let effects = rig.send(Event::LoggedOut {
            omp_path: "".into(),
            provider: "anthropic".into(),
            outcome: Ok(()),
        });
        assert!(is_probe(&effects));
        assert_eq!(
            rig.machine.status().logout.as_ref().unwrap().phase,
            LogoutPhase::Verifying
        );
        rig.probed(ready_result(&["openai-codex"]));
        assert_eq!(rig.state(), Phase::Ready);
        assert_eq!(rig.machine.status().logout, None);
    }

    #[test]
    fn a_provider_that_stays_connected_after_logout_is_reported() {
        let mut rig = Rig::reach("", Phase::Ready);
        rig.send(Event::LogOutProvider {
            provider: "anthropic".into(),
        });
        rig.send(Event::LoggedOut {
            omp_path: "".into(),
            provider: "anthropic".into(),
            outcome: Ok(()),
        });
        rig.probed(ready_result(&["anthropic"]));
        let note = rig.machine.status().logout.clone().unwrap();
        assert_eq!(
            (note.provider.as_str(), note.phase),
            ("anthropic", LogoutPhase::StillConnected)
        );
    }

    #[test]
    fn logging_out_the_last_provider_leads_to_not_connected() {
        let mut rig = Rig::reach("", Phase::Ready);
        rig.send(Event::LogOutProvider {
            provider: "anthropic".into(),
        });
        rig.send(Event::LoggedOut {
            omp_path: "".into(),
            provider: "anthropic".into(),
            outcome: Ok(()),
        });
        rig.probed(no_provider());
        assert_eq!(rig.state(), Phase::NotConnected);
        assert_eq!(rig.machine.status().logout, None);
    }

    #[test]
    fn a_failed_logout_keeps_its_reason_and_checks_again() {
        let mut rig = Rig::reach("", Phase::Ready);
        rig.send(Event::LogOutProvider {
            provider: "anthropic".into(),
        });
        let effects = rig.send(Event::LoggedOut {
            omp_path: "".into(),
            provider: "anthropic".into(),
            outcome: Err("exit code 1".into()),
        });
        assert!(is_probe(&effects));
        rig.probed(ready_result(&["anthropic"]));
        assert_eq!(rig.state(), Phase::Ready);
        let note = rig.machine.status().logout.clone().unwrap();
        assert_eq!(
            (note.phase, note.reason.as_deref()),
            (LogoutPhase::Failed, Some("exit code 1"))
        );
        // The next login forgets it.
        rig.send(Event::LogInProvider {
            provider: "anthropic".into(),
        });
        assert_eq!(rig.machine.status().logout, None);
    }

    #[test]
    fn logging_out_is_for_ready_only() {
        for state in [
            Phase::Missing,
            Phase::Installing,
            Phase::Updating,
            Phase::NotConnected,
            Phase::LoggingIn,
        ] {
            let mut rig = Rig::reach("", state);
            assert!(
                rig.send(Event::LogOutProvider {
                    provider: "anthropic".into()
                })
                .is_empty(),
                "{state:?}"
            );
        }
    }

    #[test]
    fn a_logout_result_that_nobody_waits_for_is_ignored() {
        let mut rig = Rig::reach("", Phase::Ready);
        assert!(rig
            .send(Event::LoggedOut {
                omp_path: "".into(),
                provider: "anthropic".into(),
                outcome: Ok(())
            })
            .is_empty());
        assert_eq!(rig.state(), Phase::Ready);
    }

    #[test]
    fn omp_that_disappears_while_ready_is_missing() {
        let mut rig = Rig::reach("", Phase::Ready);
        rig.send(Event::OmpMissing {
            omp_path: "".into(),
            reason: Some("could not start the agent".into()),
        });
        assert_eq!(rig.state(), Phase::Missing);
        assert_eq!(
            rig.machine.status().reason.as_deref(),
            Some("could not start the agent")
        );
        assert_eq!(rig.machine.ready(), None);
        // Said for another path, it changes nothing.
        let mut rig = Rig::reach("", Phase::Ready);
        rig.send(Event::OmpMissing {
            omp_path: "/old".into(),
            reason: None,
        });
        assert_eq!(rig.state(), Phase::Ready);
    }

    // ---- Changes of the setting ---------------------------------------------------------------

    #[test]
    fn a_change_of_the_path_restarts_the_check_from_every_state() {
        for state in [
            Phase::Checking,
            Phase::Missing,
            Phase::Installing,
            Phase::Updating,
            Phase::NotConnected,
            Phase::LoggingIn,
            Phase::Ready,
        ] {
            let mut rig = Rig::reach("", state);
            let effects = rig.send(change_path(" /new/omp "));
            assert_eq!(rig.state(), Phase::Checking, "{state:?}");
            assert!(is_probe(&effects), "{state:?}");
            assert_eq!(rig.machine.status().omp_path, "/new/omp");
            assert_eq!(rig.machine.status().version, None);
            assert!(rig
                .machine
                .status()
                .providers
                .iter()
                .all(|p| p.connected.is_none()));
        }
    }

    #[test]
    fn a_change_that_leaves_the_path_as_it_is_does_nothing() {
        let mut rig = Rig::reach("/opt/omp", Phase::Missing);
        assert!(rig.send(change_path("  /opt/omp ")).is_empty());
        assert_eq!(rig.state(), Phase::Missing);
    }

    #[test]
    fn a_result_for_the_old_path_is_ignored() {
        let mut rig = Rig::started("/old/omp");
        let old = rig.probe.clone().unwrap();
        rig.send(change_path("/new/omp"));
        // The old check ends after the setting changed.
        assert!(rig
            .send(Event::Probed {
                ticket: old,
                result: ready_result(&["anthropic"])
            })
            .is_empty());
        assert_eq!(rig.state(), Phase::Checking);
        // The new one is taken.
        rig.probed(missing());
        assert_eq!(rig.state(), Phase::Missing);
    }

    #[test]
    fn a_result_of_the_same_path_but_an_older_check_is_ignored() {
        let mut rig = Rig::started("/a");
        let first = rig.probe.clone().unwrap();
        rig.send(change_path("/b"));
        rig.send(change_path("/a"));
        assert_eq!(rig.probe.as_ref().unwrap().omp_path, "/a");
        assert!(rig
            .send(Event::Probed {
                ticket: first,
                result: ready_result(&[])
            })
            .is_empty());
        assert_eq!(rig.state(), Phase::Checking);
    }

    #[test]
    fn an_update_that_ended_for_the_old_path_does_not_move_the_new_state() {
        let mut rig = Rig::reach("/old", Phase::Updating);
        // The developer changes the path; the update of the old omp keeps running.
        rig.send(change_path("/new"));
        assert!(rig
            .send(Event::UpdateEnded {
                omp_path: "/old".into()
            })
            .is_empty());
        assert_eq!(rig.state(), Phase::Checking);
        assert!(rig
            .send(Event::UpdateFailed {
                omp_path: "/old".into(),
                reason: "x".into(),
                after: missing()
            })
            .is_empty());
        assert_eq!(rig.state(), Phase::Checking);
    }

    #[test]
    fn one_update_runs_at_a_time_and_the_next_waits_for_the_one_running() {
        let mut rig = Rig::reach("/old", Phase::Updating);
        rig.send(change_path("/new"));
        // The new omp is also outdated: no second `omp update` yet.
        let effects = rig.probed(outdated());
        assert_eq!(rig.state(), Phase::Updating);
        assert!(effects.is_empty());
        // The old one ends: the new one starts.
        let effects = rig.send(Event::UpdateEnded {
            omp_path: "/old".into(),
        });
        assert_eq!(
            effects,
            [Effect::RunUpdate {
                omp_path: "/new".into(),
                omp: omp()
            }]
        );
        // And its own end leads to Checking.
        let effects = rig.send(Event::UpdateEnded {
            omp_path: "/new".into(),
        });
        assert_eq!(rig.state(), Phase::Checking);
        assert!(is_probe(&effects));
    }

    // ---- Updates of omp while Ready ------------------------------------------------------------

    #[test]
    fn the_update_check_runs_in_ready_and_at_most_once_an_hour() {
        let mut rig = Rig::reach("", Phase::Ready);
        let effects = rig.send(Event::UpdateCheckDue);
        assert_eq!(
            effects,
            [Effect::CheckForUpdate {
                omp_path: "".into(),
                omp: omp()
            }]
        );
        rig.now += HOUR - 1;
        assert!(rig.send(Event::UpdateCheckDue).is_empty());
        rig.now += 1;
        assert!(!rig.send(Event::UpdateCheckDue).is_empty());
        // The check never changes the state by itself: a job is never made to wait for it.
        assert_eq!(rig.state(), Phase::Ready);
        assert!(rig.machine.ready().is_some());
    }

    #[test]
    fn the_update_check_is_for_ready_only() {
        for state in [
            Phase::Checking,
            Phase::Missing,
            Phase::Installing,
            Phase::Updating,
            Phase::NotConnected,
            Phase::LoggingIn,
        ] {
            let mut rig = Rig::reach("", state);
            assert!(rig.send(Event::UpdateCheckDue).is_empty(), "{state:?}");
        }
    }

    #[test]
    fn a_newer_version_moves_ready_to_updating_and_runs_the_update() {
        let mut rig = Rig::reach("", Phase::Ready);
        rig.send(Event::UpdateCheckDue);
        let effects = rig.send(Event::UpdateCheckDone {
            omp_path: "".into(),
            available: Some("18.16.0".into()),
        });
        assert_eq!(rig.state(), Phase::Updating);
        assert_eq!(
            rig.machine.status().update_target.as_deref(),
            Some("18.16.0")
        );
        assert_eq!(
            effects,
            [Effect::RunUpdate {
                omp_path: "".into(),
                omp: omp()
            }]
        );
        assert_eq!(rig.machine.ready(), None);
        rig.send(Event::UpdateEnded {
            omp_path: "".into(),
        });
        assert_eq!(rig.machine.status().update_target, None);
    }

    #[test]
    fn a_check_without_news_or_for_another_path_changes_nothing() {
        let mut rig = Rig::reach("", Phase::Ready);
        assert!(rig
            .send(Event::UpdateCheckDone {
                omp_path: "".into(),
                available: None
            })
            .is_empty());
        assert!(rig
            .send(Event::UpdateCheckDone {
                omp_path: "/old".into(),
                available: Some("19.0.0".into())
            })
            .is_empty());
        assert_eq!(rig.state(), Phase::Ready);
        // And one that arrives after Ready was left is ignored.
        let mut rig = Rig::reach("", Phase::Ready);
        rig.send(Event::OmpMissing {
            omp_path: "".into(),
            reason: None,
        });
        assert!(rig
            .send(Event::UpdateCheckDone {
                omp_path: "".into(),
                available: Some("19.0.0".into())
            })
            .is_empty());
        assert_eq!(rig.state(), Phase::Missing);
    }

    // ---- The status ----------------------------------------------------------------------------

    #[test]
    fn the_revision_grows_with_each_change_and_only_then() {
        let mut rig = Rig::started("");
        let start = rig.machine.status().revision;
        rig.send(Event::RecheckDue);
        assert_eq!(rig.machine.status().revision, start);
        rig.probed(ready_result(&["anthropic"]));
        assert_eq!(rig.machine.status().revision, start + 1);
        rig.send(change_path("/x"));
        assert_eq!(rig.machine.status().revision, start + 2);
    }

    #[test]
    fn the_status_carries_the_minimum_version() {
        assert_eq!(Machine::new().status().min_version, "18.15.0");
    }

    // ---- System Certificates --------------------------------------------------------------

    fn is_installer_failed(rig: &Rig) -> Option<String> {
        rig.machine.status().installer_failure.clone()
    }

    #[test]
    fn at_the_start_the_first_check_waits_for_the_export_and_says_so() {
        let mut rig = Rig {
            machine: Machine::new(),
            now: T0,
            probe: None,
        };
        let effects = rig.send(Event::Start {
            omp_path: "/opt/omp".into(),
            system_certificates: true,
            exporting: true,
        });
        assert_eq!(effects, [Effect::WaitForExport]);
        assert_eq!(rig.state(), Phase::Checking);
        assert!(rig.machine.status().waiting_for_export);
        assert_eq!(rig.probe, None, "no check yet");
        // The end of the export alone does not look at omp: the wait ending does.
        assert!(rig.send(Event::ExportEnded).is_empty());
        let effects = rig.send(Event::ExportWaitOver);
        assert!(is_probe(&effects));
        assert!(!rig.machine.status().waiting_for_export);
        assert!(rig.send(Event::ExportWaitOver).is_empty(), "only once");
        rig.probed(ready_result(&["anthropic"]));
        assert_eq!(rig.state(), Phase::Ready);
    }

    #[test]
    fn a_new_place_of_omp_ends_the_wait_and_checks_at_once() {
        let mut rig = Rig {
            machine: Machine::new(),
            now: T0,
            probe: None,
        };
        rig.send(Event::Start {
            omp_path: "/old".into(),
            system_certificates: true,
            exporting: true,
        });
        let effects = rig.send(change_certificates("/new", true));
        assert!(is_probe(&effects));
        assert!(!rig.machine.status().waiting_for_export);
        assert!(rig.send(Event::ExportWaitOver).is_empty());
    }

    #[test]
    fn turning_system_certificates_off_checks_again_from_missing_not_connected_and_ready() {
        for state in [Phase::Missing, Phase::NotConnected, Phase::Ready] {
            let mut rig = Rig::reach("/omp", state);
            // Turning it on looks at nothing here: the end of its export does.
            assert!(rig.send(change_certificates("/omp", true)).is_empty());
            assert_eq!(rig.state(), state);
            let effects = rig.send(change_certificates("/omp", false));
            assert!(is_probe(&effects), "{state:?}");
            assert_eq!(rig.state(), Phase::Checking, "{state:?}");
            // Off again: nothing changed.
            assert!(rig.send(change_certificates("/omp", false)).is_empty());
        }
        for state in [
            Phase::Checking,
            Phase::Installing,
            Phase::Updating,
            Phase::LoggingIn,
        ] {
            let mut rig = Rig::reach("/omp", state);
            rig.send(change_certificates("/omp", true));
            assert!(
                rig.send(change_certificates("/omp", false)).is_empty(),
                "{state:?}"
            );
            assert_eq!(rig.state(), state, "{state:?}");
        }
    }

    #[test]
    fn the_end_of_an_export_checks_again_from_missing_not_connected_and_ready() {
        for state in [Phase::Missing, Phase::NotConnected, Phase::Ready] {
            let mut rig = Rig::reach("/omp", state);
            let effects = rig.send(Event::ExportEnded);
            assert!(is_probe(&effects), "{state:?}");
            assert_eq!(rig.state(), Phase::Checking, "{state:?}");
        }
        for state in [
            Phase::Checking,
            Phase::Installing,
            Phase::Updating,
            Phase::LoggingIn,
        ] {
            let mut rig = Rig::reach("/omp", state);
            assert!(rig.send(Event::ExportEnded).is_empty(), "{state:?}");
            assert_eq!(rig.state(), state, "{state:?}");
        }
    }

    // ---- The installer --------------------------------------------------------------------

    #[test]
    fn a_check_while_installing_carries_the_installer_it_reads_the_marker_of() {
        let mut rig = Rig::reach("", Phase::Installing);
        rig.send(Event::RecheckDue);
        assert_eq!(
            rig.probe.as_ref().and_then(|t| t.installer.as_deref()),
            Some("aa")
        );
        let mut rig = Rig::reach("", Phase::LoggingIn);
        rig.send(Event::RecheckDue);
        assert_eq!(rig.probe.as_ref().and_then(|t| t.installer.clone()), None);
    }

    #[test]
    fn a_failed_installer_reports_missing_with_its_exit_code() {
        let mut rig = Rig::reach("", Phase::Installing);
        rig.send(Event::RecheckDue);
        rig.probed(ProbeResult::InstallFailed { exit_code: 22 });
        assert_eq!(rig.state(), Phase::Missing);
        assert_eq!(rig.machine.status().reason, None);
        assert_eq!(
            is_installer_failed(&rig).as_deref(),
            Some("The agent installer failed (exit code 22); see the terminal. A corporate proxy that inspects HTTPS traffic is a common cause. Turning on System Certificates in Weaver's settings may fix it.")
        );
        // The action stays `Install omp`.
        assert_eq!(rig.send(install()), opens_installer());
        assert_eq!(rig.state(), Phase::Installing);
        assert_eq!(is_installer_failed(&rig), None);
    }

    #[test]
    fn with_the_setting_on_the_text_does_not_suggest_turning_it_on_and_follows_the_setting() {
        let mut rig = Rig::reach("", Phase::Installing);
        rig.send(change_certificates("", true));
        rig.send(Event::RecheckDue);
        rig.probed(ProbeResult::InstallFailed { exit_code: 1 });
        let text = is_installer_failed(&rig).unwrap();
        assert!(text.ends_with("is a common cause."), "{text}");
        // Turned off: the check starts again, and the text, back in Missing, suggests it.
        let effects = rig.send(change_certificates("", false));
        assert!(is_probe(&effects));
        assert_eq!(is_installer_failed(&rig), None, "only shown in Missing");
        rig.probed(missing());
        assert!(is_installer_failed(&rig).unwrap().ends_with("may fix it."));
    }

    #[test]
    fn the_text_of_a_failed_installer_is_by_cause_for_the_setting_as_it_is() {
        for (code, on, expected) in [
            (90, false, "the download of the agent was blocked on both of its download sites, probably by a corporate proxy that filters downloads. See the terminal."),
            (90, true, "the download of the agent was blocked on both of its download sites, probably by a corporate proxy that filters downloads. See the terminal."),
            (91, false, "the agent could not verify a server's certificate; a corporate proxy that inspects HTTPS traffic is the usual cause. See the terminal. Turning on System Certificates in Weaver's settings may fix it."),
            (91, true, "the agent could not verify a server's certificate; a corporate proxy that inspects HTTPS traffic is the usual cause. See the terminal."),
            (92, false, "the downloaded agent was incomplete or altered, so it was not installed; a proxy may have changed it. See the terminal."),
            (93, true, "the downloaded agent does not start. See the terminal."),
        ] {
            let mut rig = Rig::reach("", Phase::Installing);
            rig.send(change_certificates("", on));
            rig.send(Event::RecheckDue);
            rig.probed(ProbeResult::InstallFailed { exit_code: code });
            assert_eq!(
                is_installer_failed(&rig),
                Some(format!(
                    "The agent installer failed (exit code {code}): {expected}"
                )),
                "code {code}, setting on: {on}"
            );
        }
    }

    #[test]
    fn the_exit_code_stays_through_the_checks_until_omp_is_found() {
        let mut rig = Rig::reach("", Phase::Installing);
        rig.send(Event::RecheckDue);
        rig.probed(ProbeResult::InstallFailed { exit_code: 56 });
        assert!(is_installer_failed(&rig).is_some());
        rig.send(Event::ExportEnded);
        rig.probed(ProbeResult::Missing {
            reason: Some("the agent not found at /x".into()),
        });
        assert_eq!(rig.state(), Phase::Missing);
        assert!(is_installer_failed(&rig).unwrap().contains("exit code 56"));
        assert_eq!(
            rig.machine.status().reason.as_deref(),
            Some("the agent not found at /x")
        );
        // omp is found: the code is forgotten, and does not come back.
        rig.send(Event::ExportEnded);
        rig.probed(ready_result(&["anthropic"]));
        assert_eq!(rig.state(), Phase::Ready);
        rig.send(Event::OmpMissing {
            omp_path: String::new(),
            reason: None,
        });
        assert_eq!(rig.state(), Phase::Missing);
        assert_eq!(is_installer_failed(&rig), None);
    }

    #[test]
    fn a_new_installer_forgets_the_code_and_a_late_answer_of_the_old_one_is_ignored() {
        let mut rig = Rig::reach("", Phase::Installing);
        rig.send(Event::RecheckDue);
        // The check for the first installer is still running when the second one starts.
        let effects = rig.send(Event::InstallOmp { nonce: "bb".into() });
        assert_eq!(effects, [Effect::OpenInstaller { nonce: "bb".into() }]);
        rig.probed(ProbeResult::InstallFailed { exit_code: 22 });
        assert_eq!(rig.state(), Phase::Installing);
        assert_eq!(is_installer_failed(&rig), None);
        // The next check is for the new one.
        rig.send(Event::RecheckDue);
        assert_eq!(
            rig.probe.as_ref().and_then(|t| t.installer.as_deref()),
            Some("bb")
        );
    }

    #[test]
    fn an_installer_that_finished_well_or_left_nothing_keeps_installing() {
        let mut rig = Rig::reach("", Phase::Installing);
        rig.send(Event::RecheckDue);
        rig.probed(missing());
        assert_eq!(rig.state(), Phase::Installing);
        assert_eq!(is_installer_failed(&rig), None);
    }
}
