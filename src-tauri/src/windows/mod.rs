//! Windows and start-up: machine `window` of the `app` feature.
//!
//! Weaver is one process for every open project, with one window per project (`p-<project hash>`)
//! and the `welcome` window when no project window is open.
//!
//! | Module           | Responsibility |
//! |------------------|----------------|
//! | [`manager`]      | [`WindowManager`]: open, focus, close, quit and restore, over the [`manager::Shell`] trait |
//! | [`registry`]     | The pure rules: which windows are open, the quit path, what an exit request means |
//! | [`local_state`]  | `state.json`: the recent projects, the last session and what System Certificates remembers; one store, owned by [`crate::state::AppState`] |
//! | [`launch`]       | The folder argument of a launch (first or second) |
//! | [`watch`]        | The file watcher of a project, batching changes for `Pictures::rebuild` |
//! | [`jobs`]         | The hook the jobs of the agent implement ([`WindowJobs`]) |
//! | [`project`]      | The hook the Machine `project` implements ([`project::ProjectHooks`], [`crate::project_setup`]) |
//! | [`editor`]       | `Open in editor` with the editor of the settings |
//! | [`unsaved`]      | The unsaved changes of a window's File view: what the UI tells, `Save all` at close and quit |
//! | [`tauri_shell`]  | The real windows, native confirmation and exit |
//! | [`fullscreen`]   | A fullscreen window sent to the background leaves fullscreen first (macOS) |
//! | [`tray`]         | The tray icon of Windows and Linux: `Show Weaver` and `Quit` |
//! | [`menu`]         | The application menu (`Cmd+Q` / `Ctrl+Q` / File › Quit, File › Open Folder… / Open Recent) |
//! | [`menu_open`]    | What the File menu opens: `Open Folder…`, `Open Recent`, `Clear Recent`, and a failed opening |
//! | [`commands`]     | Tauri commands: the welcome window and `open_in_editor` |
//!
//! # What other Tasks plug into
//!
//! All of it through `app.state::<Arc<WindowManager>>()`:
//!
//! * **Job count and suspension** (Task "Lavori dell'agente"): implement [`WindowJobs`] over the job
//!   registry and install it with [`WindowManager::set_jobs`]. `working_jobs` feeds the native
//!   confirmation of closing a window and of quitting; `window_closed` suspends the jobs of a project
//!   (ending their omp processes) and is awaited before the window closes or the app exits;
//!   `window_opened` is `window.OPENED`, with [`OpenReason::AppStart`] for the projects restored at
//!   start, whose working jobs resume.
//! * **Notification clicks** (Task "Notifiche"): [`WindowManager::focus_project`] brings the project's
//!   window forward and sends the `show-requested` event with a [`ShowRequest`] (the view, and the job
//!   whose conversation to open); [`WindowManager::focus_recent`] is the answer when no project is known;
//!   [`WindowManager::focus_order`] and [`WindowManager::show_welcome`] serve the click on the
//!   notification of the app (System Certificates turned on), routed by
//!   [`crate::notifications::route_click`]: the project window used most recently shows Settings,
//!   else the welcome window comes forward.
//! * **Open a project from elsewhere**: [`WindowManager::open_project`]; the File menu of every window
//!   does it through [`menu_open`].
//! * **Background** (`developer.CLOSE_WINDOW` → In background, `developer.REOPEN_APP`):
//!   [`WindowManager::send_to_background`] hides a project window without suspending anything,
//!   when the developer chose `Keep working in background` on closing it; it
//!   stays open for the jobs, the notifications, the restore list and the last-window rule.
//!   [`WindowManager::reopen_app`] brings them all back (the Dock, a second launch, the tray).

pub mod commands;
pub mod editor;
pub mod fullscreen;
pub mod jobs;
pub mod launch;
pub mod local_state;
pub mod manager;
pub mod menu;
pub mod menu_open;
pub mod project;
pub mod registry;
pub mod tauri_shell;
pub mod tray;
pub mod unsaved;
pub mod watch;

use std::sync::Arc;

use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_window_state::StateFlags;
use weaver_core::project::clock::SystemClock;

pub use jobs::{BoxFuture, CloseReason, NoJobs, OpenReason, WindowJobs, WindowOpened};
pub use manager::{CloseOutcome, OpenFailure, OpenOutcome, ShowRequest, ShowView, WindowManager};

use crate::startup::{self, ProjectRef, StartupReport, Step};
use crate::state::AppState;
use crate::{log_info, log_warn};

/// What the window-state plugin saves: size, position, maximised and fullscreen; never visibility.
pub const STATE_FLAGS: StateFlags = StateFlags::SIZE
    .union(StateFlags::POSITION)
    .union(StateFlags::MAXIMIZED)
    .union(StateFlags::FULLSCREEN);

/// The windows the window-state plugin leaves alone: the welcome window has a fixed size.
pub const STATE_DENYLIST: [&str; 1] = [registry::WELCOME_LABEL];

/// The window manager of the running app, once started.
pub fn manager_of(app: &AppHandle) -> Option<Arc<WindowManager>> {
    app.try_state::<Arc<WindowManager>>()
        .map(|state| state.inner().clone())
}

/// A project that could not be opened without anyone asking: `<project> could not open: <reason>`.
fn report_failure(app: &AppHandle, failure: OpenFailure) {
    let report = app.state::<StartupReport>();
    let project = ProjectRef {
        id: failure.project_id,
        name: failure.name,
    };
    let reason = failure.reason;
    startup::run_now(
        app,
        &report,
        Step::project(project, "project window", move || Err(reason)),
    );
}

/// `desktop.APP_STARTED`: creates the window manager, restores the projects of the last session
/// (and the folder this launch was given, if any), and watches for a shutdown that allows no
/// confirmation. Call from the setup of the app.
pub fn start(app: &AppHandle) {
    let state = app.state::<AppState>();
    let shell = Arc::new(tauri_shell::TauriShell::new(app.clone()));
    let store = state.local_state.clone();
    let failure_app = app.clone();
    let manager = WindowManager::new(
        shell,
        state.pictures.clone(),
        store,
        Arc::new(SystemClock),
        Arc::new(watch::NotifyWatch),
        Arc::new(move |failure| report_failure(&failure_app, failure)),
    );
    app.manage(manager.clone());
    // At most one folder picker from the File menu at a time.
    app.manage(Arc::new(menu_open::PickerLatch::default()));
    // `Open Recent` of the File menu starts from the recent projects of the last session.
    menu::refresh(app);
    // Where Weaver stays reachable while its windows are in the background (Windows and Linux).
    tray::install(app);
    // The jobs of the agent hear of every window that opens or closes, from the first one.
    crate::jobs::install_windows(app, &manager);
    crate::project_setup::install_windows(app, &manager);

    let args: Vec<String> = std::env::args().collect();
    let folder = std::env::current_dir()
        .ok()
        .and_then(|cwd| launch::folder_argument(&args, &cwd));
    let restoring = manager.clone();
    tauri::async_runtime::spawn(async move { restoring.restore(folder).await });
    watch_shutdown(manager);
}

/// `developer.QUIT_APP` from the menu: a quit the developer started disarms a pending update, like
/// `Cmd+Q`, then goes through the quit path.
pub fn request_quit(app: &AppHandle) {
    if let Some(updates) = app.try_state::<Arc<crate::updates::Updates>>() {
        updates.on_exit_requested(None);
    }
    if let Some(manager) = manager_of(app) {
        tauri::async_runtime::spawn(async move { manager.quit(0, true).await });
    }
}

/// The events of the run loop that concern windows: close requests, focus, destruction, exit
/// requests and a click on the Dock icon.
pub fn on_run_event(app: &AppHandle, event: &RunEvent) {
    let Some(manager) = manager_of(app) else {
        return;
    };
    match event {
        RunEvent::ExitRequested { api, code, .. } => {
            let windows_left = app.webview_windows().len();
            if manager.exit_requested(*code, windows_left) != registry::ExitDecision::Allow {
                api.prevent_exit();
            }
        }
        RunEvent::WindowEvent { label, event, .. } => {
            // The focus of each window and its destruction, for the notifications.
            crate::notifications::on_window_event(app, label, event);
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    // The welcome window closes at once; a project window goes through the close path.
                    if let Some(project_id) = crate::pictures::project_of_label(label) {
                        api.prevent_close();
                        manager.close_requested(project_id);
                    }
                }
                WindowEvent::Focused(true) => {
                    manager.on_focused(label);
                    // `Open Recent` checks the folders again whenever a window gets the focus.
                    menu::refresh(app);
                }
                WindowEvent::Destroyed => manager.on_destroyed(label),
                _ => {}
            }
        }
        #[cfg(target_os = "macos")]
        RunEvent::Reopen {
            has_visible_windows,
            ..
        } => manager.reopen(*has_visible_windows),
        _ => {}
    }
}

/// A shutdown that allows no confirmation (the system or a signal ends Weaver): the jobs are
/// suspended without asking, the session is written and the app exits.
fn watch_shutdown(manager: Arc<WindowManager>) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        for (name, kind) in [
            ("SIGTERM", SignalKind::terminate()),
            ("SIGHUP", SignalKind::hangup()),
            ("SIGINT", SignalKind::interrupt()),
        ] {
            let manager = manager.clone();
            tauri::async_runtime::spawn(async move {
                let mut stream = match signal(kind) {
                    Ok(stream) => stream,
                    Err(error) => return log_warn!("{name} is not handled: {error}"),
                };
                if stream.recv().await.is_some() {
                    log_info!("{name} received: suspending the jobs and quitting without asking");
                    manager.quit(0, false).await;
                }
            });
        }
    }
    #[cfg(windows)]
    {
        use tokio::signal::windows::{ctrl_break, ctrl_c, ctrl_close, ctrl_logoff, ctrl_shutdown};
        macro_rules! watch {
            ($name:literal, $make:expr) => {{
                let manager = manager.clone();
                tauri::async_runtime::spawn(async move {
                    let mut stream = match $make {
                        Ok(stream) => stream,
                        Err(error) => return log_warn!("{} is not handled: {error}", $name),
                    };
                    if stream.recv().await.is_some() {
                        log_info!(
                            "{} received: suspending the jobs and quitting without asking",
                            $name
                        );
                        manager.quit(0, false).await;
                    }
                });
            }};
        }
        watch!("Ctrl+C", ctrl_c());
        watch!("Ctrl+Break", ctrl_break());
        watch!("the console closing", ctrl_close());
        watch!("the user logging off", ctrl_logoff());
        watch!("the system shutting down", ctrl_shutdown());
    }
}
