//! What the File menu opens (`developer.OPEN_PROJECT` from any window): `Open Folder…`, an item of
//! `Open Recent` and `Clear Recent`.
//!
//! The menu event handler never blocks: each entry point reads the window the item was chosen from
//! (the Weaver window with the focus, `None` when there is none) and starts async work.
//!
//! * `Open Folder…` runs the picker of the welcome window's `open_folder` command
//!   ([`super::commands::pick_folder`]), modal to the window it was chosen from, and opens the
//!   chosen folder as `open_project` does. At most one picker from the menu is open at a time
//!   ([`PickerLatch`]): while one is open the item is ignored.
//! * An `Open Recent` item opens its canonical path as the `open_project` command does.
//! * `Clear Recent` empties the recent projects through the owner of `state.json`
//!   ([`super::WindowManager::clear_recent`]).
//!
//! An opening that fails is logged as a warning and its reason goes, as the event
//! `menu-open-failed`, to the window it was chosen from only. With no such window the welcome
//! window is opened (or brought forward) and gets it: the event, and the same failure kept in the
//! manager for the case its page is still loading ([`super::WindowManager::take_welcome_failure`]).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use super::commands::pick_folder;
use super::jobs::OpenReason;
use super::manager::{folder_name, MenuOpenFailed};
use super::registry::WELCOME_LABEL;
use crate::blocking;
use crate::events::EventName;
use crate::log_warn;

/// Whether a picker started from the menu is open.
#[derive(Default)]
pub struct PickerLatch(AtomicBool);

/// A picker started from the menu is open until this is dropped.
pub struct PickerOpen(Arc<PickerLatch>);

impl PickerLatch {
    /// Takes the latch; `None` while another picker from the menu holds it.
    pub fn try_take(self: &Arc<Self>) -> Option<PickerOpen> {
        self.0
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| PickerOpen(self.clone()))
    }
}

impl Drop for PickerOpen {
    fn drop(&mut self) {
        self.0 .0.store(false, Ordering::SeqCst);
    }
}

/// The label of the window an item was chosen from, now.
fn chosen_from(app: &AppHandle) -> Option<String> {
    super::menu::focused_window(app).map(|window| window.label().to_owned())
}

/// **Open Folder…** of the File menu: the picker is modal to the window with the focus.
pub fn open_folder(app: &AppHandle) {
    open_folder_from(app, super::menu::focused_window(app));
}

/// `Open Folder…`, modal to `from` (the window it was chosen from: the one with the focus for the
/// File menu, the asking window for the project switcher and the palette), and the opening of the
/// folder chosen. A failure reaches `from` as `menu-open-failed`.
pub fn open_folder_from(app: &AppHandle, from: Option<WebviewWindow>) {
    let Some(open) = app
        .try_state::<Arc<PickerLatch>>()
        .and_then(|latch| latch.inner().try_take())
    else {
        return;
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let from_label = from.as_ref().map(|window| window.label().to_owned());
        let chosen = pick_folder(&app, from.as_ref()).await;
        drop(open);
        match chosen {
            Ok(None) => {}
            Ok(Some(folder)) => open_path(&app, from_label.as_deref(), folder).await,
            Err(error) => report(
                &app,
                from_label.as_deref(),
                MenuOpenFailed {
                    name: None,
                    reason: error.message,
                },
            ),
        }
    });
}

/// An item of **Open Recent**: opens `path`, the canonical path of the project.
pub fn open_recent(app: &AppHandle, path: &str) {
    let from = chosen_from(app);
    let app = app.clone();
    let path = PathBuf::from(path);
    tauri::async_runtime::spawn(async move {
        open_path(&app, from.as_deref(), path).await;
    });
}

/// **Clear Recent**: empties the recent projects, leaving the last session and the windows alone.
pub fn clear_recent(app: &AppHandle) {
    let Some(manager) = super::manager_of(app) else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        let cleared = blocking::disk_named("clear_recent", None, move || manager.clear_recent())
            .await
            .and_then(|result| result);
        if let Err(error) = cleared {
            log_warn!("the recent projects were not cleared: {error}");
        }
    });
}

/// `developer.OPEN_PROJECT` for `folder`; a failure goes to the window it was chosen from.
async fn open_path(app: &AppHandle, from: Option<&str>, folder: PathBuf) {
    let Some(manager) = super::manager_of(app) else {
        return;
    };
    let name = folder_name(&folder);
    if let Err(error) = manager.open_project(folder, OpenReason::Developer).await {
        report(
            app,
            from,
            MenuOpenFailed {
                name: Some(name),
                reason: error.message,
            },
        );
    }
}

/// Logs the failure and tells the window the item was chosen from, or the welcome window when
/// there is none.
fn report(app: &AppHandle, from: Option<&str>, failure: MenuOpenFailed) {
    match &failure.name {
        Some(name) => log_warn!("{name} could not open: {}", failure.reason),
        None => log_warn!("the folder could not be opened: {}", failure.reason),
    }
    let route = route_of(from, |label| app.get_webview_window(label).is_some());
    let label = match route {
        FailureRoute::Window(label) => label,
        FailureRoute::Welcome => {
            if let Some(manager) = super::manager_of(app) {
                manager.set_welcome_failure(failure.clone());
                manager.show_welcome();
            }
            WELCOME_LABEL
        }
    };
    // `emit_to`, never `emit`: only that window's UI hears of it.
    if let Err(error) = app.emit_to(label, EventName::MenuOpenFailed.as_str(), &failure) {
        log_warn!("the window {label} was not told why a folder could not be opened: {error}");
    }
}

/// Who hears of a failed opening.
#[derive(Debug, PartialEq, Eq)]
enum FailureRoute<'a> {
    /// The window the item was chosen from, which still exists.
    Window(&'a str),
    /// No such window: the welcome window opens (or comes forward) and gets it.
    Welcome,
}

fn route_of<'a>(from: Option<&'a str>, exists: impl Fn(&str) -> bool) -> FailureRoute<'a> {
    match from.filter(|label| exists(label)) {
        Some(label) => FailureRoute::Window(label),
        None => FailureRoute::Welcome,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_picker_from_the_menu_is_open_at_a_time() {
        let latch = Arc::new(PickerLatch::default());
        let first = latch.try_take();
        assert!(first.is_some());
        assert!(
            latch.try_take().is_none(),
            "a second Open Folder… is ignored"
        );
        drop(first);
        assert!(
            latch.try_take().is_some(),
            "free again once the picker closed"
        );
    }

    #[test]
    fn a_failure_goes_to_the_window_it_was_chosen_from_alone() {
        let exists = |label: &str| label == "p-aaaa" || label == "welcome";
        assert_eq!(
            route_of(Some("p-aaaa"), exists),
            FailureRoute::Window("p-aaaa")
        );
        assert_eq!(
            route_of(Some("welcome"), exists),
            FailureRoute::Window("welcome")
        );
    }

    #[test]
    fn a_failure_with_no_window_to_say_it_goes_to_the_welcome_window() {
        let exists = |label: &str| label == "welcome";
        assert_eq!(route_of(None, exists), FailureRoute::Welcome);
        assert_eq!(route_of(Some("p-gone"), exists), FailureRoute::Welcome);
    }
}
