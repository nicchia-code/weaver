//! [`Shell`] over Tauri: the real windows, the native confirmation (`tauri-plugin-dialog`) and the
//! exit.
//!
//! * A project window is `p-<project hash>`, titled with the project's folder name, 1280 × 800 by
//!   default and never smaller than 960 × 600. It is created hidden and shown once built: the
//!   window-state plugin restores its size, position, maximised and fullscreen state while it is
//!   built (a saved position on a monitor that is gone is ignored, and the window stays centred on
//!   the primary monitor), and it never touches visibility.
//! * The welcome window is `welcome`, 640 × 480 and not resizable; its size never needs restoring,
//!   so the window-state plugin skips it (`with_denylist`).
//! * The confirmation is a native dialog, not a page of the webview, so it works with a hung webview.

use std::path::Path;

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_dialog::{
    DialogExt, MessageDialogButtons, MessageDialogKind, MessageDialogResult,
};
use tauri_plugin_window_state::AppHandleExt;

use super::fullscreen::Fullscreen;
use super::jobs::BoxFuture;
use super::manager::{ConfirmPrompt, Confirmation, HideFailed, Shell, ShowRequest};
use super::registry::WELCOME_LABEL;
use super::unsaved::{self, FilesSaveFailed};
use super::STATE_FLAGS;
use crate::blocking;
use crate::events::{self, EventName, ProjectChanged};
use crate::file_tabs::{SaveMode, SaveOutcome};
use crate::log_warn;
use crate::notes::changes::NotesBatch;
use crate::pictures::Rebuild;
use crate::state::AppState;

/// Size of a project window without a saved state.
pub const DEFAULT_SIZE: (f64, f64) = (1280.0, 800.0);
/// Smallest a project window gets.
pub const MIN_SIZE: (f64, f64) = (960.0, 600.0);
/// The welcome window.
pub const WELCOME_SIZE: (f64, f64) = (640.0, 480.0);

pub struct TauriShell {
    app: AppHandle,
    fullscreen: Fullscreen,
}

impl TauriShell {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            fullscreen: Fullscreen::default(),
        }
    }

    /// Shows, restores and focuses a window: back from the background, in the order the
    /// requirements give (`show()`, `unminimize()`, `set_focus()`).
    fn bring_forward(&self, window: &WebviewWindow) {
        self.reveal_window(window);
        let _ = window.set_focus();
    }

    /// Shows a window and restores it when minimised, without taking the focus; a window that left
    /// fullscreen to go to the background goes back to fullscreen.
    fn reveal_window(&self, window: &WebviewWindow) {
        let _ = window.show();
        let _ = window.unminimize();
        self.fullscreen.restore(window);
    }
}

/// The button a native dialog reports as a [`Confirmation`]. The platforms report a custom button
/// by its label or as `Yes` / `No` / `Ok`; anything else is a cancel. The middle button of a
/// three-button dialog is `second`: its label and what pressing it means.
fn confirmation_of(
    pressed: &MessageDialogResult,
    accept: &str,
    second: Option<(&str, Confirmation)>,
) -> Confirmation {
    match pressed {
        MessageDialogResult::Yes | MessageDialogResult::Ok => Confirmation::Accept,
        MessageDialogResult::No => second.map_or(Confirmation::Cancel, |(_, meaning)| meaning),
        MessageDialogResult::Custom(label) if label == accept => Confirmation::Accept,
        MessageDialogResult::Custom(label) => match second {
            Some((second, meaning)) if second == label => meaning,
            _ => Confirmation::Cancel,
        },
        _ => Confirmation::Cancel,
    }
}

impl Shell for TauriShell {
    fn create_project_window(&self, label: &str, title: &str) -> Result<(), String> {
        let window =
            WebviewWindowBuilder::new(&self.app, label, WebviewUrl::App("index.html".into()))
                .title(title)
                .inner_size(DEFAULT_SIZE.0, DEFAULT_SIZE.1)
                .min_inner_size(MIN_SIZE.0, MIN_SIZE.1)
                .center()
                .visible(false)
                .build()
                .map_err(|error| error.to_string())?;
        self.bring_forward(&window);
        Ok(())
    }

    fn show_welcome(&self) -> Result<(), String> {
        if let Some(window) = self.app.get_webview_window(WELCOME_LABEL) {
            self.bring_forward(&window);
            return Ok(());
        }
        let window = WebviewWindowBuilder::new(
            &self.app,
            WELCOME_LABEL,
            WebviewUrl::App("index.html".into()),
        )
        .title("Weaver")
        .inner_size(WELCOME_SIZE.0, WELCOME_SIZE.1)
        .resizable(false)
        .maximizable(false)
        .center()
        .visible(false)
        .build()
        .map_err(|error| error.to_string())?;
        self.bring_forward(&window);
        Ok(())
    }

    fn close_welcome(&self) {
        if let Some(window) = self.app.get_webview_window(WELCOME_LABEL) {
            let _ = window.destroy();
        }
    }

    fn focus(&self, label: &str) -> bool {
        match self.app.get_webview_window(label) {
            Some(window) => {
                self.bring_forward(&window);
                true
            }
            None => false,
        }
    }

    fn focus_any(&self) -> Option<String> {
        let (label, window) = self.app.webview_windows().into_iter().next()?;
        self.bring_forward(&window);
        Some(label)
    }

    fn hide(&self, label: &str) -> Result<(), String> {
        let window = self
            .app
            .get_webview_window(label)
            .ok_or_else(|| format!("the window {label} does not exist"))?;
        self.fullscreen
            .hide(&window)
            .map_err(|error| error.to_string())
    }

    fn reveal(&self, label: &str) -> bool {
        match self.app.get_webview_window(label) {
            Some(window) => {
                self.reveal_window(&window);
                true
            }
            None => false,
        }
    }

    fn destroy(&self, label: &str) {
        self.fullscreen.forget(label);
        if let Some(window) = self.app.get_webview_window(label) {
            if let Err(error) = window.destroy() {
                log_warn!("the window {label} could not be closed: {error}");
            }
        }
    }

    fn window_count(&self) -> usize {
        self.app.webview_windows().len()
    }

    fn confirm(&self, prompt: ConfirmPrompt) -> BoxFuture<Confirmation> {
        let (answer, receiver) = tokio::sync::oneshot::channel();
        let ConfirmPrompt {
            title,
            message,
            accept,
            discard,
            background,
        } = prompt;
        // The middle button: `Discard all`, or `Keep working in background`; never both.
        let second = discard
            .map(|label| (label, Confirmation::Discard))
            .or_else(|| background.map(|label| (label, Confirmation::Background)));
        let buttons = match &second {
            Some((label, _)) => MessageDialogButtons::YesNoCancelCustom(
                accept.clone(),
                label.clone(),
                "Cancel".to_owned(),
            ),
            None => MessageDialogButtons::OkCancelCustom(accept.clone(), "Cancel".to_owned()),
        };
        self.app
            .dialog()
            .message(message)
            .title(title)
            .kind(MessageDialogKind::Warning)
            .buttons(buttons)
            .show_with_result(move |pressed| {
                let second = second
                    .as_ref()
                    .map(|(label, meaning)| (label.as_str(), *meaning));
                let _ = answer.send(confirmation_of(&pressed, &accept, second));
            });
        // A dialog that never answered (the app is going away) counts as a cancel.
        Box::pin(async move { receiver.await.unwrap_or(Confirmation::Cancel) })
    }

    fn request_tab_texts(&self, project_id: &str) -> bool {
        unsaved::emit_flush(&self.app, project_id)
    }

    fn save_tab(
        &self,
        project_id: &str,
        path: &str,
        text: &str,
    ) -> BoxFuture<Result<String, String>> {
        let tabs = self.app.state::<AppState>().file_tabs.clone();
        let (project_id, path, text) = (project_id.to_owned(), path.to_owned(), text.to_owned());
        Box::pin(async move {
            let outcome =
                blocking::disk(move || tabs.save(&project_id, &path, &text, SaveMode::Check))
                    .await
                    .map_err(|error| error.message)?;
            match outcome {
                SaveOutcome::Saved { hash } => Ok(hash),
                SaveOutcome::Changed => {
                    Err("the file changed on disk since it was opened".to_owned())
                }
                SaveOutcome::Gone => Err("the file no longer exists".to_owned()),
                SaveOutcome::Failed(reason) => Err(reason),
            }
        })
    }

    fn save_failed(&self, failed: FilesSaveFailed) {
        unsaved::emit_save_failed(&self.app, &failed);
    }

    fn drop_tabs(&self, project_id: &str) {
        self.app
            .state::<AppState>()
            .file_tabs
            .close_project(project_id);
    }

    fn exit(&self, code: i32) {
        self.app.exit(code);
    }

    fn emit_show(&self, label: &str, request: &ShowRequest) {
        if let Err(error) = self
            .app
            .emit_to(label, EventName::ShowRequested.as_str(), request)
        {
            log_warn!("the window {label} was not told what to show: {error}");
        }
    }

    fn emit_hide_failed(&self, label: &str, failure: &HideFailed) {
        if let Err(error) = self
            .app
            .emit_to(label, EventName::WindowHideFailed.as_str(), failure)
        {
            log_warn!("the window {label} was not told why it could not be hidden: {error}");
        }
    }

    fn files_changed(&self, project_id: &str, root: &Path, batch: &Rebuild) {
        crate::requests::files_changed(&self.app, project_id, root, batch);
    }

    fn assets_changed(&self, project_id: &str, root: &Path) {
        let service = self.app.state::<AppState>().assets.clone();
        let project_id = project_id.to_owned();
        let root = root.to_path_buf();
        // One `read_dir`, on the blocking pool; the service tells the window when the list changed.
        tauri::async_runtime::spawn_blocking(move || {
            service.refresh(&project_id, &root);
        });
    }

    fn notes_changed(&self, project_id: &str, root: &Path, batch: NotesBatch) {
        let state = self.app.state::<AppState>();
        let changes = state.notes_changes.clone();
        let notes = state.notes.clone();
        let project_id = project_id.to_owned();
        let root = root.to_path_buf();
        // Reads the sheet and lists the folder, on the blocking pool; the service tells the window
        // when something changed that Weaver did not write itself.
        tauri::async_runtime::spawn_blocking(move || {
            changes.announce(&project_id, &root, &batch, &|| notes.last_hash(&project_id));
        });
    }

    fn flush_before_close(&self, project_id: &str) -> BoxFuture<()> {
        let app = self.app.clone();
        let project_id = project_id.to_owned();
        Box::pin(async move {
            crate::requests::flush_window(&app, &project_id).await;
            // The sheet's waiting text and the removed images of the Notes view, after the editor.
            crate::notes::flush_window(&app, &project_id).await;
        })
    }

    fn publish(&self, change: ProjectChanged) {
        if let Err(error) = events::emit_project_changed(&self.app, &change) {
            log_warn!("the UI was not told about a change of a project: {error}");
        }
    }

    fn recent_changed(&self) {
        super::menu::refresh(&self.app);
    }

    fn save_window_state(&self) {
        if let Err(error) = self.app.save_window_state(STATE_FLAGS) {
            log_warn!("the size and position of the windows were not saved: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn custom(label: &str) -> MessageDialogResult {
        MessageDialogResult::Custom(label.to_owned())
    }

    #[test]
    fn the_middle_button_of_the_close_confirmation_keeps_the_jobs_working_in_the_background() {
        let second = Some(("Keep working in background", Confirmation::Background));
        let answer =
            |pressed: MessageDialogResult| confirmation_of(&pressed, "Suspend and Close", second);
        assert_eq!(answer(custom("Suspend and Close")), Confirmation::Accept);
        assert_eq!(
            answer(custom("Keep working in background")),
            Confirmation::Background
        );
        assert_eq!(answer(custom("Cancel")), Confirmation::Cancel);
        assert_eq!(answer(MessageDialogResult::Yes), Confirmation::Accept);
        assert_eq!(answer(MessageDialogResult::No), Confirmation::Background);
        assert_eq!(answer(MessageDialogResult::Cancel), Confirmation::Cancel);
    }

    #[test]
    fn the_middle_button_of_the_unsaved_changes_confirmation_discards() {
        let second = Some(("Discard all", Confirmation::Discard));
        let answer = |pressed: MessageDialogResult| confirmation_of(&pressed, "Save all", second);
        assert_eq!(answer(custom("Save all")), Confirmation::Accept);
        assert_eq!(answer(custom("Discard all")), Confirmation::Discard);
        assert_eq!(answer(MessageDialogResult::No), Confirmation::Discard);
        assert_eq!(answer(custom("Cancel")), Confirmation::Cancel);
    }

    #[test]
    fn a_confirmation_with_two_buttons_has_no_middle_one() {
        let answer =
            |pressed: MessageDialogResult| confirmation_of(&pressed, "Suspend and Quit", None);
        assert_eq!(answer(custom("Suspend and Quit")), Confirmation::Accept);
        assert_eq!(answer(MessageDialogResult::Ok), Confirmation::Accept);
        assert_eq!(answer(MessageDialogResult::No), Confirmation::Cancel);
        assert_eq!(answer(custom("Cancel")), Confirmation::Cancel);
    }

    /// Every `.rs` file under `dir`.
    fn sources(dir: &Path, found: &mut Vec<std::path::PathBuf>) {
        for item in std::fs::read_dir(dir).unwrap() {
            let path = item.unwrap().path();
            if path.is_dir() {
                sources(&path, found);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                found.push(path);
            }
        }
    }

    /// Whether `value` holds the key `key` anywhere.
    fn has_key(value: &serde_json::Value, key: &str) -> bool {
        match value {
            serde_json::Value::Object(map) => {
                map.contains_key(key) || map.values().any(|inner| has_key(inner, key))
            }
            serde_json::Value::Array(items) => items.iter().any(|inner| has_key(inner, key)),
            _ => false,
        }
    }

    /// Tauri's native drag-drop handler is what reports the paths of dropped files, in the File
    /// view and in the request editor: both windows are built with it on, and nothing turns it off.
    #[test]
    fn the_project_and_welcome_windows_are_built_with_tauris_drag_drop_handler_on() {
        // The names are put together so that this test does not hold what it looks for.
        let disable_call = concat!("disable_", "drag_drop_handler");
        let disable_config = concat!("dragDrop", "Enabled");
        let disable_config_rust = concat!("drag_drop_", "enabled");

        let mut found = Vec::new();
        sources(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut found,
        );
        assert!(!found.is_empty());
        for file in &found {
            let text = std::fs::read_to_string(file).unwrap();
            for forbidden in [disable_call, disable_config, disable_config_rust] {
                assert!(
                    !text.contains(forbidden),
                    "{} holds {forbidden}",
                    file.display()
                );
            }
        }

        // Both windows come from builders in this file, and the configuration holds none.
        let shell = include_str!("tauri_shell.rs");
        let code = shell.split_once("#[cfg(test)]").unwrap().0;
        assert_eq!(code.matches("WebviewWindowBuilder::new(").count(), 2);
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
        assert_eq!(config["app"]["windows"], serde_json::json!([]));
        assert!(!has_key(&config, disable_config));
        assert!(!has_key(&config, disable_config_rust));
    }
}
