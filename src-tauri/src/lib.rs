//! Weaver, the desktop app: backend of the Tauri window(s).
//!
//! Module map (each later Task extends one of these without reshaping the others):
//!
//! | Module        | Responsibility |
//! |---------------|----------------|
//! | [`commands`]  | Tauri commands the UI calls; typed results and [`error::AppError`] |
//! | [`events`]    | Backend → UI events (`project-changed`, ...) and their payloads |
//! | [`bindings`]  | `ts-rs` generation and the up-to-date check of `ui/src/bindings/` |
//! | [`state`]     | App-wide state managed by Tauri |
//! | [`mutations`] | One serial mutation queue per project, holding the OS project lock |
//! | [`server`]    | The CLI↔app socket server and `runtime.json` |
//! | [`sidecar`]   | Copy of the bundled `weaver-cli` as `weaver` into `<app local data>/bin/` |
//! | [`blocking`]  | `spawn_blocking` helper: disk work never runs on the main or IPC thread |
//! | [`error`]     | The typed error of commands |
//! | [`logging`]   | The log file: daily, rotated at 10 MB, 7 kept, no contents |
//! | [`notifications`]| System notifications: a job finished or stopped, a round of questions or a permission waits, when the window is not in the foreground |
//! | [`startup`]   | Startup as isolated steps; failures reach the UI with **Open log** |
//! | [`shell`]     | Commands of the app shell: startup failures, Open log, open a link |
//! | [`pictures`]  | The in-memory picture of each open project, with its revision; opened and closed with its window (`Pictures::open(root)` when the window is created) |
//! | [`decisions`] | Applies a decision of the developer in the project's queue, checked again when applied |
//! | [`board`]     | The Board snapshot and the Delete decision |
//! | [`project_view`]| The Requirements and Specs snapshot, with a Proposal drawn over the Model |
//! | [`project_setup`]| Get Started, the files installed for the agent kept up to date, Update skills, Import from Code |
//! | [`proposal_view`]| What the card and the details of a Proposal show, worked out in the core |
//! | [`proposals`] | The decisions on a Proposal (Approve, Reject, Rework, Discard, model) and what they start |
//! | [`requests`]  | Design requests: the Markdown editor's files, Send, Discard, images, and the hooks that consume or return them |
//! | [`agent_work`]| Ghosts and facts about the agent's work, installed by the jobs |
//! | [`history`]   | The History view: the entries of a project, newest first, with the Tasks they cite |
//! | [`jobs`]      | The jobs of the agent: omp over ACP, the registry, the Jobs view, Run, Run all, Scan |
//! | [`general`]   | General: the free conversation with omp of each project, its kept session, and `weaver job new` from it |
//! | [`cli_handler`]| The app side of the `weaver` commands that write: runs them in the project's queue |
//! | [`omp`]       | The Machine `omp-setup`: where omp is, whether it is ready, installing, updating and logging in |
//! | [`rounds`]    | Question rounds: the queue of each project, the agents waiting for the developer, the prepared question of a Drift |
//! | [`file_view`] | The File view's tree: each window's listings and watches, the commands that create, rename, move and delete, released 10 s after the view is left |
//! | [`windows`]   | The Machine `window`: one window per project, the welcome window, restore at start, close and quit confirmations, `Open in editor` |
//! | [`system_certificates`] | The Machine `system-certificates`: the export of the certificates the system trusts, the status the windows show, the variables of omp processes |
//! | [`file_tabs`] | The Machine `file-tab`: the files open in the tabs of the File view, what each read, its watch, its saving; the events of the tabs and of the agent lock |
//! | [`file_lock`] | The Machine `file-lock`: which files open in a tab an agent of the project is changing, from the live updates of its conversations |
//! | [`assets`]    | The Assets view: the name and size of each open project's assets, the commands that copy in, rename, delete and drag them out |
//! | [`notes`]     | The Notes view: the project's Markdown sheet and its images, the commands that read, write, copy in and trash them, the flush before a window closes, the watcher's events |
//!
//! Folders, protocol, locks and process spawning come from `weaver-core`, so the app and the
//! `weaver` command line agree on them.

pub mod agent_work;
pub mod ask;
pub mod assets;
pub mod bindings;
pub mod blocking;
pub mod board;
pub mod cli_handler;
pub mod commands;
pub mod decisions;
pub mod drift_view;
pub mod drifts;
pub mod error;
pub mod events;
pub mod file_lock;
pub mod file_tabs;
pub mod file_view;
pub mod general;
pub mod history;
pub mod jobs;
pub mod logging;
pub mod mutations;
pub mod notes;
pub mod notifications;
pub mod omp;
pub mod pictures;
pub mod project_setup;
pub mod project_view;
pub mod proposal_view;
pub mod proposals;
pub mod requests;
pub mod rounds;
pub mod server;
pub mod settings;
pub mod shell;
pub mod sidecar;
pub mod startup;
pub mod state;
pub mod subagents;
pub mod system_certificates;
pub mod updates;
pub mod usage;
pub mod windows;

use std::sync::Arc;

use crate::startup::{StartupReport, Step};
use crate::state::AppState;
use tauri::{AppHandle, Manager, RunEvent};
use weaver_core::paths::AppDirs;

/// Starts the app. Returns when the last window closed and the app exited.
pub fn run() {
    let dirs = AppDirs::resolve().expect("the system has no config / local data folder");

    let app = tauri::Builder::default()
        // The single-instance plugin is registered first, as the plugin documentation requires.
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            // A second launch: a folder argument opens (or fronts) its window, else the most
            // recently used window comes forward.
            if let Some(manager) = windows::manager_of(app) {
                manager.second_launch(args, std::path::PathBuf::from(cwd));
            }
        }))
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(windows::STATE_FLAGS)
                .with_denylist(&windows::STATE_DENYLIST)
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::new(dirs.clone()))
        .manage(omp::OmpSetup::new())
        .manage(system_certificates::SystemCertificates::new())
        .menu(windows::menu::build)
        .on_menu_event(|app, event| windows::menu::on_event(app, event.id().as_ref()))
        .manage(StartupReport::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            shell::startup_failures,
            shell::open_log,
            shell::open_url,
            updates::update_status,
            updates::update_later,
            updates::update_install,
            settings::settings_get,
            settings::settings_set,
            board::board_snapshot,
            board::delete_task,
            proposals::approve_proposal,
            proposals::reject_proposal,
            proposals::discard_proposal,
            proposals::choose_proposal_model,
            drifts::resolve_drift,
            drifts::fix_drift_code,
            drifts::accept_drift_proposal,
            drifts::refine_drift_proposal,
            drifts::discard_drift_proposal,
            drifts::dismiss_drift,
            history::history_snapshot,
            project_view::view_snapshot,
            project_setup::project_status,
            project_setup::initialise_project,
            project_setup::update_skills,
            project_setup::set_solo_mode,
            project_setup::solo_tracked_files,
            project_setup::set_atomic_commits,
            project_setup::check_atomic_commits,
            project_setup::merge::merge_refresh,
            project_setup::merge::merge_branches,
            project_setup::merge::merge_start,
            project_setup::merge::merge_abort,
            project_setup::merge::resolve_merge_conflicts,
            project_setup::project_folder,
            project_setup::import_code,
            omp::omp_status,
            omp::omp_install,
            omp::omp_log_in,
            omp::omp_log_out,
            omp::omp_usage,
            omp::usage_status,
            system_certificates::system_certificates_status,
            system_certificates::system_certificates_refresh,
            system_certificates::system_certificates_dismiss_notice,
            system_certificates::system_certificates_dismiss_failure,
            windows::commands::welcome_state,
            windows::commands::open_project,
            windows::commands::open_folder,
            windows::commands::open_folder_from_window,
            windows::switcher::project_switcher,
            windows::git_state::project_git,
            windows::commands::remove_recent,
            windows::commands::open_in_editor,
            jobs::commands::jobs_snapshot,
            jobs::commands::job_conversation,
            jobs::commands::job_send_message,
            jobs::commands::job_stop,
            jobs::commands::job_answer_permission,
            jobs::commands::job_remove,
            jobs::commands::job_sub_agents,
            jobs::commands::job_sub_agent_conversation,
            jobs::commands::job_sub_agents_view,
            jobs::commands::run_task,
            jobs::commands::run_all,
            jobs::commands::scan_system,
            general::commands::general_snapshot,
            general::commands::general_send_message,
            general::commands::general_stop,
            general::commands::general_answer_permission,
            general::commands::general_clear,
            general::proposals::general_proposals,
            general::commands::general_cards,
            general::commands::general_card_act,
            general::commands::general_send_answers,
            general::commands::general_sub_agents,
            general::commands::general_sub_agent_conversation,
            general::commands::general_sub_agents_view,
            ask::commands::ask_open,
            ask::commands::ask_list,
            ask::commands::ask_snapshot,
            ask::commands::ask_send_message,
            ask::commands::ask_stop,
            ask::commands::ask_answer_permission,
            ask::commands::ask_close,
            ask::commands::ask_sub_agents,
            ask::commands::ask_sub_agent_conversation,
            ask::commands::ask_sub_agents_view,
            requests::request_read,
            requests::request_new,
            requests::request_save,
            requests::request_send,
            requests::request_discard,
            requests::request_leave,
            requests::request_image_add,
            requests::request_image_import,
            requests::request_image,
            requests::requests_flushed,
            windows::unsaved::files_unsaved,
            windows::unsaved::files_flushed,
            file_tabs::commands::file_open,
            file_tabs::commands::file_reload,
            file_tabs::commands::file_save,
            file_tabs::commands::file_close,
            assets::commands::assets_list,
            assets::commands::assets_copy_in,
            assets::commands::assets_rename,
            assets::commands::assets_delete,
            assets::commands::assets_drag_out,
            notes::commands::notes_read,
            notes::commands::notes_write,
            notes::commands::notes_image,
            notes::commands::notes_add_pasted,
            notes::commands::notes_add_dropped,
            notes::commands::notes_trash,
            notes::commands::notes_flushed,
            file_view::commands::files_open_view,
            file_view::commands::files_expand,
            file_view::commands::files_collapse,
            file_view::commands::files_folders,
            file_view::commands::files_leave,
            file_view::commands::files_create,
            file_view::commands::files_rename,
            file_view::commands::files_move,
            file_view::commands::files_copy_in,
            file_view::commands::files_delete,
            windows::unsaved::files_close_key,
            rounds::commands::question_round_get,
            rounds::commands::question_round_act
        ])
        .setup(move |app| {
            let handle = app.handle().clone();

            // Startup is a list of isolated steps: one that fails is logged, shown to the developer
            // and does not stop the others. Logging goes first, here, so the others can log.
            let report = app.state::<StartupReport>();
            let log_handle = handle.clone();
            startup::run_now(
                &handle,
                &report,
                Step::app("logging", move || {
                    let dir = log_handle.path().app_log_dir().map_err(|e| e.to_string())?;
                    let logger = logging::init(&dir)
                        .map_err(|e| format!("cannot open the log in {}: {e}", dir.display()))?;
                    log_info!(
                        "Weaver {} started (log level {})",
                        env!("CARGO_PKG_VERSION"),
                        logger.level().label()
                    );
                    Ok(())
                }),
            );

            // The events of the File view's tabs and of the agent lock have somewhere to go before
            // the first conversation of an agent can take a lock.
            handle
                .state::<AppState>()
                .file_events
                .attach(Arc::new(file_tabs::events::AppSink(handle.clone())));
            // The tree of the File view tells its window when a listed folder changes.
            handle
                .state::<AppState>()
                .file_view
                .attach(file_view::emitter(handle.clone()));
            // The assets of a project tell its window when their list changes.
            handle
                .state::<AppState>()
                .assets
                .attach(assets::emitter(handle.clone()));
            // The notes of a project tell its window when they change on disk (not by Weaver).
            handle
                .state::<AppState>()
                .notes_changes
                .attach(notes::changes::emitter(handle.clone()));
            // The jobs of the agent: before the windows and the CLI socket, which use them.
            let jobs = jobs::install(&handle);
            requests::install(&handle, &jobs);
            drifts::install(&handle, &jobs);
            // System notifications hear of the jobs and of the clicks on them.
            notifications::install(&handle, &jobs);
            // Settings: read the file now (a few hundred bytes) so the first command does not.
            let settings_handle = handle.clone();
            startup::spawn(
                &handle,
                Step::app("settings", move || {
                    settings_handle.state::<AppState>().settings.load_now();
                    Ok(())
                }),
            );

            // System Certificates: the bundle on disk and the setting; an export, when due, runs
            // on the blocking pool and never holds up the start. It is spawned before `omp`: the
            // age of the bundle and the Zscaler turn-on are decided before omp's first check,
            // which waits for an export that started then (`SystemCertificates::startup_export`).
            let certificates_handle = handle.clone();
            startup::spawn(
                &handle,
                Step::app("system certificates", move || {
                    system_certificates::start(&certificates_handle)
                }),
            );

            // omp: finds it, asks its version and models, and follows the `ompPath` setting.
            let omp_handle = handle.clone();
            startup::spawn(&handle, Step::app("omp", move || omp::start(&omp_handle)));

            // Updates: the check at start and every hour runs on the async runtime.
            updates::start(&handle);

            // The rest runs on the blocking pool: the window must not wait for the disk.
            let sidecar_dirs = dirs.clone();
            startup::spawn(
                &handle,
                Step::app("weaver command line", move || {
                    match sidecar::install(&sidecar_dirs) {
                        Ok(sidecar::SyncOutcome::SourceMissing) => Err(
                            "the bundled `weaver-cli` is missing or empty; it was not copied"
                                .to_owned(),
                        ),
                        Ok(_) => Ok(()),
                        Err(error) => Err(format!("cannot install `weaver`: {error}")),
                    }
                }),
            );

            let server_handle = handle.clone();
            startup::spawn(
                &handle,
                Step::app("local socket", move || {
                    let handler = cli_handler::handler(server_handle.clone());
                    let server = server::Server::start(&dirs, env!("CARGO_PKG_VERSION"), handler)
                        .map_err(|error| {
                        format!("cannot start the CLI socket server: {error}")
                    })?;
                    *server_handle
                        .state::<AppState>()
                        .server
                        .lock()
                        .expect("server slot poisoned") = Some(server);
                    Ok(())
                }),
            );

            // Windows: restores the projects of the last session, or shows the welcome window.
            windows::start(&handle);
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the Weaver app");

    app.run(|app, event| {
        windows::on_run_event(app, &event);
        on_exit_or_destroyed(app, event);
    });
}

/// What the app does itself at exit and when a window is destroyed (the window rules are in
/// [`windows::on_run_event`]).
fn on_exit_or_destroyed(app: &AppHandle, event: RunEvent) {
    match event {
        RunEvent::ExitRequested { code, .. } => {
            if let Some(updates) = app.try_state::<Arc<updates::Updates>>() {
                updates.on_exit_requested(code);
            }
        }
        RunEvent::Exit => {
            if let Some(mut server) = app
                .state::<AppState>()
                .server
                .lock()
                .expect("server slot poisoned")
                .take()
            {
                server.shutdown();
            }
            // An update chosen with Install and restart is applied now that the quit went through.
            if app
                .try_state::<Arc<updates::Updates>>()
                .is_some_and(|updates| updates.install_on_exit())
            {
                app.restart();
            }
        }
        // The picture of a project lives as long as its window (`p-<project hash>`).
        RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::Destroyed,
            ..
        } => {
            if let Some(project_id) = pictures::project_of_label(&label) {
                app.state::<AppState>().pictures.close(project_id);
                // `window.CLOSED`: the File view's listings and watches go at once.
                app.state::<AppState>().file_view.close_project(project_id);
                // The list of the assets goes with the window; a copy still running goes on to its end.
                app.state::<AppState>().assets.close_project(project_id);
                // Nothing of the notes events is kept for the project.
                app.state::<AppState>()
                    .notes_changes
                    .close_project(project_id);
                // The notes' remembered hash goes with the window; a write still running goes on to its end.
                app.state::<AppState>().notes.close_project(project_id);
            }
        }
        _ => {}
    }
}
