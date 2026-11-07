//! `state.json`: the local state that is not a setting (`requirements/data#window`).
//!
//! It lives in the local data folder of the app, never in a project:
//!
//! * `recent`: up to [`RECENT_LIMIT`] projects, most recently opened first, each `{path, name,
//!   opened}`; one entry per canonical path; an entry leaves only when pushed out by newer ones
//!   (even when its folder no longer exists), when the developer removes it from the welcome
//!   window or when `Clear Recent` of the File menu empties the list (which changes neither
//!   `lastSession` nor the open windows);
//! * `lastSession`: the canonical paths of the projects whose window was open when Weaver last
//!   quit, in opening order. It is written at quit and on each window open or close; closing a
//!   window removes it from the list, quitting does not;
//! * `systemCertificates`: `{autoEnabledAt}`, when Weaver turned System Certificates on by itself
//!   because Zscaler is installed (`requirements/data#system-certificates`). Written once, never
//!   removed; absent from the file until then;
//! * `jobsList`: the Jobs list of each project's window (`requirements/ui#job`), keyed by the
//!   project hash: `{width, hidden}`. A project without an entry, or with a value out of range or
//!   of the wrong type, counts as [`JOBS_LIST_DEFAULT_WIDTH`] px and shown.
//!
//! [`LocalState`] is the pure value and its rules; [`StateStore`] keeps it in memory and writes it
//! atomically. The store does file I/O: call it from the blocking pool.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use ts_rs::TS;
use weaver_core::{atomic, paths};

use crate::log_warn;

/// Name of the file inside the local data folder.
pub const FILE_NAME: &str = "state.json";

/// Most projects the list of recent projects keeps.
pub const RECENT_LIMIT: usize = 10;

/// The width of the Jobs list at first, in px.
pub const JOBS_LIST_DEFAULT_WIDTH: u32 = 280;
/// The narrowest and the widest the developer can make the Jobs list, in px.
pub const JOBS_LIST_MIN_WIDTH: u32 = 240;
pub const JOBS_LIST_MAX_WIDTH: u32 = 420;

/// What the window of a project remembers of its Jobs list: the width the developer chose and
/// whether the list is hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct JobsList {
    /// The chosen width in px, from 240 to 420.
    pub width: u32,
    pub hidden: bool,
}

impl Default for JobsList {
    fn default() -> Self {
        Self {
            width: JOBS_LIST_DEFAULT_WIDTH,
            hidden: false,
        }
    }
}

impl JobsList {
    /// Whether the width is one the developer can choose.
    pub fn in_range(self) -> bool {
        (JOBS_LIST_MIN_WIDTH..=JOBS_LIST_MAX_WIDTH).contains(&self.width)
    }
}

/// The entries of `jobsList` as they are in the file, so that one entry of the wrong type never
/// costs the rest of the state; anything that is not an object is an empty map.
fn lenient_entries<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, Value>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Object(entries) => entries.into_iter().collect(),
        _ => BTreeMap::new(),
    })
}

/// One recent project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentProject {
    /// The canonical path of the project folder.
    pub path: String,
    /// The folder's name.
    pub name: String,
    /// When it was last opened (UTC ISO 8601 with milliseconds).
    pub opened: String,
}

/// What `state.json` remembers of System Certificates (`requirements/data#system-certificates`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SystemCertificatesMark {
    /// When Weaver turned the setting on by itself because Zscaler is installed (UTC ISO 8601).
    /// Present means Weaver never does it again; Weaver never removes it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_enabled_at: Option<String>,
}

impl SystemCertificatesMark {
    fn is_empty(&self) -> bool {
        self.auto_enabled_at.is_none()
    }
}

/// The content of `state.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LocalState {
    pub recent: Vec<RecentProject>,
    pub last_session: Vec<String>,
    #[serde(skip_serializing_if = "SystemCertificatesMark::is_empty")]
    pub system_certificates: SystemCertificatesMark,
    /// The Jobs list of each project's window, by project hash, as the file holds it (see
    /// [`LocalState::jobs_list`], which applies the rules).
    #[serde(deserialize_with = "lenient_entries", skip_serializing_if = "BTreeMap::is_empty")]
    pub jobs_list: BTreeMap<String, Value>,
}

/// Whether two canonical paths name the same project: the same text, or on Windows the same text
/// apart from case (the rule of the project hash).
pub fn same_path(a: &str, b: &str) -> bool {
    paths::project_hash_of_canonical(Path::new(a)) == paths::project_hash_of_canonical(Path::new(b))
}

impl LocalState {
    /// The Jobs list of the project `hash`: the stored value, or 280 px and shown when there is
    /// none, when its width is out of range or when it is of the wrong type.
    pub fn jobs_list(&self, hash: &str) -> JobsList {
        self.jobs_list
            .get(hash)
            .and_then(|value| serde_json::from_value::<JobsList>(value.clone()).ok())
            .filter(|list| list.in_range())
            .unwrap_or_default()
    }

    /// The developer chose a width or hid or showed the list of the project `hash`.
    pub fn set_jobs_list(&mut self, hash: &str, list: JobsList) {
        let value = serde_json::to_value(list).expect("a JobsList is JSON");
        self.jobs_list.insert(hash.to_owned(), value);
    }

    /// What a file holds, made to respect the rules: one entry per path (the first, most recent,
    /// wins) and at most [`RECENT_LIMIT`] of them. Anything unreadable is an error.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut state: LocalState = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let mut seen: Vec<String> = Vec::new();
        state.recent.retain(|project| {
            if seen.iter().any(|path| same_path(path, &project.path)) {
                return false;
            }
            seen.push(project.path.clone());
            true
        });
        state.recent.truncate(RECENT_LIMIT);
        let mut seen: Vec<String> = Vec::new();
        state.last_session.retain(|path| {
            if seen.iter().any(|known| same_path(known, path)) {
                return false;
            }
            seen.push(path.clone());
            true
        });
        Ok(state)
    }

    /// A window of the project opened: it becomes the most recent project (once) and joins the
    /// session after the windows already open.
    pub fn record_open(&mut self, path: &str, name: &str, opened: &str) {
        self.recent
            .retain(|project| !same_path(&project.path, path));
        self.recent.insert(
            0,
            RecentProject {
                path: path.to_owned(),
                name: name.to_owned(),
                opened: opened.to_owned(),
            },
        );
        self.recent.truncate(RECENT_LIMIT);
        if !self.last_session.iter().any(|known| same_path(known, path)) {
            self.last_session.push(path.to_owned());
        }
    }

    /// The developer closed the window of the project: it will not reopen at the next start.
    pub fn record_close(&mut self, path: &str) {
        self.last_session.retain(|known| !same_path(known, path));
    }

    /// Weaver quits: the session is the projects whose window is open, in opening order.
    pub fn set_last_session(&mut self, open_paths: Vec<String>) {
        self.last_session = open_paths;
    }

    /// Removes a project from the recent list (the welcome window offers it for a folder that no
    /// longer exists). `false` when it was not there.
    pub fn remove_recent(&mut self, path: &str) -> bool {
        let before = self.recent.len();
        self.recent
            .retain(|project| !same_path(&project.path, path));
        self.recent.len() != before
    }

    /// `Clear Recent` of the File menu: empties the list of recent projects. The last session is
    /// not touched.
    pub fn clear_recent(&mut self) {
        self.recent.clear();
    }
}

/// `state.json` in memory and on disk. Every change is written before the call returns.
pub struct StateStore {
    path: PathBuf,
    state: Mutex<Option<LocalState>>,
}

impl StateStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            state: Mutex::new(None),
        }
    }

    /// The store of the app: `<local data>/state.json`.
    pub fn in_dir(local_data_dir: &Path) -> Self {
        Self::new(local_data_dir.join(FILE_NAME))
    }

    fn read_file(&self) -> LocalState {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => LocalState::parse(&text).unwrap_or_else(|reason| {
                log_warn!("{FILE_NAME} is not used, the recent projects and the last session start empty: {reason}");
                LocalState::default()
            }),
            Err(error) if error.kind() == io::ErrorKind::NotFound => LocalState::default(),
            Err(error) => {
                log_warn!("{FILE_NAME} cannot be read, the recent projects and the last session start empty: {error}");
                LocalState::default()
            }
        }
    }

    fn locked(&self) -> std::sync::MutexGuard<'_, Option<LocalState>> {
        self.state.lock().expect("local state poisoned")
    }

    /// The state now; the first call reads the file. A missing or unreadable file is an empty state.
    pub fn snapshot(&self) -> LocalState {
        self.locked()
            .get_or_insert_with(|| self.read_file())
            .clone()
    }

    /// Applies `change` and writes the file when the state differs. On a failed write the
    /// state in memory is the new one and the error is returned.
    pub fn update<R>(&self, change: impl FnOnce(&mut LocalState) -> R) -> io::Result<R> {
        let mut guard = self.locked();
        let state = guard.get_or_insert_with(|| self.read_file());
        let before = state.clone();
        let result = change(state);
        if *state != before {
            let mut text = serde_json::to_string_pretty(state).map_err(io::Error::other)?;
            text.push('\n');
            atomic::write(&self.path, text.as_bytes(), None)?;
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(n: u32) -> String {
        format!("2026-11-02T07:00:{n:02}.000Z")
    }

    #[test]
    fn opening_puts_the_project_first_once_and_in_the_session() {
        let mut state = LocalState::default();
        state.record_open("/work/a", "a", &at(1));
        state.record_open("/work/b", "b", &at(2));
        state.record_open("/work/a", "a", &at(3));
        let recent: Vec<_> = state.recent.iter().map(|p| p.path.as_str()).collect();
        assert_eq!(recent, ["/work/a", "/work/b"]);
        assert_eq!(state.recent[0].opened, at(3));
        assert_eq!(
            state.last_session,
            ["/work/a", "/work/b"],
            "opening order, once each"
        );
    }

    #[test]
    fn only_ten_recent_projects_are_kept_and_the_oldest_leaves_first() {
        let mut state = LocalState::default();
        for n in 0..12 {
            state.record_open(&format!("/work/{n}"), &format!("{n}"), &at(n));
        }
        assert_eq!(state.recent.len(), RECENT_LIMIT);
        assert_eq!(state.recent[0].path, "/work/11");
        assert_eq!(state.recent[9].path, "/work/2");
    }

    #[test]
    fn closing_a_window_leaves_the_session_but_not_the_recent_list() {
        let mut state = LocalState::default();
        state.record_open("/work/a", "a", &at(1));
        state.record_open("/work/b", "b", &at(2));
        state.record_close("/work/a");
        assert_eq!(state.last_session, ["/work/b"]);
        assert_eq!(state.recent.len(), 2);
    }

    #[test]
    fn quitting_writes_the_windows_open_then_in_opening_order() {
        let mut state = LocalState::default();
        state.record_open("/work/a", "a", &at(1));
        state.set_last_session(vec!["/work/b".into(), "/work/a".into()]);
        assert_eq!(state.last_session, ["/work/b", "/work/a"]);
    }

    #[test]
    fn a_folder_that_no_longer_exists_stays_until_pushed_out_or_removed() {
        let mut state = LocalState::default();
        state.record_open("/gone/project", "project", &at(1));
        state.record_open("/work/a", "a", &at(2));
        assert_eq!(state.recent.len(), 2);
        assert!(state.remove_recent("/gone/project"));
        assert!(!state.remove_recent("/gone/project"));
        assert_eq!(state.recent.len(), 1);
    }

    #[test]
    fn clearing_the_recent_projects_leaves_the_last_session_alone() {
        let mut state = LocalState::default();
        state.record_open("/work/a", "a", &at(1));
        state.record_open("/gone/b", "b", &at(2));
        state.clear_recent();
        assert!(state.recent.is_empty());
        assert_eq!(state.last_session, ["/work/a", "/gone/b"]);
    }

    #[test]
    fn a_file_with_repeated_or_too_many_entries_is_brought_back_to_the_rules() {
        let recent: Vec<String> = (0..14)
            .map(|n| {
                format!(
                    r#"{{"path":"/work/{}","name":"n","opened":"2026-11-02T07:00:00.000Z"}}"#,
                    n % 12
                )
            })
            .collect();
        let text = format!(
            r#"{{"recent":[{}],"lastSession":["/work/1","/work/1","/work/2"]}}"#,
            recent.join(",")
        );
        let state = LocalState::parse(&text).unwrap();
        assert_eq!(state.recent.len(), RECENT_LIMIT);
        assert_eq!(state.recent[0].path, "/work/0");
        assert_eq!(state.last_session, ["/work/1", "/work/2"]);
        assert_eq!(LocalState::parse("{}").unwrap(), LocalState::default());
        assert!(LocalState::parse("[1]").is_err());
    }

    #[test]
    fn the_store_writes_each_change_and_reads_it_back() {
        let dir = tempfile::tempdir().unwrap();
        let store = StateStore::in_dir(dir.path());
        assert_eq!(store.snapshot(), LocalState::default());
        store
            .update(|state| state.record_open("/work/a", "a", &at(1)))
            .unwrap();
        store
            .update(|state| state.record_open("/work/b", "b", &at(2)))
            .unwrap();

        let text = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(text.contains("\"lastSession\""), "{text}");
        let again = StateStore::in_dir(dir.path()).snapshot();
        assert_eq!(again.last_session, ["/work/a", "/work/b"]);
        assert_eq!(again.recent[0].name, "b");
    }

    #[test]
    fn an_unreadable_file_starts_empty_and_the_next_change_replaces_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), "{ not json").unwrap();
        let store = StateStore::in_dir(dir.path());
        assert_eq!(store.snapshot(), LocalState::default());
        store
            .update(|state| state.record_open("/work/a", "a", &at(1)))
            .unwrap();
        assert_eq!(StateStore::in_dir(dir.path()).snapshot().recent.len(), 1);
    }

    #[test]
    fn the_marker_is_written_next_to_the_projects_and_read_back_and_is_absent_before() {
        let dir = tempfile::tempdir().unwrap();
        let store = StateStore::in_dir(dir.path());
        store
            .update(|state| state.record_open("/work/a", "a", &at(1)))
            .unwrap();
        let before = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(!before.contains("systemCertificates"), "{before}");

        let instant = "2026-11-02T07:00:00.000Z".to_owned();
        store
            .update(|state| state.system_certificates.auto_enabled_at = Some(instant.clone()))
            .unwrap();
        let text = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(
            text.contains(r#""systemCertificates": {"#) && text.contains(r#""autoEnabledAt""#),
            "{text}"
        );
        let again = StateStore::in_dir(dir.path()).snapshot();
        assert_eq!(again.system_certificates.auto_enabled_at, Some(instant));
        assert_eq!(again.recent.len(), 1, "the projects are kept");
        // A later change of the projects keeps the marker: one owner, no overwriting.
        store
            .update(|state| state.record_open("/work/b", "b", &at(2)))
            .unwrap();
        let kept = StateStore::in_dir(dir.path()).snapshot();
        assert!(kept.system_certificates.auto_enabled_at.is_some());
        assert_eq!(kept.recent.len(), 2);
    }

    #[test]
    fn an_unreadable_file_counts_as_without_the_marker() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE_NAME), r#"{"systemCertificates": 7}"#).unwrap();
        let store = StateStore::in_dir(dir.path());
        assert_eq!(store.snapshot().system_certificates.auto_enabled_at, None);
    }

    #[test]
    fn a_project_without_a_jobs_list_entry_counts_as_280_px_and_shown() {
        let state = LocalState::default();
        assert_eq!(
            state.jobs_list("0123456789abcdef"),
            JobsList {
                width: 280,
                hidden: false
            }
        );
    }

    #[test]
    fn the_jobs_list_is_kept_per_project_and_written_with_the_state() {
        let dir = tempfile::tempdir().unwrap();
        let store = StateStore::in_dir(dir.path());
        store
            .update(|state| {
                state.set_jobs_list(
                    "aaaaaaaaaaaaaaaa",
                    JobsList {
                        width: 360,
                        hidden: true,
                    },
                )
            })
            .unwrap();
        store
            .update(|state| state.record_open("/work/a", "a", &at(1)))
            .unwrap();
        let text = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(text.contains(r#""jobsList""#), "{text}");
        let again = StateStore::in_dir(dir.path()).snapshot();
        assert_eq!(
            again.jobs_list("aaaaaaaaaaaaaaaa"),
            JobsList {
                width: 360,
                hidden: true
            }
        );
        assert_eq!(again.jobs_list("bbbbbbbbbbbbbbbb"), JobsList::default());
        assert_eq!(again.recent.len(), 1, "the projects are kept");
    }

    #[test]
    fn a_jobs_list_out_of_range_or_of_the_wrong_type_counts_as_the_default_and_costs_nothing_else() {
        let text = r#"{
            "recent": [{"path":"/work/a","name":"a","opened":"2026-11-02T07:00:00.000Z"}],
            "jobsList": {
                "low": {"width": 239, "hidden": true},
                "high": {"width": 421, "hidden": true},
                "edge": {"width": 420, "hidden": true},
                "text": {"width": "300", "hidden": false},
                "flag": {"width": 300, "hidden": "yes"},
                "float": {"width": 300.5, "hidden": false},
                "bare": 7
            }
        }"#;
        let state = LocalState::parse(text).unwrap();
        assert_eq!(state.recent.len(), 1);
        for hash in ["low", "high", "text", "flag", "float", "bare", "absent"] {
            assert_eq!(state.jobs_list(hash), JobsList::default(), "{hash}");
        }
        assert_eq!(
            state.jobs_list("edge"),
            JobsList {
                width: 420,
                hidden: true
            }
        );
        let wrong = LocalState::parse(r#"{"jobsList": 7}"#).unwrap();
        assert!(wrong.jobs_list.is_empty());
    }
}
