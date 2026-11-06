//! Atomic commits of a working copy: the switch the developer turns on in Settings, kept in
//! `.weaver/commits/commits.json` (`{"enabled": true}`), local only like Solo mode
//! ([`crate::solo_mode`]): never in the repository and never in the settings of Weaver.
//!
//! No file means off. A file that cannot be read, or holds another value, counts as off and is
//! reported as a [`Diagnostic`]; it is never rewritten, only the next change of the switch writes
//! it. Turning off deletes the whole `commits/` folder. Nothing about the commits Weaver makes is
//! ever stored here: this module only says whether they are on.
//!
//! Turning on needs git, as Solo mode does: the check runs `git rev-parse` through the runner of
//! Solo mode ([`Git`]), so the reasons are the same ([`Reason`]). The caller holds the project's
//! lock and never runs this on the thread of the UI.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::project::diagnostic::Diagnostic;
use crate::project::layout::{ensure_local_folder, weaver_dir, COMMITS_DIR};
use crate::project::text;
use crate::project_files::has_weaver;
use crate::solo_mode::{self, Git, Reason};

/// The file inside `commits/`.
pub const STATE_FILE: &str = "commits.json";

/// What `commits.json` says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub enabled: bool,
    /// A file that cannot be read or holds another value counts as off and is reported here; only
    /// the next change of Atomic commits rewrites it.
    pub diagnostic: Option<Diagnostic>,
}

/// `<root>/.weaver/commits/commits.json`.
pub fn state_path(root: &Path) -> PathBuf {
    weaver_dir(root).join(COMMITS_DIR).join(STATE_FILE)
}

/// The content of `commits.json` when on.
pub const STATE_TEXT: &str = "{\"enabled\": true}";

/// Reads the state of the working copy at `root`: no file means off.
pub fn read_state(root: &Path) -> State {
    let off = |reason: String| State {
        enabled: false,
        diagnostic: Some(Diagnostic::error(
            format!("{COMMITS_DIR}/{STATE_FILE}"),
            reason,
        )),
    };
    let text = match text::read_text(&state_path(root)) {
        Ok(text) => text,
        Err(text::ReadError::Io(e))
            if matches!(
                e.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return State {
                enabled: false,
                diagnostic: None,
            }
        }
        Err(e) => return off(format!("cannot read the file: {e}")),
    };
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(e) => return off(format!("cannot read the file: {e}")),
    };
    match value.get("enabled").and_then(serde_json::Value::as_bool) {
        Some(true) => State {
            enabled: true,
            diagnostic: None,
        },
        _ => off("`enabled` must be `true`".to_owned()),
    }
}

/// Whether Atomic commits is on for the working copy at `root`: what the code that commits after
/// a History entry reads. A missing, unreadable or other file is off.
pub fn is_on(root: &Path) -> bool {
    read_state(root).enabled
}

/// What Atomic commits is, for the app after a change and when Settings is shown.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inspection {
    pub enabled: bool,
    /// Why it cannot be turned on (git cannot be used here); `None` when it can.
    pub unavailable: Option<Reason>,
    /// The state file could not be read or held another value.
    pub diagnostic: Option<Diagnostic>,
}

/// Works out the state at the call: git is asked, so never on the thread of the UI.
pub fn inspect(git: &Git, root: &Path) -> Inspection {
    inspection_of(read_state(root), git, root)
}

fn inspection_of(state: State, git: &Git, root: &Path) -> Inspection {
    Inspection {
        enabled: state.enabled,
        unavailable: solo_mode::locate(git, root).err(),
        diagnostic: state.diagnostic,
    }
}

fn write_state(root: &Path) -> Result<(), Reason> {
    let path = state_path(root);
    ensure_local_folder(root, COMMITS_DIR)
        .and_then(|_| text::write_text(&path, STATE_TEXT))
        .map_err(|e| Reason::write_failed(&path, &e))
}

/// `developer.CHANGE_ATOMIC_COMMITS`: turns Atomic commits on or off. Turning on needs the folder to
/// be in a git repository and writes `commits.json`; turning off deletes the `commits/` folder and
/// needs no git. A change that fails changes nothing and says why. Turning on what is already on,
/// or off what is already off, writes nothing unless the state file was reported as a problem,
/// which the change then repairs.
pub fn set_enabled(git: &Git, root: &Path, enabled: bool) -> Result<Inspection, Reason> {
    if !has_weaver(root) {
        return Err(Reason::NotAProject);
    }
    let before = read_state(root);
    if enabled {
        solo_mode::locate(git, root)?;
        if !(before.enabled && before.diagnostic.is_none()) {
            write_state(root)?;
        }
    } else {
        let folder = weaver_dir(root).join(COMMITS_DIR);
        match fs::remove_dir_all(&folder) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(Reason::write_failed(&folder, &e)),
        }
    }
    let state = State {
        enabled,
        diagnostic: None,
    };
    Ok(inspection_of(state, git, root))
}

#[cfg(test)]
mod tests;
