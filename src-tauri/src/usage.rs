//! What the backend knows of the usage limits (`requirements/tech#usage`): the last result, the
//! reading in flight and why the last one failed, as one app-wide value in memory. Never written
//! to disk; every window gets it whole, with its revision, through the `usage-changed` event.
//!
//! [`UsageState`] is the bookkeeping only (no process, no clock, no Tauri): [`crate::omp::OmpSetup`]
//! runs the readings and tells the windows.

use std::sync::Mutex;

use serde::Serialize;
use tokio::sync::watch;
use ts_rs::TS;
use weaver_core::omp::usage::{Usage, UsageSummary};

/// Whether a reading runs or the last one failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub enum UsagePhase {
    /// No reading runs and the last one did not fail (or none was made yet).
    Idle,
    /// `omp usage --json` runs; `usage`, when there is some, is the one read last.
    Reading,
    /// The last reading failed; `failure` says why and `usage`, when there is some, is the one
    /// read last.
    Failed,
}

/// Everything the interface shows of usage. Sent whole at each change.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UsageStatus {
    /// Increases with every change; a window keeps the highest it saw.
    #[ts(type = "number")]
    pub revision: u64,
    pub phase: UsagePhase,
    /// Failed: why.
    pub failure: Option<String>,
    /// The limits read last, for the omp path that is current; `None` until a reading worked.
    pub usage: Option<Usage>,
    /// The limit of `usage` closest to being used up (`requirements/tech#usage`), computed by the
    /// core; `None` when `usage` is `None` or no shown provider has a limit.
    pub summary: Option<UsageSummary>,
}

/// The reading that runs: its number, so that a result of a superseded one is ignored, and the
/// omp path it was started for.
struct Reading {
    generation: u64,
    omp_path: String,
    done: watch::Receiver<()>,
}

struct Inner {
    revision: u64,
    next_generation: u64,
    /// The last successful reading and the omp path it was read for.
    last: Option<(String, Usage)>,
    failure: Option<String>,
    reading: Option<Reading>,
}

/// The sender of a started reading: dropped (with the end of the work) when the reading is over, so
/// that everyone waiting for it wakes up.
pub struct Started {
    pub generation: u64,
    pub omp_path: String,
    _done: watch::Sender<()>,
}

/// What an opening or a `Retry` gets from [`UsageState::begin`].
pub struct Begin {
    /// Closes (the sender is dropped) when the reading, this one or the one already running, ends.
    pub done: watch::Receiver<()>,
    /// Set when this call started a reading: the work to run.
    pub started: Option<Started>,
    /// Set when the status changed: to tell the windows.
    pub status: Option<UsageStatus>,
}

pub struct UsageState {
    inner: Mutex<Inner>,
}

impl Default for UsageState {
    fn default() -> Self {
        Self::new()
    }
}

impl UsageState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Inner {
                revision: 0,
                next_generation: 0,
                last: None,
                failure: None,
                reading: None,
            }),
        }
    }

    pub fn status(&self) -> UsageStatus {
        status_of(&self.inner.lock().expect("usage state poisoned"))
    }

    /// An opening or a `Retry` for `omp_path` (the `ompPath` setting of the omp that is Ready): a
    /// reading already running for it is joined, anything else starts a new one.
    pub fn begin(&self, omp_path: &str) -> Begin {
        let mut inner = self.inner.lock().expect("usage state poisoned");
        if let Some(reading) = inner.reading.as_ref().filter(|r| r.omp_path == omp_path) {
            return Begin {
                done: reading.done.clone(),
                started: None,
                status: None,
            };
        }
        let (sender, done) = watch::channel(());
        let generation = inner.next_generation;
        inner.next_generation += 1;
        inner.reading = Some(Reading {
            generation,
            omp_path: omp_path.to_owned(),
            done: done.clone(),
        });
        inner.failure = None;
        inner.revision += 1;
        Begin {
            done,
            started: Some(Started {
                generation,
                omp_path: omp_path.to_owned(),
                _done: sender,
            }),
            status: Some(status_of(&inner)),
        }
    }

    /// A reading ended. A result for an omp path that is no longer `current`, or of a reading that
    /// was superseded, is ignored. A successful one replaces the last result even when the dialog
    /// that asked for it was closed. Returns the status to tell the windows, when it changed.
    pub fn finish(
        &self,
        started: &Started,
        current: &str,
        result: Result<Usage, String>,
    ) -> Option<UsageStatus> {
        let mut inner = self.inner.lock().expect("usage state poisoned");
        if inner
            .reading
            .as_ref()
            .is_none_or(|r| r.generation != started.generation)
        {
            return None;
        }
        inner.reading = None;
        if started.omp_path == current {
            match result {
                Ok(usage) => {
                    inner.last = Some((started.omp_path.clone(), usage));
                    inner.failure = None;
                }
                Err(reason) => inner.failure = Some(reason),
            }
        }
        inner.revision += 1;
        Some(status_of(&inner))
    }

    /// The `ompPath` setting is now `current`: what was read, or is being read, for another path is
    /// dropped. Returns the status to tell the windows, when it changed.
    pub fn omp_path_is(&self, current: &str) -> Option<UsageStatus> {
        let mut inner = self.inner.lock().expect("usage state poisoned");
        let mut changed = false;
        if inner.last.as_ref().is_some_and(|(path, _)| path != current) {
            inner.last = None;
            changed = true;
        }
        if inner
            .reading
            .as_ref()
            .is_some_and(|r| r.omp_path != current)
        {
            inner.reading = None;
            changed = true;
        }
        if changed {
            inner.failure = None;
            inner.revision += 1;
            Some(status_of(&inner))
        } else {
            None
        }
    }
}

fn status_of(inner: &Inner) -> UsageStatus {
    let phase = if inner.reading.is_some() {
        UsagePhase::Reading
    } else if inner.failure.is_some() {
        UsagePhase::Failed
    } else {
        UsagePhase::Idle
    };
    UsageStatus {
        revision: inner.revision,
        phase,
        failure: inner.failure.clone(),
        usage: inner.last.as_ref().map(|(_, usage)| usage.clone()),
        summary: inner.last.as_ref().and_then(|(_, usage)| usage.summary()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(fetched_at: f64) -> Usage {
        Usage {
            fetched_at: Some(fetched_at),
            providers: vec![],
        }
    }

    #[test]
    fn begins_idle_with_nothing() {
        let state = UsageState::new();
        let status = state.status();
        assert_eq!(status.phase, UsagePhase::Idle);
        assert_eq!((status.failure, status.usage), (None, None));
    }

    #[test]
    fn a_reading_keeps_the_last_result_and_replaces_it_when_it_works() {
        let state = UsageState::new();
        let first = state.begin("/omp");
        assert_eq!(first.status.as_ref().unwrap().phase, UsagePhase::Reading);
        let done = state
            .finish(first.started.as_ref().unwrap(), "/omp", Ok(usage(1.0)))
            .unwrap();
        assert_eq!(done.phase, UsagePhase::Idle);
        assert_eq!(done.usage, Some(usage(1.0)));

        // The next reading shows the limits read last, marked as being read.
        let second = state.begin("/omp");
        let reading = second.status.unwrap();
        assert_eq!(reading.phase, UsagePhase::Reading);
        assert_eq!(reading.usage, Some(usage(1.0)));
        let done = state
            .finish(second.started.as_ref().unwrap(), "/omp", Ok(usage(2.0)))
            .unwrap();
        assert_eq!(done.usage, Some(usage(2.0)));
        assert!(done.revision > reading.revision);
    }

    #[test]
    fn a_failed_reading_keeps_the_last_result_and_says_why() {
        let state = UsageState::new();
        let first = state.begin("/omp");
        state.finish(first.started.as_ref().unwrap(), "/omp", Ok(usage(1.0)));
        let second = state.begin("/omp");
        let failed = state
            .finish(
                second.started.as_ref().unwrap(),
                "/omp",
                Err("timed out".to_owned()),
            )
            .unwrap();
        assert_eq!(failed.phase, UsagePhase::Failed);
        assert_eq!(failed.failure.as_deref(), Some("timed out"));
        assert_eq!(failed.usage, Some(usage(1.0)));
        // A new reading forgets the failure.
        let third = state.begin("/omp");
        let status = third.status.unwrap();
        assert_eq!((status.phase, status.failure), (UsagePhase::Reading, None));
    }

    #[test]
    fn an_opening_during_a_reading_waits_for_it() {
        let state = UsageState::new();
        let first = state.begin("/omp");
        let started = first.started.expect("started");
        let second = state.begin("/omp");
        assert!(second.started.is_none());
        assert!(second.status.is_none());
        let waiting = second.done;
        assert!(!waiting.has_changed().is_err(), "the reading still runs");
        state.finish(&started, "/omp", Ok(usage(1.0)));
        drop(started);
        assert!(waiting.has_changed().is_err(), "the reading ended");
        assert_eq!(state.status().usage, Some(usage(1.0)));
    }

    #[test]
    fn a_result_for_another_omp_path_is_ignored() {
        let state = UsageState::new();
        let first = state.begin("/old");
        let started = first.started.unwrap();
        // The path changes while the reading runs: nothing of it stays.
        let changed = state.omp_path_is("/new").unwrap();
        assert_eq!(changed.phase, UsagePhase::Idle);
        assert!(state.finish(&started, "/new", Ok(usage(1.0))).is_none());
        assert_eq!(state.status().usage, None);

        // A reading that ends after the path changed, with the old reading still current.
        let second = state.begin("/new");
        let finished = state
            .finish(second.started.as_ref().unwrap(), "/other", Ok(usage(2.0)))
            .unwrap();
        assert_eq!(finished.phase, UsagePhase::Idle);
        assert_eq!(finished.usage, None);
    }

    #[test]
    fn the_last_result_is_dropped_when_the_omp_path_changes() {
        let state = UsageState::new();
        let first = state.begin("/old");
        state.finish(first.started.as_ref().unwrap(), "/old", Ok(usage(1.0)));
        assert!(
            state.omp_path_is("/old").is_none(),
            "same path: nothing changes"
        );
        let changed = state.omp_path_is("/new").unwrap();
        assert_eq!(changed.usage, None);
        assert!(state.omp_path_is("/new").is_none());
    }

    #[test]
    fn a_reading_for_a_new_path_does_not_join_one_for_the_old_path() {
        let state = UsageState::new();
        let old = state.begin("/old");
        let new = state.begin("/new");
        assert!(new.started.is_some());
        // The old one ended after being superseded: ignored.
        assert!(state
            .finish(old.started.as_ref().unwrap(), "/new", Ok(usage(1.0)))
            .is_none());
        assert_eq!(state.status().phase, UsagePhase::Reading);
    }

    #[test]
    fn the_status_carries_the_summary_of_the_last_result_and_drops_it_with_it() {
        use weaver_core::omp::usage::{LimitStatus, ProviderUsage, UsageLimit};

        let state = UsageState::new();
        assert_eq!(state.status().summary, None);
        let with_limit = Usage {
            fetched_at: Some(1.0),
            providers: vec![ProviderUsage {
                id: "anthropic".into(),
                name: "Anthropic".into(),
                limits: vec![UsageLimit {
                    label: "Claude 5 Hour".into(),
                    resets_at: Some(9.0),
                    used: Some(85.0),
                    limit: Some(100.0),
                    unit: Some("percent".into()),
                    used_fraction: Some(0.85),
                    status: LimitStatus::AlmostUsedUp,
                }],
                no_limits: None,
            }],
        };
        let first = state.begin("/omp");
        let done = state
            .finish(first.started.as_ref().unwrap(), "/omp", Ok(with_limit))
            .unwrap();
        let summary = done.summary.expect("a limit is shown");
        assert_eq!(summary.label, "Claude 5 Hour");
        assert_eq!(summary.used_fraction, Some(0.85));
        assert_eq!(summary.status, LimitStatus::AlmostUsedUp);
        // Reading again keeps it; a change of the omp path drops it.
        assert!(state.begin("/omp").status.unwrap().summary.is_some());
        assert_eq!(state.omp_path_is("/new").unwrap().summary, None);
    }
}
