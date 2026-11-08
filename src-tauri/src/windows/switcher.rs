//! The project switcher of every project window (`requirements/tech#window`): the open projects,
//! the background ones included, each with the jobs that work in it, and the recent projects that
//! have no window, each with whether its folder still exists.
//!
//! * [`project_switcher`] answers the list at once, for the window that opens its menu;
//! * [`schedule`] tells every project window the whole list, as the event
//!   `project-switcher-changed`, at each window opened or closed, change of the recent projects and
//!   change of a project's working jobs; a list equal to the one last sent is not sent again.
//!
//! The list is read on the blocking pool (the folders are checked with `Path::is_dir`). Choosing a
//! project is the `open_project` command, and `Open Folder…` is [`super::commands::open_folder_from_window`].

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use ts_rs::TS;

use super::commands::{recent_entries_of, RecentEntry};
use super::local_state::same_path;
use super::manager::WindowManager;
use crate::blocking;
use crate::error::AppError;
use crate::events::EventName;
use crate::log_warn;

/// A project with an open window.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct OpenProjectEntry {
    /// The project hash (`p-<hash>` is the label of its window).
    pub id: String,
    /// The canonical path, shown in muted text under the name.
    pub path: String,
    /// The folder's name.
    pub name: String,
    /// How many of its jobs work: running, or waiting for the developer.
    pub working: u32,
}

/// What the menu of the project switcher shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSwitcher {
    /// Every project with an open window, a window in the background included, the most recently
    /// used first.
    pub open: Vec<OpenProjectEntry>,
    /// The recent projects without an open window, the most recent first.
    pub recent: Vec<RecentEntry>,
}

impl ProjectSwitcher {
    /// The list for `open` and the `recent` projects: a recent project that has a window is in
    /// `open` only.
    pub fn build(open: Vec<OpenProjectEntry>, recent: Vec<RecentEntry>) -> Self {
        let recent = recent
            .into_iter()
            .filter(|entry| !open.iter().any(|project| same_path(&project.path, &entry.path)))
            .collect();
        Self { open, recent }
    }
}

/// The list as it is now (blocking: the recent folders are checked on the disk).
pub fn read(manager: &WindowManager) -> ProjectSwitcher {
    let open = manager
        .open_in_focus_order()
        .into_iter()
        .map(|(project, working)| OpenProjectEntry {
            id: project.id,
            path: project.root.to_string_lossy().into_owned(),
            name: project.name,
            working: u32::try_from(working).unwrap_or(u32::MAX),
        })
        .collect();
    ProjectSwitcher::build(open, recent_entries_of(&manager.recent(), Path::is_dir))
}

/// The list last sent to the windows.
#[derive(Default)]
pub struct SwitcherPublisher {
    last: Mutex<Option<ProjectSwitcher>>,
    /// A send is waiting for the blocking pool: the changes that come meanwhile are in it.
    queued: AtomicBool,
}

impl SwitcherPublisher {
    /// Asks for a send. `false` when one is already waiting to run, which reads the list as it is
    /// by then: a burst of changes (the jobs of a project report every step) is one send.
    fn request(&self) -> bool {
        !self.queued.swap(true, Ordering::SeqCst)
    }

    /// The waiting send starts: a change from now on asks for another.
    fn started(&self) {
        self.queued.store(false, Ordering::SeqCst);
    }

    /// Reads the list with `read` and, when it is not the one last sent, sends it to every project
    /// window through `send`. Reads are one at a time, so that the last list sent is the latest
    /// one read.
    fn publish(
        &self,
        read: impl FnOnce() -> ProjectSwitcher,
        send: impl FnOnce(&ProjectSwitcher),
    ) {
        let mut last = self.last.lock().expect("switcher publisher poisoned");
        let now = read();
        if last.as_ref() == Some(&now) {
            return;
        }
        send(&now);
        *last = Some(now);
    }
}

/// Tells every project window the list, on the blocking pool, when it changed.
pub fn schedule(app: &AppHandle) {
    let Some(publisher) = app.try_state::<Arc<SwitcherPublisher>>() else {
        return;
    };
    if !publisher.request() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let Some(manager) = super::manager_of(&app) else {
            return;
        };
        let Some(publisher) = app.try_state::<Arc<SwitcherPublisher>>() else {
            return;
        };
        publisher.started();
        publisher.publish(
            || read(&manager),
            |switcher| {
                for label in app.webview_windows().into_keys() {
                    if crate::pictures::project_of_label(&label).is_none() {
                        continue;
                    }
                    // `emit_to`, never `emit`: the welcome window has no switcher.
                    if let Err(error) =
                        app.emit_to(&label, EventName::ProjectSwitcherChanged.as_str(), switcher)
                    {
                        log_warn!("the window {label} was not told the open projects: {error}");
                    }
                }
            },
        );
    });
}

/// The open projects and the recent ones, for the window that opens the menu of its switcher.
#[tauri::command]
pub async fn project_switcher(
    manager: State<'_, Arc<WindowManager>>,
) -> Result<ProjectSwitcher, AppError> {
    let manager = manager.inner().clone();
    blocking::disk_named("project_switcher", None, move || read(&manager)).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(id: &str, path: &str, working: u32) -> OpenProjectEntry {
        OpenProjectEntry {
            id: id.to_owned(),
            path: path.to_owned(),
            name: id.to_owned(),
            working,
        }
    }

    fn recent(path: &str, exists: bool) -> RecentEntry {
        RecentEntry {
            path: path.to_owned(),
            name: path.rsplit('/').next().unwrap_or(path).to_owned(),
            opened: "2026-11-02T07:00:00.000Z".to_owned(),
            exists,
        }
    }

    #[test]
    fn a_recent_project_that_has_a_window_is_listed_among_the_open_ones_only() {
        let switcher = ProjectSwitcher::build(
            vec![open("a", "/work/a", 2), open("b", "/work/b", 0)],
            vec![
                recent("/work/b", true),
                recent("/work/c", true),
                recent("/work/gone", false),
            ],
        );
        let open: Vec<_> = switcher.open.iter().map(|p| (p.id.as_str(), p.working)).collect();
        assert_eq!(open, [("a", 2), ("b", 0)], "the order given is kept");
        let recent: Vec<_> = switcher.recent.iter().map(|r| (r.path.as_str(), r.exists)).collect();
        assert_eq!(recent, [("/work/c", true), ("/work/gone", false)]);
    }

    #[test]
    fn no_open_project_leaves_the_recent_ones_as_they_are() {
        let switcher = ProjectSwitcher::build(vec![], vec![recent("/work/a", true)]);
        assert!(switcher.open.is_empty());
        assert_eq!(switcher.recent.len(), 1);
    }

    #[test]
    fn a_list_equal_to_the_last_one_sent_is_not_sent_again() {
        let publisher = SwitcherPublisher::default();
        let sent = std::cell::Cell::new(0);
        let list = |working| ProjectSwitcher::build(vec![open("a", "/work/a", working)], vec![]);
        publisher.publish(|| list(0), |_| sent.set(sent.get() + 1));
        publisher.publish(|| list(0), |_| sent.set(sent.get() + 1));
        assert_eq!(sent.get(), 1);
        // A change of the working jobs of a project is sent.
        publisher.publish(|| list(1), |_| sent.set(sent.get() + 1));
        assert_eq!(sent.get(), 2);
    }

    #[test]
    fn a_burst_of_changes_waits_for_one_send_and_the_next_change_asks_for_another() {
        let publisher = SwitcherPublisher::default();
        assert!(publisher.request());
        assert!(!publisher.request(), "one is already waiting");
        assert!(!publisher.request());
        publisher.started();
        assert!(publisher.request(), "a change after the send started is not lost");
    }

    #[test]
    fn the_list_is_camel_case_json() {
        let switcher = ProjectSwitcher::build(vec![open("a", "/work/a", 1)], vec![]);
        let json = serde_json::to_value(&switcher).unwrap();
        assert_eq!(json["open"][0]["working"], 1);
        assert_eq!(json["open"][0]["path"], "/work/a");
        assert!(json["recent"].as_array().unwrap().is_empty());
    }
}
