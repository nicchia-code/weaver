//! The Machine `shell-command` in the app (`requirements/tech#shell-command`): the three commands
//! of the settings that read, install and remove the `weaver` command in the developer's terminal.
//!
//! What is installed, and how, per system, is in [`weaver_core::shell_command`]; the copy it
//! makes reachable is the one [`crate::sidecar`] refreshes at every start. Weaver never installs
//! or removes the command by itself: only these commands do, when the developer asks.
//!
//! The work runs on the blocking pool. One action at a time in the whole app: a second install
//! or removal while one runs is refused with an error and does nothing. A failure is logged as a
//! warning, like the other failures of the app.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tauri::State;
use weaver_core::shell_command::{self, ShellCommandOutcome, ShellCommandResult};

use crate::error::{AppError, ErrorCode};
use crate::state::AppState;
use crate::{blocking, log_warn, sidecar};

/// Whether an install or a removal runs. Managed by Tauri.
#[derive(Default)]
pub struct ShellCommandBusy(Arc<AtomicBool>);

/// Held while an action runs; releases the app on drop, also when the work panics.
struct Running(Arc<AtomicBool>);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

impl ShellCommandBusy {
    /// Takes the one place for an action, or refuses when another action holds it.
    fn begin(&self) -> Result<Running, AppError> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Running(self.0.clone()))
            .map_err(|_| {
                AppError::new(
                    ErrorCode::Refused,
                    "another action on the weaver command is already running",
                )
            })
    }
}

/// The copy of the command line the terminal is made to reach.
fn copy_path(state: &AppState) -> PathBuf {
    state.dirs.bin_dir().join(sidecar::FILE_NAME)
}

/// What the terminal finds now. Read each time Settings shows.
#[tauri::command]
pub async fn shell_command_status(
    state: State<'_, AppState>,
) -> Result<ShellCommandResult, AppError> {
    let copy = copy_path(&state);
    blocking::disk_named("shell_command_status", None, move || {
        shell_command::status(&copy)
    })
    .await
}

/// `developer.INSTALL_COMMAND`.
#[tauri::command]
pub async fn shell_command_install(
    state: State<'_, AppState>,
    busy: State<'_, ShellCommandBusy>,
) -> Result<ShellCommandResult, AppError> {
    let running = busy.begin()?;
    let copy = copy_path(&state);
    let result = blocking::disk_named("shell_command_install", None, move || {
        shell_command::install(&copy)
    })
    .await;
    drop(running);
    let result = result?;
    log_failure("install", &result);
    Ok(result)
}

/// `developer.REMOVE_COMMAND`.
#[tauri::command]
pub async fn shell_command_remove(
    state: State<'_, AppState>,
    busy: State<'_, ShellCommandBusy>,
) -> Result<ShellCommandResult, AppError> {
    let running = busy.begin()?;
    let copy = copy_path(&state);
    let result = blocking::disk_named("shell_command_remove", None, move || {
        shell_command::remove(&copy)
    })
    .await;
    drop(running);
    let result = result?;
    log_failure("remove", &result);
    Ok(result)
}

/// A failed action is a warning in the log.
fn log_failure(action: &str, result: &ShellCommandResult) {
    if let ShellCommandOutcome::Failed { reason } = &result.outcome {
        log_warn!("could not {action} the weaver command: {reason}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_action_while_one_runs_is_refused_and_the_place_frees_when_it_ends() {
        let busy = ShellCommandBusy::default();
        let first = busy.begin().expect("the first action starts");

        let refused = busy.begin().err().expect("the second is refused");
        assert_eq!(refused.code, ErrorCode::Refused);

        drop(first);
        assert!(busy.begin().is_ok(), "the next action can start");
    }

    #[test]
    fn the_copy_is_the_one_the_sidecar_refreshes() {
        let dirs = weaver_core::paths::AppDirs::from_bases(
            std::path::Path::new("/c"),
            std::path::Path::new("/l"),
            None,
        );
        let state = AppState::new(dirs.clone());
        assert_eq!(
            copy_path(&state),
            dirs.bin_dir().join(sidecar::FILE_NAME)
        );
        assert!(copy_path(&state).starts_with(dirs.local_data_dir()));
    }
}
